//! Transporte MCP local reutilizable. Las apps aportan herramientas; no posee datos de negocio.
mod cancellable_io;
mod cancellation;
mod connection;
mod framing;
mod stdio_bridge;
pub use cancellable_io::write_cancellable;
pub use cancellation::{CallContext, Cancelled, ContextTool};
use gennie_core::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub use stdio_bridge::bridge;
const LIMIT: usize = 128 * 1024;
/// Peticiones en cola por conexión; una herramienta se ejecuta a la vez.
pub const MAX_PENDING_REQUESTS: usize = 64;
/// Límite del mensaje de respuesta MCP, incluido su sobre y salto de línea.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
// Mayor que la espera del navegador, menor que la espera del cliente Codex.
pub const MAX_TOOL_EXECUTION_SECS: u64 = 15 * 60;
const BRIDGE_RESPONSE_TIMEOUT_SECS: u64 = MAX_TOOL_EXECUTION_SECS + 15;
const CLIENT_TOOL_TIMEOUT_SECS: u64 = BRIDGE_RESPONSE_TIMEOUT_SECS + 15;

/// Sobre interno para resultados multimodales de herramientas propias, separado de sus datos.
pub fn with_image(data: Value, image: String, mime_type: &str) -> Result<Value> {
    if !matches!(mime_type, "image/png" | "image/jpeg" | "image/webp")
        || image.len() > 3 * 1024 * 1024
    {
        return Err("Imagen MCP no válida o demasiado grande".into());
    }
    Ok(
        json!({"__gennie_mcp_output_v1":{"data":data,"image":{"type":"image","data":image,"mimeType":mime_type}}}),
    )
}

/// Conserva bloques MCP nativos separados de los datos estructurados locales.
/// El adaptador aporta bloques del protocolo; el transporte no resuelve sus URI.
pub fn with_content(data: Value, content: Vec<Value>, is_error: bool) -> Value {
    json!({"__gennie_mcp_output_v1":{"data":data,"content":content,"isError":is_error}})
}

pub struct Tool {
    pub definition: Value,
    pub call: Box<dyn Fn(Value) -> Result<Value> + Send + Sync>,
}
#[derive(Default)]
pub struct Registry(BTreeMap<String, ContextTool>);
impl Registry {
    pub fn register<T: Into<ContextTool>>(
        &mut self,
        tools: impl IntoIterator<Item = T>,
    ) -> Result<()> {
        for tool in tools {
            let tool = tool.into();
            let name = tool.definition["name"]
                .as_str()
                .ok_or("Herramienta sin nombre")?
                .to_owned();
            if self.0.contains_key(&name) {
                return Err("Herramienta MCP duplicada".into());
            }
            self.0.insert(name, tool);
        }
        Ok(())
    }
    pub fn respond(&self, request: Value) -> Option<Value> {
        self.respond_with_context(request, CallContext::default())
    }
    pub fn respond_with_context(&self, request: Value, context: CallContext) -> Option<Value> {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let error = |code, message| json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}});
        let Some(method) = request["method"].as_str() else {
            return Some(error(-32600, "Petición inválida"));
        };
        if request["jsonrpc"] != "2.0" {
            return Some(error(-32600, "Versión JSON-RPC inválida"));
        }
        request.get("id")?;
        let result = match method {
            "initialize" => {
                let version = request
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let version = match version {
                    "2024-11-05" | "2025-03-26" | "2025-06-18" => version,
                    _ => "2025-06-18",
                };
                json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"gennie","version":env!("CARGO_PKG_VERSION")},"instructions":"Herramientas locales de Gennie. Confirma únicamente los resultados acreditados por cada herramienta."})
            }
            "ping" => json!({}),
            "tools/list" => {
                json!({"tools":self.0.values().map(|t| &t.definition).collect::<Vec<_>>()})
            }
            "tools/call" => {
                let Some(name) = request.pointer("/params/name").and_then(Value::as_str) else {
                    return Some(error(-32602, "Falta nombre de herramienta"));
                };
                let Some(tool) = self.0.get(name) else {
                    return Some(error(-32602, "Herramienta desconocida"));
                };
                let outcome = context.check().and_then(|_| {
                    (tool.call)(
                        request
                            .pointer("/params/arguments")
                            .cloned()
                            .unwrap_or(json!({})),
                        context,
                    )
                });
                match outcome {
                    Ok(value) => {
                        if let Some(output) = value.get("__gennie_mcp_output_v1") {
                            let data = &output["data"];
                            if let Some(blocks) = output["content"].as_array() {
                                let mut content = Vec::with_capacity(blocks.len() + 1);
                                content.push(json!({"type":"text","text":data.to_string()}));
                                content.extend(blocks.iter().cloned());
                                json!({"content":content,"structuredContent":data,"isError":output["isError"] == true})
                            } else {
                                json!({"content":[{"type":"text","text":data.to_string()},output["image"]],"structuredContent":data,"isError":false})
                            }
                        } else {
                            json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":false})
                        }
                    }
                    Err(e) => {
                        json!({"content":[{"type":"text","text":e.to_string()}],"isError":true})
                    }
                }
            }
            _ => return Some(error(-32601, "Método desconocido")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

#[derive(Clone)]
pub struct Launch {
    pub helper: PathBuf,
    pub socket: PathBuf,
    pub tool_names: Vec<String>,
}
impl Launch {
    /// Confirmar lo descubierto por Codex, sin asumir que configurar equivale a estar listo.
    pub fn validate_inventory(&self, inventory: &Value) -> Result<()> {
        let server = inventory["data"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["name"] == "gennie"))
            .ok_or("Codex todavía no ha descubierto el MCP local de Gennie")?;
        let tools = server["tools"]
            .as_object()
            .ok_or("Codex no devolvió las herramientas de Gennie")?;
        if !server["toolsError"].is_null()
            || self.tool_names.iter().any(|name| !tools.contains_key(name))
        {
            return Err("Codex no pudo descubrir todas las herramientas locales de Gennie. Pulsa Conectar para reintentar".into());
        }
        Ok(())
    }
    pub fn config(&self) -> Result<String> {
        // Las cadenas JSON de rutas también son cadenas básicas TOML válidas.
        Ok(format!(
            "mcp_servers.gennie={{command={},args=[\"--mcp\",{}],required=true,startup_timeout_sec=10,tool_timeout_sec={CLIENT_TOOL_TIMEOUT_SECS},default_tools_approval_mode=\"approve\"}}",
            serde_json::to_string(&self.helper.to_string_lossy())?,
            serde_json::to_string(&self.socket.to_string_lossy())?
        ))
    }
}

