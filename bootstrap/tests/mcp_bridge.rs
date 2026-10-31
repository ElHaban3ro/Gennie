//! Recorre el helper real y el host con SQLite propia, sin Codex, cuenta ni inferencia.
use gennie_core::AppModule;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
#[test]
fn stdio_preserves_parse_errors_null_ids_and_notifications_without_closing() {
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    let mut server = gennie_mcp::Server::start(
        gennie_mcp::Registry::default(),
        std::path::Path::new(env!("CARGO_BIN_EXE_gennie")),
    )
    .unwrap();
    let launch = server.launch();
    let mut child = Command::new(&launch.helper)
        .arg("--mcp")
        .arg(&launch.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (sender, replies) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let result = (|| -> Result<(), String> {
        for (request, expected) in [
            ("{broken", Some(-32700)),
            ("42", Some(-32600)),
            (r#"{"jsonrpc":"1.0","method":"ping"}"#, Some(-32600)),
            (r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#, None),
        ] {
            writeln!(input, "{request}").map_err(|e| e.to_string())?;
            input.flush().map_err(|e| e.to_string())?;
            let line = replies
                .recv_timeout(Duration::from_secs(2))
                .map_err(|e| e.to_string())?;
            let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if response["id"] != Value::Null {
                return Err("El error alteró id=null".into());
            }
            if let Some(code) = expected {
                if response["error"]["code"] != code {
                    return Err("Falta error del protocolo".into());
                }
            } else if response["result"] != json!({}) {
                return Err("Falta respuesta ping con id=null".into());
            }
        }
        writeln!(
            input,
            "{}\n{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":7,"method":"ping"})
        )
        .map_err(|e| e.to_string())?;
        input.flush().map_err(|e| e.to_string())?;
        let line = replies
            .recv_timeout(Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if response["id"] != 7 || response["result"] != json!({}) {
            return Err("La notificación consumió otra respuesta".into());
        }
        Ok(())
    })();
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    reader.join().unwrap();
    server.stop().unwrap();
    result.unwrap();
}
#[test]
fn helper_eof_cancels_host_context_even_with_a_blocked_output_consumer() {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    for blocked_output in [false, true] {
        let observed = Arc::new(Mutex::new(None::<gennie_mcp::CallContext>));
        let captured = observed.clone();
        let mut registry = gennie_mcp::Registry::default();
        registry
            .register([gennie_mcp::ContextTool {
                definition: json!({"name":"wait"}),
                call: Box::new(move |_, context| {
                    *captured.lock().unwrap() = Some(context.clone());
                    if blocked_output {
                        return Ok(json!({"payload":"x".repeat(1024 * 1024)}));
                    }
                    while !context.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    context.check()?;
                    Ok(json!({}))
                }),
            }])
            .unwrap();
        let mut server =
            gennie_mcp::Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie")))
                .unwrap();
        let launch = server.launch();
        let mut child = Command::new(&launch.helper)
            .arg("--mcp")
            .arg(&launch.socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait"}})
        )
        .unwrap();
        input.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while observed.lock().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let context = observed.lock().unwrap().clone();
        if blocked_output {
            // Conserva stdout abierto sin consumir una respuesta mayor que el pipe.
            std::thread::sleep(Duration::from_millis(100));
        }
        drop(input);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut exited = false;
        while Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !exited {
            let _ = child.kill();
        }
        let _ = child.wait();
        let deadline = Instant::now() + Duration::from_secs(2);
        while context.as_ref().is_some_and(|c| !c.is_cancelled()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        server.stop().unwrap();
        assert!(
            exited,
            "El helper quedó retenido por la respuesta o por stdout"
        );
        assert!(context.unwrap().is_cancelled());
    }
}
#[test]
fn helper_observes_stdin_eof_while_waiting_for_a_tool_response() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let saw = entered.clone();
    let done = release.clone();
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register([gennie_mcp::Tool {
            definition: json!({"name":"wait"}),
            call: Box::new(move |_| {
                saw.store(true, Ordering::Release);
                while !done.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(json!({"fixture":true}))
            }),
        }])
        .unwrap();
    let mut server =
        gennie_mcp::Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie")))
            .unwrap();
    let launch = server.launch();
    let mut child = Command::new(&launch.helper)
        .arg("--mcp")
        .arg(&launch.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait"}})
    )
    .unwrap();
    input.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !entered.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let started = entered.load(Ordering::Acquire);
    drop(input);
    let deadline = Instant::now() + Duration::from_millis(700);
    let mut exited = false;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    release.store(true, Ordering::Release);
    if !exited {
        let _ = child.kill();
    }
    let _ = child.wait();
    server.stop().unwrap();
    assert!(started, "No llegó la petición ficticia");
    assert!(
        exited,
        "El helper ignoró EOF mientras esperaba la respuesta de la herramienta"
    );
}
#[test]
fn stdio_registers_reads_and_edits_on_the_hosts_single_database() {
    let db = gennie_db::Database::memory().unwrap();
    db.migrate(
        gennie_assignments::AssignmentsModule.migration_namespace(),
        gennie_assignments::AssignmentsModule.migrations(),
    )
    .unwrap();
    let store = gennie_assignments::Store::new(db.clone());
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(gennie_assignments::tools(store.clone()))
        .unwrap();
    let mut server =
        gennie_mcp::Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie")))
            .unwrap();
    let launch = server.launch();
    let mut child = Command::new(&launch.helper)
        .arg("--mcp")
        .arg(&launch.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut call = |request: Value| -> Value {
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let init = call(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    let listed = call(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 6);
    assert!(
        tools
            .iter()
            .any(|tool| tool["name"] == "assignments_execution_get"
                && tool["annotations"]["readOnlyHint"] == true)
    );
    let document = json!({"title":"Investigar autos autónomos","description":"Preparar investigación sobre IA en autos autónomos","kind":"goal","priority":"normal","status":"active","timezone":"America/Bogota","applications":[],"preparation":{"steps":["Reunir fuentes primarias"]}});
    let saved = call(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"assignments_create","arguments":{"request_key":"integration","document":document}}}),
    );
    assert_eq!(saved["result"]["isError"], false, "{saved}");
    let event = &saved["result"]["structuredContent"]["assignment"];
    let id = event["id"].as_i64().unwrap();
    assert_eq!(
        store.get(id).unwrap().document.title,
        "Investigar autos autónomos"
    );
    assert_eq!(store.revision(), 1);
    let got = call(
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"assignments_get","arguments":{"id":id}}}),
    );
    assert_eq!(got["result"]["structuredContent"]["assignment"]["id"], id);
    let mut edited = event["document"].clone();
    edited["status"] = json!("paused");
    let updated = call(
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"assignments_update","arguments":{"id":id,"expected_revision":1,"document":edited}}}),
    );
    assert_eq!(updated["result"]["isError"], false, "{updated}");
    let conflict = call(
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"assignments_update","arguments":{"id":id,"expected_revision":1,"document":edited}}}),
    );
    assert_eq!(conflict["result"]["isError"], true);
    drop(stdin);
    assert!(child.wait().unwrap().success());
    server.stop().unwrap();
    assert!(!launch.socket.exists());
    db.close().unwrap();
}

