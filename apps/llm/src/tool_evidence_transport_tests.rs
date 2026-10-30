use super::*;
use crate::{ToolOutcome, ToolUse};

#[derive(Default, Debug)]
struct Audit {
    requests: Vec<String>,
    systems: Vec<String>,
    turns: Vec<String>,
    calls: Vec<ToolUse>,
    responses: Vec<(Option<String>, String)>,
    finished: Vec<String>,
    interruptions: Vec<(Option<String>, String, crate::ReplyInterruption)>,
}
#[derive(Default)]
struct Journal {
    audit: Mutex<Audit>,
    fail_response: bool,
}
impl ConversationJournal for Journal {
    fn activate(
        &self,
        _: &gennie_db::SqliteConnection,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
    fn append(&self, _: &str, thread: &str, speaker: &str, text: &str) -> Result<()> {
        assert_eq!(thread, "active");
        match speaker {
            "user" => self.audit.lock().unwrap().requests.push(text.into()),
            "system" => self.audit.lock().unwrap().systems.push(text.into()),
            _ => panic!("Emisor inesperado"),
        }
        Ok(())
    }
    fn begin_turn(&self, _: &str, thread: &str, turn: &str) -> Result<()> {
        assert_eq!(thread, "active");
        self.audit.lock().unwrap().turns.push(turn.into());
        Ok(())
    }
    fn record_tool(&self, _: &str, thread: &str, turn: &str, usage: &ToolUse) -> Result<()> {
        assert_eq!((thread, turn), ("active", "turn"));
        self.audit.lock().unwrap().calls.push(usage.clone());
        Ok(())
    }
    fn append_response(
        &self,
        _: &str,
        thread: &str,
        turn: &str,
        item: Option<&str>,
        text: &str,
    ) -> Result<()> {
        assert_eq!((thread, turn), ("active", "turn"));
        if self.fail_response {
            return Err("No se pudo confirmar la respuesta".into());
        }
        self.audit
            .lock()
            .unwrap()
            .responses
            .push((item.map(str::to_owned), text.into()));
        Ok(())
    }
    fn append_interrupted_response(
        &self,
        _: &str,
        thread: &str,
        turn: &str,
        item: Option<&str>,
        text: &str,
        reason: crate::ReplyInterruption,
    ) -> Result<()> {
        assert_eq!((thread, turn), ("active", "turn"));
        if self.fail_response {
            return Err("Fallo de guardado ficticio".into());
        }
        self.audit.lock().unwrap().interruptions.push((
            item.map(str::to_owned),
            text.into(),
            reason,
        ));
        Ok(())
    }
    fn finish_turn(&self, _: &str, thread: &str, turn: &str) -> Result<()> {
        assert_eq!(thread, "active");
        self.audit.lock().unwrap().finished.push(turn.into());
        Ok(())
    }
    fn committed(&self) {}
}

fn session(journal: Arc<Journal>, interrupted: bool) -> Session {
    let tail = if interrupted {
        r#"
printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"partial","delta":"Respuesta parcial"}}' '{"method":"turn/completed","params":{"turn":{"id":"turn","status":"interrupted"}}}'
"#
    } else {
        r#"
printf '%s\n' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","completedAtMs":1793232000001,"item":{"type":"mcpToolCall","id":"call","server":"gennie","tool":"browser_plugin_run","status":"completed","arguments":{"plugin_id":"mail","function_name":"list","action":"run","arguments":{"token":"PRIVATE_ARGUMENT"}},"result":{"content":[{"type":"text","text":"PRIVATE_RESULT"}],"structuredContent":{"status":"ok","plugin_id":"mail","function_name":"list","plugin_name":"Correo","data":"PRIVATE_RESULT"}}}}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","completedAtMs":1793232000002,"item":{"type":"agentMessage","id":"response","text":"Resultado literal"}}}'
printf '%s\n' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","completedAtMs":1793232000002,"item":{"type":"agentMessage","id":"response","text":"Resultado literal"}}}'
printf '%s\n' '{"method":"turn/completed","params":{"turn":{"id":"turn","status":"completed"}}}'
"#
    };
    session_with_tail(journal, tail)
}

