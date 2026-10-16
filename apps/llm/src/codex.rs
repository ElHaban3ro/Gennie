use crate::Settings;
use serde_json::{Value, json};
use soc_core::Result;
use soc_db::Database;
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const TURN_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_REPLY_BYTES: usize = 32_000;
const VOICE_PROFILE: &str = "soc_voice";
const VOICE_INSTRUCTIONS: &str = "Eres SOC, un asistente conversacional en español. Recibes transcripciones de voz que pueden contener errores: si el tema no está claro, pide una aclaración breve antes de asumirlo. Responde directamente y con brevedad para un globo de voz. Usa la búsqueda web cuando el usuario pida buscar o cuando necesites información actual; cita las fuentes al final con títulos cortos y enlaces Markdown. No afirmes que Internet no está disponible sin intentar la búsqueda web. Respeta los permisos configurados por el usuario en SOC. Los plugins y apps conectadas no están disponibles en esta conversación.";

fn turn_start_request(thread_id: &str, cwd: &Path, model: &str, effort: &str, text: &str) -> Value {
    json!({"jsonrpc":"2.0","id":4,"method":"turn/start","params":{
        "threadId":thread_id,
        "input":[{"type":"text","text":text}],
        "cwd":cwd,
        "approvalPolicy":"never",
        "permissions":VOICE_PROFILE,
        "model":model,
        "effort":effort
    }})
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StreamEvent {
    Delta(String),
    Completed(String),
}

#[derive(Default)]
struct MessageCollector {
    current_id: Option<String>,
    current_text: String,
    completed: Vec<String>,
    total_bytes: usize,
}

impl MessageCollector {
    fn accept(&mut self, event: &Value, turn_id: &str) -> Result<Option<StreamEvent>> {
        if event.pointer("/params/turnId").and_then(Value::as_str) != Some(turn_id) {
            return Ok(None);
        }
        let method = event.get("method").and_then(Value::as_str);
        if method == Some("item/agentMessage/delta") {
            let item_id = event.pointer("/params/itemId").and_then(Value::as_str);
            if self.current_id.as_deref() != item_id && !self.current_text.is_empty() {
                return Err("El proveedor entrelazó mensajes de respuesta".into());
            }
            self.current_id = item_id.map(str::to_owned);
            if let Some(delta) = event.pointer("/params/delta").and_then(Value::as_str) {
                if self.total_bytes + self.current_text.len() + delta.len() > MAX_REPLY_BYTES {
                    return Err("La respuesta excedió el límite".into());
                }
                self.current_text.push_str(delta);
                if !delta.is_empty() {
                    return Ok(Some(StreamEvent::Delta(delta.to_owned())));
                }
            }
        } else if method == Some("item/completed")
            && event.pointer("/params/item/type").and_then(Value::as_str) == Some("agentMessage")
        {
            let text = if self.current_text.is_empty() {
                event
                    .pointer("/params/item/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            } else {
                std::mem::take(&mut self.current_text)
            };
            self.current_id = None;
            if !text.trim().is_empty() {
                if self.total_bytes + text.len() > MAX_REPLY_BYTES {
                    return Err("La respuesta excedió el límite".into());
                }
                self.total_bytes += text.len();
                self.completed.push(text.clone());
                return Ok(Some(StreamEvent::Completed(text)));
            }
        }
        Ok(None)
    }

    fn finish(&mut self) -> Option<StreamEvent> {
        self.current_id = None;
        let text = std::mem::take(&mut self.current_text);
        if text.trim().is_empty() {
            None
        } else {
            self.total_bytes += text.len();
            self.completed.push(text.clone());
            Some(StreamEvent::Completed(text))
        }
    }
}

pub(crate) fn load_thread(db: &Database) -> Result<Option<String>> {
    db.with_connection(|connection| {
        let mut statement =
            connection.prepare("SELECT thread_id FROM codex_conversation WHERE id=1")?;
        let mut rows = statement.query([])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    })
}

pub(crate) fn save_thread(db: &Database, thread_id: &str) -> Result<()> {
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        tx.execute("INSERT INTO codex_conversation(id,thread_id) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET thread_id=excluded.thread_id,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')", [thread_id])?;
        tx.commit()?;
        Ok(())
    })
}

