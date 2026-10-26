//! Transporte MCP local reutilizable. Las apps aportan herramientas; no posee datos de negocio.
use gennie_core::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
const LIMIT: usize = 128 * 1024;
const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;

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

pub struct Tool {
    pub definition: Value,
    pub call: Box<dyn Fn(Value) -> Result<Value> + Send + Sync>,
}
#[derive(Default)]
pub struct Registry(BTreeMap<String, Tool>);
impl Registry {
    pub fn register(&mut self, tools: impl IntoIterator<Item = Tool>) -> Result<()> {
        for tool in tools {
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
                match (tool.call)(
                    request
                        .pointer("/params/arguments")
                        .cloned()
                        .unwrap_or(json!({})),
                ) {
                    Ok(value) => {
                        if let Some(output) = value.get("__gennie_mcp_output_v1") {
                            let data = &output["data"];
                            json!({"content":[{"type":"text","text":data.to_string()},output["image"]],"structuredContent":data,"isError":false})
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
            "mcp_servers.gennie={{command={},args=[\"--mcp\",{}],required=true,startup_timeout_sec=10,tool_timeout_sec=30,default_tools_approval_mode=\"approve\"}}",
            serde_json::to_string(&self.helper.to_string_lossy())?,
            serde_json::to_string(&self.socket.to_string_lossy())?
        ))
    }
}

pub struct Server {
    launch: Launch,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
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
                                if let Ok(client) = std::thread::Builder::new()
                                    .name("gennie-mcp-client".into())
                                    .spawn(move || {
                                        let _ = serve(stream, &registry, &stop);
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
fn serve(mut stream: UnixStream, registry: &Registry, stopping: &AtomicBool) -> Result<()> {
    // En macOS los sockets aceptados pueden heredar O_NONBLOCK del listener.
    // write_all debe poder esperar al consumidor de una imagen grande.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut pending = Vec::new();
    let mut buffer = [0; 4096];
    while !stopping.load(Ordering::Acquire) {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => pending.extend_from_slice(&buffer[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            if end > LIMIT {
                return Err("Petición MCP demasiado grande".into());
            }
            let line: Vec<_> = pending.drain(..=end).collect();
            let response = match serde_json::from_slice(&line) {
                Ok(request) => registry.respond(request),
                Err(_) => Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"JSON inválido"}}),
                ),
            };
            if let Some(response) = response {
                let response = serde_json::to_vec(&response)?;
                if response.len() > RESPONSE_LIMIT {
                    return Err("Respuesta MCP demasiado grande".into());
                }
                stream.write_all(&response)?;
                stream.write_all(b"\n")?;
            }
        }
        if pending.len() > LIMIT {
            return Err("Petición MCP demasiado grande".into());
        }
    }
    Ok(())
}
/// Helper stdio sin SQLite ni acceso a recursos de las apps. El host ejecuta las herramientas.
pub fn bridge(socket: &Path) -> Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut response = BufReader::new(stream.try_clone()?);
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut line = Vec::new();
        if input
            .by_ref()
            .take((LIMIT + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() > LIMIT {
            return Err("Petición MCP demasiado grande".into());
        }
        let request: Value = serde_json::from_slice(&line)?;
        stream.write_all(&line)?;
        if !line.ends_with(b"\n") {
            stream.write_all(b"\n")?;
        }
        if request.get("id").is_some() {
            let mut line = Vec::new();
            let n = response
                .by_ref()
                .take((RESPONSE_LIMIT + 1) as u64)
                .read_until(b'\n', &mut line)?;
            if n == 0 || n > RESPONSE_LIMIT || line.last() != Some(&b'\n') {
                return Err("Respuesta MCP incompleta o demasiado grande".into());
            }
            output.write_all(&line)?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(with_image(json!({}), "a".repeat(RESPONSE_LIMIT), "image/jpeg").is_err());
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
