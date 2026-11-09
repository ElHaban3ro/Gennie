//! Prueba real optativa: llamada MCP silenciosa que excede tres minutos.
use super::*;
use std::sync::atomic::AtomicU64;

#[test]
#[ignore = "Consume Codex real y espera más de 180 segundos en una herramienta MCP ficticia"]
fn real_long_silent_mcp_turn() {
    let output = PathBuf::from(std::env::var_os("GENNIE_LONG_TURN_OUTPUT").unwrap());
    let value: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("GENNIE_LONG_TURN_SETTINGS").unwrap()).unwrap(),
    )
    .unwrap();
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
    let calls = Arc::new(AtomicU64::new(0));
    let duration_ms = Arc::new(AtomicU64::new(0));
    let mut registry = gennie_mcp::Registry::default();
    let observed = calls.clone();
    let duration = duration_ms.clone();
    registry.register([gennie_mcp::ContextTool {
        definition:json!({"name":"observe_delayed_result","description":"Observación de prueba; termina tras unos 190 segundos y devuelve un valor ficticio.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true}}),
        call:Box::new(move |_, context| {
            observed.fetch_add(1,Ordering::SeqCst);
            let started=Instant::now();
            while started.elapsed() < Duration::from_secs(190) {
                context.check()?;
                std::thread::sleep(Duration::from_millis(100));
            }
            duration.store(started.elapsed().as_millis() as u64,Ordering::SeqCst);
            Ok(json!({"status":"observed","value":"LARGO-OK"}))
        }),
    }]).unwrap();
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let root = std::env::temp_dir().join(format!(
        "gennie-long-turn-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let started = Instant::now();
    let process = ProcessHandle::default();
    let result = (|| -> Result<Value> {
        let db = Database::memory()?;
        db.migrate(
            crate::LlmModule::default().name(),
            crate::LlmModule::default().migrations(),
        )?;
        let control = turn::Control::new(process.clone());
        let scope = Scope::with_timeout(&control, Duration::from_secs(250))?;
        let mut session = Session::spawn(
            process.clone(),
            root.clone(),
            &settings,
            Some(&server.launch()),
            &[
                "web_search=\"disabled\"".into(),
                "features.multi_agent=false".into(),
            ],
            true,
            Some(scope.ticket.context.clone()),
        )?;
        session.bind_turn(Some(scope.ticket.clone()));
        session.authenticate_model(&settings)?;
        session.journal = Some(Arc::new(Journal::default()));
        let thread=session.open_thread(None,&instructions(&settings,&session.tool_names))?["thread"]["id"].as_str().ok_or("Falta hilo")?.to_owned();
        save_recorded_thread(&db, &thread, None, session.journal.as_deref())?;
        session.thread_id = thread.clone();
        session.send(json!({"id":13,"method":"mcpServerStatus/list","params":{"threadId":thread,"serverName":"gennie","detail":"toolsAndAuthOnly"}}))?;
        server
            .launch()
            .validate_inventory(&session.response(13, HANDSHAKE_TIMEOUT)?)?;
        let mut messages = Vec::new();
        let (answer, protocol) = super::super::turn_observation::capture(|| {
            session.speak_stream_reply(
            "Invoca observe_delayed_result una sola vez, espera su resultado completo y comunica literalmente el valor observado. La herramienta tarda más de tres minutos; no adelantes el resultado.",None,|event| {
                if let StreamEvent::Completed(text)=event {messages.push(text);}
            })
        });
        let mut diagnostic = json!({});
        super::diagnostics::finish(
            &mut diagnostic,
            &answer,
            started.elapsed().as_millis(),
            true,
        );
        let timeout = scope.timed_out.load(Ordering::Acquire);
        drop(scope);
        session.bind_turn(None);
        drop(session);
        db.close()?;
        Ok(
            json!({"status":if answer.is_ok(){"turn_finished"}else{"turn_failed"},"failure":diagnostic["failure"],
            "scope_timed_out":timeout,"messages":messages,"protocol":protocol}),
        )
    })();
    let stopped = server.stop().is_ok();
    let mut report = result.unwrap_or_else(|_| json!({"status":"setup_failed"}));
    report["elapsed_ms"] = json!(started.elapsed().as_millis());
    report["calls"] = json!(calls.load(Ordering::SeqCst));
    report["tool_elapsed_ms"] = json!(duration_ms.load(Ordering::SeqCst));
    report["server_stopped"] = json!(stopped);
    std::fs::write(&output, serde_json::to_vec(&report).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(root);
}
