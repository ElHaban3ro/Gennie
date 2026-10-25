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
        let directory = std::env::temp_dir().join(format!(
            "gennie-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
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
                "llm",
                "transcription",
                "decisions",
                "tts",
                "reminders_plugin",
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
                    &format!("SELECT count(*) FROM _soc_migrations_{}", module.name()),
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
