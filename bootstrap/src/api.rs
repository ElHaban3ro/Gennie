//! Contrato del escritorio: datos propios, sin objetos UI ni conexiones SQL.
use crate::{
    frb_generated::StreamSink,
    runtime::{self, Operation},
};

#[derive(Clone, Debug, Default)]
pub struct AssistantSettings {
    pub model: String,
    pub effort: String,
    pub read_files: bool,
    pub write_files: bool,
    pub run_commands: bool,
    pub local_actions: bool,
}
#[derive(Clone, Debug, Default)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub voice: String,
    pub speed_percent: u32,
    pub unload_minutes: Option<u32>,
}
#[derive(Clone, Debug, Default)]
pub struct DictationSettings {
    pub enabled: bool,
    pub shortcut: String,
    pub hold: bool,
    pub unload_minutes: u32,
    pub microphone: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub updated_at: String,
    pub last_message: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct AssistantProfile {
    pub id: String,
    pub name: String,
    pub avatar_revision: Option<String>,
}
#[derive(Clone, Debug)]
pub struct ChatCursor {
    pub message_id: String,
    pub created_at: String,
}
#[derive(Clone, Debug)]
pub struct ReplyQuote {
    pub source_message_id: Option<String>,
    pub speaker: String,
    pub text: String,
    pub created_at: String,
    pub truncated: bool,
}
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub call_id: String,
    pub server: String,
    pub tool: String,
    pub plugin_id: Option<String>,
    pub function_name: Option<String>,
    pub display_name: Option<String>,
    pub operation: Option<String>,
    pub outcome: ToolCallOutcome,
    pub detail: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Copy, Debug)]
