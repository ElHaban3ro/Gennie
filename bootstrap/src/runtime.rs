//! Un único coordinador posee los controladores y los detiene antes de cerrar SQLite.
use crate::{api::*, frb_generated::StreamSink, registry, startup};
use gennie_core::{AppModule, AppState, Result, Shutdown};
use gennie_db::Database;
use gennie_presence::{PresenceModule, assistants, history};
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
    StopResponse(String),
    Select(Option<String>),
    ReadChatPage(
        Option<ChatCursor>,
        Sender<std::result::Result<ChatPage, String>>,
    ),
    ArchitectRequest(String, Sender<std::result::Result<String, String>>),
    ProposalRequest(String, Sender<std::result::Result<String, String>>),
    RenameAssistant(String),
    SaveAssistantProfile(String, Option<Vec<u8>>),
    ReadAssistantAvatar(String, Sender<std::result::Result<Vec<u8>, String>>),
    SaveAssignment(Option<String>, Option<u32>, String, String),
    DeleteAssignment(String, u32),
    Send(String),
    SendReply(String, String),
    NewChat,
    Connect,
    OpenBrowser,
    Rename(String, String),
    Delete(String),
    Assistant(AssistantSettings),
    Voice(VoiceSettings),
    Preview(VoiceSettings),
    Dictation(DictationSettings),
    Motion(bool),
    Model(LocalModel, bool),
    Memory(LocalModel, Sender<Option<u64>>),
    Prepare,
    Release,
    Authorize,
    Stop,
}
struct Handle {
    sender: SyncSender<(Operation, Reply)>,
    thread: JoinHandle<()>,
}
static PLUGINS: Mutex<Option<gennie_plugins::Controller>> = Mutex::new(None);
static EXECUTIONS: Mutex<Option<gennie_assignments::execution::Controller>> = Mutex::new(None);
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
            if let Ok(mut slot) = PLUGINS.lock() {
                *slot = Some(backend.plugins.clone());
            }
            if let Ok(mut slot) = EXECUTIONS.lock() {
                *slot = backend.execution_control.clone();
            }
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
                    Ok((Operation::Memory(model, reply), _)) => {
                        let _ = reply.send(backend.model_memory(model));
                    }
                    Ok((Operation::ArchitectRequest(request, reply), _)) => {
                        let result = backend
                            .architect_request(&request)
                            .map_err(|e| e.to_string());
                        let _ = reply.send(result);
                    }
                    Ok((Operation::ProposalRequest(request, reply), _)) => {
                        let result = backend
                            .proposal_request(&request)
                            .map_err(|e| e.to_string());
                        let _ = reply.send(result);
                    }
                    Ok((Operation::ReadChatPage(before, reply), _)) => {
                        let _ = reply
                            .send(chat_page(&backend.state.db, before).map_err(|e| e.to_string()));
                    }
                    Ok((Operation::ReadAssistantAvatar(revision, reply), _)) => {
                        let result = revision
                            .parse::<i64>()
                            .map_err(|_| "Revisión de imagen inválida".into())
                            .and_then(|revision| {
                                gennie_presence::avatar::load(
                                    &backend.state.db,
                                    assistants::DEFAULT_ASSISTANT_ID,
                                    revision,
                                )
                            });
                        let _ = reply.send(result.map_err(|error| error.to_string()));
                    }
                    Ok((operation, reply)) => {
                        if !matches!(&operation, Operation::StopResponse(_)) {
                            backend.snapshot.error = None;
                            backend.snapshot.error_recorded = false;
                            backend.snapshot.confirmation = None;
                        }
                        let result = backend.apply(operation).and_then(|()| backend.refresh());
                        if let Err(e) = &result {
                            backend.record_error(e.to_string(), false, false);
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
pub(crate) fn read_chat_page(before: Option<ChatCursor>) -> std::result::Result<ChatPage, String> {
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    let (unused, _) = mpsc::channel();
    sender
        .try_send((Operation::ReadChatPage(before, reply), unused))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result.recv().map_err(|_| "El backend dejó de responder")?
}

pub(crate) fn read_assistant_avatar(revision: String) -> std::result::Result<Vec<u8>, String> {
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    let (unused, _) = mpsc::channel();
    sender
        .try_send((Operation::ReadAssistantAvatar(revision, reply), unused))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result.recv().map_err(|_| "El backend dejó de responder")?
}

fn message_interruption(value: gennie_llm::ReplyInterruption) -> MessageInterruption {
    match value {
        gennie_llm::ReplyInterruption::Cancelled => MessageInterruption::Cancelled,
        gennie_llm::ReplyInterruption::Failed => MessageInterruption::Failed,
    }
}

fn chat_page(db: &Database, before: Option<ChatCursor>) -> Result<ChatPage> {
    let cursor = before
        .map(|value| -> Result<assistants::Cursor> {
            Ok(assistants::Cursor {
                message_id: value.message_id.parse()?,
                created_at: value.created_at,
            })
        })
        .transpose()?;
    let page = assistants::page(
        db,
        assistants::DEFAULT_ASSISTANT_ID,
        cursor.as_ref(),
        60,
        24_000,
    )?;
    Ok(ChatPage {
        messages: page
            .messages
            .into_iter()
            .map(|entry| ChatEntry {
                id: entry.id.to_string(),
                execution: entry.execution.map(|e| ExecutionCall {
                    state: e.state,
                    assignment_id: e.assignment_id.to_string(),
                    instance_id: e.instance_id,
                    policy_id: e.policy_id,
                    policy_revision: e.policy_revision,
                    run_id: e.run_id.to_string(),
                }),
                delegation: entry.delegation.map(|d| DelegationCall {
                    job_id: d.job_id.to_string(),
                    source_sequence: d.source_sequence.to_string(),
                    plugin_id: d.plugin_id,
                }),
                tools: entry
                    .tools
                    .into_iter()
                    .map(|record| {
                        let usage = record.usage;
                        ToolCall {
                            call_id: usage.call_id,
                            server: usage.server,
                            tool: usage.tool,
                            plugin_id: usage.plugin_id,
                            function_name: usage.function_name,
                            display_name: usage.display_name,
                            operation: usage.operation,
                            outcome: match usage.outcome {
                                gennie_llm::ToolOutcome::Running => ToolCallOutcome::Running,
                                gennie_llm::ToolOutcome::Completed => ToolCallOutcome::Completed,
                                gennie_llm::ToolOutcome::Failed => ToolCallOutcome::Failed,
                                gennie_llm::ToolOutcome::Unknown => ToolCallOutcome::Unknown,
                            },
                            detail: usage.detail,
                            created_at: record.created_at,
                            updated_at: record.updated_at,
                        }
                    })
                    .collect(),
                conversation_id: entry.conversation_id.to_string(),
                session_started_at: entry.session_started_at,
                starts_session: entry.starts_session,
                quote: entry.quote.map(|q| ReplyQuote {
                    source_message_id: q.source_message_id.map(|id| id.to_string()),
                    speaker: q.speaker,
                    text: q.text,
                    created_at: q.created_at,
                    truncated: q.truncated,
                }),
                message: Message {
                    speaker: entry.speaker,
                    text: entry.text,
                    created_at: entry.created_at,
                    interruption: entry.interruption.map(message_interruption),
                },
            })
            .collect(),
        before: page.before.map(|cursor| ChatCursor {
            message_id: cursor.message_id.to_string(),
            created_at: cursor.created_at,
        }),
    })
}

pub(crate) fn proposal_request(request: String) -> std::result::Result<String, String> {
    if request.len() > 16 * 1024 {
        return Err("Petición de propuestas demasiado grande".into());
    }
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    let (unused, _) = mpsc::channel();
    sender
        .try_send((Operation::ProposalRequest(request, reply), unused))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result
        .recv()
        .map_err(|_| "El backend dejó de responder".to_owned())?
}

pub(crate) fn architect_request(request: String) -> std::result::Result<String, String> {
    if request.len() > 64 * 1024 {
        return Err("Petición de arquitectos demasiado grande".into());
    }
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    let (unused, _) = mpsc::channel();
    sender
        .try_send((Operation::ArchitectRequest(request, reply), unused))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result
        .recv()
        .map_err(|_| "El backend dejó de responder".to_string())?
}

pub(crate) fn model_memory(model: LocalModel) -> std::result::Result<Option<u64>, String> {
    let sender = HANDLE
        .lock()
        .map_err(|_| "Estado inválido")?
        .as_ref()
        .ok_or("El backend no está iniciado")?
        .sender
        .clone();
    let (reply, result) = mpsc::channel();
    let (unused, _) = mpsc::channel();
    sender
        .try_send((Operation::Memory(model, reply), unused))
        .map_err(|_| "El backend está ocupado o se está cerrando")?;
    result
        .recv()
        .map_err(|_| "El backend dejó de responder".into())
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
        *PLUGINS.lock().map_err(|_| "Estado de plugins inválido")? = None;
        *EXECUTIONS
            .lock()
            .map_err(|_| "Estado de ejecuciones inválido")? = None;
        SINKS.lock().map_err(|_| "Canal inválido")?.clear();
        *CURRENT.lock().map_err(|_| "Estado inválido")? = None;
        outcome?;
    }
    Ok(())
}

pub(crate) fn execution_request(request: String) -> std::result::Result<String, String> {
    if request.len() > 128 * 1024 {
        return Err("Petición de ejecución demasiado grande".into());
    }
    let controller = EXECUTIONS
        .lock()
        .map_err(|_| "Estado de ejecuciones inválido")?
        .clone()
        .ok_or("El ejecutor necesita activar Encargos y Plugins")?;
    let value = serde_json::from_str(&request).map_err(|_| "Petición JSON no válida")?;
    controller
        .request(value)
        .and_then(|v| Ok(serde_json::to_string(&v)?))
        .map_err(|e| e.to_string())
}

pub(crate) fn plugin_request(request: String) -> std::result::Result<String, String> {
    if request.len() > 128 * 1024 {
        return Err("Petición de plugins demasiado grande".into());
    }
    let controller = PLUGINS
        .lock()
        .map_err(|_| "Estado de plugins inválido")?
        .clone()
        .ok_or("Backend de plugins no iniciado")?;
    let request = serde_json::from_str(&request).map_err(|_| "Petición JSON no válida")?;
    controller
        .request(request)
        .and_then(|result| Ok(serde_json::to_string(&result)?))
        .map_err(|error| error.to_string())
}
/// Enlaza el commit con la preparación técnica sin convertir un fallo posterior en fallo de guardado.
fn proposal_tools(
    assignments: gennie_assignments::Store,
    db: Database,
    invocation: Arc<std::sync::Mutex<Option<String>>>,
    guide: gennie_assignments::proposals::GuideEvaluator,
) -> Vec<gennie_mcp::Tool> {
    let origin_db = db.clone();
    let turn_db = db;
    let turn_invocation = invocation.clone();
    gennie_assignments::proposals::tools(
        assignments,
        Arc::new(move || {
            let thread = invocation
                .lock()
                .map_err(|_| "Contexto emisor inválido")?
                .clone()
                .ok_or("No hay un bot emisor")?;
            let id = history::conversation_for_thread(&origin_db, &thread)?;
            let origin = gennie_presence::delegation::origin(&origin_db, id)?
                .ok_or("La sesión ya no está disponible")?;
            Ok(gennie_assignments::proposals::Origin {
                conversation_id: id,
                epoch: origin.epoch,
            })
        }),
        guide,
        Arc::new(move || {
            let thread = turn_invocation
                .lock()
                .map_err(|_| "Contexto emisor inválido")?
                .clone()
                .ok_or("No hay una intervención del bot")?;
            let turn = history::user_turn_for_thread(&turn_db, &thread)?;
            Ok(gennie_assignments::proposals::BotTurn {
                origin: gennie_assignments::proposals::Origin {
                    conversation_id: turn.conversation_id,
                    epoch: turn.epoch,
                },
                message_id: turn.message_id,
                text: turn.text,
                truncated: turn.truncated,
                created_at: turn.created_at,
            })
        }),
    )
}

fn assignment_tool_with_preparation(
    tool: &mut gennie_mcp::Tool,
    prepare: impl Fn(&serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
) {
    let previous = std::mem::replace(
        &mut tool.call,
        Box::new(|_| Err("Herramienta en preparación".into())),
    );
    tool.call = Box::new(move |args| {
        let mut response = previous(args)?;
        if response["assignment"].is_object() {
            response["plugin_preparation"] = match prepare(&response["assignment"]) {
                Ok(value) => value,
                Err(error) => {
                    serde_json::json!({"status":"not_started","message":error.to_string()})
                }
            };
        }
        Ok(response)
    });
}
struct PluginJournal {
    db: Database,
    invocation: Arc<Mutex<Option<String>>>,
    llm: Arc<gennie_llm::LlmModule>,
}
impl gennie_plugins::BuildJournal for PluginJournal {
    fn messages(
        &self,
        origin: &gennie_llm::delegation::Origin,
        job_id: i64,
        plugin_id: &str,
        legacy: bool,
        through_sequence: i64,
    ) -> Result<Option<Vec<gennie_llm::delegation::BotMessage>>> {
        gennie_presence::delegation::build_messages(
            &self.db,
            origin,
            job_id,
            plugin_id,
            legacy,
            through_sequence,
        )
    }
    fn origin(&self) -> Result<i64> {
        let thread=self.invocation.lock().map_err(|_|"Contexto emisor inválido")?.clone().ok_or("No hay un turno emisor para este trabajo. Selecciona un chat de origen desde Plugins")?;
        history::conversation_for_thread(&self.db, &thread)
    }
    fn exists(&self, id: i64) -> Result<bool> {
        history::conversation_exists(&self.db, id)
    }
    fn target(&self, id: i64) -> Result<gennie_llm::delegation::Origin> {
        self.llm
            .controller()?
            .delegations()
            .origin(id)?
            .ok_or_else(|| "El chat de origen ya no está disponible".into())
    }
    fn deliver(&self, update: &gennie_llm::delegation::Update) -> Result<()> {
        self.llm.controller()?.delegations().accept(update)
    }
}
fn configure_browser_permissions(
    browser: &gennie_browser::Controller,
    settings: Option<&gennie_llm::Settings>,
) {
    browser.set_file_access_allowed(settings.is_some_and(|s| s.read_files));
    browser.set_automation_allowed(settings.is_some_and(|s| s.local_actions && s.run_commands));
    browser.set_program_allowed(
        settings
            .is_some_and(|s| s.read_files && s.write_files && s.run_commands && s.local_actions),
    );
}

struct Backend {
    executions: Option<gennie_assignments::execution::Runner>,
    execution_control: Option<gennie_assignments::execution::Controller>,
    guide: gennie_proactivity::Controller,
    plugins: gennie_plugins::Controller,
    plugins_observed: u64,
    state: AppState,
    assignments: gennie_assignments::Store,
    assignments_observed: u64,
    assignments_enabled: bool,
    mcp: Option<gennie_mcp::Server>,
    browser: gennie_browser::Controller,
    modules: Vec<Arc<dyn AppModule>>,
    started: Vec<usize>,
    llm: Option<gennie_llm::Controller>,
    voice: Option<gennie_tts::Controller>,
    decisions: Option<gennie_decisions::Controller>,
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
        let secrets = gennie_config::Secrets::load(Path::new(env))?;
        let mut backend = Self::open_config_with_secrets(config, helper, secrets)?;
        // Adelantar conexión y descubrimiento al arranque, fuera del hilo Flutter.
        if let Some(controller) = &backend.llm {
            controller.connect()?;
            backend.snapshot.connection = "Conectando…".into();
            backend.refresh_controls();
        }
        Ok(backend)
    }
    #[cfg(test)]
    fn open_config(config: gennie_config::Config, helper: &str) -> Result<Self> {
        Self::open_config_with_secrets(config, helper, gennie_config::Secrets::default())
    }
    fn open_config_with_secrets(
        config: gennie_config::Config,
        helper: &str,
        secrets: gennie_config::Secrets,
    ) -> Result<Self> {
        gennie_transcription::configure_helper(helper.into())?;
        let db = Database::open(&config.database_path, config.db_busy_timeout_ms)?;
        let state = AppState {
            config: Arc::new(config),
            db,
            shutdown: Shutdown::default(),
        };

        let mut mcp = None;
        let presence = Arc::new(PresenceModule::default());
        let revision = presence.revision();
        let transcription = Arc::new(gennie_transcription::TranscriptionModule::default());
        let decisions = Arc::new(gennie_decisions::DecisionsModule::default());
        let llm = Arc::new(gennie_llm::LlmModule::with_journal(
            presence.conversation_journal(state.db.clone()),
        ));
        let tts = Arc::new(gennie_tts::TtsModule::default());
        let reminders = Arc::new(gennie_reminders_plugin::RemindersPluginModule::default());
        let browser_module =
            Arc::new(gennie_browser::BrowserModule::default().with_database(state.db.clone()));
        let browser = browser_module.controller();
        let assignments = gennie_assignments::Store::new(state.db.clone());
        let permissions_db = state.db.clone();
        let provider_browser = browser.clone();
        let helper_db = state.db.clone();
        let helper_llm = llm.clone();
        let plugins_module = Arc::new(gennie_plugins::PluginsModule::new(
            state.db.clone(),
            browser.clone(),
            Arc::new(PluginJournal {
                db: state.db.clone(),
                invocation: llm.invocation_context(),
                llm: llm.clone(),
            }),
            gennie_plugins::AgentSettings {
                user: Arc::new(move || {
                    // El transporte MCP manual no necesita conversación ni inferencia.
                    let settings = gennie_llm::architects::runtime_settings(
                        &permissions_db,
                        gennie_llm::architects::Role::Bot,
                    )?;
                    configure_browser_permissions(&provider_browser, Some(&settings));
                    Ok(settings)
                }),
                helper: Arc::new(move || {
                    // Un módulo LLM apagado tampoco puede iniciar constructores técnicos.
                    helper_llm.controller()?;
                    gennie_llm::architects::runtime_settings(
                        &helper_db,
                        gennie_llm::architects::Role::Helper,
                    )
                }),
            },
            Path::new(helper).to_path_buf(),
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
                .join("Library/Application Support/Gennie/plugins/builds"),
        ));
        let plugins = plugins_module.controller();
        let catalogue_browser = browser.clone();
        let catalogue_plugins = plugins.clone();
        let assignments=assignments.with_catalogue(Arc::new(move |ids| {
            let mut records=if ids.iter().any(|id|!id.starts_with("mcp:")) {
                match catalogue_browser.call(serde_json::json!({"method":"plugins","action":"list"})) {
                    Ok(result)=>result["plugins"].as_array().ok_or("Catálogo web inválido")?.clone(),
                    Err(_)=>vec![serde_json::json!({"kind":"web_unavailable"})],
                }
            }else{Vec::new()};
            for id in ids.iter().filter_map(|id|id.strip_prefix("mcp:")) {
                match catalogue_plugins.request(serde_json::json!({"action":"mcp_get","server_id":id})) {
                    Ok(result)=>records.push(serde_json::json!({"kind":"mcp","id":format!("mcp:{id}"),"server":result["server"]})),
                    Err(_)=>records.push(serde_json::json!({"kind":"mcp_unavailable"})),
                }
            }
            Ok(records)
        }));

        let mut modules = registry::build(
            presence,
            transcription.clone(),
            decisions.clone(),
            llm.clone(),
            tts.clone(),
            reminders,
            browser_module,
        );
        modules.push(plugins_module);
        let guide_db = state.db.clone();
        let proactivity = Arc::new(gennie_proactivity::ProactivityModule::with_guide(
            secrets.openrouter_api_key,
            Arc::new(move || {
                let profile =
                    gennie_llm::architects::load(&guide_db, gennie_llm::architects::Role::Guide)?;
                let model = match profile.model.as_str() {
                    "typesafe/jev-1.14" => gennie_proactivity::Model::Jev,
                    "openai/gpt-6-luna-decisions" => gennie_proactivity::Model::OpenaiDecisions,
                    _ => return Err("Modelo de guía no admitido".into()),
                };
                Ok(gennie_proactivity::GuideSettings {
                    revision: profile.revision,
                    enabled: profile.enabled,
                    model,
                    personality: profile.personality,
                    instructions: profile.instructions,
                })
            }),
        ));
        let guide = proactivity.controller();
        llm.configure_notice_guide(Arc::new(crate::executions::NoticeGuide {
            store: assignments.clone(),
            guide: guide.clone(),
        }))?;
        modules.push(proactivity);
        let setup = (|| {
            let order = registry::order(&modules)?;
            let active = startup::register(&state.db, &modules)?;
            registry::validate_active(&modules, &active)?;
            for i in &order {
                state
                    .db
                    .migrate(modules[*i].migration_namespace(), modules[*i].migrations())?;
            }
            let mut tools = gennie_mcp::Registry::default();
            if active.contains("assignments") {
                let scope_db = state.db.clone();
                let scope_invocation = llm.invocation_context();
                let mut assignment_tools = gennie_assignments::tools_with_origin(
                    assignments.clone(),
                    Arc::new(move || {
                        let thread = scope_invocation
                            .lock()
                            .map_err(|_| "Contexto emisor inválido")?
                            .clone();
                        let Some(thread) = thread else {
                            return Ok(None);
                        };
                        let id = history::conversation_for_thread(&scope_db, &thread)?;
                        let origin = gennie_presence::delegation::origin(&scope_db, id)?
                            .ok_or("La sesión ya no está disponible")?;
                        Ok(Some(gennie_assignments::proposals::Origin {
                            conversation_id: id,
                            epoch: origin.epoch,
                        }))
                    }),
                );
                for tool in &mut assignment_tools {
                    if matches!(
                        tool.definition["name"].as_str(),
                        Some("assignments_create" | "assignments_update")
                    ) {
                        let plugins = plugins.clone();
                        let available = active.contains("plugins")
                            && active.contains("browser")
                            && active.contains("llm");
                        assignment_tool_with_preparation(tool, move |assignment| {
                            if available {
                                plugins.prepare_assignment(assignment, None)
                            } else {
                                Ok(
                                    serde_json::json!({"status":"not_started","message":"El constructor de plugins no está disponible"}),
                                )
                            }
                        });
                    }
                }
                tools.register(assignment_tools)?;
                if active.contains("llm") {
                    let evaluator = guide.clone();
                    tools.register(proposal_tools(
                        assignments.clone(),
                        state.db.clone(),
                        llm.invocation_context(),
                        Arc::new(move |state, questions| evaluator.evaluate(state, questions)),
                    ))?;
                }
            }
            if active.contains("browser") {
                let mut browser_tools = gennie_browser::contextual_tools(browser.clone());
                if active.contains("plugins") {
                    for tool in &mut browser_tools {
                        if tool.definition["name"] == "browser_plugins" {
                            let previous = std::mem::replace(
                                &mut tool.call,
                                Box::new(|_, _| Err("Herramienta en preparación".into())),
                            );
                            let plugins = plugins.clone();
                            let schema = tool.definition["inputSchema"].clone();
                            tool.call = Box::new(move |args, context| {
                                if args["action"] == "delete" {
                                    gennie_browser::validate_arguments(&schema, &args)?;
                                    return plugins.request_with_context(args, context);
                                }
                                let response = previous(args.clone(), context)?;
                                plugins.catalogue_changed(&args, &response)?;
                                Ok(response)
                            });
                        }
                    }
                }
                tools.register(browser_tools)?;
            }
            if active.contains("plugins") && active.contains("llm") {
                let mut plugin_tools = gennie_plugins::contextual_tools(plugins.clone());
                if !active.contains("browser") {
                    // Las fuentes MCP no necesitan el navegador del constructor web.
                    plugin_tools.retain(|tool| {
                        matches!(
                            tool.definition["name"].as_str(),
                            Some("plugin_mcp_catalog" | "plugin_mcp_run")
                        )
                    });
                }
                tools.register(plugin_tools)?;
            }
            let server = gennie_mcp::Server::start(tools, Path::new(helper))?;
            llm.configure_tools(server.launch())?;
            mcp = Some(server);
            startup::start_all(&modules, &order, &active, &state).map(|started| (started, active))
        })();
        let (started, active) = match setup {
            Ok(v) => v,
            Err(e) => {
                if let Some(mut server) = mcp.take() {
                    let _ = server.stop();
                }
                let _ = state.db.close();
                return Err(e);
            }
        };
        let mut backend = Self {
            executions: None,
            execution_control: None,
            plugins,
            plugins_observed: 0,
            state,
            guide,
            assignments,
            assignments_observed: 0,
            assignments_enabled: active.contains("assignments"),
            mcp,
            browser,
            modules,
            started,
            llm: None,
            voice: None,
            decisions: None,
            dictation: None,
            snapshot: Snapshot {
                connection: "Sin conectar".into(),
                timezone: gennie_assignments::system_timezone(),
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
        if active.contains("decisions") {
            backend.decisions = Some(decisions.manual_controller()?);
        }
        backend.refresh()?;
        if active.contains("assignments")
            && active.contains("plugins")
            && active.contains("browser")
            && active.contains("llm")
        {
            let assignments = backend.assignments.clone();
            let db = backend.state.db.clone();
            backend.plugins.reconcile_assignments(Arc::new(move || {
                let mut records = Vec::new();
                let mut offset = 0;
                loop {
                    let page = assignments.list_unresolved(offset, 100)?;
                    let len = page.len();
                    for record in page {
                        records.push(serde_json::to_value(record)?);
                    }
                    if len < 100 {
                        break;
                    }
                    offset += 100;
                }
                let chat = history::load_recent(&db, None)?.active_id;
                Ok((records, chat))
            }))?;
        }
        if active.contains("assignments") && (active.contains("plugins") || active.contains("llm"))
        {
            let lock = backend
                .state
                .config
                .database_path
                .with_extension("executions.lock");
            backend.executions = Some(gennie_assignments::execution::Runner::start(
                backend.assignments.clone(),
                Arc::new(crate::executions::Host {
                    guide: Some(backend.guide.clone()),
                    plugins: backend.plugins.clone(),
                    execution_available: active.contains("plugins"),
                    bot: backend.llm.as_ref().map(|c| c.delegations()),
                    db: backend.state.db.clone(),
                }),
                &lock,
            )?);
            backend.execution_control = backend
                .executions
                .as_ref()
                .map(|runner| runner.controller());
        } else if active.contains("assignments") {
            backend.execution_control = Some(gennie_assignments::execution::Controller::view(
                backend.assignments.clone(),
                Arc::new(crate::executions::Host {
                    guide: Some(backend.guide.clone()),
                    plugins: backend.plugins.clone(),
                    execution_available: active.contains("plugins"),
                    bot: backend.llm.as_ref().map(|c| c.delegations()),
                    db: backend.state.db.clone(),
                }),
            ));
        }
        Ok(backend)
    }
    fn refresh(&mut self) -> Result<()> {
        let revision = self.revision.load(std::sync::atomic::Ordering::Acquire);
        let profile = assistants::load(&self.state.db, assistants::DEFAULT_ASSISTANT_ID)?;
        self.snapshot.assistant_profile = Some(AssistantProfile {
            id: profile.id,
            name: profile.name,
            avatar_revision: profile.avatar_revision.map(|v| v.to_string()),
        });
        self.snapshot.history_revision = revision.to_string();
        let selected = self
            .snapshot
            .selected_id
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()?;
        let history = history::load_recent(&self.state.db, selected)?;
        self.snapshot.conversations = history
            .conversations
            .into_iter()
            .map(|c| Conversation {
                id: c.id.to_string(),
                title: c.title,
                updated_at: c.updated_at,
                last_message: c.last_message,
            })
            .collect();
        self.snapshot.messages = history
            .selected_messages
            .into_iter()
            .map(|m| Message {
                speaker: m.speaker,
                text: m.text,
                created_at: m.created_at,
                interruption: m.interruption.map(message_interruption),
            })
            .collect();
        self.snapshot.active_session_started_at =
            history::session_started_at(&self.state.db, history.active_id)?;
        self.snapshot.active_id = history.active_id.map(|id| id.to_string());
        self.snapshot.selected_id = history.selected_id.map(|id| id.to_string());
        self.snapshot.assignments_enabled = self.assignments_enabled;
        let assignments_revision = self.assignments.revision();
        self.snapshot.execution_revision = Some(assignments_revision.to_string());
        let assignments = self.assignments.list(0, 100)?;
        let upcoming = self.assignments.upcoming_execution_times(
            &assignments
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
        )?;
        self.snapshot.assignments = assignments
            .into_iter()
            .map(|r| {
                let mut resolution = r
                    .method_resolution
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({}));
                resolution["sources"] = serde_json::to_value(
                    self.plugins
                        .assignment_sources(&serde_json::to_value(&r)?)?,
                )?;
                Ok(AssignmentRecord {
                    next_execution_at: upcoming.get(&r.id).cloned(),
                    id: r.id.to_string(),
                    instance_id: Some(r.instance_id.clone()),
                    revision: r.revision,
                    document: serde_json::to_string(&r.document)?,
                    method_resolution: if resolution.as_object().is_some_and(|v| !v.is_empty()) {
                        Some(serde_json::to_string(&resolution)?)
                    } else {
                        None
                    },
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.assignments_observed = assignments_revision;
        self.refresh_controls();
        self.snapshot.reduced_motion = gennie_presence::reduced_motion(&self.state.db)?;
        self.observed = revision;
        Ok(())
    }
    fn model_memory(&mut self, model: LocalModel) -> Option<u64> {
        match model {
            LocalModel::Laya => self.decisions.as_ref()?.resident_bytes(),
            LocalModel::Voice => self.voice.as_ref()?.resident_bytes(),
            LocalModel::Dictation => self.dictation.as_mut()?.resident_bytes(),
        }
    }
    fn model_info(&self) -> Vec<ModelInfo> {
        let laya = self.decisions.as_ref();
        let voice = self.voice.as_ref();
        let dictation = self.dictation.as_ref();
        vec![
            ModelInfo {
                model: LocalModel::Laya,
                state: laya.map_or(ModelState::Unavailable, |c| match c.model_status() {
                    gennie_decisions::ModelStatus::Unloaded => ModelState::Unloaded,
                    gennie_decisions::ModelStatus::Loading => ModelState::Loading,
                    gennie_decisions::ModelStatus::Ready => ModelState::Ready,
                    gennie_decisions::ModelStatus::Failed(_) => ModelState::Failed,
                }),
                unload_minutes: laya
                    .and_then(|c| c.settings().unload_seconds.map(|n| (n / 60) as u32)),
                detail: laya.and_then(|c| match c.model_status() {
                    gennie_decisions::ModelStatus::Failed(e) => Some(e),
                    _ => None,
                }),
            },
            ModelInfo {
                model: LocalModel::Voice,
                state: voice.map_or(ModelState::Unavailable, |c| match c.model_status() {
                    gennie_tts::ModelStatus::Disabled => ModelState::Disabled,
                    gennie_tts::ModelStatus::Unloaded => ModelState::Unloaded,
                    gennie_tts::ModelStatus::Loading => ModelState::Loading,
                    gennie_tts::ModelStatus::Ready => ModelState::Ready,
                    gennie_tts::ModelStatus::Failed => ModelState::Failed,
                }),
                unload_minutes: voice
                    .and_then(|c| c.settings().unload_seconds.map(|n| (n / 60) as u32)),
                detail: None,
            },
            ModelInfo {
                model: LocalModel::Dictation,
                state: dictation.map_or(ModelState::Unavailable, |c| match c.model_status() {
                    gennie_transcription::ModelStatus::Disabled => ModelState::Disabled,
                    gennie_transcription::ModelStatus::Unloaded => ModelState::Unloaded,
                    gennie_transcription::ModelStatus::Downloading => ModelState::Downloading,
                    gennie_transcription::ModelStatus::Loading => ModelState::Loading,
                    gennie_transcription::ModelStatus::Ready => ModelState::Ready,
                    gennie_transcription::ModelStatus::Failed => ModelState::Failed,
                }),
                unload_minutes: dictation.map(|c| (c.settings.unload_seconds / 60) as u32),
                detail: None,
            },
        ]
    }
    fn architect_request(&mut self, request: &str) -> Result<String> {
        use gennie_llm::architects::{self, Profile};
        #[derive(serde::Deserialize)]
        #[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
        enum Request {
            List,
            Get { id: String },
            Create { profile: Box<Profile> },
            Remove { id: String, revision: i64 },
            Save { profile: Box<Profile> },
        }
        let request: Request =
            serde_json::from_str(request).map_err(|_| "Petición de arquitectos no válida")?;
        let response = match request {
            Request::List => serde_json::json!({"profiles":architects::list(&self.state.db)?}),
            Request::Get { id } => {
                serde_json::json!({"profile":architects::load_by_id(&self.state.db, &id)?})
            }
            Request::Create { profile } => {
                serde_json::json!({"profile":architects::create(&self.state.db, &profile)?})
            }
            Request::Remove { id, revision } => {
                architects::remove(&self.state.db, &id, revision)?;
                serde_json::json!({"removed":id})
            }
            Request::Save { profile } if !profile.is_base() => {
                serde_json::json!({"profile":architects::save(&self.state.db, &profile)?})
            }
            Request::Save { profile } => {
                let llm = self.llm.as_ref().ok_or("Conversación desactivada")?;
                let saved = self
                    .plugins
                    .while_builder_idle(|| llm.save_architect(*profile))?;
                self.refresh_controls();
                serde_json::json!({"profile":saved})
            }
        };
        Ok(serde_json::to_string(&response)?)
    }
    fn refresh_controls(&mut self) {
        self.plugins.cancel_if_disallowed();
        let settings = self.llm.as_ref().map(|c| c.settings());
        configure_browser_permissions(&self.browser, settings.as_ref());
        self.snapshot.models = self.model_info();
        if let Some(progress) = self.llm.as_ref().and_then(|c| c.turn_progress()) {
            self.snapshot.turn = Some(TurnProgress {
                id: progress.id,
                stopping: progress.stopping,
            });
        }
        self.snapshot.busy =
            self.snapshot.turn.is_some() || self.llm.as_ref().is_some_and(|c| c.is_pending());
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
    fn proposal_request(&mut self, request: &str) -> Result<String> {
        #[derive(serde::Deserialize)]
        #[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
        enum Request {
            Availability,
            Get {
                review_id: String,
            },
            List {
                assignment_id: i64,
                #[serde(default)]
                archived: bool,
                before: Option<gennie_assignments::proposals::history::Cursor>,
            },
            Begin {
                request_key: String,
                assignments: Vec<gennie_assignments::proposals::Reference>,
                instructions: String,
                use_guide: bool,
            },
            Resolve {
                proposal_id: i64,
                apply: bool,
            },
            Cancel {
                review_id: String,
            },
            Archive {
                review_id: String,
            },
        }
        if !self.assignments_enabled {
            return Err("Encargos está desactivado".into());
        }
        let mut result = match serde_json::from_str::<Request>(request)? {
            Request::Availability => {
                serde_json::json!({"guide_available":self.guide.available()?,"bot_available":self.llm.is_some()})
            }
            Request::List {
                assignment_id,
                archived,
                before,
            } => self
                .assignments
                .reviews_page(assignment_id, archived, before.as_ref())?,
            Request::Archive { review_id } => self.assignments.archive_review(&review_id)?,
            Request::Get { review_id } => self.assignments.review(&review_id)?,
            Request::Begin {
                request_key,
                assignments,
                instructions,
                use_guide,
            } => {
                let bot = self.llm.as_ref().ok_or("Conversación desactivada")?;
                if bot.is_pending() {
                    return Err("Espera a que termine la respuesta de Gennie".into());
                }
                if use_guide && !self.guide.available()? {
                    return Err(
                        "Configura y habilita la guía remota antes de solicitar su valoración"
                            .into(),
                    );
                }
                let id = history::load_recent(&self.state.db, None)?
                    .active_id
                    .ok_or("Inicia una conversación con Gennie antes de solicitar propuestas")?;
                let origin = gennie_presence::delegation::origin(&self.state.db, id)?
                    .ok_or("La conversación ya no está disponible")?;
                self.assignments
                    .begin_review(gennie_assignments::proposals::Begin {
                        request_key,
                        assignments,
                        origin: gennie_assignments::proposals::Origin {
                            conversation_id: id,
                            epoch: origin.epoch,
                        },
                        instructions,
                        use_guide,
                    })?
            }
            Request::Cancel { review_id } => self.assignments.cancel_review(&review_id)?,
            Request::Resolve { proposal_id, apply } => {
                let result = self.assignments.resolve_proposal(proposal_id, apply)?;
                self.refresh()?;
                self.publish();
                result
            }
        };
        if result.get("scope").is_some() || result.get("reviews").is_some() {
            let active = history::active_conversation(&self.state.db)?;
            let current = active
                .map(|id| gennie_presence::delegation::origin(&self.state.db, id))
                .transpose()?
                .flatten();
            let decorate = |review: &mut serde_json::Value| {
                if review.get("scope").is_some() {
                    let same = current.as_ref().is_some_and(|origin| {
                        review["scope"]["origin"]["conversation_id"] == origin.conversation_id
                            && review["scope"]["origin"]["epoch"] == origin.epoch
                    });
                    review["origin_current"] = serde_json::json!(same);
                    review["bot_available"] = serde_json::json!(self.llm.is_some());
                }
            };
            if let Some(reviews) = result["reviews"].as_array_mut() {
                for review in reviews {
                    decorate(review);
                }
            } else {
                decorate(&mut result);
            }
        }
        Ok(serde_json::to_string(&result)?)
    }
    fn submit_message(&mut self, text: String, reply_to: Option<i64>) -> Result<()> {
        if text.trim().is_empty() {
            return Err("Escribe un mensaje".into());
        }
        if text.len() > 64 * 1024 {
            return Err("El mensaje es demasiado largo".into());
        }
        let controller = self.llm.as_ref().ok_or("Conversación desactivada")?;
        if self.snapshot.turn.is_some() {
            return Err("Gennie aún está terminando la respuesta".into());
        }
        let id = controller.speak_turn(&text, reply_to)?;
        self.snapshot.turn = Some(TurnProgress {
            id,
            stopping: false,
        });
        self.snapshot.reply_interruption = None;
        self.snapshot.acknowledged_text = None;
        self.snapshot.reply.clear();
        self.last_completed = None;
        self.failed_turn = false;
        Ok(())
    }
    fn apply(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::StopResponse(id) => {
                if self
                    .llm
                    .as_ref()
                    .map(|c| c.cancel_turn(&id))
                    .transpose()?
                    .unwrap_or(false)
                {
                    self.snapshot.error = None;
                    self.snapshot.error_recorded = false;
                    self.snapshot.confirmation = None;
                    if let Some(progress) = self.snapshot.turn.as_mut().filter(|t| t.id == id) {
                        progress.stopping = true;
                    }
                    self.speech_queue.clear();
                    self.narration.clear();
                    if let Some(voice) = &self.voice {
                        voice.interrupt()?;
                    }
                }
            }
            Operation::SaveAssignment(id, revision, key, document) => {
                if !self.assignments_enabled {
                    return Err("El módulo de encargos está desactivado".into());
                }
                let document = serde_json::from_str(&document)?;
                let record = match (id, revision) {
                    (None, None) => self.assignments.create(&key, document)?,
                    (Some(id), Some(revision)) => {
                        self.assignments.update(id.parse()?, revision, document)?
                    }
                    _ => return Err("Edición de encargo incompleta".into()),
                };
                let has_web = record
                    .document
                    .applications
                    .iter()
                    .any(|a| a.kind == gennie_assignments::AppKind::Web);
                let confirmation = if has_web {
                    let chat = self
                        .snapshot
                        .selected_id
                        .as_deref()
                        .and_then(|s| s.parse::<i64>().ok());
                    match chat {
                        None => "Encargo guardado. Selecciona un chat para delegar la preparación del plugin.".to_owned(),
                        Some(chat) => match self.plugins.prepare_assignment(&serde_json::to_value(record)?, Some(chat)) {
                            Ok(result) if result["sources"].as_array().is_some_and(|s| s.iter().all(|v| v["status"] == "started")) => "Encargo guardado. Delegué la preparación del plugin; sus avances llegarán al chat seleccionado.".to_owned(),
                            Ok(_) => "Encargo guardado. La preparación del plugin necesita atención; revisa Plugins.".to_owned(),
                            Err(e) => format!("Encargo guardado. No pude iniciar el plugin: {e}"),
                        },
                    }
                } else {
                    "Encargo guardado.".to_owned()
                };
                self.snapshot.confirmation = Some(confirmation);
            }
            Operation::DeleteAssignment(id, revision) => {
                if !self.assignments_enabled {
                    return Err("El módulo de encargos está desactivado".into());
                }
                self.assignments.delete(id.parse()?, revision)?;
            }
            Operation::Select(id) => {
                if self.snapshot.selected_id != id {
                    self.snapshot.reply.clear();
                }
                self.snapshot.selected_id = id;
            }
            Operation::Send(text) => self.submit_message(text, None)?,
            Operation::SendReply(text, id) => self.submit_message(text, Some(id.parse()?))?,
            Operation::NewChat => {
                if self.snapshot.turn.is_some() {
                    return Err("Gennie aún está terminando la respuesta".into());
                }
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
            Operation::OpenBrowser => self.browser.open()?,
            Operation::RenameAssistant(name) => {
                assistants::rename(&self.state.db, assistants::DEFAULT_ASSISTANT_ID, &name)?;
                self.snapshot.confirmation = Some("Nombre del asistente guardado".into());
            }
            Operation::SaveAssistantProfile(name, avatar) => {
                gennie_presence::avatar::save_profile(
                    &self.state.db,
                    assistants::DEFAULT_ASSISTANT_ID,
                    &name,
                    avatar.as_deref(),
                )?;
                self.snapshot.confirmation = Some("Perfil del asistente guardado".into());
            }
            Operation::Rename(id, title) => {
                history::rename(&self.state.db, id.parse()?, &title)?;
                self.snapshot.confirmation = Some("Nombre guardado".into());
            }
            Operation::Delete(id) => {
                history::delete(&self.state.db, id.parse()?)?;
                self.revision
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
                self.snapshot.confirmation = Some("Chat eliminado de Gennie".into());
            }
            Operation::Assistant(s) => {
                let llm = self.llm.as_ref().ok_or("Conversación desactivada")?;
                self.plugins.while_builder_idle(|| {
                    let mut bot = gennie_llm::architects::load(
                        &self.state.db,
                        gennie_llm::architects::Role::Bot,
                    )?;
                    bot.model = s.model;
                    bot.effort = Some(s.effort);
                    bot.read_files = s.read_files;
                    bot.write_files = s.write_files;
                    bot.run_commands = s.run_commands;
                    bot.local_actions = s.local_actions;
                    llm.save_architect(bot).map(|_| ())
                })?;
            }
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
            Operation::Model(model, loaded) => match model {
                LocalModel::Laya => {
                    let c = self.decisions.as_ref().ok_or("Laya no disponible")?;
                    if loaded {
                        c.load_model()?;
                    } else {
                        c.unload_model();
                    }
                }
                LocalModel::Voice => {
                    let c = self.voice.as_ref().ok_or("Voz no disponible")?;
                    if loaded {
                        if !c.settings().enabled {
                            return Err("Activa la voz en Configuración".into());
                        }
                        c.preload()?;
                    } else {
                        self.speech_queue.clear();
                        self.narration.clear();
                        c.interrupt()?;
                    }
                }
                LocalModel::Dictation => {
                    let c = self.dictation.as_mut().ok_or("Dictado no disponible")?;
                    if loaded {
                        if !c.settings.enabled {
                            return Err("Activa el dictado en Configuración".into());
                        }
                        c.prepare();
                    } else {
                        c.cancel();
                    }
                }
            },
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
            Operation::Stop
            | Operation::Memory(..)
            | Operation::ReadChatPage(..)
            | Operation::ProposalRequest(..)
            | Operation::ArchitectRequest(..)
            | Operation::ReadAssistantAvatar(..) => unreachable!(),
        }
        Ok(())
    }
    fn record_error(&mut self, text: String, provider_error: bool, active: bool) {
        self.snapshot.error = Some(text.clone());
        self.snapshot.error_recorded = false;
        let target = if active {
            history::load_recent(&self.state.db, None)
                .ok()
                .and_then(|h| h.active_id)
                .or_else(|| self.snapshot.selected_id.as_deref()?.parse().ok())
        } else {
            self.snapshot
                .selected_id
                .as_deref()
                .and_then(|id| id.parse().ok())
        };
        match history::record_system_message(
            &self.state.db,
            target,
            &format!("Error: {text}"),
            provider_error,
        ) {
            Ok(true) => {
                self.revision
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
                match self.refresh() {
                    Ok(()) => self.snapshot.error_recorded = true,
                    Err(error) => {
                        self.snapshot.error = Some(format!(
                            "{text}. No se pudo leer el aviso guardado: {error}"
                        ))
                    }
                }
            }
            Ok(false) => {}
            Err(error) => {
                self.snapshot.error = Some(format!("{text}. No se pudo guardar el aviso: {error}"))
            }
        }
    }
    fn observe_speech(
        &mut self,
        id: &str,
        event: gennie_llm::SpeechEvent,
        errors: &mut Vec<(String, bool)>,
    ) -> bool {
        if self.snapshot.turn.as_ref().is_none_or(|t| t.id != id) {
            return false;
        }
        let mut refresh = false;
        use gennie_llm::SpeechEvent;
        match event {
            SpeechEvent::Connecting => self.snapshot.connection = "Conectando…".into(),
            SpeechEvent::Connected => self.snapshot.connection = "Conectado".into(),
            SpeechEvent::Delta(text) => {
                self.snapshot.reply.push_str(&text);
                self.speech_queue.extend(self.narration.push(&text));
            }
            SpeechEvent::Complete(text) => {
                self.last_completed = Some(text.clone());
                if self.snapshot.reply.is_empty() {
                    self.speech_queue.push_back(text);
                } else if let Some(last) = self.narration.finish() {
                    self.speech_queue.push_back(last);
                }
                self.snapshot.reply.clear();
                self.snapshot.reply_interruption = None;
            }
            SpeechEvent::UserSaved(text) => {
                self.snapshot.acknowledged_text = Some(text);
                self.snapshot.acknowledgement = self.snapshot.acknowledgement.wrapping_add(1);
            }
            SpeechEvent::Interrupted {
                text,
                saved,
                reason,
            } => {
                self.speech_queue.clear();
                self.narration.clear();
                if let Some(voice) = &self.voice {
                    let _ = voice.interrupt();
                }
                self.last_completed = None;
                self.failed_turn = false;
                if saved {
                    self.snapshot.reply.clear();
                    self.snapshot.reply_interruption = None;
                } else {
                    self.snapshot.reply = text;
                    self.snapshot.reply_interruption = Some(message_interruption(reason));
                    errors.push((
                        "No se pudo guardar la respuesta interrumpida; el texto sigue visible"
                            .into(),
                        false,
                    ));
                }
                refresh = true;
            }
            SpeechEvent::HistoryWarning(text) => errors.push((text, false)),
            SpeechEvent::Finished { cancelled, error } => {
                self.snapshot.turn = None;
                refresh = true;
                if cancelled {
                    self.snapshot.connection = "Sin conectar".into();
                    self.snapshot.confirmation = Some("Respuesta detenida".into());
                    self.speech_queue.clear();
                    self.narration.clear();
                    if let Some(voice) = &self.voice {
                        let _ = voice.interrupt();
                    }
                } else if let Some(error) = error {
                    self.snapshot.connection = "Sin conectar".into();
                    self.failed_turn = true;
                    errors.push((error, true));
                }
            }
        }
        refresh
    }
    fn tick(&mut self) {
        let mut dirty = self.snapshot.models != self.model_info();
        let mut refresh_history = false;
        let mut errors = Vec::new();
        let mut dictated = Vec::new();
        if let Some(c) = &mut self.dictation {
            for event in c.tick() {
                use gennie_transcription::Event::*;
                match event {
                    Text {
                        text,
                        finalized: true,
                    } => {
                        dictated.push(text);
                    }
                    DictationRequested => {
                        self.speech_queue.clear();
                        self.narration.clear();
                        if let Some(v) = &self.voice {
                            let _ = v.interrupt();
                            let _ = v.preload();
                        }
                    }
                    Error(e) | NoSpeech(e) => errors.push((e, false)),
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
        for text in dictated {
            if let Err(error) = self.submit_message(text, None) {
                errors.push((error.to_string(), false));
            }
        }
        if let Some(c) = &self.llm {
            let events = c.poll();
            let pending = c.is_pending();
            for event in events {
                use gennie_llm::Event::*;
                match event {
                    Connected => self.snapshot.connection = "Conectado".into(),
                    Connecting => self.snapshot.connection = "Conectando…".into(),
                    Disconnected(_) => {
                        self.snapshot.connection = "Sin conectar".into();
                    }
                    Speech { id, event } => {
                        refresh_history |= self.observe_speech(&id, event, &mut errors);
                    }
                    NewChatReady => {
                        self.snapshot.selected_id = None;
                        self.snapshot.confirmation = Some("Nuevo chat preparado".into());
                        refresh_history = true;
                    }
                    SettingsSaved(_) => {
                        self.snapshot.confirmation = Some("Ajustes del asistente guardados".into())
                    }
                    Failed(e) => {
                        errors.push((e, true));
                        self.failed_turn = true;
                    }
                    HistoryWarning(e) | SettingsSaveFailed(e) => {
                        errors.push((e, false));
                    }
                }
                dirty = true;
            }
            if (pending || self.snapshot.turn.is_some()) != self.snapshot.busy {
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
                        errors.push((e, false));
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
        for (error, provider_error) in errors {
            self.record_error(error, provider_error, true);
        }
        let history_changed =
            self.observed != self.revision.load(std::sync::atomic::Ordering::Acquire);
        let assignments_changed = self.assignments_observed != self.assignments.revision();
        let plugins_changed = self.plugins_observed != self.plugins.revision();
        self.plugins_observed = self.plugins.revision();
        dirty |= history_changed || assignments_changed || plugins_changed;
        if dirty {
            if history_changed || assignments_changed || refresh_history || plugins_changed {
                if let Err(e) = self.refresh() {
                    self.snapshot.error = Some(e.to_string());
                    self.snapshot.error_recorded = false;
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
        if let Some(control) = &self.execution_control {
            control.request_stop();
        }
        if let Some(mut c) = self.dictation.take() {
            c.cancel();
        }
        self.llm.take();
        self.voice.take();
        self.decisions.take();
        let result = startup::stop_all(&self.modules, &self.started, &self.state);
        let mcp_result = self.mcp.take().map(|mut mcp| mcp.stop()).unwrap_or(Ok(()));
        let ingress = self
            .execution_control
            .take()
            .map(|c| c.close_ingress())
            .unwrap_or(Ok(()));
        let executions = self
            .executions
            .take()
            .map(|mut r| r.join())
            .unwrap_or(Ok(()));
        ingress
            .and(result)
            .and(executions)
            .and(mcp_result)
            .and(self.state.db.close())
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