#[test]
#[ignore = "Requiere el Codex instalado; sin autenticación ni turnos de inferencia"]
fn installed_codex_discovers_and_calls_assignments_without_inference() {
    run_codex_fixture(false);
}

#[test]
#[ignore = "Consume una respuesta real de Codex con una petición ficticia; requiere sesión ChatGPT"]
fn real_delegation_guides_event_setup_without_requesting_screenshots() {
    run_codex_fixture(true);
}

fn run_codex_fixture(evaluate_dialogue: bool) {
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let db = gennie_db::Database::memory().unwrap();
    db.migrate(
        gennie_assignments::AssignmentsModule.migration_namespace(),
        gennie_assignments::AssignmentsModule.migrations(),
    )
    .unwrap();
    let store = gennie_assignments::Store::new(db.clone());
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(gennie_assignments::tools(store.clone()))
        .unwrap();
    let mut server =
        gennie_mcp::Server::start(registry, std::path::Path::new(env!("CARGO_BIN_EXE_gennie")))
            .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "gennie-codex-mcp-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    if evaluate_dialogue {
        let auth = std::env::var_os("CODEX_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".codex")
            })
            .join("auth.json");
        assert!(
            auth.is_file(),
            "La prueba requiere una sesión ChatGPT existente"
        );
        std::os::unix::fs::symlink(auth, directory.join("auth.json")).unwrap();
    }
    struct Process(std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let cli=std::env::var_os("GENNIE_TEST_CODEX").unwrap_or_else(|| "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex".into());
    let mut process = Process(
        Command::new(cli)
            .args([
                "app-server",
                "--listen",
                "stdio://",
                "-c",
                &server.launch().config().unwrap(),
                "-c",
                "features.plugins=false",
                "-c",
                "features.apps=false",
                "-c",
                "web_search=\"disabled\"",
            ])
            .env("CODEX_HOME", &directory)
            .current_dir(&directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut stdin = process.0.stdin.take().unwrap();
    let stdout = process.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if let Ok(line) = line {
                if tx.send(line).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
    });
    let mut call = |request: Value| -> Value {
        writeln!(stdin, "{request}").unwrap();
        stdin.flush().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let line = rx
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("Codex no respondió en 20 s");
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["id"] == request["id"] {
                assert!(value.get("error").is_none(), "{value}");
                return value["result"].clone();
            }
        }
    };
    call(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"gennie_mcp_test","version":"1"},"capabilities":{"experimentalApi":true}}}),
    );
    let inventory = call(
        json!({"jsonrpc":"2.0","id":2,"method":"mcpServerStatus/list","params":{"serverName":"gennie"}}),
    );
    assert!(
        inventory.to_string().contains("assignments_create"),
        "{inventory}"
    );
    server.launch().validate_inventory(&inventory).unwrap();
    let mut params = json!({"cwd":directory,"ephemeral":true,"approvalPolicy":"never"});
    if evaluate_dialogue {
        params["model"] = json!("gpt-6-luna");
        params["developerInstructions"] =
            json!(include_str!("../../apps/llm/src/assistant-instructions.md"));
        params["config"] = json!({"features.shell_tool":false,"features.unified_exec":false,"features.apply_patch_freeform":false});
    }
    let thread = call(json!({"jsonrpc":"2.0","id":3,"method":"thread/start","params":params}));
    let thread_id = thread["thread"]["id"].as_str().unwrap();
    let ready = call(
        json!({"jsonrpc":"2.0","id":13,"method":"mcpServerStatus/list","params":{"threadId":thread_id,"serverName":"gennie","detail":"toolsAndAuthOnly"}}),
    );
    server.launch().validate_inventory(&ready).unwrap();
    if evaluate_dialogue {
        let turn = call(
            json!({"jsonrpc":"2.0","id":14,"method":"turn/start","params":{"threadId":thread_id,"effort":"low","input":[{"type":"text","text":"Me gustaría que me ayudaras a revisar las tareas de mi Apple Notes."}]}}),
        );
        let turn_id = turn["turn"]["id"].as_str().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut reply = String::new();
        let mut completed = false;
        while let Ok(line) =
            rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        {
            let event: Value = serde_json::from_str(&line).unwrap();
            if event["method"] == "item/completed"
                && event.pointer("/params/item/type").and_then(Value::as_str)
                    == Some("agentMessage")
            {
                reply.push_str(
                    event
                        .pointer("/params/item/text")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                reply.push('\n');
            }
            if event["method"] == "turn/completed"
                && event.pointer("/params/turn/id").and_then(Value::as_str) == Some(turn_id)
            {
                completed = event.pointer("/params/turn/status").and_then(Value::as_str)
                    == Some("completed");
                break;
            }
        }
        drop(stdin);
        drop(process);
        reader.join().unwrap();
        server.stop().unwrap();
        db.close().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        eprintln!("Respuesta al ejemplo ficticio: {reply}");
        assert!(completed, "La prueba no completó un turno en 30 s");
        let text = reply.to_lowercase();
        assert!(
            !text.contains("captura")
                && !text.contains("pégame")
                && !text.contains("pásame")
                && !text.contains("pegas")
                && !text.contains("pega aquí")
                && !text.contains("compartes"),
            "Pidió trasladar contenido al chat: {reply}"
        );
        assert!(
            [
                "revisión",
                "revisar",
                "seguimiento",
                "hora",
                "diari",
                "puntual",
                "seman"
            ]
            .iter()
            .any(|word| text.contains(word)),
            "No orientó hacia la revisión solicitada: {reply}"
        );
        assert!(
            !text.contains("ya está conectado"),
            "Afirmó una conexión inexistente"
        );
        assert!(
            ![
                "no está implementad",
                "no están implementad",
                "acceso todavía queda pendiente",
                "no se ejecutan automáticamente",
                "no hará que"
            ]
            .iter()
            .any(|phrase| text.contains(phrase)),
            "Añadió una advertencia técnica automática: {reply}"
        );
        assert!(reply.len() > 10);
        return;
    }
    let document = json!({"title":"Prueba local sin inferencia","description":"Verificar el registro desde Codex","kind":"goal","priority":"normal","status":"active","timezone":"UTC"});
    let saved = call(
        json!({"jsonrpc":"2.0","id":4,"method":"mcpServer/tool/call","params":{"threadId":thread_id,"server":"gennie","tool":"assignments_create","arguments":{"request_key":"codex-no-inference","document":document}}}),
    );
    assert!(!saved.to_string().contains("\"isError\":true"), "{saved}");
    assert_eq!(store.list(0, 100).unwrap().len(), 1);
    drop(stdin);
    drop(process);
    reader.join().unwrap();
    server.stop().unwrap();
    db.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
