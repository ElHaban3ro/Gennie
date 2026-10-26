//! El helper stdio conserva contenido de imagen que supera el límite antiguo de texto.
use gennie_mcp::{Registry, Server, Tool};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

#[test]
#[ignore = "Requiere Codex instalado; solo descubre herramientas, sin cuenta ni inferencia"]
fn installed_codex_discovers_full_browser_catalog_without_inference() {
    let module = gennie_browser::BrowserModule::default();
    let mut registry = Registry::default();
    registry
        .register(gennie_browser::tools(module.controller()))
        .unwrap();
    let mut server =
        Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie"))).unwrap();
    let launch = server.launch();
    assert_eq!(launch.tool_names.len(), 21);
    let directory =
        std::env::temp_dir().join(format!("gennie-browser-discovery-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(directory.clone());
    let binary = std::env::var_os("GENNIE_TEST_CODEX").unwrap_or_else(|| "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex".into());
    let mut child = Command::new(binary)
        .args([
            "app-server",
            "--listen",
            "stdio://",
            "-c",
            &launch.config().unwrap(),
            "-c",
            "features.apps=false",
            "-c",
            "features.plugins=false",
        ])
        .env("CODEX_HOME", &directory)
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let result = (|| -> Result<(), String> {
        let mut call = |request: Value| -> Result<Value, String> {
            writeln!(input, "{request}").map_err(|e| e.to_string())?;
            input.flush().map_err(|e| e.to_string())?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
            loop {
                let line = receiver
                    .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .map_err(|_| "Codex no respondió en 12 s")?;
                let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                if response["id"] == request["id"] {
                    if response.get("error").is_some() {
                        return Err("Codex rechazó el descubrimiento".into());
                    }
                    return Ok(response["result"].clone());
                }
            }
        };
        call(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"gennie_browser_probe","version":"1"},"capabilities":{"experimentalApi":true}}}),
        )?;
        let inventory = call(
            json!({"jsonrpc":"2.0","id":2,"method":"mcpServerStatus/list","params":{"serverName":"gennie"}}),
        )?;
        launch
            .validate_inventory(&inventory)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    drop(input);
    let _ = child.kill();
    let _ = child.wait();
    reader.join().unwrap();
    server.stop().unwrap();
    result.unwrap();
}

#[test]
fn browser_images_cross_the_real_stdio_bridge_without_truncation() {
    let mut registry = Registry::default();
    registry
        .register([Tool {
            definition: json!({"name":"test_browser_image","inputSchema":{"type":"object"}}),
            call: Box::new(|_| {
                gennie_mcp::with_image(json!({"bytes":150000}), "A".repeat(200000), "image/jpeg")
            }),
        }])
        .unwrap();
    let mut server =
        Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie"))).unwrap();
    let launch = server.launch();
    let mut child = Command::new(&launch.helper)
        .arg("--mcp")
        .arg(&launch.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin,"{}", json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"test_browser_image","arguments":{}}})).unwrap();
    stdin.flush().unwrap();
    let output = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let received = receiver.recv_timeout(std::time::Duration::from_secs(5));
    if received.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    reader.join().unwrap();
    let line = received
        .expect("El puente no devolvió una línea completa en 5 s")
        .unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(
        response["result"]["content"][1]["data"]
            .as_str()
            .unwrap()
            .len(),
        200000
    );
    assert_eq!(
        response["result"]["structuredContent"],
        json!({"bytes":150000})
    );
    drop(stdin);
    assert!(child.wait().unwrap().success());
    server.stop().unwrap();
}
