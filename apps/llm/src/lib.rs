pub mod architects;
pub mod builder;
mod codex;
pub mod context;
pub mod delegation;
pub mod notices;
mod reply;
mod settings;
mod title;
mod tool_use;
mod turn;
pub use codex::mcp_client;
pub use tool_use::{ToolOutcome, ToolUse};
pub use turn::Progress as TurnProgress;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuotedMessage {
    pub speaker: String,
    pub text: String,
    pub created_at: String,
    pub truncated: bool,
}

pub use settings::Settings;

use gennie_core::{AppModule, AppState, Migration, Result};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyInterruption {
    Cancelled,
    Failed,
}
impl ReplyInterruption {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err("Estado de respuesta desconocido".into()),
        }
    }
}

/// Persistencia local inyectada por bootstrap; se invoca únicamente desde el worker.
pub trait ConversationJournal: Send + Sync {
    fn delegation_origin(&self, _conversation: i64) -> Result<Option<delegation::Origin>> {
        Err("El diario no admite delegaciones".into())
    }
    fn delegation_context(
        &self,
        _origin: &delegation::Origin,
    ) -> Result<Option<Vec<delegation::ContextMessage>>> {
        Err("El diario no admite contexto de delegaciones".into())
    }
    /// Usa la conexión prestada: mensaje y recibo del inbox se confirman juntos.
    fn deliver_delegation(
        &self,
        _connection: &gennie_db::SqliteConnection,
        _update: &delegation::Update,
        _text: &str,
    ) -> Result<bool> {
        Err("El diario no admite mensajes de delegaciones".into())
    }
    fn deliver_execution_notice(
        &self,
        _connection: &gennie_db::SqliteConnection,
        _update: &notices::Update,
        _text: &str,
    ) -> Result<bool> {
        Err("El diario no admite resultados de Encargos".into())
    }
    /// Participa en la transacción del hilo, sin abrir otra conexión ni confirmar por separado.
    fn activate(
        &self,
        connection: &gennie_db::SqliteConnection,
        provider: &str,
        thread_id: &str,
        title: Option<&str>,
    ) -> Result<()>;
    fn append(&self, provider: &str, thread_id: &str, speaker: &str, text: &str) -> Result<()>;
    fn begin_turn(&self, _provider: &str, _thread: &str, _turn: &str) -> Result<()> {
        Ok(())
    }
    fn record_tool(
        &self,
        _provider: &str,
        _thread: &str,
        _turn: &str,
        _usage: &ToolUse,
    ) -> Result<()> {
        Err("El diario no admite evidencia de herramientas".into())
    }
    fn append_response(
        &self,
        provider: &str,
        thread: &str,
        _turn: &str,
        _item: Option<&str>,
        text: &str,
    ) -> Result<()> {
        self.append(provider, thread, "assistant", text)
    }
    fn append_interrupted_response(
        &self,
        _provider: &str,
        _thread: &str,
        _turn: &str,
        _item: Option<&str>,
        _text: &str,
        _reason: ReplyInterruption,
    ) -> Result<()> {
        Err("El diario no admite respuestas interrumpidas".into())
    }
    fn finish_turn(&self, _provider: &str, _thread: &str, _turn: &str) -> Result<()> {
        Ok(())
    }
    /// Guarda entrada y cita juntas; rechaza referencias que no pueda comprobar.
    fn append_user(
        &self,
        provider: &str,
        thread_id: &str,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<Option<QuotedMessage>> {
        if reply_to.is_some() {
            return Err("El diario no admite citas".into());
        }
        self.append(provider, thread_id, "user", text)?;
        Ok(None)
    }

    /// Insertar entrada/cita y reservar avisos en una transacción del propietario.
    fn prepare_user(
        &self,
        provider: &str,
        thread: &str,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<context::PreparedUser> {
        Ok(context::PreparedUser {
            quote: self.append_user(provider, thread, text, reply_to)?,
            context: None,
            batch_id: None,
        })
    }
    fn confirm_context(&self, _provider: &str, _thread: &str, _batch: i64) -> Result<()> {
        Err("El diario no admite recibos de contexto".into())
    }
    fn uncertain_context(&self, _provider: &str, _thread: &str, _batch: i64) -> Result<()> {
        Err("El diario no admite recibos de contexto".into())
    }
    /// Solo reutilizar un chat visible, activo y sin mensajes persistidos.
    fn is_empty(&self, provider: &str, thread_id: &str) -> Result<bool> {
        let _ = (provider, thread_id);
        Ok(false)
    }
    /// Reserva una sola generación y devuelve el primer intercambio ya persistido.
    fn claim_title_input(
        &self,
        provider: &str,
        thread_id: &str,
    ) -> Result<Option<(String, String)>> {
        let _ = (provider, thread_id);
        Ok(None)
    }
    fn update_title(&self, provider: &str, thread_id: &str, title: &str) -> Result<()> {
        let _ = (provider, thread_id, title);
        Ok(())
    }
    /// Se emite después del commit de activación.
    fn committed(&self);
}

fn reuse_empty_chat(
    db: &gennie_db::Database,
    journal: Option<&dyn ConversationJournal>,
) -> Result<bool> {
    match (journal, codex::load_thread(db)?) {
        (Some(journal), Some(thread_id)) => journal.is_empty("codex", &thread_id),
        _ => Ok(false),
    }
}

#[cfg(test)]
type TestConnector = Arc<dyn Fn(codex::ProcessHandle) -> Result<codex::Session> + Send + Sync>;
pub struct LlmModule {
    invocation: Arc<Mutex<Option<String>>>,
    journal: Option<Arc<dyn ConversationJournal>>,
    tools: Mutex<Option<gennie_mcp::Launch>>,
    notice_guide: Mutex<Option<notices::guidance::GuideFactory>>,
    worker: Mutex<Option<Worker>>,
    #[cfg(test)]
    test_connector: Mutex<Option<TestConnector>>,
}
struct Worker {
    sender: SyncSender<Command>,
    events: Arc<Mutex<Receiver<Event>>>,
    pending: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    process: codex::ProcessHandle,
    turns: turn::Control,
    handle: JoinHandle<()>,
    delegations: delegation::Worker,
    settings: Arc<Mutex<Settings>>,
}
#[derive(Clone)]
pub struct Controller {
    delegations: delegation::Receiver,
    sender: SyncSender<Command>,
    events: Arc<Mutex<Receiver<Event>>>,
    pending: Arc<AtomicBool>,
    settings: Arc<Mutex<Settings>>,
    turns: turn::Control,
}
enum Command {
    Connect,
    Speak(String, Option<i64>, turn::Ticket),
    NewChat,
    SaveSettings(Settings),
    SaveArchitect(
        architects::Profile,
        mpsc::Sender<std::result::Result<architects::Profile, String>>,
    ),
    Shutdown,
}
#[derive(Debug)]
pub enum Event {
    Connected,
    Connecting,
    Disconnected(String),
    HistoryWarning(String),
    Speech { id: String, event: SpeechEvent },
    NewChatReady,
    Failed(String),
    SettingsSaved(Settings),
    SettingsSaveFailed(String),
}

#[derive(Debug)]
pub enum SpeechEvent {
    Connecting,
    Connected,
    UserSaved(String),
    Delta(String),
    Complete(String),
    Interrupted {
        text: String,
        saved: bool,
        reason: ReplyInterruption,
    },
    HistoryWarning(String),
    Finished {
        cancelled: bool,
        error: Option<String>,
    },
}

impl Default for LlmModule {
    fn default() -> Self {
        Self {
            invocation: Arc::default(),
            journal: None,
            tools: Mutex::new(None),
            notice_guide: Mutex::new(None),
            worker: Mutex::new(None),
            #[cfg(test)]
            test_connector: Mutex::new(None),
        }
    }
}
impl LlmModule {
    /// Composición previa al arranque; la guía se consulta solo para políticas consentidas.
    pub fn configure_notice_guide(&self, guide: notices::guidance::GuideFactory) -> Result<()> {
        if self
            .worker
            .lock()
            .map_err(|_| "Worker de conversación inválido")?
            .is_some()
        {
            return Err("Configura la guía antes de arrancar".into());
        }
        *self
            .notice_guide
            .lock()
            .map_err(|_| "Guía de avisos inválida")? = Some(guide);
        Ok(())
    }
    /// Hilo emisor del turno en curso; no sigue la selección de lectura de la UI.
    pub fn invocation_context(&self) -> Arc<Mutex<Option<String>>> {
        self.invocation.clone()
    }
    /// Composición antes de start; no se modifica una sesión en marcha.
    pub fn configure_tools(&self, tools: gennie_mcp::Launch) -> Result<()> {
        if self.worker.lock().map_err(|_| "Worker inválido")?.is_some() {
            return Err("Configura MCP antes de iniciar LLM".into());
        }
        *self
            .tools
            .lock()
            .map_err(|_| "Configuración MCP inválida")? = Some(tools);
        Ok(())
    }
    pub fn with_journal(journal: Arc<dyn ConversationJournal>) -> Self {
        Self {
            journal: Some(journal),
            ..Self::default()
        }
    }
}
impl AppModule for LlmModule {
    fn name(&self) -> &'static str {
        "llm"
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        // Los pendientes del bot solo se atienden después de habilitar el cliente
        // de guía si está activo; no confundir su arranque con indisponibilidad.
        &["proactivity"]
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
            Migration {
                version: 3,
                sql: include_str!("../migrations/0003_memory.sql"),
            },
            Migration {
                version: 4,
                sql: include_str!("../migrations/0004_remove_memory.sql"),
            },
            Migration {
                version: 5,
                sql: include_str!("../migrations/0005_architects.sql"),
            },
            Migration {
                version: 6,
                sql: include_str!("../migrations/0006_delegations.sql"),
            },
            Migration {
                version: 7,
                sql: include_str!("../migrations/0007_delegation_render_metrics.sql"),
            },
            Migration {
                version: 8,
                sql: include_str!("../migrations/0008_execution_notices.sql"),
            },
            Migration {
                version: 9,
                sql: include_str!("../migrations/0009_contextual_notices.sql"),
            },
            Migration {
                version: 10,
                sql: include_str!("../migrations/0010_expandable_architects.sql"),
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
        #[cfg(test)]
        let test_connector = self
            .test_connector
            .lock()
            .map_err(|_| "Fixture de sesión inválido")?
            .take();
        let process = codex::ProcessHandle::default();
        let turns = turn::Control::new(process.clone());
        let worker_turns = turns.clone();
        let db = state.db.clone();
        let journal = self.journal.clone();
        let tools = self
            .tools
            .lock()
            .map_err(|_| "Configuración MCP inválida")?
            .clone();
        architects::list(&db)?;
        let settings = Arc::new(Mutex::new(settings::load(&db)?));
        let invocation = self.invocation.clone();
        let worker_settings = settings.clone();
        let worker_pending = pending.clone();
        let worker_stopping = stopping.clone();
        let worker_process = process.clone();
        let delegations = delegation::Worker::start(
            db.clone(),
            journal.clone(),
            event_sender.clone(),
            self.notice_guide
                .lock()
                .map_err(|_| "Guía de avisos inválida")?
                .clone(),
        )?;
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
                            Ok(next) => {
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
                    if let Command::SaveArchitect(profile, reply) = command {
                        let bot = profile.id == architects::Role::Bot.id();
                        let result = architects::save(&db, &profile);
                        if result.is_ok() && bot {
                            // La fila confirmada ya está validada; no volver a leer después del commit.
                            let next = profile.settings();
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
                        worker_pending.store(false, Ordering::SeqCst);
                        let _ = reply.send(result.map_err(|e| e.to_string()));
                        continue;
                    }
                    let ticket = match &command {
                        Command::Speak(_, _, ticket) => Some(ticket.clone()),
                        _ => None,
                    };
                    let emit_connection = |connecting: bool| {
                        let event = if let Some(ticket) = &ticket {
                            Event::Speech {
                                id: ticket.id.clone(),
                                event: if connecting {
                                    SpeechEvent::Connecting
                                } else {
                                    SpeechEvent::Connected
                                },
                            }
                        } else if connecting {
                            Event::Connecting
                        } else {
                            Event::Connected
                        };
                        let _ = event_sender.send(event);
                    };
                    let current = worker_settings
                        .lock()
                        .expect("Ajustes de conversación inválidos")
                        .clone();
                    let result = (|| -> Result<Option<Event>> {
                        if let Some(ticket) = &ticket {
                            ticket.context.check()?;
                        }
                        if matches!(&command, Command::NewChat)
                            && reuse_empty_chat(&db, journal.as_deref())?
                        {
                            return Ok(Some(Event::NewChatReady));
                        }
                        if session.is_none() {
                            emit_connection(true);
                            let scripted = {
                                #[cfg(test)]
                                {
                                    test_connector
                                        .as_ref()
                                        .map(|connect| connect(worker_process.clone()))
                                        .transpose()?
                                }
                                #[cfg(not(test))]
                                {
                                    None::<codex::Session>
                                }
                            };
                            session = Some(if let Some(scripted) = scripted {
                                scripted
                            } else if matches!(&command, Command::NewChat) {
                                codex::Session::connect_recorded_for_turn(
                                    &db,
                                    worker_process.clone(),
                                    &current,
                                    true,
                                    journal.clone(),
                                    tools.clone(),
                                    ticket.clone(),
                                )?
                            } else {
                                codex::Session::connect_recorded_for_turn(
                                    &db,
                                    worker_process.clone(),
                                    &current,
                                    false,
                                    journal.clone(),
                                    tools.clone(),
                                    ticket.clone(),
                                )?
                            });
                            emit_connection(false);
                            if matches!(&command, Command::NewChat) {
                                return Ok(Some(Event::NewChatReady));
                            }
                        }
                        match command {
                            Command::Speak(text, reply_to, ticket) => {
                                session
                                    .as_mut()
                                    .expect("sesión creada")
                                    .bind_turn(Some(ticket.clone()));
                                *invocation
                                    .lock()
                                    .map_err(|_| "Contexto de turno inválido")? = Some(
                                    session
                                        .as_ref()
                                        .expect("sesión creada")
                                        .thread_id()
                                        .to_owned(),
                                );
                                session
                                    .as_mut()
                                    .expect("sesión creada")
                                    .speak_stream_reply(&text, reply_to, |event| {
                                        let event = match event {
                                            codex::StreamEvent::UserSaved(text) => {
                                                SpeechEvent::UserSaved(text)
                                            }
                                            codex::StreamEvent::Delta(text) => {
                                                SpeechEvent::Delta(text)
                                            }
                                            codex::StreamEvent::Completed(text) => {
                                                SpeechEvent::Complete(text)
                                            }
                                            codex::StreamEvent::Interrupted {
                                                text,
                                                saved,
                                                reason,
                                            } => SpeechEvent::Interrupted {
                                                text,
                                                saved,
                                                reason,
                                            },
                                            codex::StreamEvent::PersistenceWarning(text) => {
                                                SpeechEvent::HistoryWarning(text)
                                            }
                                        };
                                        let _ = event_sender.send(Event::Speech {
                                            id: ticket.id.clone(),
                                            event,
                                        });
                                    })?;
                                Ok(None)
                            }
                            Command::NewChat => {
                                session.as_mut().expect("sesión creada").new_chat(&db)?;
                                Ok(Some(Event::NewChatReady))
                            }
                            Command::Connect
                            | Command::Shutdown
                            | Command::SaveSettings(_)
                            | Command::SaveArchitect(..) => Ok(None),
                        }
                    })();
                    if let Ok(mut origin) = invocation.lock() {
                        *origin = None;
                    }
                    if let Some(ticket) = ticket {
                        let cancelled = ticket.context.is_cancelled();
                        let mut error = result.err().map(|e| e.to_string());
                        if !cancelled
                            && let Some(message) = &mut error
                            && let Some(current) = &session
                            && let Err(save_error) = current.record_error(message)
                        {
                            *message = format!(
                                "{message}. No se pudo guardar en el historial: {save_error}"
                            );
                        }
                        if cancelled || error.is_some() {
                            session = None;
                            worker_process.kill();
                        } else if let Some(current) = &mut session {
                            current.bind_turn(None);
                        }
                        worker_turns.finish(&ticket.id);
                        worker_pending.store(false, Ordering::SeqCst);
                        let _ = event_sender.send(Event::Speech {
                            id: ticket.id.clone(),
                            event: SpeechEvent::Finished {
                                cancelled,
                                error: if cancelled { None } else { error },
                            },
                        });
                        continue;
                    }
                    match result {
                        Ok(Some(event)) => {
                            let _ = event_sender.send(event);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            let mut message = error.to_string();
                            if let Some(current) = &session
                                && let Err(save_error) = current.record_error(&message)
                            {
                                message = format!(
                                    "{message}. No se pudo guardar en el historial: {save_error}"
                                );
                            }
                            session = None;
                            worker_process.kill();
                            let _ = event_sender.send(Event::Disconnected(message.clone()));
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
            turns,
            handle,
            delegations,
            settings,
        });
        Ok(())
    }
    fn stop(&self, _state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de conversación inválido")?;
        if let Some(mut worker) = guard.take() {
            let gate = worker.delegations.close_ingress();
            worker.stopping.store(true, Ordering::SeqCst);
            let _ = worker.sender.try_send(Command::Shutdown);
            worker.turns.cancel_current();
            worker.process.cancel();
            drop(worker.sender);
            let conversation = worker
                .handle
                .join()
                .map_err(|_| "Worker de conversación terminó inesperadamente");
            let deliveries = worker.delegations.stop();
            gate?;
            conversation?;
            deliveries?;
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
            delegations: worker.delegations.receiver.clone(),
            sender: worker.sender.clone(),
            events: worker.events.clone(),
            pending: worker.pending.clone(),
            settings: worker.settings.clone(),
            turns: worker.turns.clone(),
        })
    }
}
impl Controller {
    pub fn delegations(&self) -> delegation::Receiver {
        self.delegations.clone()
    }
    fn submit(&self, command: Command) -> Result<()> {
        if self.pending.swap(true, Ordering::SeqCst) {
            return Err("Gennie aún está respondiendo".into());
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
    /// Commit corto en el worker serial; nunca espera red ni consume inferencia.
    pub fn save_architect(&self, profile: architects::Profile) -> Result<architects::Profile> {
        profile.validate()?;
        let (reply, result) = mpsc::channel();
        self.submit(Command::SaveArchitect(profile, reply))?;
        result
            .recv()
            .map_err(|_| "El worker terminó antes de confirmar el arquitecto")?
            .map_err(Into::into)
    }
    pub fn speak(&self, text: &str) -> Result<()> {
        if text.trim().is_empty() || text.chars().count() > 1200 {
            return Err("Mensaje vacío o demasiado largo".into());
        }
        self.speak_turn(text, None).map(|_| ())
    }
    pub fn speak_reply(&self, text: &str, reply_to: i64) -> Result<()> {
        if text.trim().is_empty() || text.chars().count() > 1200 || reply_to <= 0 {
            return Err("Mensaje o referencia de respuesta inválidos".into());
        }
        self.speak_turn(text, Some(reply_to)).map(|_| ())
    }
    pub fn speak_turn(&self, text: &str, reply_to: Option<i64>) -> Result<String> {
        if text.trim().is_empty()
            || text.chars().count() > 1200
            || reply_to.is_some_and(|id| id <= 0)
        {
            return Err("Mensaje o referencia de respuesta inválidos".into());
        }
        if self.pending.swap(true, Ordering::SeqCst) {
            return Err("Gennie aún está respondiendo".into());
        }
        let ticket = match self.turns.begin() {
            Ok(t) => t,
            Err(e) => {
                self.pending.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };
        let id = ticket.id.clone();
        if self
            .sender
            .try_send(Command::Speak(text.into(), reply_to, ticket))
            .is_err()
        {
            self.turns.finish(&id);
            self.pending.store(false, Ordering::SeqCst);
            return Err("No se pudo enviar el mensaje".into());
        }
        Ok(id)
    }
    pub fn turn_progress(&self) -> Option<TurnProgress> {
        self.turns.progress()
    }
    pub fn cancel_turn(&self, id: &str) -> Result<bool> {
        self.turns.cancel(id)
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
    use gennie_config::Config;
    use gennie_core::Shutdown;
    use gennie_db::Database;
    use std::{path::PathBuf, sync::Arc};

    struct EmptyJournal;
    impl ConversationJournal for EmptyJournal {
        fn activate(
            &self,
            _: &gennie_db::SqliteConnection,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<()> {
            unreachable!()
        }
        fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
            unreachable!()
        }
        fn is_empty(&self, provider: &str, thread_id: &str) -> Result<bool> {
            assert_eq!(provider, "codex");
            assert_eq!(thread_id, "empty-thread");
            Ok(true)
        }
        fn committed(&self) {
            unreachable!()
        }
    }

    #[test]
    fn repeated_new_chat_reuses_persisted_empty_thread_without_connecting() {
        let module = LlmModule::default();
        let db = Database::memory().unwrap();
        db.migrate(module.name(), module.migrations()).unwrap();
        assert!(!reuse_empty_chat(&db, Some(&EmptyJournal)).unwrap());
        save_thread(&db, "empty-thread").unwrap();
        for _ in 0..10 {
            assert!(reuse_empty_chat(&db, Some(&EmptyJournal)).unwrap());
            assert_eq!(load_thread(&db).unwrap().as_deref(), Some("empty-thread"));
        }
        assert!(!reuse_empty_chat(&db, None).unwrap());
    }

    #[test]
    fn new_chat_worker_reuses_empty_thread_even_without_a_session() {
        let module = LlmModule::with_journal(Arc::new(EmptyJournal));
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
        save_thread(&state.db, "empty-thread").unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        // La cola real procesa cada pulsación: ninguna debe intentar conectar.
        let sender = module
            .worker
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .sender
            .clone();
        for _ in 0..10 {
            sender.send(Command::NewChat).unwrap();
            let event = controller
                .events
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap();
            assert!(matches!(event, Event::NewChatReady));
            assert_eq!(
                load_thread(&state.db).unwrap().as_deref(),
                Some("empty-thread")
            );
        }
        module.stop(&state).unwrap();
    }

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
            "gennie-llm-{}-{}.sqlite3",
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
            ..Default::default()
        };
        settings::save(&db, &next).unwrap();
        assert_eq!(settings::load(&db).unwrap(), next);
        assert_eq!(load_thread(&db).unwrap().as_deref(), Some("saved-thread"));
    }
    #[test]
    fn legacy_memories_are_removed_without_losing_the_conversation() {
        let path = std::env::temp_dir().join(format!(
            "gennie-llm-remove-memory-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let module = LlmModule::default();
        let db = Database::open(&path, 500).unwrap();
        db.migrate(module.name(), &module.migrations()[..3])
            .unwrap();
        save_thread(&db, "saved-thread").unwrap();
        db.with_connection(|connection| {
            connection.execute(
                "INSERT INTO llm_memory(text,source) VALUES(?1,?2)",
                ["Dato antiguo", "Chat"],
            )?;
            Ok(())
        })
        .unwrap();

        db.migrate(module.name(), module.migrations()).unwrap();
        assert_eq!(load_thread(&db).unwrap().as_deref(), Some("saved-thread"));
        assert_eq!(settings::load(&db).unwrap(), Settings::default());
        db.with_connection(|connection| {
            let count: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='llm_memory'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
        db.close().unwrap();

        let reopened = Database::open(&path, 500).unwrap();
        reopened
            .migrate(module.name(), module.migrations())
            .unwrap();
        assert_eq!(
            load_thread(&reopened).unwrap().as_deref(),
            Some("saved-thread")
        );
        reopened.close().unwrap();
        let _ = std::fs::remove_file(path);
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
        let cwd = std::env::temp_dir().join(format!("gennie-llm-probe-{}", std::process::id()));
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
    fn real_app_server_accepts_web_search_request() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let base = std::env::temp_dir().join(format!(
            "gennie-llm-web-probe-{}-{}",
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
    #[ignore = "crea dos hilos temporales y consume un turno real de la suscripción ChatGPT"]
    fn real_app_server_new_chat_persists_and_resumes() {
        let db = Database::memory().unwrap();
        let module = LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        save_thread(&db, "missing-legacy-thread").unwrap();
        let base = std::env::temp_dir().join(format!(
            "gennie-llm-new-chat-probe-{}-{}",
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

#[cfg(test)]
mod reply_input_tests {
    use super::*;
    #[test]
    fn queue_preserves_literal_spaces_and_newlines_for_ordinary_and_quoted_inputs() {
        for quoted in [false, true] {
            let (sender, commands) = mpsc::sync_channel(4);
            let (_, events) = mpsc::channel();
            let controller = Controller {
                delegations: delegation::Receiver::new(
                    gennie_db::Database::memory().unwrap(),
                    None,
                )
                .0,
                sender,
                events: Arc::new(Mutex::new(events)),
                pending: Arc::new(AtomicBool::new(false)),
                turns: turn::Control::new(Default::default()),
                settings: Arc::new(Mutex::new(Settings::default())),
            };
            let text = "  Texto literal\ncon salto  ";
            if quoted {
                controller.speak_reply(text, 42).unwrap();
            } else {
                controller.speak(text).unwrap();
            }
            match commands.recv().unwrap() {
                Command::Speak(saved, source, _) => {
                    assert_eq!(saved, text);
                    assert_eq!(source, if quoted { Some(42) } else { None });
                }
                _ => panic!("Se esperaba una entrada de conversación"),
            }
        }
    }
}
