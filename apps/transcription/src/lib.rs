mod audio;
pub use audio::input_devices;
mod controller;
pub mod model;
#[cfg(target_os = "macos")]
mod permissions;
mod process;
mod settings;
mod worker;
pub use controller::{Controller, ModelStatus};
pub use settings::{Mode, Settings};
pub fn normalize_shortcut(value: &str) -> Option<String> {
    value
        .parse::<handy_keys::Hotkey>()
        .ok()
        .map(|key| key.to_string())
}
use soc_core::{AppModule, AppState, Migration, Result};
use std::sync::{
    Mutex,
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::JoinHandle;
pub use worker::emit as worker_emit;
pub use worker::{Event, run_worker};

pub struct TranscriptionModule {
    worker: Mutex<Option<PreferenceWorker>>,
}
struct PreferenceWorker {
    sender: SyncSender<Settings>,
    replies: Option<Receiver<std::result::Result<Settings, String>>>,
    handle: JoinHandle<()>,
}
impl Default for TranscriptionModule {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}
impl AppModule for TranscriptionModule {
    fn name(&self) -> &'static str {
        "transcription"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_preferences.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_microphone_timeout.sql"),
            },
            Migration {
                version: 3,
                sql: include_str!("../migrations/0003_bubble_default.sql"),
            },
        ]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut guard = self.worker.lock().map_err(|_| "Worker inválido")?;
        if guard.is_some() {
            return Ok(());
        }
        let db = state.db.clone();
        let (sender, rx) = mpsc::sync_channel::<Settings>(4);
        let (tx, replies) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("transcription-preferences".into())
            .spawn(move || {
                for s in rx {
                    let result = settings::save(&db, &s)
                        .map(|()| s)
                        .map_err(|_| "No se pudieron guardar los ajustes".to_string());
                    if tx.send(result).is_err() {
                        break;
                    }
                }
            })?;
        *guard = Some(PreferenceWorker {
            sender,
            replies: Some(replies),
            handle,
        });
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        if let Some(w) = self.worker.lock().map_err(|_| "Worker inválido")?.take() {
            drop(w.sender);
            w.handle
                .join()
                .map_err(|_| "Worker de transcripción terminó inesperadamente")?;
        }
        Ok(())
    }
}
impl TranscriptionModule {
    pub fn controller(&self, state: &AppState) -> Result<Controller> {
        self.create_controller(state, None)
    }
    pub fn review_controller(&self, state: &AppState) -> Result<Controller> {
        self.create_controller(state, Some("Ctrl+Option+Shift+R"))
    }
    fn create_controller(
        &self,
        state: &AppState,
        review_shortcut: Option<&str>,
    ) -> Result<Controller> {
        let mut s = settings::load(&state.db)?;
        if let Some(shortcut) = review_shortcut {
            s.enabled = true;
            s.shortcut = shortcut.into();
        }
        let mut guard = self.worker.lock().map_err(|_| "Worker inválido")?;
        let w = guard.as_mut().ok_or("Transcripción no iniciada")?;
        Controller::new(
            s,
            w.sender.clone(),
            w.replies.take().ok_or("Transcripción ya conectada")?,
        )
    }
}
