mod desktop;
mod repository;
use slint::ComponentHandle;
use soc_core::{AppModule, AppState, Migration, Result};
use std::sync::{
    Mutex,
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::JoinHandle;

slint::include_modules!();

pub struct PresenceModule {
    worker: Mutex<Option<Worker>>,
}
struct Worker {
    sender: SyncSender<bool>,
    replies: Option<Receiver<std::result::Result<bool, String>>>,
    handle: JoinHandle<()>,
}
impl Default for PresenceModule {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}
impl AppModule for PresenceModule {
    fn name(&self) -> &'static str {
        "presence"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[Migration {
            version: 1,
            sql: include_str!("../migrations/0001_preferences.sql"),
        }]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Worker de presencia inválido")?;
        if worker.is_some() {
            return Ok(());
        }
        let db = state.db.clone();
        let (sender, receiver) = mpsc::sync_channel(8);
        let (reply_sender, replies) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("presence-preferences".into())
            .spawn(move || {
                for reduced in receiver {
                    let result =
                        repository::save(&db, reduced)
                            .map(|()| reduced)
                            .map_err(|error| {
                                eprintln!("event=preference_save_failed error={error}");
                                error.to_string()
                            });
                    let _ = reply_sender.send(result);
                }
            })?;
        *worker = Some(Worker {
            sender,
            replies: Some(replies),
            handle,
        });
        eprintln!("event=module_started module=presence");
        Ok(())
    }
    fn stop(&self, _state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de presencia inválido")?;
        let worker = guard.take();
        if let Some(Worker { sender, handle, .. }) = worker {
            drop(sender);
            handle
                .join()
                .map_err(|_| "Worker de preferencias terminó inesperadamente")?;
            eprintln!("event=module_stopped module=presence");
        }
        Ok(())
    }
}

impl PresenceModule {
    pub fn run(&self, state: &AppState) -> Result<()> {
        desktop::initialize()?;
        let reduced = repository::load(&state.db)?;
        let ui = PresenceWindow::new()?;
        ui.set_reduced_motion(reduced);
        let weak = ui.as_weak();
        ui.on_toggle(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_active(!ui.get_active());
                eprintln!("event=presence_state active={}", ui.get_active());
            }
        });
        let shutdown = state.shutdown.clone();
        ui.on_exit(move || {
            shutdown.request();
            let _ = slint::quit_event_loop();
        });
        let (sender, replies) = {
            let mut worker = self.worker.lock().map_err(|_| "Worker inválido")?;
            let worker = worker.as_mut().ok_or("Presencia no iniciada")?;
            (
                worker.sender.clone(),
                worker.replies.take().ok_or("La interfaz ya se ejecutó")?,
            )
        };
        let reply_timer = slint::Timer::default();
        let weak = ui.as_weak();
        reply_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(25),
            move || {
                if let Ok(result) = replies.try_recv()
                    && let Some(ui) = weak.upgrade()
                {
                    ui.set_persist_pending(false);
                    match result {
                        Ok(reduced) => {
                            ui.set_reduced_motion(reduced);
                            ui.set_status_text("Guardado · M · A · ESC".into());
                        }
                        Err(_) => ui.set_status_text("Error al guardar · ver registro".into()),
                    }
                }
            },
        );
        let weak = ui.as_weak();
        ui.on_motion_changed(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_persist_pending() {
                    return;
                }
                let next = !ui.get_reduced_motion();
                match sender.try_send(next) {
                    Ok(()) => {
                        ui.set_persist_pending(true);
                        ui.set_status_text("Guardando preferencia…".into());
                    }
                    Err(_) => ui.set_status_text("Error al guardar · reintentar M".into()),
                }
            }
        });
        let about = AboutWindow::new()?;
        let weak_about = about.as_weak();
        ui.on_about(move || {
            if let Some(about) = weak_about.upgrade() {
                let _ = about.show();
            }
        });
        let hit_timer = slint::Timer::default();
        let weak = ui.as_weak();
        let mut hit_error_logged = false;
        hit_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            move || {
                if let Some(ui) = weak.upgrade()
                    && let Err(error) =
                        desktop::update_hit_test(ui.window(), ui.get_actual_diameter())
                    && !hit_error_logged
                {
                    eprintln!("event=hit_test_failed error={error}");
                    hit_error_logged = true;
                }
            },
        );
        desktop::prepare(ui.window())?;
        ui.show()?;
        let fatal_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let startup_error = fatal_error.clone();
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            if let Some(ui) = weak.upgrade() {
                use slint::winit_030::WinitWindowAccessor;
                match ui.window().winit_window().await {
                    Ok(_) => {
                        if let Err(error) = desktop::position(ui.window()) {
                            eprintln!("event=window_position_failed error={error}");
                            *startup_error.borrow_mut() = Some(error.to_string());
                            let _ = slint::quit_event_loop();
                        } else {
                            eprintln!("event=ui_ready");
                        }
                    }
                    Err(error) => {
                        eprintln!("event=window_creation_failed error={error}");
                        *startup_error.borrow_mut() = Some(error.to_string());
                        let _ = slint::quit_event_loop();
                    }
                }
            }
        })?;
        ui.run()?;
        hit_timer.stop();
        reply_timer.stop();
        ui.hide()?;
        about.hide()?;
        if let Some(error) = fatal_error.borrow_mut().take() {
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;
    use soc_core::AppState;
    use soc_core::{Database, Shutdown};
    use std::sync::Arc;
    fn state() -> AppState {
        AppState {
            config: Arc::new(soc_core_config()),
            db: Database::memory().unwrap(),
            shutdown: Shutdown::default(),
        }
    }
    fn soc_core_config() -> soc_config::Config {
        soc_config::Config {
            database_path: "unused".into(),
            db_busy_timeout_ms: 50,
        }
    }
    #[test]
    fn worker_confirms_commit_and_reports_failure() {
        let state = state();
        let module = PresenceModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let (sender, replies) = {
            let mut worker = module.worker.lock().unwrap();
            let worker = worker.as_mut().unwrap();
            (worker.sender.clone(), worker.replies.take().unwrap())
        };
        sender.send(true).unwrap();
        assert!(
            replies
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .unwrap()
        );
        assert!(repository::load(&state.db).unwrap());
        state.db.close().unwrap();
        sender.send(false).unwrap();
        assert!(
            replies
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
        drop(sender);
        module.stop(&state).unwrap();
    }
}