pub struct Server {
    launch: Launch,
    stopping: Arc<AtomicBool>,
    clients: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}
struct ClientConnection(Arc<AtomicUsize>);
impl Drop for ClientConnection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Server {
    pub fn start(registry: Registry, helper: &Path) -> Result<Self> {
        let tool_names = registry.0.keys().cloned().collect();
        let helper = helper
            .canonicalize()
            .map_err(|_| "No se encuentra el helper MCP")?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = format!(
            "{}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        // Ruta corta: sockaddr_un en macOS está limitado a 104 bytes.
        let directory = PathBuf::from(format!("/tmp/gennie-mcp-{}-{nonce}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let socket = directory.join("rpc");
        let start = (|| -> Result<_> {
            let listener = UnixListener::bind(&socket)?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            let stopping = Arc::new(AtomicBool::new(false));
            let stop = stopping.clone();
            let connections = Arc::new(AtomicUsize::new(0));
            let active_connections = connections.clone();
            let worker = std::thread::Builder::new()
                .name("gennie-mcp".into())
                .spawn(move || {
                    let registry = Arc::new(registry);
                    let mut clients: Vec<JoinHandle<()>> = Vec::new();
                    while !stop.load(Ordering::Acquire) {
                        let mut i = 0;
                        while i < clients.len() {
                            if clients[i].is_finished() {
                                let _ = clients.swap_remove(i).join();
                            } else {
                                i += 1;
                            }
                        }
                        match listener.accept() {
                            Ok((stream, _)) => {
                                if clients.len() >= 16 {
                                    continue;
                                }
                                let registry = registry.clone();
                                let stop = stop.clone();
                                active_connections.fetch_add(1, Ordering::AcqRel);
                                let connection = ClientConnection(active_connections.clone());
                                if let Ok(client) = std::thread::Builder::new()
                                    .name("gennie-mcp-client".into())
                                    .spawn(move || {
                                        let _connection = connection;
                                        let _ = connection::serve(stream, &registry, stop);
                                    })
                                {
                                    clients.push(client);
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(20))
                            }
                            Err(_) => break,
                        }
                    }
                    stop.store(true, Ordering::Release);
                    for client in clients {
                        let _ = client.join();
                    }
                })?;
            Ok(Self {
                launch: Launch {
                    helper,
                    socket: socket.clone(),
                    tool_names,
                },
                stopping,
                clients: connections,
                worker: Some(worker),
            })
        })();
        if start.is_err() {
            let _ = std::fs::remove_file(&socket);
            let _ = std::fs::remove_dir(&directory);
        }
        start
    }
    pub fn launch(&self) -> Launch {
        self.launch.clone()
    }
    /// Clientes conectados al host privado; no acredita que una herramienta haya tenido éxito.
    pub fn has_clients(&self) -> bool {
        self.clients.load(Ordering::Acquire) > 0 && !self.stopping.load(Ordering::Acquire)
    }
    pub fn stop(&mut self) -> Result<()> {
        self.stopping.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "Falló el cierre MCP")?;
        }
        if self.launch.socket.exists() {
            std::fs::remove_file(&self.launch.socket)?;
        }
        if let Some(parent) = self.launch.socket.parent()
            && parent.exists()
        {
            std::fs::remove_dir(parent)?;
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn encode_response(mut response: Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(&response)?;
    if bytes.len() < MAX_RESPONSE_BYTES {
        return Ok(bytes);
    }
    // Solo la primera copia exacta de datos locales es redundante. Los bloques
    // text del servidor pueden contener datos ausentes de structuredContent.
    if let Some(duplicate) = response["result"]
        .get("structuredContent")
        .map(Value::to_string)
        && let Some(item) = response["result"]["content"]
            .as_array_mut()
            .and_then(|c| c.first_mut())
        && item["type"] == "text"
        && item["text"].as_str() == Some(duplicate.as_str())
    {
        item["text"] = json!("Resultado completo disponible en structuredContent.");
        bytes = serde_json::to_vec(&response)?;
        if bytes.len() < MAX_RESPONSE_BYTES {
            return Ok(bytes);
        }
    }
    // La operación ya pudo ejecutarse. Informar el límite sin cortar el canal ni simular éxito.
    Ok(serde_json::to_vec(
        &json!({"jsonrpc":"2.0","id":response["id"],"result":{
            "content":[{"type":"text","text":"El resultado supera el tamaño del canal. La operación pudo completarse; consulta su estado antes de repetirla y recupera los datos por páginas o en una salida más pequeña."}],
            "structuredContent":{"code":"response_too_large","outcome":"unknown"},"isError":true
        }}),
    )?)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    #[test]
    fn compaction_preserves_native_text_binary_and_error_or_reports_unknown() {
        let mut registry = Registry::default();
        registry.register([Tool {
            definition:json!({"name":"native"}),
            call:Box::new(|_|Ok(with_content(
                json!({"payload":"x".repeat(MAX_RESPONSE_BYTES / 2 + 1024)}),
                vec![json!({"type":"text","text":"Texto remoto imprescindible"}),json!({"type":"image","data":"A".repeat(300000),"mimeType":"image/png"})],
                true,
            ))),
        }]).unwrap();
        let response = registry
            .respond(
                json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"native"}}),
            )
            .unwrap();
        assert!(serde_json::to_vec(&response).unwrap().len() >= MAX_RESPONSE_BYTES);
        let encoded = encode_response(response).unwrap();
        assert!(encoded.len() < MAX_RESPONSE_BYTES);
        let result: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(result["result"]["isError"], true);
        assert_eq!(
            result["result"]["content"][1]["text"],
            "Texto remoto imprescindible"
        );
        assert_eq!(
            result["result"]["content"][2]["data"]
                .as_str()
                .unwrap()
                .len(),
            300000
        );
        let oversized = json!({"jsonrpc":"2.0","id":9,"result":{"content":[{"type":"image","mimeType":"image/png","data":"A".repeat(MAX_RESPONSE_BYTES)}],"structuredContent":{},"isError":false}});
        let result: Value = serde_json::from_slice(&encode_response(oversized).unwrap()).unwrap();
        assert_eq!(result["id"], 9);
        assert_eq!(result["result"]["isError"], true);
        assert_eq!(result["result"]["structuredContent"]["outcome"], "unknown");
    }
    #[test]
    fn oversized_tool_result_does_not_close_the_transport() {
        let mut registry = Registry::default();
        registry
            .register([Tool {
                definition: json!({"name":"large_result"}),
                call: Box::new(|_| Ok(json!({"data":"x".repeat(MAX_RESPONSE_BYTES / 2 + 1024)}))),
            }])
            .unwrap();
        let mut server = Server::start(registry, &std::env::current_exe().unwrap()).unwrap();
        let mut stream = UnixStream::connect(&server.launch.socket).unwrap();
        // Comprueba integridad/continuidad, no un SLA de serialización en debug.
        // El margen cubre la contención con otros fixtures de varios MiB.
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"large_result\"}}\n").unwrap();
        let mut line = String::new();
        let received = reader.read_line(&mut line).unwrap();
        assert!(received > 0, "Una respuesta grande cerró el transporte MCP");
        let result: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(result["id"], 1);
        assert_eq!(result["result"]["isError"], false);
        assert_eq!(
            result["result"]["structuredContent"]["data"]
                .as_str()
                .unwrap()
                .len(),
            MAX_RESPONSE_BYTES / 2 + 1024
        );
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n")
            .unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        let ping: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(ping["id"], 2);
        assert!(ping.get("result").is_some());
        server.stop().unwrap();
    }

