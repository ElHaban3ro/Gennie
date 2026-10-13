use soc_config::Config;
use soc_core::{AppModule, AppState, Migration, Result, Shutdown};
use soc_db::Database;
use soc_presence::PresenceModule;
use std::{collections::BTreeSet, path::Path, sync::Arc};

pub fn voice_review() -> Result<()> {
    let (config, _) = Config::load(Path::new(".env"))?;
    let db = Database::memory()?;
    let state = AppState {
        config: Arc::new(config),
        db: db.clone(),
        shutdown: Shutdown::default(),
    };
    let transcription = soc_transcription::TranscriptionModule::default();
    let decisions = soc_decisions::DecisionsModule::default();
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
                    soc_transcription::Event::Listening => println!("Habla ahora"),
                    soc_transcription::Event::ModelLoaded => println!("Nemotron listo"),
                    soc_transcription::Event::Text {
                        text,
                        finalized: true,
                    } if !pending => {
                        control.submit(&text, false)?;
                        pending = true;
                    }
                    soc_transcription::Event::Text {
                        finalized: true, ..
                    } => println!("Espera al resultado anterior antes de otra frase"),
                    soc_transcription::Event::Error(message)
                    | soc_transcription::Event::NoSpeech(message) => println!("{message}"),
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
    let module = soc_decisions::DecisionsModule::default();
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
        let mut active = BTreeSet::new();
        for module in modules {
            tx.execute(
                "INSERT INTO modules(name,active) VALUES(?1,?2) ON CONFLICT(name) DO NOTHING",
                soc_db::params![module.name(), module.active_by_default()],
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
pub fn run() -> Result<()> {
    let (config, _secrets) = Config::load(Path::new(".env"))?;
    let db = Database::open(&config.database_path, config.db_busy_timeout_ms)?;
    eprintln!(
        "event=database_ready connections=1 busy_timeout_ms={}",
        config.db_busy_timeout_ms
    );
    let state = AppState {
        config: Arc::new(config),
        db: db.clone(),
        shutdown: Shutdown::default(),
    };
    let result = (|| {
        let presence = Arc::new(PresenceModule::default());
        let transcription = Arc::new(soc_transcription::TranscriptionModule::default());
        let decisions = Arc::new(soc_decisions::DecisionsModule::default());
        let modules =
            crate::registry::build(presence.clone(), transcription.clone(), decisions.clone());
        let order = crate::registry::order(&modules)?;
        let active = register(&db, &modules)?;
        crate::registry::validate_active(&modules, &active)?;
        for i in &order {
            db.migrate(modules[*i].name(), modules[*i].migrations())?;
        }
        let started = start_all(&modules, &order, &active, &state)?;
        let ui_result = if active.contains("presence") {
            eprintln!("event=ui_starting");
            presence.run_with_decisions(
                &state,
                if active.contains("transcription") {
                    Some(&transcription)
                } else {
                    None
                },
                if active.contains("decisions") {
                    Some(&decisions)
                } else {
                    None
                },
            )
        } else {
            eprintln!("event=no_active_surface");
            Ok(())
        };
        let stop_result = stop_all(&modules, &started, &state);
        ui_result.and(stop_result)
    })();
    let close = db.close();
    if close.is_ok() {
        eprintln!("event=shutdown_complete");
    } else {
        eprintln!("event=database_close_failed");
    }
    result.and(close)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            Arc::new(soc_transcription::TranscriptionModule::default()),
            Arc::new(soc_decisions::DecisionsModule::default()),
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
