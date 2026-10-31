use super::*;
use gennie_config::Config;
use std::path::PathBuf;
#[path = "runtime_tests/assignment_evaluation/mod.rs"]
mod assignment_evaluation;
#[path = "runtime_tests/assignment_overview.rs"]
mod assignment_overview;
#[path = "runtime_tests/chat_cancellation.rs"]
mod chat_cancellation;
#[path = "runtime_tests/chat_review_probe.rs"]
mod chat_review_probe;
#[path = "runtime_tests/contextual_notice_probe.rs"]
mod contextual_notice_probe;
#[path = "runtime_tests/historical_copy.rs"]
mod historical_copy;
#[path = "runtime_tests/plugin_inventory.rs"]
mod plugin_inventory;
#[path = "runtime_tests/web_attempts.rs"]
mod web_attempts;

#[test]
fn proposal_tools_bind_real_user_rows_refine_and_require_human_application() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='assignments'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let thread = "refinement-origin";
    let id = append_conversation(&backend, thread, "Genera propuestas para este encargo")
        .parse::<i64>()
        .unwrap();
    let document:gennie_assignments::Document=serde_json::from_value(serde_json::json!({"title":"Resumen","description":"Revisar novedades","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"}})).unwrap();
    let record = backend
        .assignments
        .create("refinement-fixture", document.clone())
        .unwrap();
    let context = Arc::new(std::sync::Mutex::new(Some(thread.to_owned())));
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(gennie_assignments::tools(backend.assignments.clone()))
        .unwrap();
    registry
        .register(proposal_tools(
            backend.assignments.clone(),
            backend.state.db.clone(),
            context.clone(),
            Arc::new(|_, _| panic!("No guía remota")),
        ))
        .unwrap();
    let call = |registry: &gennie_mcp::Registry, name: &str, arguments: serde_json::Value| {
        registry.respond(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}})).unwrap()
    };
    let catalog = call(&registry, "assignments_list", serde_json::json!({}));
    let catalog: serde_json::Value =
        serde_json::from_str(catalog["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let entry = &catalog["assignments"][0];
    assert_eq!(entry["instance_id"], record.instance_id);
    let reference = serde_json::json!({"id":entry["id"],"instance_id":entry["instance_id"],"revision":entry["revision"]});
    assert_eq!(
        call(
            &registry,
            "assignments_review_start",
            serde_json::json!({"assignments":[reference],"use_guide":true,"message_id":999})
        )["result"]["isError"],
        true
    );
    let started = call(
        &registry,
        "assignments_review_start",
        serde_json::json!({"assignments":[reference]}),
    );
    assert_eq!(started["result"]["isError"], false);
    let review: serde_json::Value =
        serde_json::from_str(started["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        review["chat_request"]["text"],
        "Genera propuestas para este encargo"
    );
    assert_eq!(review["chat_request"]["conversation_id"], id);
    assert_eq!(review["scope"]["use_guide"], false);
    let before_history = backend.snapshot.history_revision.clone();
    backend.tick();
    assert_eq!(backend.snapshot.history_revision, before_history);
    assert_eq!(
        backend.snapshot.execution_revision,
        Some(backend.assignments.revision().to_string())
    );
    assert!(backend.assignments.get(record.id).unwrap().document == document);
    let proposal = serde_json::json!({"assignment_id":record.id,"title":"Aclarar el horario","rationale":"Concentrar el resumen","expected_benefit":"Menos interrupciones","related_ids":[],"questions":["¿A qué hora prefieres recibirlo?"],"document":null});
    let initial = call(
        &registry,
        "assignments_propose",
        serde_json::json!({"review_id":review["id"],"proposals":[proposal]}),
    );
    assert_eq!(initial["result"]["isError"], false);
    assert_eq!(
        call(
            &registry,
            "assignments_review_refine",
            serde_json::json!({"review_id":review["id"]})
        )["result"]["isError"],
        true
    );
    let journal =
        gennie_presence::PresenceModule::default().conversation_journal(backend.state.db.clone());
    journal
        .append("codex", thread, "user", "A las nueve, por favor")
        .unwrap();
    let forged = call(
        &registry,
        "assignments_review_refine",
        serde_json::json!({"review_id":review["id"],"message_id":999,"use_guide":true}),
    );
    assert_eq!(forged["result"]["isError"], true);
    let next = call(
        &registry,
        "assignments_review_refine",
        serde_json::json!({"review_id":review["id"]}),
    );
    assert_eq!(next["result"]["isError"], false);
    let next: serde_json::Value =
        serde_json::from_str(next["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        next["version"]["response"]["text"],
        "A las nueve, por favor"
    );
    assert_eq!(next["scope"]["use_guide"], false);
    let mut changed = serde_json::to_value(document).unwrap();
    changed["recurrence"]["local_time"] = serde_json::json!("09:00");
    let proposal = serde_json::json!({"assignment_id":record.id,"title":"Resumen a las nueve","rationale":"Horario elegido por el usuario","expected_benefit":"Una entrega acordada","related_ids":[],"questions":[],"document":changed});
    assert_eq!(
        call(
            &registry,
            "assignments_propose",
            serde_json::json!({"review_id":next["id"],"proposals":[proposal]})
        )["result"]["isError"],
        false
    );
    assert_eq!(backend.assignments.get(record.id).unwrap().revision, 1);
    let current: serde_json::Value = serde_json::from_str(
        &backend
            .proposal_request(
                &serde_json::json!({"action":"get","review_id":next["id"]}).to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    let pid = current["proposals"][0]["id"].clone();
    let request = serde_json::json!({"action":"resolve","proposal_id":pid,"apply":true});
    backend.proposal_request(&request.to_string()).unwrap();
    backend.proposal_request(&request.to_string()).unwrap();
    assert_eq!(backend.assignments.get(record.id).unwrap().revision, 2);
    let archive = serde_json::json!({"action":"archive","review_id":review["id"]}).to_string();
    backend.proposal_request(&archive).unwrap();
    backend.proposal_request(&archive).unwrap();
    let current: serde_json::Value = serde_json::from_str(
        &backend
            .proposal_request(
                &serde_json::json!({"action":"get","review_id":next["id"]}).to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(current["archived"], true);
    assert_eq!(current["proposals"][0]["applied_revision"], 2);
    let active: serde_json::Value = serde_json::from_str(
        &backend
            .proposal_request(
                &serde_json::json!({"action":"list","assignment_id":record.id}).to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(active["total"], 0);
    let archived: serde_json::Value = serde_json::from_str(
        &backend
            .proposal_request(
                &serde_json::json!({"action":"list","assignment_id":record.id,"archived":true})
                    .to_string(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(archived["total"], 2);
    assert_eq!(
        call(
            &registry,
            "assignments_review_refine",
            serde_json::json!({"review_id":next["id"]})
        )["result"]["isError"],
        true
    );
    *context.lock().unwrap() = None;
    assert_eq!(
        call(
            &registry,
            "assignments_review_get",
            serde_json::json!({"review_id":next["id"]})
        )["result"]["isError"],
        true
    );
    backend
        .state
        .db
        .with_connection(|c| {
            for table in [
                "codex_conversation",
                "plugin_build_jobs",
                "plugin_mcp_attempts",
                "assignment_execution_runs",
            ] {
                assert_eq!(
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))?,
                    0
                );
            }
            Ok(())
        })
        .unwrap();
    drop(registry);
    backend.close().unwrap();
    let mut reopened = fixture.open();
    assert_eq!(reopened.assignments.get(record.id).unwrap().revision, 2);
    assert_eq!(
        reopened
            .assignments
            .review(next["id"].as_str().unwrap())
            .unwrap()["chat_request"]["text"],
        "Genera propuestas para este encargo"
    );
    assert_eq!(
        reopened
            .assignments
            .review(next["id"].as_str().unwrap())
            .unwrap()["version"]["round"],
        2
    );
    reopened.close().unwrap();
}

#[test]
fn proposals_use_human_scope_and_atomic_review_without_starting_models_or_jobs() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute(
            "UPDATE modules SET active=1 WHERE name IN ('llm','assignments')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let chat = append_conversation(&backend, "proposal-origin", "Solicitud inicial");
    let document: gennie_assignments::Document=serde_json::from_value(serde_json::json!({"title":"Resumen","description":"Agrupar novedades","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"}})).unwrap();
    let row = backend
        .assignments
        .create("proposal-task", document.clone())
        .unwrap();
    let request = serde_json::json!({"action":"begin","request_key":"human-review","assignments":[{"id":row.id,"instance_id":row.instance_id,"revision":row.revision}],"instructions":"Agrupar sin avisos repetidos","use_guide":false});
    let review: serde_json::Value =
        serde_json::from_str(&backend.proposal_request(&request.to_string()).unwrap()).unwrap();
    let mut remote = request.clone();
    remote["use_guide"] = serde_json::json!(true);
    assert!(backend.proposal_request(&remote.to_string()).is_err());
    let origin = gennie_presence::delegation::origin(&backend.state.db, chat.parse().unwrap())
        .unwrap()
        .unwrap();
    let mut proposed = document;
    proposed.description = "Un resumen con los cambios nuevos".into();
    let no_guide: gennie_assignments::proposals::GuideEvaluator =
        Arc::new(|_, _| panic!("No debe llamar la guía"));
    let saved = backend
        .assignments
        .submit_proposals(
            None,
            review["id"].as_str().unwrap(),
            &gennie_assignments::proposals::Origin {
                conversation_id: origin.conversation_id,
                epoch: origin.epoch,
            },
            vec![gennie_assignments::proposals::Candidate {
                assignment_id: row.id,
                title: "Agrupar novedades".into(),
                rationale: "Evitar duplicados".into(),
                expected_benefit: "Menos interrupciones".into(),
                related_ids: vec![],
                questions: vec![],
                document: Some(proposed),
            }],
            &no_guide,
        )
        .unwrap();
    assert_eq!(backend.assignments.get(row.id).unwrap().revision, 1);
    let apply=serde_json::json!({"action":"resolve","proposal_id":saved["proposals"][0]["id"],"apply":true}).to_string();
    backend.proposal_request(&apply).unwrap();
    backend.proposal_request(&apply).unwrap();
    assert_eq!(backend.assignments.get(row.id).unwrap().revision, 2);
    backend
        .state
        .db
        .with_connection(|c| {
            assert_eq!(
                c.query_row("SELECT count(*) FROM plugin_build_jobs", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                c.query_row("SELECT count(*) FROM codex_conversation", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    let mut reopened = fixture.open();
    assert_eq!(reopened.assignments.get(row.id).unwrap().revision, 2);
    reopened.close().unwrap();
}

// Sin audio, red, modelos ni servicios OS; LLM solo se activa para reutilizar un chat local.
struct Fixture {
    directory: PathBuf,
    config: Config,
}
impl Fixture {
    fn new() -> Self {
        static NEXT_FIXTURE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "gennie-runtime-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self {
            config: Config {
                database_path: directory.join("test.sqlite3"),
                db_busy_timeout_ms: 500,
            },
            directory,
        }
    }
    fn seed_selection(&self) {
        let db = Database::open(&self.config.database_path, 500).unwrap();
        db.migrate(
            "bootstrap",
            &[gennie_core::Migration {
                version: 1,
                sql: include_str!("../migrations/0001_modules.sql"),
            }],
        )
        .unwrap();
        db.with_connection(|connection| {
            let tx = connection.transaction()?;
            for name in [
                "presence",
                "assignments",
                "llm",
                "transcription",
                "decisions",
                "tts",
                "reminders_plugin",
                "browser",
                "plugins",
                "proactivity",
            ] {
                tx.execute(
                    "INSERT INTO modules(name,active) VALUES(?1,?2)",
                    gennie_db::params![name, name == "presence"],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        db.close().unwrap();
    }
    fn open(&self) -> Backend {
        let helper = std::env::current_exe().unwrap();
        // El helper usa OnceLock global; prepara una vez antes de abrir fixtures paralelas.
        static HELPER_READY: std::sync::Once = std::sync::Once::new();
        HELPER_READY.call_once(|| gennie_transcription::configure_helper(helper.clone()).unwrap());
        Backend::open_config(self.config.clone(), helper.to_str().unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn append_conversation(backend: &Backend, thread: &str, text: &str) -> String {
    let journal = PresenceModule::default().conversation_journal(backend.state.db.clone());
    backend
        .state
        .db
        .with_connection(|connection| {
            let tx = connection.transaction()?;
            journal.activate(&tx, "codex", thread, None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
    journal.committed();
    journal.append("codex", thread, "user", text).unwrap();
    journal
        .append("codex", thread, "assistant", "Respuesta conservada")
        .unwrap();
    history::load(&backend.state.db, None)
        .unwrap()
        .active_id
        .unwrap()
        .to_string()
}

#[test]
fn new_chat_reuses_empty_active_chat_and_returns_from_reading_after_reopen() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let older = append_conversation(&backend, "older", "Mensaje anterior");
    let journal = PresenceModule::default().conversation_journal(backend.state.db.clone());
    backend
        .state
        .db
        .with_connection(|connection| {
            let tx = connection.transaction()?;
            journal.activate(&tx, "codex", "empty", None)?;
            tx.execute(
                "INSERT INTO codex_conversation(id,thread_id) VALUES(1,'empty')",
                [],
            )?;
            tx.execute("UPDATE modules SET active=1 WHERE name='llm'", [])?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    drop(backend);

    let mut backend = fixture.open();
    let empty = backend.snapshot.active_id.clone().unwrap();
    for _ in 0..10 {
        backend
            .apply(Operation::Select(Some(older.clone())))
            .unwrap();
        backend.refresh().unwrap();
        assert!(!backend.snapshot.messages.is_empty());
        let revision = backend.revision.load(std::sync::atomic::Ordering::Acquire);
        backend.snapshot.confirmation = None;
        backend.apply(Operation::NewChat).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while (backend.snapshot.confirmation.is_none() || backend.snapshot.busy)
            && std::time::Instant::now() < deadline
        {
            backend.tick();
            std::thread::yield_now();
        }
        assert_eq!(
            backend.snapshot.confirmation.as_deref(),
            Some("Nuevo chat preparado")
        );
        assert_eq!(
            backend.snapshot.selected_id.as_deref(),
            Some(empty.as_str())
        );
        assert_eq!(backend.snapshot.active_id.as_deref(), Some(empty.as_str()));
        assert!(backend.snapshot.messages.is_empty());
        assert_eq!(backend.snapshot.conversations.len(), 2);
        assert_eq!(
            backend.revision.load(std::sync::atomic::Ordering::Acquire),
            revision
        );
        assert!(!backend.snapshot.busy);
        assert!(backend.snapshot.error.is_none());
    }
    backend.close().unwrap();
}

#[test]
fn coordinator_preserves_read_selection_mutations_and_shutdown_across_reopen() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    assert!(backend.llm.is_none());
    assert!(backend.voice.is_none());
    assert!(backend.dictation.is_none());
    assert_eq!(backend.started.len(), 1);
    assert_eq!(backend.modules[backend.started[0]].name(), "presence");
    // Los módulos apagados también reciben todas sus migraciones.
    for module in &backend.modules {
        backend
            .state
            .db
            .with_connection(|connection| {
                let count: i64 = connection.query_row(
                    &format!(
                        "SELECT count(*) FROM _soc_migrations_{}",
                        module.migration_namespace()
                    ),
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(count as usize, module.migrations().len());
                Ok(())
            })
            .unwrap();
    }

    let first = append_conversation(&backend, "first", "Primer mensaje");
    let active = append_conversation(&backend, "active", "Mensaje activo");
    backend.refresh().unwrap();
    assert_eq!(backend.snapshot.active_id.as_deref(), Some(active.as_str()));
    backend
        .apply(Operation::Select(Some(first.clone())))
        .unwrap();
    backend.refresh().unwrap();
    assert_eq!(
        backend.snapshot.selected_id.as_deref(),
        Some(first.as_str())
    );
    assert_eq!(backend.snapshot.active_id.as_deref(), Some(active.as_str()));
    assert_eq!(backend.snapshot.messages[0].text, "Primer mensaje");
    assert!(
        backend
            .apply(Operation::Send(
                "No enviar sin módulo de conversación".into()
            ))
            .is_err()
    );
    let before = history::load(&backend.state.db, Some(first.parse().unwrap())).unwrap();

    backend
        .apply(Operation::Rename(first.clone(), "Nombre guardado".into()))
        .unwrap();
    backend.refresh().unwrap();
    assert_eq!(
        backend.snapshot.confirmation.as_deref(),
        Some("Nombre guardado")
    );
    assert!(
        backend
            .apply(Operation::Rename(first.clone(), "   ".into()))
            .is_err()
    );
    assert!(
        backend
            .apply(Operation::Rename(first.clone(), "ñ".repeat(201)))
            .is_err()
    );
    backend.refresh().unwrap();
    let after = history::load(&backend.state.db, Some(first.parse().unwrap())).unwrap();
    assert_eq!(before.selected_messages, after.selected_messages);
    assert_eq!(
        backend
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == first)
            .unwrap()
            .title,
        "Nombre guardado"
    );

    let motion = !backend.snapshot.reduced_motion;
    backend.apply(Operation::Motion(motion)).unwrap();
    backend.refresh().unwrap();
    assert_eq!(backend.snapshot.reduced_motion, motion);
    assert_eq!(
        gennie_presence::reduced_motion(&backend.state.db).unwrap(),
        motion
    );
    let clone = backend.state.db.clone();
    backend.close().unwrap();
    backend.close().unwrap();
    assert!(clone.with_connection(|_| Ok(())).is_err());
    drop(backend);

    let mut reopened = fixture.open();
    assert_eq!(reopened.snapshot.reduced_motion, motion);
    assert_eq!(
        reopened.snapshot.active_id.as_deref(),
        Some(active.as_str())
    );
    assert_eq!(
        reopened
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == first)
            .unwrap()
            .title,
        "Nombre guardado"
    );
    reopened
        .apply(Operation::Select(Some(first.clone())))
        .unwrap();
    reopened.refresh().unwrap();
    assert_eq!(reopened.snapshot.messages.len(), 2);
    reopened.apply(Operation::Delete(first.clone())).unwrap();
    reopened.refresh().unwrap();
    assert!(
        !reopened
            .snapshot
            .conversations
            .iter()
            .any(|c| c.id == first)
    );
    assert_eq!(
        reopened.snapshot.active_id.as_deref(),
        Some(active.as_str())
    );
    assert_eq!(reopened.snapshot.messages[0].text, "Mensaje activo");
    assert!(reopened.apply(Operation::Delete(first.clone())).is_err());
    reopened
        .state
        .db
        .with_connection(|connection| {
            let count: i64 = connection.query_row(
                "SELECT count(*) FROM presence_history WHERE conversation_id=?1",
                [first.parse::<i64>().unwrap()],
                |row| row.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    reopened.close().unwrap();
    drop(reopened);
    let mut final_open = fixture.open();
    assert_eq!(final_open.snapshot.conversations.len(), 1);
    assert_eq!(final_open.snapshot.messages.len(), 2);
    final_open.close().unwrap();
}

#[test]
fn missing_helper_fails_before_creating_the_database() {
    let fixture = Fixture::new();
    let missing = fixture.directory.join("missing-helper");
    let result = Backend::open_config(fixture.config.clone(), missing.to_str().unwrap());
    assert!(result.is_err());
    assert!(!fixture.config.database_path.exists());
}

#[test]
fn failed_history_save_preserves_completed_reply_until_it_is_persisted() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let id = append_conversation(&backend, "failed-save", "Primera pregunta");
    let journal = PresenceModule::default().conversation_journal(backend.state.db.clone());
    // El mismo texto en un turno anterior no demuestra que el turno actual esté guardado.
    journal
        .append("codex", "failed-save", "assistant", "Respuesta sin guardar")
        .unwrap();
    journal
        .append("codex", "failed-save", "user", "Nueva pregunta")
        .unwrap();
    backend.refresh().unwrap();
    assert_eq!(backend.snapshot.active_id.as_deref(), Some(id.as_str()));
    backend.last_completed = Some("Respuesta sin guardar".into());
    backend.failed_turn = true;
    backend.refresh_controls();
    assert_eq!(backend.snapshot.reply, "Respuesta sin guardar");
    let stored_count = backend.snapshot.messages.len();

    journal
        .append("codex", "failed-save", "assistant", "Respuesta sin guardar")
        .unwrap();
    backend.refresh().unwrap();
    assert!(backend.snapshot.reply.is_empty());
    assert_eq!(backend.snapshot.messages.len(), stored_count + 1);
    assert_eq!(
        backend.snapshot.messages.last().unwrap().text,
        "Respuesta sin guardar"
    );
    backend.close().unwrap();
}

#[test]
fn desktop_resolves_relative_database_against_the_env_file_not_flutter_working_directory() {
    let config = Config {
        database_path: "data/gennie.sqlite3".into(),
        db_busy_timeout_ms: 500,
    };
    assert_eq!(
        resolve_config_path(config, "/checkout/.env").database_path,
        PathBuf::from("/checkout/data/gennie.sqlite3")
    );
    let config = Config {
        database_path: "/absolute/database.sqlite3".into(),
        db_busy_timeout_ms: 500,
    };
    assert_eq!(
        resolve_config_path(config, "/checkout/.env").database_path,
        PathBuf::from("/absolute/database.sqlite3")
    );
}

#[test]
fn system_notices_persist_without_duplicating_provider_errors_or_switching_read_selection() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let first = append_conversation(&backend, "first-notice", "Primer chat");
    let second = append_conversation(&backend, "second-notice", "Chat activo");
    backend.snapshot.selected_id = Some(first.clone());
    backend.refresh().unwrap();
    backend.record_error(
        "El proveedor rechazó la operación (código -32600)".into(),
        true,
        true,
    );
    assert!(backend.snapshot.error_recorded);
    assert_eq!(
        backend.snapshot.selected_id.as_deref(),
        Some(first.as_str())
    );
    let selected = history::load(&backend.state.db, Some(first.parse().unwrap())).unwrap();
    assert_eq!(selected.selected_messages.len(), 2);
    assert_eq!(selected.messages.len(), 3);
    let notice = selected.messages.last().unwrap();
    assert_eq!(notice.speaker, "system");
    assert_eq!(
        notice.text,
        "Error: El proveedor rechazó la operación (código -32600)"
    );
    assert!(!notice.created_at.is_empty());
    // El mismo error ya guardado por el worker no debe repetirse.
    backend.record_error(
        "El proveedor rechazó la operación (código -32600)".into(),
        true,
        true,
    );
    assert_eq!(
        history::load(&backend.state.db, None)
            .unwrap()
            .messages
            .len(),
        3
    );
    // Dos avisos locales iguales en instantes distintos son dos sucesos reales.
    backend.record_error("Sin audio".into(), false, true);
    backend.record_error("Sin audio".into(), false, true);
    backend.close().unwrap();
    drop(backend);
    let mut backend = fixture.open();
    assert_eq!(backend.snapshot.active_id.as_deref(), Some(second.as_str()));
    assert_eq!(backend.snapshot.messages.len(), 5);
    backend.state.db.close().unwrap();
    backend.record_error("Fallo visible".into(), false, true);
    assert!(!backend.snapshot.error_recorded);
    assert!(
        backend
            .snapshot
            .error
            .as_deref()
            .unwrap()
            .contains("No se pudo guardar el aviso")
    );
}

#[test]
fn system_notice_before_first_chat_is_local_and_does_not_create_provider_session() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.record_error("Sin conectar".into(), false, true);
    assert!(backend.snapshot.error_recorded);
    assert!(backend.snapshot.active_id.is_none());
    assert_eq!(
        backend.snapshot.conversations[0].title,
        "Avisos del sistema"
    );
    assert_eq!(backend.snapshot.messages[0].speaker, "system");
    assert!(backend.llm.is_none());
    backend.close().unwrap();
}

#[test]
fn model_controls_report_unavailable_modules_and_reject_requests() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    assert_eq!(backend.snapshot.models.len(), 3);
    for model in [LocalModel::Laya, LocalModel::Voice, LocalModel::Dictation] {
        let info = backend
            .snapshot
            .models
            .iter()
            .find(|info| info.model == model)
            .unwrap();
        assert_eq!(info.state, ModelState::Unavailable);
        assert!(backend.apply(Operation::Model(model, true)).is_err());
        assert!(backend.apply(Operation::Model(model, false)).is_err());
    }
    backend.close().unwrap();
}

#[test]
fn memory_query_does_not_mutate_snapshot_or_create_processes() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.confirmation = Some("Conservar aviso".into());
    for model in [LocalModel::Laya, LocalModel::Voice, LocalModel::Dictation] {
        assert_eq!(backend.model_memory(model), None);
    }
    assert_eq!(
        backend.snapshot.confirmation.as_deref(),
        Some("Conservar aviso")
    );
    backend.close().unwrap();
}

#[test]
fn assignment_commit_refreshes_snapshot_and_survives_reopen_without_provider() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='assignments'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let document=serde_json::json!({"title":"Investigar IA","description":"Comparar fuentes de autos autónomos","kind":"goal","priority":"high","status":"active","timezone":"America/Bogota"}).to_string();
    backend
        .apply(Operation::SaveAssignment(
            None,
            None,
            "runtime-event".into(),
            document.clone(),
        ))
        .unwrap();
    backend.refresh().unwrap();
    assert!(backend.snapshot.assignments_enabled);
    assert_eq!(backend.snapshot.assignments.len(), 1);
    assert!(backend.llm.is_none());
    assert!(backend.snapshot.confirmation.is_some());
    let record = backend.snapshot.assignments[0].clone();
    let mut edited: gennie_assignments::Document = serde_json::from_str(&record.document).unwrap();
    edited.status = gennie_assignments::Status::Paused;
    backend
        .assignments
        .update(record.id.parse().unwrap(), record.revision, edited)
        .unwrap();
    backend.tick();
    assert!(backend.snapshot.assignments[0].document.contains("paused"));
    backend.close().unwrap();
    let mut reopened = fixture.open();
    assert_eq!(reopened.snapshot.assignments.len(), 1);
    assert!(reopened.snapshot.assignments[0].document.contains("paused"));
    reopened.close().unwrap();
}

#[test]
fn plugin_origin_comes_from_the_emitting_turn_not_the_active_or_selected_chat() {
    use gennie_plugins::BuildJournal;
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let first = append_conversation(&backend, "plugin-first", "Petición inicial");
    let second = append_conversation(&backend, "plugin-second", "Otra conversación");
    let invocation = Arc::new(Mutex::new(Some("plugin-first".into())));
    let llm = Arc::new(gennie_llm::LlmModule::with_journal(
        PresenceModule::default().conversation_journal(backend.state.db.clone()),
    ));
    llm.start(&backend.state).unwrap();
    let journal = PluginJournal {
        db: backend.state.db.clone(),
        invocation: invocation.clone(),
        llm: llm.clone(),
    };
    backend.snapshot.selected_id = Some(second.clone());
    let origin = journal.origin().unwrap();
    assert_eq!(origin.to_string(), first);
    *invocation.lock().unwrap() = Some("plugin-second".into());
    let target = journal.target(origin).unwrap();
    let update = gennie_llm::delegation::Update {
        event_key: "plugin-origin-test:1".into(),
        delegation_id: "plugin-job:7".into(),
        request_id: "request:7".into(),
        origin: target,
        source_id: "plugin_builder".into(),
        job_id: 7,
        plugin_id: "fixture".into(),
        display_name: "Integración de prueba".into(),
        objective: "Petición inicial".into(),
        sequence: 1,
        state: gennie_llm::delegation::State::Running,
        kind: gennie_llm::delegation::Kind::Progress,
        detail: "Avance PRIVADO del primer trabajo".into(),
    };
    journal.deliver(&update).unwrap();
    for _ in 0..200 {
        let delivered:bool=backend.state.db.with_connection(|c|Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM presence_delegation_messages WHERE event_key='plugin-origin-test:1')",[],|r|r.get(0))?)).unwrap();
        if delivered {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let history = history::load(&backend.state.db, Some(origin)).unwrap();
    assert_eq!(history.active_id.unwrap().to_string(), second);
    assert_eq!(
        history.selected_messages.last().unwrap().text,
        "Integración de prueba: La preparación del plugin está en curso."
    );
    assert_eq!(
        history.selected_messages.last().unwrap().speaker,
        "assistant"
    );
    llm.stop(&backend.state).unwrap();
    *invocation.lock().unwrap() = None;
    assert!(journal.origin().is_err());
    backend.close().unwrap();
}

#[test]
fn assignment_creation_delegates_after_commit_and_keeps_success_when_preparation_fails() {
    use gennie_mcp::{Registry, Tool};
    use serde_json::json;
    let saved = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = saved.clone();
    let mut tool = Tool {
        definition: json!({"name":"assignments_create"}),
        call: Box::new(move |_| {
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(
                json!({"assignment":{"id":9,"document":{"applications":[{"kind":"web","name":"Correo","url":"https://example.com"}]}},"execution_enabled":false}),
            )
        }),
    };
    let observed = saved.clone();
    assignment_tool_with_preparation(&mut tool, move |assignment| {
        assert!(observed.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(assignment["id"], 9);
        Err("Constructor no disponible".into())
    });
    let mut registry = Registry::default();
    registry.register([tool]).unwrap();
    let result=registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_create","arguments":{}}})).unwrap();
    let data = &result["result"]["structuredContent"];
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(data["assignment"]["id"], 9);
    assert_eq!(data["plugin_preparation"]["status"], "not_started");
    assert_eq!(
        data["plugin_preparation"]["message"],
        "Constructor no disponible"
    );
}

#[test]
fn assignment_update_reports_queued_preparation_and_does_not_trigger_after_failed_save() {
    use gennie_mcp::{Registry, Tool};
    use serde_json::json;
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut tool = Tool {
        definition: json!({"name":"assignments_update"}),
        call: Box::new(|args| {
            if args["expected_revision"] != 3 {
                return Err("Conflicto de revisión".into());
            }
            Ok(
                json!({"assignment":{"id":9,"revision":4,"document":{"applications":[{"kind":"web","name":"Correo","url":"https://example.com"}]}},"execution_enabled":false}),
            )
        }),
    };
    let observed = count.clone();
    assignment_tool_with_preparation(&mut tool, move |assignment| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(assignment["revision"], 4);
        Ok(json!({"sources":[{"status":"started","job_id":8,"state":"queued"}]}))
    });
    let mut registry = Registry::default();
    registry.register([tool]).unwrap();
    let call = |revision| {
        registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_update","arguments":{"expected_revision":revision}}})).unwrap()
    };
    let failed = call(2);
    assert_eq!(failed["result"]["isError"], true);
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    let success = call(3);
    assert_eq!(success["result"]["isError"], false);
    assert_eq!(
        success["result"]["structuredContent"]["plugin_preparation"]["sources"][0]["state"],
        "queued"
    );
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
}
#[test]
fn plugin_startup_applies_saved_browser_permissions_before_the_first_request() {
    let browser = gennie_browser::BrowserModule::new(
        std::env::temp_dir().join("gennie-permissions-no-runtime"),
    )
    .controller();
    let settings = gennie_llm::Settings {
        local_actions: true,
        run_commands: true,
        ..Default::default()
    };
    super::configure_browser_permissions(&browser, Some(&settings));
    assert!(
        browser
            .automate(serde_json::json!({"method":"task_page","action":"create"}))
            .unwrap_err()
            .to_string()
            .contains("módulo de navegador")
    );
    super::configure_browser_permissions(&browser, None);
    assert!(
        browser
            .automate(serde_json::json!({"method":"task_page","action":"create"}))
            .unwrap_err()
            .to_string()
            .contains("control de aplicaciones")
    );
}

#[test]
fn architect_configuration_uses_real_worker_without_starting_a_model_or_changing_chat() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='llm'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let result: serde_json::Value =
        serde_json::from_str(&backend.architect_request(r#"{"action":"list"}"#).unwrap()).unwrap();
    assert_eq!(result["profiles"].as_array().unwrap().len(), 3);
    let mut helper = result["profiles"][1].clone();
    helper["personality"] = serde_json::json!("Metódico");
    helper["model"] = serde_json::json!("helper-model");
    let request = serde_json::json!({"action":"save","profile":helper}).to_string();
    let saved: serde_json::Value =
        serde_json::from_str(&backend.architect_request(&request).unwrap()).unwrap();
    assert_eq!(saved["profile"]["revision"], 2);
    assert!(backend.architect_request(&request).is_err());
    assert_eq!(backend.llm.as_ref().unwrap().settings().model, "gpt-6-luna");
    assert_eq!(
        gennie_llm::architects::runtime_settings(
            &backend.state.db,
            gennie_llm::architects::Role::Helper
        )
        .unwrap()
        .model,
        "helper-model"
    );
    let mut bot = result["profiles"][0].clone();
    bot["instructions"] = serde_json::json!("Responde con ejemplos.");
    backend
        .architect_request(&serde_json::json!({"action":"save","profile":bot}).to_string())
        .unwrap();
    assert_eq!(
        backend.llm.as_ref().unwrap().settings().instructions,
        "Responde con ejemplos."
    );
    backend
        .state
        .db
        .with_connection(|c| {
            assert_eq!(
                c.query_row("SELECT count(*) FROM codex_conversation", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                c.query_row("SELECT count(*) FROM presence_history", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                c.query_row("SELECT count(*) FROM plugin_build_jobs", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    assert_eq!(
        backend.llm.as_ref().unwrap().settings().instructions,
        "Responde con ejemplos."
    );
    backend.close().unwrap();
}

#[test]
fn architect_catalog_expands_without_llm_and_reopens_without_replacing_bases() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    assert!(backend.llm.is_none());
    let result: serde_json::Value =
        serde_json::from_str(&backend.architect_request(r#"{"action":"list"}"#).unwrap()).unwrap();
    let bases = result["profiles"].clone();
    let mut draft = bases[1].clone();
    draft["name"] = serde_json::json!("Investigador reutilizable");
    draft["role"] = serde_json::json!("researcher");
    draft["enabled"] = serde_json::json!(false);
    let created: serde_json::Value = serde_json::from_str(
        &backend
            .architect_request(&serde_json::json!({"action":"create","profile":draft}).to_string())
            .unwrap(),
    )
    .unwrap();
    let id = created["profile"]["id"].as_str().unwrap();
    let mut profile = created["profile"].clone();
    profile["personality"] = serde_json::json!("Preciso");
    let saved: serde_json::Value = serde_json::from_str(
        &backend
            .architect_request(&serde_json::json!({"action":"save","profile":profile}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["profile"]["revision"], 2);
    backend.close().unwrap();
    let mut backend = fixture.open();
    let result: serde_json::Value =
        serde_json::from_str(&backend.architect_request(r#"{"action":"list"}"#).unwrap()).unwrap();
    assert_eq!(result["profiles"].as_array().unwrap().len(), 4);
    assert_eq!(
        &result["profiles"].as_array().unwrap()[..3],
        bases.as_array().unwrap()
    );
    let loaded: serde_json::Value = serde_json::from_str(
        &backend
            .architect_request(&serde_json::json!({"action":"get","id":id}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(loaded["profile"], saved["profile"]);
    assert!(
        backend
            .architect_request(r#"{"action":"remove","id":"gennie","revision":1}"#)
            .is_err()
    );
    assert!(
        backend
            .architect_request(
                &serde_json::json!({"action":"remove","id":id,"revision":1}).to_string()
            )
            .is_err()
    );
    backend
        .architect_request(&serde_json::json!({"action":"remove","id":id,"revision":2}).to_string())
        .unwrap();
    backend
        .state
        .db
        .with_connection(|c| {
            for table in [
                "codex_conversation",
                "presence_history",
                "plugin_build_jobs",
            ] {
                assert_eq!(
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))?,
                    0
                );
            }
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
}

#[test]
fn manual_mcp_reads_saved_bot_permissions_without_starting_conversation() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='plugins'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    assert!(backend.llm.is_none());
    let mut bot =
        gennie_llm::architects::load(&backend.state.db, gennie_llm::architects::Role::Bot).unwrap();
    bot.run_commands = false;
    bot.local_actions = false;
    bot = gennie_llm::architects::save(&backend.state.db, &bot).unwrap();
    let saved = backend.plugins.request(serde_json::json!({"action":"mcp_save","expected_revision":0,"specification":{"id":"manual_fixture","name":"Prueba propia","connection":{"type":"http","url":"http://127.0.0.1:9/mcp"},"timeout_seconds":1}})).unwrap();
    let request = serde_json::json!({"action":"mcp_connect","server_id":"manual_fixture","expected_revision":saved["server"]["revision"]});
    let error = backend.plugins.request(request).unwrap_err().to_string();
    assert!(
        error.contains("comandos y control de aplicaciones"),
        "{error}"
    );
    bot.run_commands = true;
    bot.local_actions = true;
    bot.read_files = false;
    bot.write_files = false;
    gennie_llm::architects::save(&backend.state.db, &bot).unwrap();
    let saved = backend.plugins.request(serde_json::json!({"action":"mcp_save","expected_revision":0,"specification":{"id":"stdio_fixture","name":"Programa no enviado","connection":{"type":"stdio","command":"/usr/bin/false","args":[]},"timeout_seconds":1}})).unwrap();
    let error = backend.plugins.request(serde_json::json!({"action":"mcp_connect","server_id":"stdio_fixture","expected_revision":saved["server"]["revision"]})).unwrap_err().to_string();
    assert!(error.contains("requiere los cuatro permisos"), "{error}");
    backend
        .state
        .db
        .with_connection(|c| {
            for table in [
                "codex_conversation",
                "plugin_mcp_attempts",
                "plugin_build_jobs",
            ] {
                let count: i64 =
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
                assert_eq!(count, 0);
            }
            Ok(())
        })
        .unwrap();
    assert!(backend.llm.is_none());
    backend.close().unwrap();
}

#[test]
fn plugin_outbox_reaches_bot_and_origin_history_after_restart_without_inference() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let first = append_conversation(&backend, "outbox-first", "Solicitud de plugin");
    let second = append_conversation(&backend, "outbox-second", "OTHER_SESSION_PRIVATE");
    let target = gennie_presence::delegation::origin(&backend.state.db, first.parse().unwrap())
        .unwrap()
        .unwrap();
    backend.state.db.with_connection(|c| {
        c.execute("INSERT INTO plugin_build_jobs(request_key,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,max_attempts,max_seconds,summary,delegation_origin,legacy_origin) VALUES('outbox-restart','fixture','https://example.com','Preparar plugin',?1,0,'interrupted',8,600,'PRIVATE_HELPER_ERROR',?2,0)",gennie_db::params![target.conversation_id,serde_json::to_string(&target)?])?;
        c.execute("INSERT INTO plugin_build_events(job_id,message,source_state,kind) VALUES(1,'PRIVATE_HELPER_ERROR','interrupted','status')",[])?;
        Ok(())
    }).unwrap();
    backend.close().unwrap();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute(
            "UPDATE modules SET active=1 WHERE name IN ('llm','plugins')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let mut delivered = false;
    for _ in 0..200 {
        backend.tick();
        delivered = backend
            .state
            .db
            .with_connection(|c| {
                Ok(c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM presence_delegation_messages)",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        if delivered {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(delivered);
    let history = history::load(&backend.state.db, Some(first.parse().unwrap())).unwrap();
    assert_eq!(history.active_id.unwrap().to_string(), second);
    assert_eq!(history.selected_messages.len(), 3);
    assert_eq!(history.selected_messages[2].speaker, "assistant");
    assert!(history.selected_messages[2].text.contains("se interrumpió"));
    assert!(
        !history.selected_messages[2]
            .text
            .contains("PRIVATE_HELPER_ERROR")
    );
    let pages = chat_page(&backend.state.db, None).unwrap();
    let notice = pages
        .messages
        .iter()
        .find(|m| m.delegation.is_some())
        .unwrap();
    assert_eq!(notice.conversation_id, first);
    assert_eq!(notice.delegation.as_ref().unwrap().job_id, "1");
    let presented = backend
        .plugins
        .request(serde_json::json!({"action":"get_job","job_id":1}))
        .unwrap();
    assert_eq!(presented["job"]["bot_presentation"]["status"], "ready");
    assert_eq!(
        presented["job"]["bot_presentation"]["messages"][0]["text"],
        history.selected_messages[2].text
    );
    assert!(
        !presented["job"]["bot_presentation"]
            .to_string()
            .contains("PRIVATE_HELPER_ERROR")
    );
    backend
        .state
        .db
        .with_connection(|c| {
            assert_eq!(
                c.query_row("SELECT state FROM llm_delegation_updates", [], |r| r
                    .get::<_, String>(0))?,
                "delivered"
            );
            assert_eq!(
                c.query_row("SELECT count(*) FROM codex_conversation", [], |r| r
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                c.query_row("SELECT delivered FROM plugin_build_events", [], |r| r
                    .get::<_, i64>(0))?,
                1
            );
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        history::load(&backend.state.db, Some(first.parse().unwrap()))
            .unwrap()
            .selected_messages
            .len(),
        3
    );
    let presented = backend
        .plugins
        .request(serde_json::json!({"action":"get_job","job_id":1}))
        .unwrap();
    assert_eq!(
        presented["job"]["bot_presentation"]["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    history::delete(&backend.state.db, first.parse().unwrap()).unwrap();
    let stale = backend
        .plugins
        .request(serde_json::json!({"action":"get_job","job_id":1}))
        .unwrap();
    assert_eq!(
        stale["job"]["bot_presentation"]["status"],
        "origin_unavailable"
    );
    assert!(
        stale["job"]["bot_presentation"]["messages"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    backend.close().unwrap();
}

#[test]
fn execution_outbox_reaches_bot_origin_once_with_plugins_off_and_no_model_or_effect() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let origin_id = append_conversation(&backend, "execution-origin", "Petición autorizada")
        .parse::<i64>()
        .unwrap();
    let origin = gennie_presence::delegation::origin(&backend.state.db, origin_id)
        .unwrap()
        .unwrap();
    let active = append_conversation(&backend, "execution-other", "OTHER_SESSION_PRIVATE");
    let document:gennie_assignments::Document=serde_json::from_value(serde_json::json!({"title":"Revisión ficticia","description":"Consultar novedades","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"},"method":{"plugin_id":"fixture","functions":["read"]},"authorization":{"scope":"Solo la fuente ficticia","allowed_routes":["http"]}})).unwrap();
    let assignment = backend
        .assignments
        .create("execution-notice-fixture", document)
        .unwrap();
    let destination = gennie_assignments::execution::notices::Delivery {
        conversation_id: origin_id,
        bot_id: origin.bot_id.clone(),
        epoch: origin.epoch,
        provider_thread: origin.provider_thread.clone(),
    };
    let policy = serde_json::json!({"assignment":{"id":assignment.id,"instance_id":assignment.instance_id,"revision":assignment.revision,"document":assignment.document},"trigger":"delivery","calendar":{"kind":"weekly","zone":"America/Bogota","local_time":"08:00","weekdays":[1,2,3,4,5,6,7],"dst":{"repeated":"earlier","missing":"skip"}},"target":{"kind":"web","plugin_id":"fixture","function":"read","revision":1},"arguments":{"query":"original"},"route":"http","misfire":"skip_missed","max_lateness_minutes":5,"delivery":destination});
    let notice = gennie_assignments::execution::notices::Notice {
        event_key: "assignment-execution:1".into(),
        destination,
        assignment_id: assignment.id,
        instance_id: assignment.instance_id.clone(),
        policy_id: "notice-policy".into(),
        policy_revision: 1,
        run_id: 1,
        title: "Revisión ficticia".into(),
        scheduled_at: "2026-10-29T13:00:00Z".into(),
        state: "uncertain".into(),
        code: "effect_unconfirmed".into(),
        observation: None,
        observation_truncated: false,
        guidance: None,
    };
    let update = gennie_llm::notices::Update {
        event_key: notice.event_key.clone(),
        origin,
        reference: gennie_llm::notices::Reference {
            assignment_id: assignment.id,
            instance_id: assignment.instance_id.clone(),
            policy_id: notice.policy_id.clone(),
            policy_revision: 1,
            run_id: 1,
            state: "uncertain".into(),
        },
        title: notice.title.clone(),
        scheduled_at: notice.scheduled_at.clone(),
        state: notice.state.clone(),
        code: notice.code.clone(),
        observation: None,
        observation_truncated: false,
        guidance: None,
    };
    backend.state.db.with_connection(|c|{
        let tx=c.transaction()?;
        tx.execute("INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state) VALUES('notice-policy',?1,?2,'delivery',1,0,'review_required')",gennie_db::params![assignment.id,assignment.instance_id])?;
        tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES('notice-policy',1,?1)",[policy.to_string()])?;
        tx.execute("INSERT INTO assignment_execution_runs(id,policy_id,policy_revision,scheduled_at,state,code) VALUES(1,'notice-policy',1,'2026-10-29T13:00:00Z','uncertain','effect_unconfirmed')",[])?;
        tx.execute("INSERT INTO assignment_execution_notices(run_id,document) VALUES(1,?1)",[serde_json::to_string(&notice)?])?;
        // Salida preparada de antemano: el worker no tiene que consultar ningún modelo.
        tx.execute("INSERT INTO llm_execution_notices(event_key,bot_id,document,state,rendered_text) VALUES(?1,'gennie',?2,'ready','El resultado quedó sin confirmar; comprueba el efecto antes de reactivar.')",gennie_db::params![update.event_key,serde_json::to_string(&update)?])?;
        tx.execute("UPDATE modules SET active=1 WHERE name IN ('assignments','llm')",[])?;
        tx.commit()?;Ok(())
    }).unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    let control = backend.execution_control.clone().unwrap();
    let receiver = backend.llm.as_ref().unwrap().delegations();
    let mut confirmed = false;
    for _ in 0..100 {
        confirmed=backend.state.db.with_connection(|c|Ok(c.query_row("SELECT (SELECT count(*) FROM assignment_execution_notices WHERE state='accepted')=1 AND (SELECT count(*) FROM llm_execution_notices WHERE state='delivered')=1 AND (SELECT count(*) FROM presence_execution_messages)=1",[],|r|r.get::<_,bool>(0))?)).unwrap();
        if confirmed {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(confirmed);
    let flags = control
        .request(serde_json::json!({"action":"list","assignment_id":assignment.id}))
        .unwrap();
    assert_eq!(flags["runner_active"], false);
    assert_eq!(flags["notices_pending"], 0);
    assert!(
        control
            .request(serde_json::json!({"action":"notice_origin"}))
            .is_ok()
    );
    assert_eq!(
        history::active_conversation(&backend.state.db)
            .unwrap()
            .unwrap()
            .to_string(),
        active
    );
    let page = chat_page(&backend.state.db, None).unwrap();
    let messages = page
        .messages
        .iter()
        .filter(|m| m.execution.is_some())
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].conversation_id, origin_id.to_string());
    assert_eq!(messages[0].execution.as_ref().unwrap().run_id, "1");
    backend
        .state
        .db
        .with_connection(|c| {
            for table in [
                "codex_conversation",
                "plugin_build_jobs",
                "plugin_mcp_attempts",
            ] {
                assert_eq!(
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))?,
                    0
                );
            }
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    assert!(receiver.accept_execution_notice(&update).is_err());
    assert!(
        control
            .request(serde_json::json!({"action":"list","assignment_id":assignment.id}))
            .is_err()
    );
    let mut reopened = fixture.open();
    assert_eq!(
        chat_page(&reopened.state.db, None)
            .unwrap()
            .messages
            .iter()
            .filter(|m| m.execution.is_some())
            .count(),
        1
    );
    reopened.close().unwrap();
}

#[test]
fn unavailable_contextual_guide_records_the_result_and_exposes_human_status_without_models() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let origin_id = append_conversation(&backend, "contextual-origin", "Seguimiento autorizado")
        .parse::<i64>()
        .unwrap();
    let origin = gennie_presence::delegation::origin(&backend.state.db, origin_id)
        .unwrap()
        .unwrap();
    let active = append_conversation(&backend, "contextual-other", "OTHER_PRIVATE_CONTEXT");
    let document:gennie_assignments::Document=serde_json::from_value(serde_json::json!({"title":"Revisar cambios","description":"Consultar novedades de prueba","kind":"goal","priority":"normal","status":"active","timezone":"America/Bogota","preparation":{"interval_minutes":30,"rationale":"Agrupar novedades"},"follow_up":{"scope":"Fuente de prueba","signals":[],"decision_guidance":"Evitar avisos repetidos"},"method":{"plugin_id":"fixture","functions":["read"]},"authorization":{"scope":"Solo lectura ficticia","allowed_routes":["http"]}})).unwrap();
    let assignment = backend
        .assignments
        .create("contextual-fixture", document)
        .unwrap();
    let destination = gennie_assignments::execution::notices::Delivery {
        conversation_id: origin_id,
        bot_id: origin.bot_id,
        epoch: origin.epoch,
        provider_thread: origin.provider_thread,
    };
    let policy = serde_json::json!({"contextual_notice":true,"assignment":{"id":assignment.id,"instance_id":assignment.instance_id,"revision":assignment.revision,"document":assignment.document},"trigger":"preparation","calendar":{"kind":"interval","anchor":1793278800,"seconds":1800},"target":{"kind":"web","plugin_id":"fixture","function":"read","revision":1},"arguments":{},"route":"http","misfire":"skip_missed","max_lateness_minutes":5,"delivery":destination});
    let notice = gennie_assignments::execution::notices::Notice {
        event_key: "assignment-execution:1".into(),
        destination,
        assignment_id: assignment.id,
        instance_id: assignment.instance_id.clone(),
        policy_id: "contextual-policy".into(),
        policy_revision: 1,
        run_id: 1,
        title: "Revisar cambios".into(),
        scheduled_at: "2026-10-29T13:00:00Z".into(),
        state: "succeeded".into(),
        code: "reported_success".into(),
        observation: Some("Observación ficticia".into()),
        observation_truncated: false,
        guidance: Some(gennie_assignments::execution::notices::Guidance {
            objective: "Consultar novedades de prueba".into(),
            scope: "Fuente de prueba".into(),
            signals: vec![],
            decision_guidance: "Evitar avisos repetidos".into(),
        }),
    };
    backend.state.db.with_connection(|c|{let tx=c.transaction()?;
        tx.execute("INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state) VALUES('contextual-policy',?1,?2,'preparation',1,0,'paused')",gennie_db::params![assignment.id,assignment.instance_id])?;
        tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES('contextual-policy',1,?1)",[policy.to_string()])?;
        tx.execute("INSERT INTO assignment_execution_runs(id,policy_id,policy_revision,scheduled_at,state,code,output) VALUES(1,'contextual-policy',1,'2026-10-29T13:00:00Z','succeeded','reported_success','{\"value\":\"fixture\"}')",[])?;
        tx.execute("INSERT INTO assignment_execution_notices(run_id,document) VALUES(1,?1)",[serde_json::to_string(&notice)?])?;
        tx.execute("UPDATE modules SET active=1 WHERE name IN ('assignments','llm')",[])?;tx.commit()?;Ok(())
    }).unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    let control = backend.execution_control.clone().unwrap();
    let mut ready = false;
    for _ in 0..100 {
        let status = control
            .request(serde_json::json!({"action":"run_details","run_id":1}))
            .unwrap();
        ready = status["notification"]["action"] == "record";
        if ready {
            assert_eq!(status["notification"]["guide_state"], "unavailable");
            assert!(status["notification"]["bot_revision"].is_null());
            assert!(status["notification"]["bot_duration_ms"].is_null());
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ready);
    assert_eq!(
        history::active_conversation(&backend.state.db)
            .unwrap()
            .unwrap()
            .to_string(),
        active
    );
    backend
        .state
        .db
        .with_connection(|c| {
            for table in [
                "presence_execution_messages",
                "codex_conversation",
                "plugin_build_jobs",
                "plugin_mcp_attempts",
            ] {
                assert_eq!(
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))?,
                    0
                );
            }
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM assignment_execution_notices WHERE state='accepted'",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    let mut reopened = fixture.open();
    let status = reopened
        .execution_control
        .as_ref()
        .unwrap()
        .request(serde_json::json!({"action":"run_details","run_id":1}))
        .unwrap();
    assert_eq!(status["notification"]["action"], "record");
    assert_eq!(status["state"], "succeeded");
    reopened.close().unwrap();
}
