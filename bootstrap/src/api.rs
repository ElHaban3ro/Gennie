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
pub struct Message {
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
    pub assignments: Vec<AssignmentRecord>,
    pub timezone: String,
    pub assignments_enabled: bool,
    pub models: Vec<ModelInfo>,
    pub conversations: Vec<Conversation>,
    pub messages: Vec<Message>,
    pub selected_id: Option<String>,
    pub active_id: Option<String>,
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
pub fn send_message(text: String) -> Result<Snapshot, String> {
    runtime::request(Operation::Send(text))
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

/// Catálogo, constructor aislado y pruebas manuales; sin ejecución periódica.
pub fn plugin_request(request: String) -> Result<String, String> {
    runtime::plugin_request(request)
}
