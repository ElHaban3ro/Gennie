//! Un único coordinador posee los controladores y los detiene antes de cerrar SQLite.
use crate::{api::*, frb_generated::StreamSink, registry, startup};
use gennie_core::{AppModule, AppState, Result, Shutdown};
use gennie_db::Database;
use gennie_presence::{PresenceModule, history};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender, SyncSender},
    },
    thread::JoinHandle,
    time::Duration,
};

type Reply = Sender<std::result::Result<Snapshot, String>>;
pub(crate) enum Operation {
    Select(Option<String>),
    Send(String),
    NewChat,
    Connect,
    Rename(String, String),
    Delete(String),
    Assistant(AssistantSettings),
    Voice(VoiceSettings),
    Preview(VoiceSettings),
    Dictation(DictationSettings),
    Motion(bool),
    Prepare,
    Release,
    Authorize,
    Stop,
}
struct Handle {
    sender: SyncSender<(Operation, Reply)>,
    thread: JoinHandle<()>,
}
static HANDLE: Mutex<Option<Handle>> = Mutex::new(None);
static CURRENT: Mutex<Option<Snapshot>> = Mutex::new(None);
static SINKS: Mutex<Vec<StreamSink<Snapshot>>> = Mutex::new(Vec::new());

pub(crate) fn start(env: String, helper: String) -> std::result::Result<Snapshot, String> {
    let mut guard = HANDLE.lock().map_err(|_| "Estado del backend inválido")?;
    if guard.is_some() {
        return CURRENT
            .lock()
            .map_err(|_| "Estado inválido")?
            .clone()
            .ok_or("El backend está iniciando".into());
    }
    let (sender, commands) = mpsc::sync_channel::<(Operation, Reply)>(16);
    let (ready_tx, ready) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("gennie-desktop".into())
        .spawn(move || {
            let mut backend = match Backend::open(&env, &helper) {
                Ok(b) => b,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            backend.publish();
            let _ = ready_tx.send(Ok(backend.snapshot.clone()));
            loop {
                match commands.recv_timeout(Duration::from_millis(25)) {
                    Ok((Operation::Stop, reply)) => {
                        let result = backend
                            .close()
                            .map(|()| backend.snapshot.clone())
                            .map_err(|e| e.to_string());
                        let _ = reply.send(result);
                        break;
                    }
                    Ok((operation, reply)) => {
                        backend.snapshot.error = None;
                        backend.snapshot.confirmation = None;
                        let result = backend.apply(operation).and_then(|()| backend.refresh());
                        if let Err(e) = &result {
                            backend.snapshot.error = Some(e.to_string());
                        }
                        backend.publish();
                        let _ = reply.send(
                            result
                                .map(|()| backend.snapshot.clone())
                                .map_err(|e| e.to_string()),
                        );
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                backend.tick();
            }
        })
        .map_err(|e| e.to_string())?;
    match ready
        .recv()
        .map_err(|_| "El backend terminó durante el arranque")?
    {
        Ok(snapshot) => {
            *guard = Some(Handle { sender, thread });
            Ok(snapshot)
        }
        Err(e) => {
            let _ = thread.join();
            Err(e)
        }
    }
}
pub(crate) fn request(op: Operation) -> std::result::Result<Snapshot, String> {
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    sender
        .try_send((op, reply))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result.recv().map_err(|_| "El backend dejó de responder")?
}
pub(crate) fn watch(sink: StreamSink<Snapshot>) -> std::result::Result<(), String> {
    // El mismo orden de locks que publish evita perder la primera actualización.
    let mut sinks = SINKS.lock().map_err(|_| "Canal inválido")?;
    if let Some(snapshot) = CURRENT.lock().map_err(|_| "Estado inválido")?.clone() {
        sink.add(snapshot).map_err(|_| "Canal cerrado")?;
    }
    sinks.push(sink);
    Ok(())
}
pub(crate) fn stop() -> std::result::Result<(), String> {
    let mut guard = HANDLE.lock().map_err(|_| "Estado inválido")?;
    if let Some(handle) = guard.take() {
        let (reply, result) = mpsc::channel();
        let sent = handle.sender.send((Operation::Stop, reply)).is_ok();
        let outcome = if sent {
            result
                .recv()
                .map_err(|_| "Cierre interrumpido".to_string())
                .and_then(|v| v.map(|_| ()))
        } else {
            Err("El backend terminó inesperadamente".into())
        };
        handle
            .thread
            .join()
            .map_err(|_| "El coordinador terminó inesperadamente")?;
        SINKS.lock().map_err(|_| "Canal inválido")?.clear();
        *CURRENT.lock().map_err(|_| "Estado inválido")? = None;
        outcome?;
    }
    Ok(())
}

struct Backend {
    state: AppState,
    modules: Vec<Arc<dyn AppModule>>,
    started: Vec<usize>,
    llm: Option<gennie_llm::Controller>,
    voice: Option<gennie_tts::Controller>,
    dictation: Option<gennie_transcription::Controller>,
    snapshot: Snapshot,
    narration: gennie_presence::narration::Narration,
    speech_queue: std::collections::VecDeque<String>,
    revision: Arc<std::sync::atomic::AtomicU64>,
    observed: u64,
    closed: bool,
    last_completed: Option<String>,
    failed_turn: bool,
}
impl Backend {
    fn open(env: &str, helper: &str) -> Result<Self> {
        let config = desktop_config(env)?;
        Self::open_config(config, helper)
    }
    fn open_config(config: gennie_config::Config, helper: &str) -> Result<Self> {
        gennie_transcription::configure_helper(helper.into())?;
        let db = Database::open(&config.database_path, config.db_busy_timeout_ms)?;
        let state = AppState {
            config: Arc::new(config),
            db,
            shutdown: Shutdown::default(),
        };
        let presence = Arc::new(PresenceModule::default());
        let revision = presence.revision();
        let transcription = Arc::new(gennie_transcription::TranscriptionModule::default());
        let decisions = Arc::new(gennie_decisions::DecisionsModule::default());
        let llm = Arc::new(gennie_llm::LlmModule::with_journal(
            presence.conversation_journal(state.db.clone()),
        ));
        let tts = Arc::new(gennie_tts::TtsModule::default());
        let reminders = Arc::new(gennie_reminders_plugin::RemindersPluginModule::default());
        let modules = registry::build(
            presence,
            transcription.clone(),
            decisions,
            llm.clone(),
            tts.clone(),
            reminders,
        );
        let setup = (|| {
            let order = registry::order(&modules)?;
            let active = startup::register(&state.db, &modules)?;
            registry::validate_active(&modules, &active)?;
            for i in &order {
                state
                    .db
                    .migrate(modules[*i].name(), modules[*i].migrations())?;
            }
            startup::start_all(&modules, &order, &active, &state).map(|started| (started, active))
        })();
        let (started, active) = match setup {
            Ok(v) => v,
            Err(e) => {
                let _ = state.db.close();
                return Err(e);
            }
        };
        let mut backend = Self {
            state,
            modules,
            started,
            llm: None,
            voice: None,
            dictation: None,
            snapshot: Snapshot {
                connection: "Sin conectar".into(),
                ..Snapshot::default()
            },
            revision,
            narration: Default::default(),
            speech_queue: Default::default(),
            observed: 0,
            closed: false,
            last_completed: None,
            failed_turn: false,
        };
        // Drop limpia los módulos ya iniciados si falla cualquiera de estos controladores.
        if active.contains("llm") {
            backend.llm = Some(llm.controller()?);
        }
        if active.contains("tts") {
            backend.voice = Some(tts.controller()?);
        }
        if active.contains("transcription") {
            backend.dictation = Some(transcription.controller(&backend.state)?);
        }
        backend.refresh()?;
        Ok(backend)
    }
    fn refresh(&mut self) -> Result<()> {
        let revision = self.revision.load(std::sync::atomic::Ordering::Acquire);
        let selected = self
            .snapshot
            .selected_id
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()?;
        let history = history::load(&self.state.db, selected)?;
        self.snapshot.conversations = history
            .conversations
            .into_iter()
            .map(|c| Conversation {
                id: c.id.to_string(),
                title: c.title,
                updated_at: c.updated_at,
            })
            .collect();
        self.snapshot.messages = history
            .selected_messages
            .into_iter()
            .map(|m| Message {
                speaker: m.speaker,
                text: m.text,
                created_at: m.created_at,
            })
            .collect();
        self.snapshot.active_id = history.active_id.map(|id| id.to_string());
        self.snapshot.selected_id = history.selected_id.map(|id| id.to_string());
        self.refresh_controls();
        self.snapshot.reduced_motion = gennie_presence::reduced_motion(&self.state.db)?;
        self.observed = revision;
        Ok(())
    }
    fn refresh_controls(&mut self) {
        self.snapshot.busy = self.llm.as_ref().is_some_and(|c| c.is_pending());
        self.snapshot.assistant = self.llm.as_ref().map(|c| {
            let s = c.settings();
            AssistantSettings {
                model: s.model,
                effort: s.effort,
                read_files: s.read_files,
                write_files: s.write_files,
                run_commands: s.run_commands,
                local_actions: s.local_actions,
            }
        });
        self.snapshot.voice = self.voice.as_ref().map(|c| {
            let s = c.settings();
            VoiceSettings {
                enabled: s.enabled,
                voice: s.voice,
                speed_percent: s.speed_percent,
                unload_minutes: s.unload_seconds.map(|n| (n / 60) as u32),
            }
        });
        if let Some(c) = &self.dictation {
            let s = &c.settings;
            self.snapshot.dictation = Some(DictationSettings {
                enabled: s.enabled,
                shortcut: s.shortcut.clone(),
                hold: s.mode == gennie_transcription::Mode::Hold,
                unload_minutes: (s.unload_seconds / 60) as u32,
                microphone: s.microphone_name.clone(),
            });
            self.snapshot.dictation_status = c.settings_status.clone();
        }
        if self.failed_turn
            && let Some(text) = &self.last_completed
        {
            let saved = self
                .snapshot
                .messages
                .iter()
                .rev()
                .take_while(|message| message.speaker != "user")
                .any(|message| message.speaker == "assistant" && message.text == *text);
            if saved {
                self.snapshot.reply.clear();
            } else {
                self.snapshot.reply = text.clone();
            }
        }
    }
    fn apply(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::Select(id) => {
                if self.snapshot.selected_id != id {
                    self.snapshot.reply.clear();
                }
                self.snapshot.selected_id = id;
            }
            Operation::Send(text) => {
                if text.trim().is_empty() {
                    return Err("Escribe un mensaje".into());
                }
                if text.len() > 64 * 1024 {
                    return Err("El mensaje es demasiado largo".into());
                }
                if self.snapshot.selected_id != self.snapshot.active_id {
                    return Err("Este chat es de consulta. Abre el chat activo para enviar".into());
                }
                self.llm
                    .as_ref()
                    .ok_or("Conversación desactivada")?
                    .speak(&text)?;
                self.snapshot.acknowledged_text = None;
                self.snapshot.reply.clear();
                self.last_completed = None;
                self.failed_turn = false;
            }
            Operation::NewChat => {
                self.llm
                    .as_ref()
                    .ok_or("Conversación desactivada")?
                    .new_chat()?;
                self.snapshot.reply.clear();
                self.speech_queue.clear();
                self.narration.clear();
            }
            Operation::Connect => self
                .llm
                .as_ref()
                .ok_or("Conversación desactivada")?
                .connect()?,
            Operation::Rename(id, title) => {
                history::rename(&self.state.db, id.parse()?, &title)?;
                self.snapshot.confirmation = Some("Nombre guardado".into());
            }
            Operation::Delete(id) => {
                history::delete(&self.state.db, id.parse()?)?;
                self.snapshot.confirmation = Some("Chat eliminado de Gennie".into());
            }
            Operation::Assistant(s) => self
                .llm
                .as_ref()
                .ok_or("Conversación desactivada")?
                .save_settings(gennie_llm::Settings {
                    model: s.model,
                    effort: s.effort,
                    read_files: s.read_files,
                    write_files: s.write_files,
                    run_commands: s.run_commands,
                    local_actions: s.local_actions,
                })?,
            Operation::Voice(s) => self
                .voice
                .as_ref()
                .ok_or("Voz desactivada")?
                .save(voice_settings(s))?,
            Operation::Preview(s) => {
                self.voice
                    .as_ref()
                    .ok_or("Voz desactivada")?
                    .preview(voice_settings(s))?;
            }
            Operation::Dictation(s) => {
                let c = self.dictation.as_mut().ok_or("Dictado desactivado")?;
                let mut settings = c.settings.clone();
                settings.enabled = s.enabled;
                settings.shortcut = s.shortcut;
                settings.mode = if s.hold {
                    gennie_transcription::Mode::Hold
                } else {
                    gennie_transcription::Mode::Toggle
                };
                settings.unload_seconds = u64::from(s.unload_minutes) * 60;
                settings.microphone_name = s.microphone;
                c.save(settings)?;
            }
            Operation::Motion(value) => {
                gennie_presence::save_reduced_motion(&self.state.db, value)?;
                self.snapshot.confirmation = Some("Preferencia guardada".into());
            }
            Operation::Prepare => self
                .dictation
                .as_mut()
                .ok_or("Dictado desactivado")?
                .prepare(),
            Operation::Release => self
                .dictation
                .as_mut()
                .ok_or("Dictado desactivado")?
                .cancel(),
            Operation::Authorize => self
                .dictation
                .as_mut()
                .ok_or("Dictado desactivado")?
                .retry_keys(),
            Operation::Stop => unreachable!(),
        }
        Ok(())
    }
    fn tick(&mut self) {
        let mut dirty = false;
        if let Some(c) = &mut self.dictation {
            for event in c.tick() {
                use gennie_transcription::Event::*;
                match event {
                    Text {
                        text,
                        finalized: true,
                    } => {
                        if let Some(llm) = &self.llm
                            && let Err(e) = llm.speak(&text)
                        {
                            self.snapshot.error = Some(e.to_string());
                        }
                    }
                    DictationRequested => {
                        self.speech_queue.clear();
                        self.narration.clear();
                        if let Some(v) = &self.voice {
                            let _ = v.interrupt();
                            let _ = v.preload();
                        }
                    }
                    Error(e) | NoSpeech(e) => self.snapshot.error = Some(e),
                    Saved => {
                        self.snapshot.confirmation = Some("Ajustes de dictado guardados".into())
                    }
                    _ => {}
                }
                dirty = true;
            }
            if self.snapshot.dictation_status != c.settings_status {
                dirty = true;
            }
        }
        if let Some(c) = &self.llm {
            for event in c.poll() {
                use gennie_llm::Event::*;
                match event {
                    Connected => self.snapshot.connection = "Conectado".into(),
                    Connecting => self.snapshot.connection = "Conectando…".into(),
                    Disconnected(e) => {
                        self.snapshot.connection = "Sin conectar".into();
                        self.snapshot.error = Some(e);
                    }
                    ReplyDelta(text) => {
                        self.snapshot.reply.push_str(&text);
                        self.speech_queue.extend(self.narration.push(&text));
                    }
                    Reply(text) => {
                        self.last_completed = Some(text.clone());
                        if self.snapshot.reply.is_empty() {
                            self.speech_queue.push_back(text);
                        } else if let Some(last) = self.narration.finish() {
                            self.speech_queue.push_back(last);
                        }
                        self.snapshot.reply.clear();
                    }
                    UserMessageSaved(text) => {
                        self.snapshot.acknowledged_text = Some(text);
                        self.snapshot.acknowledgement =
                            self.snapshot.acknowledgement.wrapping_add(1);
                    }
                    NewChatReady => {
                        self.snapshot.selected_id = None;
                        self.snapshot.confirmation = Some("Nuevo chat preparado".into());
                    }
                    SettingsSaved(_) => {
                        self.snapshot.confirmation = Some("Ajustes del asistente guardados".into())
                    }
                    Failed(e) => {
                        self.snapshot.error = Some(e);
                        self.failed_turn = true;
                    }
                    HistoryWarning(e) | SettingsSaveFailed(e) => {
                        self.snapshot.error = Some(e);
                    }
                }
                dirty = true;
            }
            if c.is_pending() != self.snapshot.busy {
                dirty = true;
            }
        }
        if self.voice.is_none() {
            self.speech_queue.clear();
        }
        if let Some(c) = &self.voice {
            while let Some(text) = self.speech_queue.front() {
                match c.speak(&gennie_presence::speech_text(text)) {
                    Ok(_) => {
                        self.speech_queue.pop_front();
                    }
                    Err(_) => break,
                }
            }
            for event in c.poll() {
                use gennie_tts::Event::*;
                match event {
                    Saved(_) => {
                        self.snapshot.confirmation = Some("Ajustes de voz guardados".into())
                    }
                    SaveFailed(e) | Failed(_, e) | PreloadFailed(e) => {
                        self.snapshot.error = Some(e)
                    }
                    _ => {}
                }
                dirty = true;
            }
            let status = format!("{:?}", c.model_status());
            if status != self.snapshot.voice_status {
                self.snapshot.voice_status = status;
                dirty = true;
            }
        }
        let history_changed =
            self.observed != self.revision.load(std::sync::atomic::Ordering::Acquire);
        dirty |= history_changed;
        if dirty {
            if history_changed {
                if let Err(e) = self.refresh() {
                    self.snapshot.error = Some(e.to_string());
                }
            } else {
                self.refresh_controls();
            }
            self.publish();
        }
    }
    fn publish(&self) {
        if let Ok(mut sinks) = SINKS.lock() {
            if let Ok(mut current) = CURRENT.lock() {
                *current = Some(self.snapshot.clone());
            }
            sinks.retain(|sink| sink.add(self.snapshot.clone()).is_ok());
        }
    }
    fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if let Some(mut c) = self.dictation.take() {
            c.cancel();
        }
        self.llm.take();
        self.voice.take();
        let result = startup::stop_all(&self.modules, &self.started, &self.state);
        result.and(self.state.db.close())
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            eprintln!("event=backend_shutdown_failed error={e}");
        }
    }
}
fn voice_settings(s: VoiceSettings) -> gennie_tts::Settings {
    gennie_tts::Settings {
        enabled: s.enabled,
        voice: s.voice,
        speed_percent: s.speed_percent,
        unload_seconds: s.unload_minutes.map(|n| u64::from(n) * 60),
    }
}