pub(super) fn successful_tool_session_for_probe() -> Session {
    session(Arc::new(Journal::default()), false)
}
fn session_with_tail(journal: Arc<Journal>, tail: &str) -> Session {
    session_with_process(journal, tail, ProcessHandle::default())
}
fn session_with_process(journal: Arc<Journal>, tail: &str, process: ProcessHandle) -> Session {
    let script = r#"
while IFS= read -r request; do
 case "$request" in
 *'"id":4,'*)
  printf '%s\n' '{"id":4,"result":{"turn":{"id":"turn"}}}'
  printf '%s\n' '{"method":"item/started","params":{"threadId":"foreign","turnId":"turn","startedAtMs":1793232000000,"item":{"type":"mcpToolCall","id":"foreign","server":"private","tool":"foreign","status":"inProgress","arguments":{}}}}'
  printf '%s\n' '{"method":"item/started","params":{"threadId":"active","turnId":"turn","startedAtMs":1793232000000,"item":{"type":"mcpToolCall","id":"call","server":"gennie","tool":"browser_plugin_run","status":"inProgress","arguments":{"plugin_id":"mail","function_name":"list","action":"run","arguments":{"token":"PRIVATE_ARGUMENT"}}}}}'
  __TAIL__ ;;
 *'"id":7,'*) printf '%s\n' '{"id":7,"result":{"thread":{"id":"active","name":null}}}' ;;
 *) exit 3 ;;
 esac
done
"#;
    let mut child = Command::new("/bin/sh")
        .args(["-c", &script.replace("__TAIL__", tail)])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    *process.0.lock().unwrap() = Some(child);
    let (sender, lines) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    Session {
        stdin,
        lines,
        reader: Some(reader),
        process,
        thread_id: "active".into(),
        journal: Some(journal),
        cwd: std::env::temp_dir(),
        model: "fixture".into(),
        effort: "low".into(),
        title_effort: None,
        settings: Settings::default(),
        tool_names: Vec::new(),
        has_mcp: false,
        cancellation: None,
        ticket: None,
    }
}

#[test]
fn actual_receive_path_correlates_events_keeps_payloads_private_and_saves_each_item_once() {
    let journal = Arc::new(Journal::default());
    let mut session = session(journal.clone(), false);
    let mut events = Vec::new();
    session
        .speak_stream("  Entrada literal\n", |event| events.push(event))
        .unwrap();
    let audit = journal.audit.lock().unwrap();
    assert_eq!(audit.requests, ["  Entrada literal\n"]);
    assert_eq!(audit.turns, ["turn"]);
    assert_eq!(audit.finished, ["turn"]);
    assert_eq!(audit.calls.len(), 2);
    assert_eq!(audit.calls[0].outcome, ToolOutcome::Running);
    assert_eq!(audit.calls[1].outcome, ToolOutcome::Completed);
    assert_eq!(audit.calls[1].display_name.as_deref(), Some("Correo"));
    assert_eq!(
        audit.responses,
        [(Some("response".into()), "Resultado literal".into())]
    );
    assert!(!format!("{audit:?}").contains("PRIVATE"));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, StreamEvent::Completed(_)))
            .count(),
        1
    );
}

#[test]
fn turn_observation_reaches_actual_server_request_failure_without_private_values() {
    let mut session = failed_server_request_session();
    let (result, trace) =
        turn_observation::capture(|| session.speak_stream("PRIVATE_INPUT", |_| {}));
    assert_eq!(
        result.unwrap_err().to_string(),
        "El proveedor solicitó una acción no disponible para la conversación por voz"
    );
    assert_eq!(trace["events_seen"], 4);
    assert_eq!(trace["events"][3]["kind"], "server_request");
    assert_eq!(trace["events"][3]["method"], "item/tool/requestUserInput");
    assert!(!trace.to_string().contains("PRIVATE"));
}

pub(super) fn failed_server_request_session() -> Session {
    session_with_tail(
        Arc::new(Journal::default()),
        r#"
printf '%s\n' '{"id":"PRIVATE_ID","method":"item/tool/requestUserInput","params":{"threadId":"active","turnId":"turn","questions":[{"question":"PRIVATE_QUESTION","token":"PRIVATE_TOKEN"}]}}'
"#,
    )
}