pub enum ToolCallOutcome {
    Running,
    Completed,
    Failed,
    Unknown,
}
#[derive(Clone, Debug)]
pub struct DelegationCall {
    pub job_id: String,
    pub source_sequence: String,
    pub plugin_id: String,
}
#[derive(Clone, Debug)]
pub struct ExecutionCall {
    pub state: String,
    pub assignment_id: String,
    pub instance_id: String,
    pub policy_id: String,
    pub policy_revision: u32,
    pub run_id: String,
}
#[derive(Clone, Debug)]
pub struct ChatEntry {
    pub id: String,
    pub conversation_id: String,
    pub session_started_at: String,
    pub starts_session: bool,
    pub quote: Option<ReplyQuote>,
    pub tools: Vec<ToolCall>,
    pub delegation: Option<DelegationCall>,
    pub execution: Option<ExecutionCall>,
    pub message: Message,
}
#[derive(Clone, Debug)]
pub struct ChatPage {
    pub messages: Vec<ChatEntry>,
    pub before: Option<ChatCursor>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageInterruption {
    Cancelled,
    Failed,
}
#[derive(Clone, Debug)]
pub struct TurnProgress {
    pub id: String,
    pub stopping: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Message {
    pub interruption: Option<MessageInterruption>,
    pub speaker: String,
    pub text: String,
    pub created_at: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalModel {
    Laya,
    Voice,
    Dictation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelState {
    Unavailable,
    Disabled,
    Unloaded,
    Downloading,
    Loading,
    Ready,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub model: LocalModel,
    pub state: ModelState,
    pub unload_minutes: Option<u32>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AssignmentRecord {
    /// Fecha persistida más cercana de una política activa y vigente; no garantiza ejecución.
    pub next_execution_at: Option<String>,
    pub instance_id: Option<String>,
    /// Resolución técnica actual; no forma parte del documento editable ni prueba ejecución.
    pub method_resolution: Option<String>,
    pub id: String,
    pub revision: u32,
    /// Documento JSON validado por el módulo assignments; el frontend solo lo presenta/edita.
    pub document: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub turn: Option<TurnProgress>,
    pub reply_interruption: Option<MessageInterruption>,
    pub execution_revision: Option<String>,
    pub assignments: Vec<AssignmentRecord>,
    pub timezone: String,
    pub assignments_enabled: bool,
    pub models: Vec<ModelInfo>,
    pub assistant_profile: Option<AssistantProfile>,
    pub history_revision: String,
    pub conversations: Vec<Conversation>,
    pub messages: Vec<Message>,
    pub selected_id: Option<String>,
    pub active_id: Option<String>,
    pub active_session_started_at: Option<String>,
    pub busy: bool,
    pub connection: String,
    pub reply: String,
    pub error: Option<String>,
    pub error_recorded: bool,
    pub confirmation: Option<String>,
    pub acknowledged_text: Option<String>,
    pub acknowledgement: u32,
    pub dictation_status: String,
    pub voice_status: String,
    pub assistant: Option<AssistantSettings>,
    pub dictation: Option<DictationSettings>,
    pub voice: Option<VoiceSettings>,
    pub reduced_motion: bool,
}

pub fn initialize(env_file: String, helper_path: String) -> Result<Snapshot, String> {
    runtime::start(env_file, helper_path)
}
pub fn watch(sink: StreamSink<Snapshot>) -> Result<(), String> {
    runtime::watch(sink)
}
pub fn select_conversation(id: Option<String>) -> Result<Snapshot, String> {
    runtime::request(Operation::Select(id))
}
/// Lectura acotada independiente de la selección y del contexto del proveedor.
pub fn load_chat_page(before: Option<ChatCursor>) -> Result<ChatPage, String> {
    runtime::read_chat_page(before)
}
pub fn rename_assistant(name: String) -> Result<Snapshot, String> {
    runtime::request(Operation::RenameAssistant(name))
}
/// None conserva la imagen; bytes vacíos restauran el pingüino. Guardado atómico.
pub fn save_assistant_profile(name: String, avatar: Option<Vec<u8>>) -> Result<Snapshot, String> {
    runtime::request(Operation::SaveAssistantProfile(name, avatar))
}
pub fn load_assistant_avatar(revision: String) -> Result<Vec<u8>, String> {
    runtime::read_assistant_avatar(revision)
}
/// Prepara una previsualización local sin modificar datos ni abrir archivos.
pub fn prepare_assistant_avatar(image: Vec<u8>) -> Result<Vec<u8>, String> {
    gennie_presence::avatar::normalize(&image).map_err(|e| e.to_string())
}
pub fn send_message(text: String) -> Result<Snapshot, String> {
    runtime::request(Operation::Send(text))
}
pub fn reply_to_message(text: String, source_message_id: String) -> Result<Snapshot, String> {
    runtime::request(Operation::SendReply(text, source_message_id))
}
/// Solicita detener únicamente esta identidad; el resultado llega por watch.
pub fn stop_response(turn_id: String) -> Result<(), String> {
    if turn_id.is_empty() || turn_id.len() > 200 {
        return Err("Identidad de respuesta inválida".into());
    }
    runtime::request(Operation::StopResponse(turn_id)).map(|_| ())
}
pub fn new_chat() -> Result<Snapshot, String> {
    runtime::request(Operation::NewChat)
}
pub fn connect() -> Result<Snapshot, String> {
    runtime::request(Operation::Connect)
}
pub fn open_browser() -> Result<Snapshot, String> {
    runtime::request(Operation::OpenBrowser)
}
pub fn rename_conversation(id: String, title: String) -> Result<Snapshot, String> {
    runtime::request(Operation::Rename(id, title))
}
pub fn delete_conversation(id: String) -> Result<Snapshot, String> {
    runtime::request(Operation::Delete(id))
}
pub fn save_assistant(settings: AssistantSettings) -> Result<Snapshot, String> {
    runtime::request(Operation::Assistant(settings))
}
pub fn save_voice(settings: VoiceSettings) -> Result<Snapshot, String> {
    runtime::request(Operation::Voice(settings))
}
pub fn preview_voice(settings: VoiceSettings) -> Result<Snapshot, String> {
    runtime::request(Operation::Preview(settings))
}
pub fn save_dictation(settings: DictationSettings) -> Result<Snapshot, String> {
    runtime::request(Operation::Dictation(settings))
}
pub fn save_reduced_motion(value: bool) -> Result<Snapshot, String> {
    runtime::request(Operation::Motion(value))
}
pub fn prepare_dictation() -> Result<Snapshot, String> {
    runtime::request(Operation::Prepare)
}
pub fn release_dictation() -> Result<Snapshot, String> {
    runtime::request(Operation::Release)
}
pub fn authorize_shortcut() -> Result<Snapshot, String> {
    runtime::request(Operation::Authorize)
}
pub fn microphones() -> Result<Vec<String>, String> {
    gennie_transcription::input_devices().map_err(|e| e.to_string())
}
pub fn shutdown() -> Result<(), String> {
    runtime::stop()
}

pub fn set_model_loaded(model: LocalModel, loaded: bool) -> Result<Snapshot, String> {
    runtime::request(Operation::Model(model, loaded))
}

/// RSS del proceso propio del modelo. Solo se consulta al abrir su tooltip.
pub fn model_memory_bytes(model: LocalModel) -> Result<Option<u64>, String> {
    runtime::model_memory(model)
}

/// Registra o sustituye un plan. No programa ejecuciones ni notificaciones.
pub fn save_assignment(
    id: Option<String>,
    expected_revision: Option<u32>,
    request_key: String,
    document: String,
) -> Result<Snapshot, String> {
    runtime::request(Operation::SaveAssignment(
        id,
        expected_revision,
        request_key,
        document,
    ))
}

/// Elimina el encargo únicamente si su revisión sigue vigente.
pub fn delete_assignment(id: String, expected_revision: u32) -> Result<Snapshot, String> {
    runtime::request(Operation::DeleteAssignment(id, expected_revision))
}

/// Selección humana y revisión de borradores; no ejecuta ni activa encargos.
pub fn assignment_proposal_request(request: String) -> Result<String, String> {
    runtime::proposal_request(request)
}

/// Configuración humana y consulta del diario de ejecuciones explícitas.
pub fn execution_request(request: String) -> Result<String, String> {
    runtime::execution_request(request)
}

/// Catálogo, constructor aislado y pruebas manuales; sin ejecución periódica.
pub fn plugin_request(request: String) -> Result<String, String> {
    runtime::plugin_request(request)
}

/// Configuración de instancias por rol; no inicia modelos ni trabajos.
pub fn architect_request(request: String) -> Result<String, String> {
    runtime::architect_request(request)
}