// La distribución usa datos de usuario; el lanzador de desarrollo pasa el .env del checkout.
fn desktop_config(env_file: &str) -> Result<gennie_config::Config> {
    if !env_file.is_empty() || std::env::var_os("GENNIE_DATABASE_PATH").is_some() {
        return gennie_config::Config::load(Path::new(env_file))
            .map(|(config, _)| resolve_config_path(config, env_file));
    }
    let home = std::env::var_os("HOME").ok_or("No se pudo localizar el directorio de usuario")?;
    let path = Path::new(&home).join("Library/Application Support/Gennie/gennie.sqlite3");
    let mut values = std::collections::BTreeMap::new();
    values.insert(
        "GENNIE_DATABASE_PATH".into(),
        path.to_string_lossy().into_owned(),
    );
    if let Ok(timeout) = std::env::var("GENNIE_DB_BUSY_TIMEOUT_MS") {
        values.insert("GENNIE_DB_BUSY_TIMEOUT_MS".into(), timeout);
    }
    gennie_config::Config::from_values(&values)
}

fn resolve_config_path(mut config: gennie_config::Config, env_file: &str) -> gennie_config::Config {
    if !env_file.is_empty()
        && config.database_path.is_relative()
        && let Some(parent) = Path::new(env_file).parent()
    {
        config.database_path = parent.join(&config.database_path);
    }
    config
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
