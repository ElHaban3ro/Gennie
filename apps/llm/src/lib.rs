mod codex;
mod resources;
mod settings;

pub use settings::Settings;

use soc_core::{AppModule, AppState, Migration, Result};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

pub struct LlmModule {
    worker: Mutex<Option<Worker>>,
}
struct Worker {
    sender: SyncSender<Command>,
    events: Arc<Mutex<Receiver<Event>>>,
    pending: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    process: codex::ProcessHandle,
    handle: JoinHandle<()>,
    settings: Arc<Mutex<Settings>>,
}
#[derive(Clone)]
pub struct Controller {
    sender: SyncSender<Command>,
    events: Arc<Mutex<Receiver<Event>>>,
    pending: Arc<AtomicBool>,
    settings: Arc<Mutex<Settings>>,
}
enum Command {
    Connect,
    Speak(String),
    NewChat,
    SaveSettings(Settings),
    Shutdown,
}
#[derive(Debug)]
pub enum Event {
    Connected,
    Connecting,
    Disconnected(String),
    ReplyDelta(String),
    Reply(String),
    NewChatReady,
    Failed(String),
    SettingsSaved(Settings),
    SettingsSaveFailed(String),
}

impl Default for LlmModule {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}
impl AppModule for LlmModule {
    fn name(&self) -> &'static str {
        "llm"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_conversation.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_preferences.sql"),
            },
        ]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de conversación inválido")?;
        if guard.is_some() {
            return Ok(());
        }
        let (sender, commands) = mpsc::sync_channel(4);
        let (event_sender, events) = mpsc::channel();
        let events = Arc::new(Mutex::new(events));
        let pending = Arc::new(AtomicBool::new(false));
        let stopping = Arc::new(AtomicBool::new(false));
        let process = codex::ProcessHandle::default();
        let db = state.db.clone();
        let settings = Arc::new(Mutex::new(settings::load(&db)?));
        let worker_settings = settings.clone();
        let worker_pending = pending.clone();
        let worker_stopping = stopping.clone();
        let worker_process = process.clone();
        let handle = std::thread::Builder::new()
            .name("llm-conversation".into())
            .spawn(move || {
                let mut session = None;
                while let Ok(command) = commands.recv() {
                    if worker_stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    if matches!(command, Command::Shutdown) {
                        break;
                    }
                    if let Command::SaveSettings(next) = command {
                        let result = settings::save(&db, &next);
                        match result {
                            Ok(()) => {
                                *worker_settings
                                    .lock()
                                    .expect("Ajustes de conversación inválidos") = next.clone();
                                session = None;
                                worker_process.kill();
                                let _ = event_sender.send(Event::Disconnected(
                                    "Ajustes guardados; se conectará al hablar".into(),
                                ));
                                let _ = event_sender.send(Event::SettingsSaved(next));
                            }
                            Err(error) => {
                                let _ =
                                    event_sender.send(Event::SettingsSaveFailed(error.to_string()));
                            }
                        }
                        worker_pending.store(false, Ordering::SeqCst);
                        continue;
                    }
                    let current = worker_settings
                        .lock()
                        .expect("Ajustes de conversación inválidos")
                        .clone();
                    let result = (|| -> Result<Option<Event>> {
                        if session.is_none() {
                            let _ = event_sender.send(Event::Connecting);
                            session = Some(if matches!(&command, Command::NewChat) {
                                codex::Session::connect_new(&db, worker_process.clone(), &current)?
                            } else {
                                codex::Session::connect(&db, worker_process.clone(), &current)?
                            });
                            let _ = event_sender.send(Event::Connected);
                            if matches!(&command, Command::NewChat) {
                                return Ok(Some(Event::NewChatReady));
                            }
                        }
                        match command {
                            Command::Speak(text) => {
                                session.as_mut().expect("sesión creada").speak_stream(
                                    &text,
                                    |event| {
                                        let event = match event {
                                            codex::StreamEvent::Delta(text) => {
                                                Event::ReplyDelta(text)
                                            }
                                            codex::StreamEvent::Completed(text) => {
                                                Event::Reply(text)
                                            }
                                        };
                                        let _ = event_sender.send(event);
                                    },
                                )?;
                                Ok(None)
                            }
                            Command::NewChat => {
                                session.as_mut().expect("sesión creada").new_chat(&db)?;
                                Ok(Some(Event::NewChatReady))
                            }
                            Command::Connect | Command::Shutdown | Command::SaveSettings(_) => {
                                Ok(None)
                            }
                        }
                    })();
                    match result {
                        Ok(Some(event)) => {
                            let _ = event_sender.send(event);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            let message = error.to_string();
                            if error.downcast_ref::<codex::WebSearchMissing>().is_none() {
                                session = None;
                                worker_process.kill();
                                let _ = event_sender.send(Event::Disconnected(message.clone()));
                            }
                            let _ = event_sender.send(Event::Failed(message));
                        }
                    }
                    worker_pending.store(false, Ordering::SeqCst);
                }
                worker_process.kill();
            })?;
        *guard = Some(Worker {
            sender,
            events,
            pending,
            stopping,
            process,
            handle,
            settings,
        });
        Ok(())
    }
    fn stop(&self, _state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de conversación inválido")?;
        if let Some(worker) = guard.take() {
            worker.stopping.store(true, Ordering::SeqCst);
            let _ = worker.sender.try_send(Command::Shutdown);
            worker.process.kill();
            drop(worker.sender);
            worker
                .handle
                .join()
                .map_err(|_| "Worker de conversación terminó inesperadamente")?;
        }
        Ok(())
    }
}
impl LlmModule {
    pub fn controller(&self) -> Result<Controller> {
        let guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de conversación inválido")?;
        let worker = guard.as_ref().ok_or("Conversación no iniciada")?;
        Ok(Controller {
            sender: worker.sender.clone(),
            events: worker.events.clone(),
            pending: worker.pending.clone(),
            settings: worker.settings.clone(),
        })
    }
}
impl Controller {
    fn submit(&self, command: Command) -> Result<()> {
        if self.pending.swap(true, Ordering::SeqCst) {
            return Err("SOC aún está respondiendo".into());
        }
        if self.sender.try_send(command).is_err() {
            self.pending.store(false, Ordering::SeqCst);
            return Err("No se pudo enviar el mensaje".into());
        }
        Ok(())
    }
    pub fn connect(&self) -> Result<()> {
        self.submit(Command::Connect)
    }
    pub fn new_chat(&self) -> Result<()> {
        self.submit(Command::NewChat)
    }
    pub fn settings(&self) -> Settings {
        self.settings
            .lock()
            .expect("Ajustes de conversación inválidos")
            .clone()
    }
    pub fn save_settings(&self, next: Settings) -> Result<()> {
        next.validate()?;
        self.submit(Command::SaveSettings(next))
    }
    pub fn speak(&self, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 1200 {
            return Err("Mensaje vacío o demasiado largo".into());
        }
        self.submit(Command::Speak(text.into()))
    }
    pub fn is_pending(&self) -> bool {
        self.pending.load(Ordering::SeqCst)
    }
    pub fn poll(&self) -> Vec<Event> {
        let Ok(events) = self.events.lock() else {
            return vec![Event::Failed(
                "Eventos de conversación no disponibles".into(),
            )];
        };
        events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex::{load_thread, save_thread};
    use soc_config::Config;
    use soc_core::Shutdown;
    use soc_db::Database;
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn stop_wakes_idle_worker_while_controller_is_still_held() {
        let module = LlmModule::default();
        let state = AppState {
            config: Arc::new(Config {
                database_path: PathBuf::from("unused.sqlite3"),
                db_busy_timeout_ms: 500,
            }),
            db: Database::memory().unwrap(),
            shutdown: Shutdown::default(),
        };
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let _controller = module.controller().unwrap();
        module.stop(&state).unwrap();
        module.stop(&state).unwrap();
    }

    #[test]
    fn conversation_id_is_reused_after_reopening_database_handle() {
        let path = std::env::temp_dir().join(format!(
            "soc-llm-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Database::open(&path, 500).unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        assert_eq!(load_thread(&db).unwrap(), None);
        save_thread(&db, "thread-one").unwrap();
        db.close().unwrap();
        let reopened = Database::open(&path, 500).unwrap();
        assert_eq!(
            load_thread(&reopened).unwrap().as_deref(),
            Some("thread-one")
        );
        save_thread(&reopened, "thread-two").unwrap();
        assert_eq!(
            load_thread(&reopened).unwrap().as_deref(),
            Some("thread-two")
        );
        reopened.close().unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn settings_are_saved_and_reloaded_without_changing_thread() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        save_thread(&db, "saved-thread").unwrap();
        assert_eq!(settings::load(&db).unwrap(), Settings::default());
        let next = Settings {
            read_files: true,
            write_files: false,
            run_commands: false,
            local_actions: false,
            model: "gpt-6-astra".into(),
            effort: "high".into(),
        };
        settings::save(&db, &next).unwrap();
        assert_eq!(settings::load(&db).unwrap(), next);
        assert_eq!(load_thread(&db).unwrap().as_deref(), Some("saved-thread"));
    }
    #[test]
    fn legacy_codex_thread_survives_llm_migration() {
        let db = Database::memory().unwrap();
        db.migrate(
            "codex",
            &[Migration {
                version: 1,
                sql: "CREATE TABLE codex_conversation (id INTEGER PRIMARY KEY CHECK (id = 1), thread_id TEXT NOT NULL CHECK (length(thread_id) > 0), updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')));",
            }],
        )
        .unwrap();
        save_thread(&db, "existing-thread").unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        assert_eq!(
            load_thread(&db).unwrap().as_deref(),
            Some("existing-thread")
        );
    }
    #[test]
    #[ignore = "consume un turno real de la suscripción ChatGPT mediante Codex CLI"]
    fn real_app_server_reuses_one_thread_for_two_messages() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let cwd = std::env::temp_dir().join(format!("soc-llm-probe-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).unwrap();
        let mut session = codex::Session::connect_in(
            &db,
            codex::ProcessHandle::default(),
            cwd.clone(),
            &Settings::default(),
        )
        .unwrap();
        let thread = load_thread(&db).unwrap().unwrap();
        assert!(
            !session
                .speak("Di hola en una frase breve.")
                .unwrap()
                .is_empty()
        );
        assert!(
            !session
                .speak("Responde de nuevo en una frase breve.")
                .unwrap()
                .is_empty()
        );
        assert_eq!(load_thread(&db).unwrap().unwrap(), thread);
        drop(session);
        let resumed = codex::Session::connect_in(
            &db,
            codex::ProcessHandle::default(),
            cwd.clone(),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(load_thread(&db).unwrap().unwrap(), thread);
        drop(resumed);
        let _ = std::fs::remove_dir_all(cwd);
    }

    #[test]
    #[ignore = "consume una búsqueda web real de la suscripción ChatGPT mediante Codex CLI"]
    fn real_app_server_verifies_requested_web_search() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let base = std::env::temp_dir().join(format!(
            "soc-llm-web-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut session = codex::Session::connect_in(
            &db,
            codex::ProcessHandle::default(),
            base.clone(),
            &Settings::default(),
        )
        .unwrap();
        let reply = session
            .speak("Busca en Internet cuál es el ave viva más grande por peso y altura y cita una fuente.")
            .unwrap();
        assert!(!reply.trim().is_empty());
        drop(session);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    #[ignore = "consume dos turnos reales de la suscripción ChatGPT mediante Codex CLI"]
    fn real_app_server_searches_after_transcription_correction() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let base = std::env::temp_dir().join(format!(
            "soc-llm-correction-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut session = codex::Session::connect_in(
            &db,
            codex::ProcessHandle::default(),
            base.clone(),
            &Settings::default(),
        )
        .unwrap();
        let _first = session
            .speak("Por favor, te internet colecer había más grande del mundo")
            .unwrap();
        let reply = session
            .speak("Na verdade, te pregunté por el avión más grande del mundo")
            .unwrap();
        assert!(
            reply.contains("http"),
            "la respuesta debe incluir una fuente"
        );
        drop(session);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    #[ignore = "crea dos hilos temporales y consume un turno real de la suscripción ChatGPT"]
    fn real_app_server_new_chat_persists_and_resumes() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        save_thread(&db, "missing-legacy-thread").unwrap();
        let base = std::env::temp_dir().join(format!(
            "soc-llm-new-chat-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut session = codex::Session::connect_new_in(
            &db,
            codex::ProcessHandle::default(),
            base.clone(),
            &Settings::default(),
        )
        .unwrap();
        let previous = load_thread(&db).unwrap().unwrap();
        assert_ne!(previous, "missing-legacy-thread");
        session.new_chat(&db).unwrap();
        let current = load_thread(&db).unwrap().unwrap();
        assert_ne!(current, previous);
        assert!(!session.speak("Di hola brevemente.").unwrap().is_empty());
        assert_eq!(load_thread(&db).unwrap().unwrap(), current);
        drop(session);
        let resumed = codex::Session::connect_in(
            &db,
            codex::ProcessHandle::default(),
            base.clone(),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(load_thread(&db).unwrap().unwrap(), current);
        drop(resumed);
        let _ = std::fs::remove_dir_all(base);
    }
}
