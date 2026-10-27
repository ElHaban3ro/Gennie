use super::*;
use gennie_config::Config;
use std::path::PathBuf;

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
            .apply(Operation::Send("No enviar desde consulta".into()))
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
    let journal = PluginJournal {
        db: backend.state.db.clone(),
        invocation: invocation.clone(),
        revision: backend.revision.clone(),
    };
    backend.snapshot.selected_id = Some(second.clone());
    let origin = journal.origin().unwrap();
    assert_eq!(origin.to_string(), first);
    *invocation.lock().unwrap() = Some("plugin-second".into());
    journal
        .deliver(origin, "plugin-origin-test:1", "Avance del primer trabajo")
        .unwrap();
    let history = history::load(&backend.state.db, Some(origin)).unwrap();
    assert_eq!(history.active_id.unwrap().to_string(), second);
    assert_eq!(
        history.selected_messages.last().unwrap().text,
        "Avance del primer trabajo"
    );
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
    let mut settings = gennie_llm::Settings::default();
    settings.local_actions = true;
    settings.run_commands = true;
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
