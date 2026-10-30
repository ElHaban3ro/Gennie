//! Doble transporte MCP real; ninguna generación o cuenta del usuario.
use super::*;
use gennie_llm::mcp_client::{Client, Control, Transport};

struct Journal;
impl BuildJournal for Journal {
    fn target(&self, _: i64) -> Result<gennie_llm::delegation::Origin> {
        Err("No hay conversación".into())
    }
    fn origin(&self) -> Result<i64> {
        Err("No hay conversación".into())
    }
    fn exists(&self, _: i64) -> Result<bool> {
        Ok(false)
    }
    fn deliver(&self, _: &gennie_llm::delegation::Update) -> Result<()> {
        Err("No hay entrega".into())
    }
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "Requiere Codex/helper instalados; dos canales MCP ficticios, sin cuenta ni inferencia"]
fn installed_proxy_preserves_native_results_and_uncertain_gate_through_codex() {
    let root = std::env::temp_dir().join(format!(
        "gennie-native-proxy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let _directory = Directory(root.clone());
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let blocks = json!([
        {"type":"text","text":"Dato ficticio del servidor"},
        {"type":"image","mimeType":"image/png","data":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6m/8AAAAASUVORK5CYII=","_meta":{"private":"PRIVATE_PROTOCOL"},"annotations":{"audience":["assistant"],"priority":0.7}},
        {"type":"audio","mimeType":"audio/wav","data":"UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA="},
        {"type":"resource_link","uri":"fixture://item","name":"Recurso ficticio"},
        {"type":"resource","resource":{"uri":"fixture://text","text":"Texto ficticio","_meta":{"private":"PRIVATE_PROTOCOL"}}},
        {"type":"resource","resource":{"uri":"fixture://blob","blob":"YmxvYg==","mimeType":"application/octet-stream"}}
    ]);
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = count.clone();
    let remote_blocks = blocks.clone();
    let mut remote_registry = gennie_mcp::Registry::default();
    remote_registry.register([Tool{
        definition:json!({"name":"media","inputSchema":{"type":"object","properties":{"fail":{"type":"boolean"}},"required":["fail"],"additionalProperties":false}}),
        call:Box::new(move|args|{
            observed.fetch_add(1,Ordering::SeqCst);
            Ok(gennie_mcp::with_content(json!({"count":6,"_meta":"SERVICE_DATA"}),remote_blocks.as_array().unwrap().clone(),args["fail"]==true))
        }),
    }]).unwrap();
    let mut remote = gennie_mcp::Server::start(remote_registry, &helper).unwrap();
    let remote_launch = remote.launch();
    let db = Database::memory().unwrap();
    db.migrate("plugins", MIGRATIONS).unwrap();
    let browser = gennie_browser::BrowserModule::new(root.join("browser"));
    let module = PluginsModule::new(
        db.clone(),
        browser.controller(),
        Arc::new(Journal),
        AgentSettings::shared(Arc::new(|| Ok(Settings::default()))),
        helper.clone(),
        root.join("plugins"),
    );
    let controller = module.controller();
    controller.0.mcp.start().unwrap();
    controller.0.enabled.store(true, Ordering::Release);
    let mut proxy_registry = gennie_mcp::Registry::default();
    proxy_registry
        .register(contextual_tools(controller.clone()))
        .unwrap();
    let mut proxy = gennie_mcp::Server::start(proxy_registry, &helper).unwrap();
    let proxy_launch = proxy.launch();
    let result = (|| -> Result<()> {
        controller.request(json!({"action":"mcp_save","expected_revision":0,"specification":{"id":"native_fixture","name":"Fuente ficticia","connection":{"type":"stdio","command":helper,"args":["--mcp",remote_launch.socket]},"timeout_seconds":5}}))?;
        let discovered = controller.request(
            json!({"action":"mcp_connect","server_id":"native_fixture","expected_revision":1}),
        )?;
        let allowed=controller.request(json!({"action":"mcp_permissions","server_id":"native_fixture","expected_revision":discovered["server"]["revision"],"allowed_tools":["media"],"enabled":true}))?;
        let revision = allowed["server"]["revision"]
            .as_i64()
            .ok_or("Sin revisión")?;
        let transport = Transport::Stdio {
            command: helper.to_string_lossy().into(),
            args: vec!["--mcp".into(), proxy_launch.socket.to_string_lossy().into()],
            cwd: None,
            env_vars: vec![],
        };
        let mut client = Client::connect(root.join("caller"), &transport, 5, Control::default())?;
        client.inventory()?;
        for failed in [false, true] {
            let output=client.call("plugin_mcp_run",json!({"server_id":"native_fixture","tool":"media","expected_revision":revision,"arguments":{"fail":failed}}))?;
            assert_eq!(output["isError"], failed);
            assert_eq!(output["content"].as_array().unwrap().len(), 8);
            for (index, kind) in [
                "text",
                "image",
                "audio",
                "resource_link",
                "resource",
                "resource",
            ]
            .iter()
            .enumerate()
            {
                assert_eq!(output["content"][index + 2]["type"], *kind);
            }
            assert_eq!(output["content"][3]["data"], blocks[1]["data"]);
            assert_eq!(output["content"][4]["data"], blocks[2]["data"]);
            assert_eq!(output["content"][5]["uri"], "fixture://item");
            assert_eq!(output["content"][6]["resource"]["text"], "Texto ficticio");
            assert_eq!(output["content"][7]["resource"]["blob"], "YmxvYg==");
            assert!(!output.to_string().contains("PRIVATE_PROTOCOL"));
            let data = &output["structuredContent"];
            assert_eq!(data["status"], if failed { "tool_failed" } else { "ok" });
            assert_eq!(data["review_required"], failed);
            assert_eq!(data["source"]["server_id"], "native_fixture");
            assert_eq!(data["result"]["structuredContent"]["count"], 6);
            assert_eq!(data["result"]["structuredContent"]["_meta"], "SERVICE_DATA");
            assert!(data["result"].get("content").is_none());
            assert!(
                !output["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(blocks[1]["data"].as_str().unwrap())
            );
        }
        let blocked=client.call("plugin_mcp_run",json!({"server_id":"native_fixture","tool":"media","expected_revision":revision,"arguments":{"fail":false}}))?;
        assert_eq!(blocked["structuredContent"]["status"], "review_required");
        assert_eq!(count.load(Ordering::SeqCst), 2);
        db.with_connection(|c| {
            let attempts: i64 =
                c.query_row("SELECT count(*) FROM plugin_mcp_attempts", [], |r| r.get(0))?;
            assert_eq!(attempts, 2);
            let saved: String = c.query_row(
                "SELECT group_concat(reason) FROM plugin_mcp_attempts",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(saved, "reported_success,tool_error");
            Ok(())
        })?;
        drop(client);
        Ok(())
    })();
    controller.0.mcp.stop().unwrap();
    proxy.stop().unwrap();
    remote.stop().unwrap();
    db.close().unwrap();
    result.unwrap();
}

#[test]
#[ignore = "Requiere Codex/helper actuales; cadena stdio ficticia, sin cuentas, modelos o red"]
fn installed_proxy_eof_cancels_external_transport_and_keeps_a_single_uncertain_effect() {
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(format!(
        "gennie-cancel-proxy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let _directory = Directory(root.clone());
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        })
        .canonicalize()
        .unwrap();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = count.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    let remote_cancelled = cancelled.clone();
    let mut remote_registry = gennie_mcp::Registry::default();
    remote_registry.register([gennie_mcp::ContextTool {
        definition: json!({"name":"write","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}),
        call: Box::new(move |_, context| {
            observed.fetch_add(1, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(6);
            while !context.is_cancelled() && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(5)); }
            remote_cancelled.store(context.is_cancelled(), Ordering::Release);
            context.check()?;
            Err("El fixture no recibió cancelación".into())
        }),
    }]).unwrap();
    let mut remote = gennie_mcp::Server::start(remote_registry, &helper).unwrap();
    let db = Database::memory().unwrap();
    db.migrate("plugins", MIGRATIONS).unwrap();
    let browser = gennie_browser::BrowserModule::new(root.join("browser"));
    let module = PluginsModule::new(
        db.clone(),
        browser.controller(),
        Arc::new(Journal),
        AgentSettings::shared(Arc::new(|| Ok(Settings::default()))),
        helper.clone(),
        root.join("plugins"),
    );
    let controller = module.controller();
    controller.0.mcp.start().unwrap();
    controller.0.enabled.store(true, Ordering::Release);
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(contextual_tools(controller.clone()))
        .unwrap();
    let mut proxy = gennie_mcp::Server::start(registry, &helper).unwrap();
    let result = (|| -> Result<()> {
        controller.request(json!({"action":"mcp_save","expected_revision":0,"specification":{"id":"cancel_fixture","name":"Fuente ficticia","connection":{"type":"stdio","command":helper,"args":["--mcp",remote.launch().socket]},"timeout_seconds":5}}))?;
        let discovered = controller.request(
            json!({"action":"mcp_connect","server_id":"cancel_fixture","expected_revision":1}),
        )?;
        let allowed = controller.request(json!({"action":"mcp_permissions","server_id":"cancel_fixture","expected_revision":discovered["server"]["revision"],"allowed_tools":["write"],"enabled":true}))?;
        let mut child = Command::new(&helper)
            .arg("--mcp")
            .arg(&proxy.launch().socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut input = child.stdin.take().ok_or("Falta stdin propio")?;
        let sent = writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"plugin_mcp_run","arguments":{"server_id":"cancel_fixture","tool":"write","expected_revision":allowed["server"]["revision"],"arguments":{}}}})
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while count.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(input);
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut exited = false;
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !exited {
            let _ = child.kill();
        }
        let _ = child.wait();
        sent?;
        assert!(exited);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !cancelled.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            cancelled.load(Ordering::Acquire),
            "EOF no llegó al servidor externo"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while proxy.has_clients() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let attempts =
            controller.request(json!({"action":"mcp_attempts","server_id":"cancel_fixture"}))?;
        assert_eq!(attempts["attempts"][0]["state"], "uncertain");
        assert_eq!(attempts["attempts"][0]["reason"], "transport_error");
        let blocked = controller.request(json!({"action":"mcp_run","server_id":"cancel_fixture","tool":"write","expected_revision":allowed["server"]["revision"],"arguments":{}}))?;
        assert_eq!(blocked["status"], "review_required");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        db.with_connection(|c| {
            let count: i64 =
                c.query_row("SELECT count(*) FROM plugin_mcp_attempts", [], |r| r.get(0))?;
            assert_eq!(count, 1);
            Ok(())
        })?;
        Ok(())
    })();
    controller.0.mcp.stop().unwrap();
    proxy.stop().unwrap();
    remote.stop().unwrap();
    db.close().unwrap();
    result.unwrap();
}