#[test]
fn turn_observation_reaches_actual_completed_and_malformed_receive_paths() {
    let mut complete = session(Arc::new(Journal::default()), false);
    let (result, trace) =
        turn_observation::capture(|| complete.speak_stream("PRIVATE_INPUT", |_| {}));
    result.unwrap();
    assert!(
        trace["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["turn_status"] == "completed")
    );
    assert!(!trace.to_string().contains("Resultado literal"));
    assert!(!trace.to_string().contains("PRIVATE"));
    let mut malformed = session_with_tail(
        Arc::new(Journal::default()),
        "printf '%s\\n' 'PRIVATE_MALFORMED'",
    );
    let (result, trace) = turn_observation::capture(|| malformed.speak_stream("Consulta", |_| {}));
    assert!(result.is_err());
    assert_eq!(
        trace["events"].as_array().unwrap().last().unwrap()["kind"],
        "parse_failure"
    );
    assert!(!trace.to_string().contains("PRIVATE"));
}
#[test]
fn interruption_closes_the_evidence_scope_without_inventing_a_completed_reply() {
    let journal = Arc::new(Journal::default());
    let mut session = session(journal.clone(), true);
    let mut events = Vec::new();
    assert!(
        session
            .speak_stream("Consulta", |event| events.push(event))
            .is_err()
    );
    let audit = journal.audit.lock().unwrap();
    assert_eq!(audit.finished, ["turn"]);
    assert!(audit.responses.is_empty());
    assert_eq!(audit.calls[0].outcome, ToolOutcome::Running);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,StreamEvent::Delta(text) if text=="Respuesta parcial"))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, StreamEvent::Completed(_)))
    );
}

#[test]
fn user_stop_seals_only_received_fragment_and_keeps_completed_item_and_literal_input() {
    use std::time::Duration;
    let journal = Arc::new(Journal::default());
    let mut session = session_with_tail(
        journal.clone(),
        r#"
printf '%s\n' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","item":{"type":"agentMessage","id":"first","text":"Completo antes de detener"}}}'
printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"fragment","delta":"  Recibido á🦀\n"}}'
while IFS= read -r ignored; do :; done
"#,
    );
    let control = crate::turn::Control::new(session.process.clone());
    let ticket = control.begin().unwrap();
    session.bind_turn(Some(ticket.clone()));
    let (sender, received) = mpsc::channel();
    let task = std::thread::spawn(move || {
        let mut events = Vec::new();
        let result = session.speak_stream(" Entrada literal\n", |event| {
            if matches!(event, StreamEvent::Delta(_)) {
                sender.send(()).unwrap();
            }
            events.push(event);
        });
        (result, events)
    });
    received.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(control.cancel(&ticket.id).unwrap());
    let (result, events) = task.join().unwrap();
    assert!(result.unwrap_err().is::<gennie_mcp::Cancelled>());
    let audit = journal.audit.lock().unwrap();
    assert_eq!(audit.requests, [" Entrada literal\n"]);
    assert_eq!(
        audit.responses,
        [(Some("first".into()), "Completo antes de detener".into())]
    );
    assert_eq!(
        audit.interruptions,
        [(
            Some("fragment".into()),
            "  Recibido á🦀\n".into(),
            crate::ReplyInterruption::Cancelled
        )]
    );
    assert_eq!(audit.finished, ["turn"]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Interrupted { saved: true, .. }))
    );
}

#[test]
fn stopping_before_user_commit_keeps_input_unacknowledged_and_failed_fragment_save_stays_visible() {
    let journal = Arc::new(Journal::default());
    let mut session = session(journal.clone(), false);
    let control = crate::turn::Control::new(session.process.clone());
    let ticket = control.begin().unwrap();
    session.bind_turn(Some(ticket.clone()));
    control.cancel(&ticket.id).unwrap();
    let mut events = Vec::new();
    assert!(
        session
            .speak_stream("No enviado", |e| events.push(e))
            .is_err()
    );
    assert!(events.is_empty());
    assert!(journal.audit.lock().unwrap().requests.is_empty());
    let journal = Arc::new(Journal {
        fail_response: true,
        ..Default::default()
    });
    let mut session = session_with_tail(
        journal.clone(),
        r#"
printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"partial","delta":"Conservar visible"}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"active","turn":{"id":"turn","status":"failed"}}}'
"#,
    );
    let mut events = Vec::new();
    assert!(session.speak_stream("Entrada", |e| events.push(e)).is_err());
    assert!(events.iter().any(|e|matches!(e,StreamEvent::Interrupted{text,saved:false,reason:crate::ReplyInterruption::Failed} if text=="Conservar visible")));
    assert!(journal.audit.lock().unwrap().interruptions.is_empty());
}

