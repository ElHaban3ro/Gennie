//! Ensayo optativo del adaptador conversacional real con herramientas de lectura aisladas.
use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(name: &str, args: Value) -> Result<Value> {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/probes/real_web_code_tool.py");
    let mut child = Command::new("python3")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let request = json!({"name":name,"arguments":args});
    let mut input = child.stdin.take().ok_or("Falta stdin de herramienta")?;
    input.write_all(&serde_json::to_vec(&request)?)?;
    drop(input);
    let output = child.wait_with_output()?;
    if !output.status.success() || output.stdout.len() > 128 * 1024 {
        return Err("Falló la herramienta de prueba".into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn usage(dir: &Path, thread: &str, out: &mut Vec<Value>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            usage(&path, thread, out);
            continue;
        }
        if path.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let mut matches = false;
        let mut count = None;
        for line in text.lines() {
            let Ok(item) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if item["type"] == "session_meta" && item["payload"]["id"] == thread {
                matches = true;
            }
            if item["payload"]["type"] == "token_count" {
                count = item["payload"]["info"].get("total_token_usage").cloned();
            }
        }
        if matches && let Some(value) = count {
            out.push(value);
        }
    }
}

#[test]
#[ignore = "Consulta dos páginas públicas y ejecuta JavaScript de lectura bajo Deno aislado; consume Codex real"]
fn real_web_code_gennie_conversation() {
    let settings_path = std::env::var_os("GENNIE_REAL_WEB_SETTINGS").unwrap();
    let value: Value = serde_json::from_slice(&std::fs::read(settings_path).unwrap()).unwrap();
    // El perfil y sus instrucciones son los del bot; el banco restringe temporalmente
    // archivos, comandos y control local para equiparar las tres variantes.
    let settings = Settings {
        read_files: false,
        write_files: false,
        run_commands: false,
        local_actions: false,
        model: value["model"].as_str().unwrap().into(),
        effort: value["effort"].as_str().unwrap().into(),
        personality: value["personality"].as_str().unwrap_or("").into(),
        instructions: value["instructions"].as_str().unwrap_or("").into(),
    };
    let mut registry = gennie_mcp::Registry::default();
    registry.register([
        gennie_mcp::Tool { definition:json!({"name":"inspect_public_page","description":"Lee el HTML real de la página pública 1 o 2 del sitio autorizado.","inputSchema":{"type":"object","properties":{"page":{"type":"integer","enum":[1,2]}},"required":["page"],"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
            call:Box::new(|args|invoke("inspect_public_page",args)) },
        gennie_mcp::Tool { definition:json!({"name":"run_javascript","description":"Ejecuta JavaScript en Deno con acceso de solo lectura a las dos páginas autorizadas y compara su JSON con un oráculo independiente. Máximo cuatro ejecuciones por hilo.","inputSchema":{"type":"object","properties":{"source":{"type":"string","minLength":1,"maxLength":32000}},"required":["source"],"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
            call:Box::new(|args|invoke("run_javascript",args)) },
    ]).unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let root = std::env::temp_dir().join(format!(
        "gennie-real-web-bot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let started = Instant::now();
    let process = ProcessHandle::default();
    let watchdog_process = process.clone();
    let (done, wait) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if wait.recv_timeout(Duration::from_secs(180)).is_err() {
            watchdog_process.cancel();
        }
    });
    let result = (|| -> Result<Value> {
        let db = Database::memory()?;
        db.migrate(
            crate::LlmModule::default().name(),
            crate::LlmModule::default().migrations(),
        )?;
        let control = turn::Control::new(process.clone());
        let scope_seconds = match std::env::var("GENNIE_REAL_WEB_SCOPE_SECONDS").as_deref() {
            Ok("60") => 60,
            _ => 180,
        };
        let scope = Scope::with_timeout(&control, Duration::from_secs(scope_seconds))?;
        let mut session = Session::spawn(
            process.clone(),
            root.clone(),
            &settings,
            Some(&server.launch()),
            &[
                "web_search=\"disabled\"".into(),
                "features.multi_agent=false".into(),
                "features.apply_patch_freeform=false".into(),
            ],
            true,
            Some(scope.ticket.context.clone()),
        )?;
        session.bind_turn(Some(scope.ticket.clone()));
        session.authenticate_model(&settings)?;
        session.journal = Some(Arc::new(Journal::default()));
        let composed = instructions(&settings, &session.tool_names);
        let response = session.open_thread(None, &composed)?;
        let thread = response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("Falta hilo")?
            .to_owned();
        save_recorded_thread(&db, &thread, None, session.journal.as_deref())?;
        session.thread_id = thread.clone();
        session.send(json!({"id":13,"method":"mcpServerStatus/list","params":{"threadId":thread,"serverName":"gennie","detail":"toolsAndAuthOnly"}}))?;
        server
            .launch()
            .validate_inventory(&session.response(13, HANDSHAKE_TIMEOUT)?)?;
        let prompt = std::fs::read_to_string(
            std::env::var_os("GENNIE_REAL_WEB_PROMPT").ok_or("Falta tarea")?,
        )?;
        let mut messages = Vec::new();
        let (answer, turn_protocol) = super::super::turn_observation::capture(|| {
            session.speak_stream_reply(&prompt, None, |event| {
                if let StreamEvent::Completed(text) = event {
                    messages.push(text);
                }
            })
        });
        let mut diagnostic = json!({});
        super::diagnostics::finish(
            &mut diagnostic,
            &answer,
            started.elapsed().as_millis(),
            true,
        );
        let scope_timed_out = scope.timed_out.load(Ordering::Acquire);
        drop(scope);
        session.bind_turn(None);
        drop(session);
        let mut counts = Vec::new();
        usage(&root, &thread, &mut counts);
        db.close()?;
        Ok(
            json!({"status":if answer.is_ok(){"turn_finished"}else{"turn_failed"},"thread_id":thread,
            "messages":messages,"usage_candidates":counts,"turn_protocol":turn_protocol,
            "failure":diagnostic["failure"],"scope_timed_out":scope_timed_out,
            "scope_seconds":scope_seconds}),
        )
    })();
    let _ = done.send(());
    let _ = watchdog.join();
    let stopped = server.stop().is_ok();
    let report = match result {
        Ok(mut value) => {
            value["server_stopped"] = json!(stopped);
            value
        }
        Err(_) => json!({"status":"technical_failure","server_stopped":stopped}),
    };
    let output = std::env::var_os("GENNIE_REAL_WEB_AGENT_OUTPUT").unwrap();
    std::fs::write(
        output,
        serde_json::to_vec(&json!({"agent":report,"elapsed_ms":started.elapsed().as_millis()}))
            .unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}