#[derive(Clone, Default)]
pub struct ProcessHandle(Arc<Mutex<Option<Child>>>);
impl ProcessHandle {
    pub fn kill(&self) {
        if let Ok(mut guard) = self.0.lock()
            && let Some(mut child) = guard.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct Session {
    stdin: ChildStdin,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    process: ProcessHandle,
    thread_id: String,
    cwd: PathBuf,
    model: String,
    effort: String,
    settings: Settings,
}

impl Session {
    pub fn connect(db: &Database, process: ProcessHandle, settings: &Settings) -> Result<Self> {
        Self::connect_in(db, process, soc_directory()?, settings)
    }
    pub fn connect_new(db: &Database, process: ProcessHandle, settings: &Settings) -> Result<Self> {
        Self::connect_in_with_mode(db, process, soc_directory()?, true, settings)
    }
    #[cfg(test)]
    pub(crate) fn connect_new_in(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
    ) -> Result<Self> {
        Self::connect_in_with_mode(db, process, base, true, settings)
    }
    pub(crate) fn connect_in(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
    ) -> Result<Self> {
        Self::connect_in_with_mode(db, process, base, false, settings)
    }
    fn connect_in_with_mode(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        new_chat: bool,
        settings: &Settings,
    ) -> Result<Self> {
        settings.validate()?;
        let cwd = base.join("workspace");
        std::fs::create_dir_all(&cwd)?;
        let codex_home = prepare_codex_home(&base)?;
        let binary = codex_binary()
            .canonicalize()
            .unwrap_or_else(|_| codex_binary());
        let binary_rule = serde_json::to_string(&binary.to_string_lossy().to_string())?;
        let filesystem = filesystem_config(settings, &binary_rule);
        let mut command = Command::new(&binary);
        command.args(["app-server", "--listen", "stdio://"]).args([
            "-c",
            &filesystem,
            "-c",
            "default_permissions=\"soc_voice\"",
            "-c",
            "permissions.soc_voice.network.enabled=false",
            "-c",
            "features.plugins=false",
            "-c",
            "features.apps=false",
            "-c",
            "web_search=\"live\"",
        ]);
        for feature in ["shell_tool", "unified_exec"] {
            command.args([
                "-c",
                &format!("features.{feature}={}", settings.run_commands),
            ]);
        }
        for feature in ["computer_use", "browser_use", "browser_use_external"] {
            command.args([
                "-c",
                &format!("features.{feature}={}", settings.local_actions),
            ]);
        }
        let mut child = command
            .current_dir(&cwd)
            .env("CODEX_HOME", codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "No se pudo abrir Codex CLI. Instálalo o comprueba su ruta")?;
        let stdin = child.stdin.take().ok_or("Codex no abrió stdin")?;
        let stdout = child.stdout.take().ok_or("Codex no abrió stdout")?;
        *process.0.lock().map_err(|_| "Proceso de Codex inválido")? = Some(child);
        let (sender, lines) = mpsc::channel();
        let reader = std::thread::Builder::new()
            .name("codex-output".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            if sender.send(line).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            })?;
        let mut session = Self {
            stdin,
            lines,
            reader: Some(reader),
            process,
            thread_id: String::new(),
            cwd,
            model: String::new(),
            effort: settings.effort.clone(),
            settings: settings.clone(),
        };
        session.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"soc","title":"soc","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}}))?;
        session.response(1, HANDSHAKE_TIMEOUT)?;
        session.send(json!({"jsonrpc":"2.0","method":"initialized","params":{}}))?;
        session.send(
            json!({"jsonrpc":"2.0","id":2,"method":"account/read","params":{"refreshToken":false}}),
        )?;
        let account = session.response(2, HANDSHAKE_TIMEOUT)?;
        if account.pointer("/account/type").and_then(Value::as_str) != Some("chatgpt") {
            return Err(
                "Codex no está enlazado con ChatGPT. Ejecuta `codex login` y pulsa Conectar".into(),
            );
        }
        session.send(
            json!({"jsonrpc":"2.0","id":5,"method":"model/list","params":{"includeHidden":false}}),
        )?;
        let models = session.response(5, HANDSHAKE_TIMEOUT)?;
        let entries = models
            .get("data")
            .and_then(Value::as_array)
            .ok_or("El proveedor no devolvió modelos")?;
        let selected = entries
            .iter()
            .find(|model| {
                model.get("model").and_then(Value::as_str) == Some(settings.model.as_str())
            })
            .ok_or("El modelo elegido no está disponible en el catálogo de Codex")?;
        let supports_effort = selected
            .get("supportedReasoningEfforts")
            .and_then(Value::as_array)
            .is_some_and(|efforts| {
                efforts.iter().any(|effort| {
                    effort.get("reasoningEffort").and_then(Value::as_str)
                        == Some(settings.effort.as_str())
                })
            });
        if !supports_effort {
            return Err("El modelo elegido no admite el esfuerzo configurado".into());
        }
        session.model = settings.model.clone();
        let saved = if new_chat { None } else { load_thread(db)? };
        let instructions = instructions(settings);
        let (method, params) = match saved.as_deref() {
            Some(thread_id) => (
                "thread/resume",
                json!({"threadId":thread_id,"cwd":session.cwd,"approvalPolicy":"never","permissions":VOICE_PROFILE,"model":session.model,"developerInstructions":instructions}),
            ),
            None => (
                "thread/start",
                json!({"cwd":session.cwd,"approvalPolicy":"never","permissions":VOICE_PROFILE,"model":session.model,"developerInstructions":instructions}),
            ),
        };
        session.send(json!({"jsonrpc":"2.0","id":3,"method":method,"params":params}))?;
        let thread = session.response(3, HANDSHAKE_TIMEOUT)?;
        let thread_id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("El proveedor no devolvió identificador de conversación")?;
        if saved
            .as_deref()
            .is_some_and(|saved_id| saved_id != thread_id)
        {
            return Err("El proveedor reanudó una conversación distinta".into());
        }
        if saved.is_none() {
            save_thread(db, thread_id)?;
        }
        session.thread_id = thread_id.into();
        Ok(session)
    }

    pub fn new_chat(&mut self, db: &Database) -> Result<()> {
        self.send(
            json!({"jsonrpc":"2.0","id":6,"method":"thread/start","params":{
                "cwd":self.cwd,
                "approvalPolicy":"never",
                "permissions":VOICE_PROFILE,
                "model":self.model,
                "developerInstructions":instructions(&self.settings)
            }}),
        )?;
        let thread = self.response(6, HANDSHAKE_TIMEOUT)?;
        let thread_id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("El proveedor no devolvió identificador del nuevo chat")?;
        if thread_id == self.thread_id {
            return Err("El proveedor no creó un chat distinto".into());
        }
        save_thread(db, thread_id)?;
        self.thread_id = thread_id.into();
        Ok(())
    }

    #[cfg(test)]
    pub fn speak(&mut self, text: &str) -> Result<String> {
        let mut replies = Vec::new();
        self.speak_stream(text, |event| {
            if let StreamEvent::Completed(reply) = event {
                replies.push(reply);
            }
        })?;
        Ok(replies.join("\n\n"))
    }

    pub fn speak_stream(
        &mut self,
        text: &str,
        mut on_message: impl FnMut(StreamEvent),
    ) -> Result<()> {
        self.turn(text, &mut on_message)
    }

    fn turn(&mut self, text: &str, on_message: &mut impl FnMut(StreamEvent)) -> Result<()> {
        self.send(turn_start_request(
            &self.thread_id,
            &self.cwd,
            &self.model,
            &self.effort,
            text,
        ))?;
        let turn = self.response(4, HANDSHAKE_TIMEOUT)?;
        let turn_id = turn
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or("El proveedor no inició el turno")?;
        let deadline = Instant::now() + TURN_TIMEOUT;
        let mut messages = MessageCollector::default();
        loop {
            let event = self.next(deadline)?;
            if event.get("id").is_some() && event.get("method").is_some() {
                return Err(
                    "El proveedor solicitó una acción no disponible para la conversación por voz"
                        .into(),
                );
            }
            let method = event.get("method").and_then(Value::as_str);
            if let Some(message) = messages.accept(&event, turn_id)? {
                on_message(message);
            }
            if method == Some("turn/completed")
                && event.pointer("/params/turn/id").and_then(Value::as_str) == Some(turn_id)
            {
                let status = event.pointer("/params/turn/status").and_then(Value::as_str);
                if status != Some("completed") {
                    return Err("El proveedor no completó la respuesta".into());
                }
                if let Some(message) = messages.finish() {
                    on_message(message);
                }
                if messages.completed.is_empty() {
                    return Err("El proveedor terminó sin texto de respuesta".into());
                }
                return Ok(());
            }
        }
    }

    fn send(&mut self, message: Value) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, &message)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }
    fn response(&mut self, id: i64, timeout: Duration) -> Result<Value> {
        let deadline = Instant::now() + timeout;
        loop {
            let value = self.next(deadline)?;
            if value.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(error) = value.get("error") {
                    let code = error
                        .get("code")
                        .and_then(Value::as_i64)
                        .unwrap_or_default();
                    return Err(format!("El proveedor rechazó la operación (código {code})").into());
                }
                return value
                    .get("result")
                    .cloned()
                    .ok_or_else(|| "El proveedor devolvió una respuesta incompleta".into());
            }
            if value.get("id").is_some() && value.get("method").is_some() {
                return Err("El proveedor solicitó una acción no disponible".into());
            }
        }
    }
    fn next(&self, deadline: Instant) -> Result<Value> {
        let wait = deadline.saturating_duration_since(Instant::now());
        if wait.is_zero() {
            return Err("El proveedor excedió el tiempo de espera".into());
        }
        let line = match self.lines.recv_timeout(wait) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                return Err("El proveedor excedió el tiempo de espera".into());
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("El proceso Codex CLI terminó antes de responder".into());
            }
        };
        if line.len() > 1_000_000 {
            return Err("Mensaje del proveedor demasiado grande".into());
        }
        Ok(serde_json::from_str(&line)?)
    }
}
fn filesystem_config(settings: &Settings, binary_rule: &str) -> String {
    let root = if settings.write_files {
        "write"
    } else if settings.read_files {
        "read"
    } else {
        "deny"
    };
    format!(
        "permissions.soc_voice.filesystem={{\":root\"=\"{root}\",\":minimal\"=\"read\",{binary_rule}=\"read\"}}"
    )
}
fn instructions(settings: &Settings) -> String {
    let mut text = VOICE_INSTRUCTIONS.to_owned();
    if settings.local_actions {
        text.push_str(" Puedes controlar la interfaz de aplicaciones locales mediante comandos de macOS, por ejemplo osascript/System Events, cuando el usuario lo pida. Respeta los permisos del sistema y no afirmes que una acción ocurrió sin comprobarla.");
    } else {
        text.push_str(
            " No controles la interfaz de otras aplicaciones, tampoco mediante comandos.",
        );
    }
    if !settings.run_commands {
        text.push_str(" No ejecutes comandos ni procesos locales.");
    }
    if !settings.read_files {
        text.push_str(" No leas archivos locales.");
    }
    if !settings.write_files {
        text.push_str(" No escribas ni modifiques archivos locales.");
    }
    text
}
impl Drop for Session {
    fn drop(&mut self) {
        self.process.kill();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
fn soc_directory() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or("No se encuentra HOME para Codex")?;
    let path = Path::new(&home).join("Library/Application Support/soc/codex");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
fn codex_binary() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let bundled = Path::new(
            "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex",
        );
        if bundled.is_file() {
            return bundled.into();
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = Path::new(&home).join(".local/bin/codex");
        if candidate.is_file() {
            return candidate;
        }
    }
    for candidate in ["/opt/homebrew/bin/codex", "/usr/local/bin/codex"] {
        let path = Path::new(candidate);
        if path.is_file() {
            return path.into();
        }
    }
    "codex".into()
}

fn prepare_codex_home(base: &Path) -> Result<PathBuf> {
    let isolated = base.join("cli-home");
    std::fs::create_dir_all(&isolated)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        std::fs::set_permissions(&isolated, std::fs::Permissions::from_mode(0o700))?;
        let auth = isolated.join("auth.json");
        if std::fs::symlink_metadata(&auth).is_err() {
            let source = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".codex")))
                .ok_or("No se encuentra la sesión de Codex")?
                .join("auth.json");
            if source.is_file() {
                symlink(source, auth)?;
            }
        }
    }
    Ok(isolated)
}