#[test]
fn actual_worker_cancels_exact_generation_recovers_next_input_and_drains_on_shutdown() {
    use gennie_core::{AppModule, AppState};
    use std::time::{Duration, Instant};
    let journal = Arc::new(Journal::default());
    let source = journal.clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let connector: crate::TestConnector = Arc::new(move |process| {
        let n = observed.fetch_add(1, Ordering::SeqCst);
        let tail = if n == 2 {
            r#"if [ "$delivered" = yes ]; then
printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"last","delta":"Último fragmento"}}'; while IFS= read -r ignored; do :; done
else
delivered=yes
printf '%s\n' '{"method":"item/completed","params":{"threadId":"active","turnId":"turn","item":{"type":"agentMessage","id":"done","text":"Respuesta siguiente"}}}' '{"method":"turn/completed","params":{"turn":{"id":"turn","status":"completed"}}}'
fi"#
        } else {
            r#"printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"fragment","delta":"Fragmento recibido"}}'; while IFS= read -r ignored; do :; done"#
        };
        Ok(session_with_process(source.clone(), tail, process))
    });
    let module = crate::LlmModule {
        test_connector: Mutex::new(Some(connector)),
        ..Default::default()
    };
    let state = AppState {
        config: Arc::new(gennie_config::Config {
            database_path: PathBuf::from("unused.sqlite3"),
            db_busy_timeout_ms: 50,
        }),
        db: Database::memory().unwrap(),
        shutdown: Default::default(),
    };
    state
        .db
        .migrate(module.name(), module.migrations())
        .unwrap();
    module.start(&state).unwrap();
    let controller = module.controller().unwrap();
    let wait = |id: &str, finished: bool| {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            events.extend(controller.poll());
            if events.iter().any(|e|matches!(e,crate::Event::Speech{id:own,event} if own==id && if finished{matches!(event,crate::SpeechEvent::Finished{..})}else{matches!(event,crate::SpeechEvent::Delta(_))})){return events;}
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("No llegó el evento del fixture");
    };
    let first = controller.speak_turn("Primera", None).unwrap();
    wait(&first, false);
    assert!(controller.cancel_turn(&first).unwrap());
    let events = wait(&first, true);
    assert!(events.iter().any(|e| matches!(
        e,
        crate::Event::Speech {
            event: crate::SpeechEvent::Finished {
                cancelled: true,
                error: None
            },
            ..
        }
    )));
    assert!(!controller.is_pending());
    assert!(controller.turn_progress().is_none());
    let second = controller.speak_turn("Segunda", None).unwrap();
    wait(&second, false);
    assert!(!controller.cancel_turn(&first).unwrap());
    assert_eq!(controller.turn_progress().unwrap().id, second);
    assert!(controller.cancel_turn(&second).unwrap());
    wait(&second, true);
    let third = controller.speak_turn("Tercera", None).unwrap();
    let events = wait(&third, true);
    assert!(events.iter().any(|e|matches!(e,crate::Event::Speech{event:crate::SpeechEvent::Complete(text),..} if text=="Respuesta siguiente")));
    let fourth = controller.speak_turn("Cuarta", None).unwrap();
    wait(&fourth, false);
    module.stop(&state).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(journal.audit.lock().unwrap().interruptions.len(), 3);
    state.db.close().unwrap();
}

