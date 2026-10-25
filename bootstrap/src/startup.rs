use gennie_config::Config;
use gennie_core::{AppModule, AppState, Migration, Result, Shutdown};
use gennie_db::Database;
use std::{collections::BTreeSet, path::Path, sync::Arc};

pub fn voice_review() -> Result<()> {
    let (config, _) = Config::load(Path::new(".env"))?;
    let db = Database::memory()?;
    let state = AppState {
        config: Arc::new(config),
        db: db.clone(),
        shutdown: Shutdown::default(),
    };
    let transcription = gennie_transcription::TranscriptionModule::default();
    let decisions = gennie_decisions::DecisionsModule::default();
    let result = (|| {
        for module in [
            &transcription as &dyn AppModule,
            &decisions as &dyn AppModule,
        ] {
            db.migrate(module.name(), module.migrations())?;
            module.start(&state)?;
        }
        let mut voice = transcription.review_controller(&state)?;
        println!("{}", voice.settings_status);
        let control = decisions.controller()?;
        println!(
            "Prueba sin acciones ni historial: Ctrl+Option+Shift+R. Mantén pulsado para hablar. Termina automáticamente en 3 minutos."
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let mut pending = false;
        while std::time::Instant::now() < deadline {
            for event in voice.tick() {
                match event {
                    gennie_transcription::Event::Listening => println!("Habla ahora"),
                    gennie_transcription::Event::ModelLoaded => println!("Nemotron listo"),
                    gennie_transcription::Event::Text {
                        text,
                        finalized: true,
                    } if !pending => {
                        control.submit(&text, false)?;
                        pending = true;
                    }
                    gennie_transcription::Event::Text {
                        finalized: true, ..
                    } => println!("Espera al resultado anterior antes de otra frase"),
                    gennie_transcription::Event::Error(message)
                    | gennie_transcription::Event::NoSpeech(message) => println!("{message}"),
                    _ => {}
                }
            }
            for outcome in control.poll() {
                println!("{}", serde_json::to_string(&outcome)?);
                pending = false;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        voice.cancel();
        control.cancel();
        Ok(())
    })();
    let stop_voice = transcription.stop(&state);
    let stop_decisions = decisions.stop(&state);
    result.and(stop_voice).and(stop_decisions).and(db.close())
}

pub fn command(text: &str, execute: bool) -> Result<()> {
    let (config, _) = Config::load(Path::new(".env"))?;
    let db = if execute {
        Database::open(&config.database_path, config.db_busy_timeout_ms)?
    } else {
        Database::memory()?
    };
    let state = AppState {
        config: Arc::new(config),
        db: db.clone(),
        shutdown: Shutdown::default(),
    };
    let module = gennie_decisions::DecisionsModule::default();
    let result = (|| {
        db.migrate(module.name(), module.migrations())?;
        module.start(&state)?;
        let controller = module.controller()?;
        controller.submit(text, execute)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(130);
        loop {
            if let Some(outcome) = controller.poll().into_iter().next() {
                println!("{}", serde_json::to_string(&outcome)?);
                return if outcome.prediction.is_some() {
                    Ok(())
                } else {
                    Err("Falló el proceso local".into())
                };
            }
            if std::time::Instant::now() >= deadline {
                return Err("Control local excedió el tiempo de espera".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    })();
    let stop = module.stop(&state);
    let close = db.close();
    result.and(stop).and(close)
}

pub fn register(db: &Database, modules: &[Arc<dyn AppModule>]) -> Result<BTreeSet<String>> {
    db.migrate(
        "bootstrap",
        &[Migration {
            version: 1,
            sql: include_str!("../migrations/0001_modules.sql"),
        }],
    )?;
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        if modules.iter().any(|module| module.name() == "llm") {
            // Conserva la elección del módulo anterior al renombrarlo.
            tx.execute(
                "INSERT INTO modules(name,active) SELECT 'llm',active FROM modules WHERE name='codex' ON CONFLICT(name) DO NOTHING",
                [],
            )?;
        }
        let mut active = BTreeSet::new();
        for module in modules {
            tx.execute(
                "INSERT INTO modules(name,active) VALUES(?1,?2) ON CONFLICT(name) DO NOTHING",
                gennie_db::params![module.name(), module.active_by_default()],
            )?;
            let enabled: bool = tx.query_row(
                "SELECT active FROM modules WHERE name=?1",
                [module.name()],
                |row| row.get(0),
            )?;
            if enabled {
                active.insert(module.name().into());
            }
        }
        tx.commit()?;
        Ok(active)
    })
}
pub fn stop_all(modules: &[Arc<dyn AppModule>], started: &[usize], state: &AppState) -> Result<()> {
    state.shutdown.request();
    let mut errors = Vec::new();
    for i in started.iter().rev() {
        if let Err(error) = modules[*i].stop(state) {
            eprintln!(
                "event=module_stop_failed module={} error={error}",
                modules[*i].name()
            );
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}
pub fn start_all(
    modules: &[Arc<dyn AppModule>],
    order: &[usize],
    active: &BTreeSet<String>,
    state: &AppState,
) -> Result<Vec<usize>> {
    let mut started = Vec::new();
    for i in order
        .iter()
        .filter(|i| active.contains(modules[**i].name()))
    {
        if let Err(error) = modules[*i].start(state) {
            // La app que falla también debe limpiar inicialización parcial.
            if let Err(cleanup_error) = modules[*i].stop(state) {
                eprintln!(
                    "event=partial_start_cleanup_failed module={} error={cleanup_error}",
                    modules[*i].name()
                );
            }
            let _ = stop_all(modules, &started, state);
            return Err(error);
        }
        started.push(*i);
    }
    Ok(started)
}
#[cfg(test)]
mod tests {
    use super::*;
    use gennie_presence::PresenceModule;
    use std::sync::Mutex;
    struct Fake {
        name: &'static str,
        fail_start: bool,
        fail_stop: bool,
        events: Arc<Mutex<Vec<String>>>,
    }
    impl AppModule for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn start(&self, _: &AppState) -> Result<()> {
            self.events
                .lock()
                .unwrap()
                .push(format!("start:{}", self.name));
            if self.fail_start {
                Err("original start error".into())
            } else {
                Ok(())
            }
        }
        fn stop(&self, _: &AppState) -> Result<()> {
            self.events
                .lock()
                .unwrap()
                .push(format!("stop:{}", self.name));
            if self.fail_stop {
                Err("secondary stop error".into())
            } else {
                Ok(())
            }
        }
    }
    fn state() -> AppState {
        AppState {
            config: Arc::new(Config {
                database_path: "unused".into(),
                db_busy_timeout_ms: 50,
            }),
            db: Database::memory().unwrap(),
            shutdown: Shutdown::default(),
        }
    }
    #[test]
    fn registration_preserves_selection_and_excludes_orphans() {
        let state = state();
        let modules = crate::registry::build(
            Arc::new(PresenceModule::default()),
            Arc::new(gennie_transcription::TranscriptionModule::default()),
            Arc::new(gennie_decisions::DecisionsModule::default()),
            Arc::new(gennie_llm::LlmModule::default()),
            Arc::new(gennie_tts::TtsModule::default()),
            Arc::new(gennie_reminders_plugin::RemindersPluginModule::default()),
        );
        assert!(register(&state.db, &modules).unwrap().contains("presence"));
        state
            .db
            .with_connection(|c| {
                c.execute_batch(
                    "UPDATE modules SET active=0; INSERT INTO modules VALUES('orphan',1);",
                )?;
                Ok(())
            })
            .unwrap();
        assert!(register(&state.db, &modules).unwrap().is_empty());
    }
    #[test]
    fn registration_preserves_legacy_codex_selection() {
        let state = state();
        state
            .db
            .migrate(
                "bootstrap",
                &[Migration {
                    version: 1,
                    sql: include_str!("../migrations/0001_modules.sql"),
                }],
            )
            .unwrap();
        state
            .db
            .with_connection(|connection| {
                connection.execute("INSERT INTO modules(name,active) VALUES('codex',0)", [])?;
                Ok(())
            })
            .unwrap();
        let modules = crate::registry::build(
            Arc::new(PresenceModule::default()),
            Arc::new(gennie_transcription::TranscriptionModule::default()),
            Arc::new(gennie_decisions::DecisionsModule::default()),
            Arc::new(gennie_llm::LlmModule::default()),
            Arc::new(gennie_tts::TtsModule::default()),
            Arc::new(gennie_reminders_plugin::RemindersPluginModule::default()),
        );
        assert!(!register(&state.db, &modules).unwrap().contains("llm"));
    }
    #[test]
    fn startup_failure_cleans_in_reverse_and_keeps_original_error() {
        let state = state();
        let events = Arc::new(Mutex::new(Vec::new()));
        let modules: Vec<Arc<dyn AppModule>> = vec![
            Arc::new(Fake {
                name: "a",
                fail_start: false,
                fail_stop: true,
                events: events.clone(),
            }),
            Arc::new(Fake {
                name: "b",
                fail_start: false,
                fail_stop: false,
                events: events.clone(),
            }),
            Arc::new(Fake {
                name: "c",
                fail_start: true,
                fail_stop: false,
                events: events.clone(),
            }),
        ];
        let active = BTreeSet::from(["a".into(), "b".into(), "c".into()]);
        assert_eq!(
            start_all(&modules, &[0, 1, 2], &active, &state)
                .unwrap_err()
                .to_string(),
            "original start error"
        );
        assert_eq!(
            *events.lock().unwrap(),
            [
                "start:a", "start:b", "start:c", "stop:c", "stop:b", "stop:a"
            ]
        );
        assert!(state.shutdown.requested());
    }
    #[test]
    fn presence_lifecycle_is_idempotent() {
        let state = state();
        let presence = PresenceModule::default();
        state
            .db
            .migrate(presence.name(), presence.migrations())
            .unwrap();
        presence.start(&state).unwrap();
        presence.start(&state).unwrap();
        presence.stop(&state).unwrap();
        presence.stop(&state).unwrap();
    }
}