#[cfg(test)]
mod tests {
    use super::{MessageCollector, StreamEvent, filesystem_config, turn_start_request};
    use crate::Settings;
    use serde_json::json;

    #[test]
    fn separate_agent_messages_are_emitted_without_joining_their_text() {
        let mut messages = MessageCollector::default();
        let events = [
            json!({"method":"item/agentMessage/delta","params":{"turnId":"turn-1","itemId":"a","delta":"Voy a consultar CPU y memoria."}}),
            json!({"method":"item/completed","params":{"turnId":"turn-1","item":{"id":"a","type":"agentMessage","text":"Voy a consultar CPU y memoria."}}}),
            json!({"method":"item/agentMessage/delta","params":{"turnId":"turn-1","itemId":"b","delta":"No pude consultar top."}}),
            json!({"method":"item/completed","params":{"turnId":"turn-1","item":{"id":"b","type":"agentMessage","text":"No pude consultar top."}}}),
        ];
        let replies: Vec<_> = events
            .iter()
            .filter_map(|event| messages.accept(event, "turn-1").unwrap())
            .collect();
        assert_eq!(
            replies,
            [
                StreamEvent::Delta("Voy a consultar CPU y memoria.".into()),
                StreamEvent::Completed("Voy a consultar CPU y memoria.".into()),
                StreamEvent::Delta("No pude consultar top.".into()),
                StreamEvent::Completed("No pude consultar top.".into()),
            ]
        );
        assert_eq!(messages.finish(), None);
    }

    #[test]
    fn filesystem_permissions_follow_saved_switches() {
        let mut settings = Settings::default();
        assert!(filesystem_config(&settings, "\"/codex\"").contains("\":root\"=\"write\""));
        settings.write_files = false;
        assert!(filesystem_config(&settings, "\"/codex\"").contains("\":root\"=\"read\""));
        settings.read_files = false;
        assert!(filesystem_config(&settings, "\"/codex\"").contains("\":root\"=\"deny\""));
    }

    #[test]
    fn user_text_is_sent_without_content_based_additions() {
        for text in [
            "¿Cómo veo las sesiones abiertas en Instagram?",
            "Busca en Internet y cita una fuente.",
            "No, me refería al avión.",
        ] {
            let request = turn_start_request(
                "thread-1",
                std::path::Path::new("/tmp/soc-test"),
                "gpt-6-luna",
                "low",
                text,
            );
            assert_eq!(
                request["params"]["input"],
                json!([{"type":"text","text":text}])
            );
        }
    }
}