#[test]
fn actual_worker_preserves_partial_provider_failure_and_reports_it_instead_of_user_stop() {
    use gennie_core::{AppModule, AppState};
    use std::time::{Duration, Instant};
    let journal = Arc::new(Journal::default());
    let source = journal.clone();
    let connector: crate::TestConnector = Arc::new(move |process| {
        Ok(session_with_process(
            source.clone(),
            r#"
printf '%s\n' '{"method":"item/agentMessage/delta","params":{"threadId":"active","turnId":"turn","itemId":"partial","delta":"Fragmento antes del fallo"}}' '{"method":"turn/completed","params":{"turn":{"id":"turn","status":"failed"}}}'
"#,
            process,
        ))
    });
    let module = crate::LlmModule {
        test_connector: Mutex::new(Some(connector)),
        ..Default::default()
    };
    let state = AppState {
        config: Arc::new(gennie_config::Config {
            database_path: PathBuf::from("unused.sqlite3"),
            db_busy_timeout_ms: 50,
        }),
        db: Database::memory().unwrap(),
        shutdown: Default::default(),
    };
    state
        .db
        .migrate(module.name(), module.migrations())
        .unwrap();
    module.start(&state).unwrap();
    let controller = module.controller().unwrap();
    let id = controller.speak_turn("Entrada", None).unwrap();
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        events.extend(controller.poll());
        if events.iter().any(|e| {
            matches!(
                e,
                crate::Event::Speech {
                    event: crate::SpeechEvent::Finished { .. },
                    ..
                }
            )
        }) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    module.stop(&state).unwrap();
    assert!(events.iter().any(|e|matches!(e,crate::Event::Speech{id:own,event:crate::SpeechEvent::Interrupted{saved:true,reason:crate::ReplyInterruption::Failed,..}} if own==&id)));
    assert!(events.iter().any(|e|matches!(e,crate::Event::Speech{id:own,event:crate::SpeechEvent::Finished{cancelled:false,error:Some(_),..}} if own==&id)));
    assert_eq!(journal.audit.lock().unwrap().systems.len(), 1);
    state.db.close().unwrap();
}
#[test]
fn failed_response_commit_keeps_visible_text_and_closes_the_pending_evidence() {
    let journal = Arc::new(Journal {
        fail_response: true,
        ..Journal::default()
    });
    let mut session = session(journal.clone(), false);
    let mut events = Vec::new();
    assert!(
        session
            .speak_stream("Consulta", |event| events.push(event))
            .is_err()
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,StreamEvent::Completed(text) if text=="Resultado literal"))
    );
    let audit = journal.audit.lock().unwrap();
    assert!(audit.responses.is_empty());
    assert_eq!(audit.finished, ["turn"]);
    assert_eq!(audit.calls.len(), 2);
}

#[test]
#[ignore = "Consume una respuesta real; herramienta ficticia de solo lectura, sin cuentas ni navegador"]
fn real_model_emits_correlated_mcp_evidence_without_persisting_tool_payloads() {
    use gennie_core::AppModule;
    #[derive(Default)]
    struct RealJournal(Mutex<Vec<ToolUse>>);
    impl ConversationJournal for RealJournal {
        fn activate(
            &self,
            _: &gennie_db::SqliteConnection,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }
        fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn record_tool(&self, _: &str, _: &str, _: &str, usage: &ToolUse) -> Result<()> {
            self.0.lock().unwrap().push(usage.clone());
            Ok(())
        }
        fn committed(&self) {}
    }
    let mut registry = gennie_mcp::Registry::default();
    registry.register([gennie_mcp::Tool {
        definition:json!({"name":"evidence_fixture","description":"Consulta ficticia local de solo lectura. Devuelve el código de la prueba sin red ni efectos.","inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false}}),
        call:Box::new(|_|Ok(json!({"code":"EVIDENCE-27","private_fixture":"PRIVATE_RESULT"}))),
    }]).unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let base = std::env::temp_dir().join(format!(
        "gennie-real-evidence-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(base.clone());
    let db = Database::memory().unwrap();
    let module = crate::LlmModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
    let journal = Arc::new(RealJournal::default());
    let process = ProcessHandle::default();
    let timeout = process.clone();
    let (done, finish) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish.recv_timeout(Duration::from_secs(30)).is_err() {
            timeout.kill();
        }
    });
    let result = (|| -> Result<String> {
        let settings = Settings {
            read_files: false,
            write_files: false,
            run_commands: false,
            local_actions: false,
            ..Settings::default()
        };
        let mut session = Session::connect_in_with_mode(
            &db,
            process,
            base,
            true,
            &settings,
            Some(journal.clone()),
            Some(server.launch()),
        )?;
        session.speak("Prueba aislada: llama una vez a la herramienta MCP evidence_fixture y responde solo con el código que devuelva. No uses otras herramientas. Esta herramienta es ficticia, de solo lectura, sin red ni acceso a archivos o cuentas.")
    })();
    let _ = done.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    let reply = result.unwrap();
    assert!(reply.contains("EVIDENCE-27"));
    let audit = journal.0.lock().unwrap();
    assert!(audit.iter().any(|u| u.server == "gennie"
        && u.tool == "evidence_fixture"
        && u.outcome == ToolOutcome::Running));
    assert!(audit.iter().any(|u| u.server == "gennie"
        && u.tool == "evidence_fixture"
        && u.outcome == ToolOutcome::Completed));
    assert!(!format!("{audit:?}").contains("PRIVATE_RESULT"));
}