    #[test]
    fn unrepresentable_result_is_explicit_and_connected_clients_are_observed() {
        let response = json!({"jsonrpc":"2.0","id":7,"result":{
            "content":[{"type":"text","text":"large"}],
            "structuredContent":{"data":"x".repeat(MAX_RESPONSE_BYTES)},"isError":false
        }});
        let result: Value = serde_json::from_slice(&encode_response(response).unwrap()).unwrap();
        assert_eq!(result["id"], 7);
        assert_eq!(result["result"]["isError"], true);
        assert_eq!(
            result["result"]["structuredContent"]["code"],
            "response_too_large"
        );
        assert!(result["result"]["structuredContent"].get("data").is_none());

        let mut server =
            Server::start(Registry::default(), &std::env::current_exe().unwrap()).unwrap();
        assert!(!server.has_clients());
        let mut stream = UnixStream::connect(&server.launch.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
            .unwrap();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut String::new())
            .unwrap();
        assert!(server.has_clients());
        drop(stream);
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while server.has_clients() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!server.has_clients());
        server.stop().unwrap();
    }

    #[test]
    fn image_results_are_multimodal_without_duplicating_binary_in_structured_data() {
        let mut registry = Registry::default();
        registry
            .register([Tool {
                definition: json!({"name":"image"}),
                call: Box::new(|_| {
                    with_image(
                        json!({"artifact":"private.jpg"}),
                        "aA==".into(),
                        "image/jpeg",
                    )
                }),
            }])
            .unwrap();
        let result = registry
            .respond(
                json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"image"}}),
            )
            .unwrap();
        assert_eq!(result["result"]["content"][1]["type"], "image");
        assert_eq!(result["result"]["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(
            result["result"]["structuredContent"],
            json!({"artifact":"private.jpg"})
        );
        assert!(with_image(json!({}), "data".into(), "text/html").is_err());
        assert!(with_image(json!({}), "a".repeat(MAX_RESPONSE_BYTES), "image/jpeg").is_err());
    }
    #[test]
    fn multiple_clients_are_served_and_idle_connections_do_not_block_shutdown() {
        let mut server =
            Server::start(Registry::default(), &std::env::current_exe().unwrap()).unwrap();
        let path = server.launch().socket;
        let first = UnixStream::connect(&path).unwrap();
        let mut second = UnixStream::connect(&path).unwrap();
        second
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        second
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
            .unwrap();
        let mut line = String::new();
        BufReader::new(second.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert!(line.contains("result"));
        let now = std::time::Instant::now();
        server.stop().unwrap();
        assert!(now.elapsed() < Duration::from_secs(1));
        assert!(!path.exists());
        drop(first);
        drop(second);
    }
    #[test]
    fn protocol_errors_and_notifications_do_not_become_successful_calls() {
        let registry = Registry::default();
        assert!(
            registry
                .respond(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
                .is_none()
        );
        assert_eq!(
            registry.respond(json!({"id":1,"method":"ping"})).unwrap()["error"]["code"],
            -32600
        );
        assert_eq!(
            registry
                .respond(json!({"jsonrpc":"2.0","id":2,"method":"missing"}))
                .unwrap()["error"]["code"],
            -32601
        );
        assert_eq!(registry.respond(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"missing"}})).unwrap()["error"]["code"],-32602);
    }
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    #[test]
    fn discovery_requires_every_registered_tool_and_rejects_reported_errors() {
        let launch = Launch {
            helper: "helper".into(),
            socket: "rpc".into(),
            tool_names: vec!["assignments_create".into(), "assignments_get".into()],
        };
        assert!(launch.validate_inventory(&json!({"data":[]})).is_err());
        assert!(launch.validate_inventory(&json!({"data":[{"name":"gennie","tools":{"assignments_create":{}},"toolsError":null}]})).is_err());
        assert!(launch.validate_inventory(&json!({"data":[{"name":"gennie","tools":{"assignments_create":{},"assignments_get":{}},"toolsError":"failed"}]})).is_err());
        assert!(launch.validate_inventory(&json!({"data":[{"name":"gennie","tools":{"assignments_create":{},"assignments_get":{}},"toolsError":null}]})).is_ok());
    }
}
