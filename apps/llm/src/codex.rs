use crate::{ConversationJournal, Settings};
use gennie_core::Result;
use gennie_db::Database;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(crate) struct RetryableProviderError;
impl std::fmt::Display for RetryableProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Se interrumpió la comunicación con Codex")
    }
}
impl std::error::Error for RetryableProviderError {}

fn retryable_provider_info(info: &Value) -> bool {
    if matches!(
        info.as_str(),
        Some("serverOverloaded" | "internalServerError" | "rateLimitExceeded")
    ) {
        return true;
    }
    for kind in [
        "httpConnectionFailed",
        "responseStreamConnectionFailed",
        "responseStreamDisconnected",
        "responseTooManyFailedAttempts",
    ] {
        if let Some(value) = info.get(kind) {
            return value
                .get("httpStatusCode")
                .and_then(Value::as_u64)
                .is_none_or(|status| status == 408 || status == 429 || status >= 500);
        }
    }
    false
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const TURN_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_REPLY_BYTES: usize = 32_000;
const VOICE_PROFILE: &str = "gennie_voice";
const ASSISTANT_INSTRUCTIONS: &str = include_str!("assistant-instructions.md");

#[cfg(test)]
#[path = "browser_tests.rs"]
mod browser_tests;

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
    UserSaved(String),
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

#[cfg(test)]
pub(crate) fn save_thread(db: &Database, thread_id: &str) -> Result<()> {
    save_recorded_thread(db, thread_id, None, None)
}
fn save_recorded_thread(
    db: &Database,
    thread_id: &str,
    title: Option<&str>,
    journal: Option<&dyn ConversationJournal>,
) -> Result<()> {
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        if let Some(journal) = journal { journal.activate(&tx, "codex", thread_id, title)?; }
        tx.execute("INSERT INTO codex_conversation(id,thread_id) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET thread_id=excluded.thread_id,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')", [thread_id])?;
        tx.commit()?;
        Ok(())
    })?;
    if let Some(journal) = journal {
        journal.committed();
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct ProcessHandle(Arc<Mutex<Option<Child>>>, Arc<AtomicBool>);
impl ProcessHandle {
    pub fn cancel(&self) {
        self.1.store(true, Ordering::Release);
        self.kill();
    }
    pub fn kill(&self) {
        if let Ok(mut guard) = self.0.lock()
            && let Some(mut child) = guard.take()
        {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

enum ConnectionMode {
    Conversation {
        db: Database,
        new_chat: bool,
    },
    Builder {
        instructions: String,
        saved_thread: Option<String>,
    },
}

pub struct Session {
    stdin: ChildStdin,
    lines: Receiver<String>,
    reader: Option<JoinHandle<()>>,
    process: ProcessHandle,
    thread_id: String,
    journal: Option<Arc<dyn ConversationJournal>>,
    cwd: PathBuf,
    model: String,
    effort: String,
    title_effort: Option<String>,
    settings: Settings,
    tool_names: Vec<String>,
}

impl Session {
    pub fn connect_recorded(
        db: &Database,
        process: ProcessHandle,
        settings: &Settings,
        new_chat: bool,
        journal: Option<Arc<dyn ConversationJournal>>,
        tools: Option<gennie_mcp::Launch>,
    ) -> Result<Self> {
        Self::connect_in_with_mode(
            db,
            process,
            gennie_directory()?,
            new_chat,
            settings,
            journal,
            tools,
        )
    }
    #[cfg(test)]
    pub(crate) fn connect_new_in(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
    ) -> Result<Self> {
        Self::connect_in_with_mode(db, process, base, true, settings, None, None)
    }
    #[cfg(test)]
    pub(crate) fn connect_in(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
    ) -> Result<Self> {
        Self::connect_in_with_mode(db, process, base, false, settings, None, None)
    }
    fn connect_in_with_mode(
        db: &Database,
        process: ProcessHandle,
        base: PathBuf,
        new_chat: bool,
        settings: &Settings,
        journal: Option<Arc<dyn ConversationJournal>>,
        tools: Option<gennie_mcp::Launch>,
    ) -> Result<Self> {
        Self::connect_scoped(
            process,
            base,
            settings,
            journal,
            tools,
            ConnectionMode::Conversation {
                db: db.clone(),
                new_chat,
            },
        )
    }
    fn connect_scoped(
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
        journal: Option<Arc<dyn ConversationJournal>>,
        tools: Option<gennie_mcp::Launch>,
        mode: ConnectionMode,
    ) -> Result<Self> {
        settings.validate()?;
        let builder = matches!(&mode, ConnectionMode::Builder { .. });
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
            "default_permissions=\"gennie_voice\"",
            "-c",
            "permissions.gennie_voice.network.enabled=false",
            "-c",
            "permissions.gennie_title.filesystem={\":root\"=\"deny\",\":minimal\"=\"read\"}",
            "-c",
            "permissions.gennie_title.network.enabled=false",
            "-c",
            "features.plugins=false",
            "-c",
            "features.apps=false",
            "-c",
            "web_search=\"live\"",
        ]);
        if let Some(tools) = &tools {
            command.args(["-c", &tools.config()?]);
        }
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
            .process_group(0)
            .spawn()
            .map_err(|_| "No se pudo abrir Codex CLI. Instálalo o comprueba su ruta")?;
        let stdin = child.stdin.take().ok_or("Codex no abrió stdin")?;
        let stdout = child.stdout.take().ok_or("Codex no abrió stdout")?;
        {
            let mut handle = process.0.lock().map_err(|_| "Proceso de Codex inválido")?;
            if process.1.load(Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Constructor cancelado".into());
            }
            *handle = Some(child);
        }
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
            journal,
            cwd,
            model: String::new(),
            effort: settings.effort.clone(),
            title_effort: None,
            settings: settings.clone(),
            tool_names: tools
                .as_ref()
                .map(|tools| tools.tool_names.clone())
                .unwrap_or_default(),
        };
        session.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"Gennie","title":"Gennie","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}}))?;
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
        session.title_effort = crate::title::minimum_effort(selected);
        session.model = settings.model.clone();
        let saved = match &mode {
            ConnectionMode::Conversation { db, new_chat } => {
                if *new_chat {
                    None
                } else {
                    load_thread(db)?
                }
            }
            ConnectionMode::Builder { saved_thread, .. } => saved_thread.clone(),
        };
        let instructions = match &mode {
            ConnectionMode::Conversation { .. } => instructions(settings, &session.tool_names),
            ConnectionMode::Builder { instructions, .. } => instructions.clone(),
        };
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
        if let ConnectionMode::Conversation { db, .. } = &mode {
            save_recorded_thread(
                db,
                thread_id,
                thread
                    .pointer("/thread/name")
                    .map(|value| value.as_str().unwrap_or("")),
                session.journal.as_deref(),
            )?;
        }
        debug_assert!(!builder || session.journal.is_none());
        session.thread_id = thread_id.into();
        if let Some(tools) = &tools {
            session.discover_tools(tools)?;
        }
        Ok(session)
    }

    pub(crate) fn connect_builder(
        process: ProcessHandle,
        base: PathBuf,
        settings: &Settings,
        tools: gennie_mcp::Launch,
        instructions: String,
        saved_thread: Option<String>,
    ) -> Result<Self> {
        Self::connect_scoped(
            process,
            base,
            settings,
            None,
            Some(tools),
            ConnectionMode::Builder {
                instructions,
                saved_thread,
            },
        )
    }
    pub(crate) fn thread_id(&self) -> &str {
        &self.thread_id
    }
    pub(crate) fn builder_goal(&mut self, objective: &str) -> Result<()> {
        self.send(json!({"jsonrpc":"2.0","id":20,"method":"thread/goal/get","params":{"threadId":self.thread_id}}))?;
        let current = self.response(20, HANDSHAKE_TIMEOUT)?;
        if let Some(goal) = current.get("goal").filter(|goal| !goal.is_null())
            && goal["objective"].as_str() != Some(objective)
        {
            return Err("El objetivo guardado del constructor no coincide con este trabajo".into());
        }
        // El app-server inicia turnos automáticos con un objetivo activo. Gennie
        // controla la continuación y el reloj, por lo que lo conserva en pausa.
        self.send(json!({"jsonrpc":"2.0","id":21,"method":"thread/goal/set","params":{"threadId":self.thread_id,"origin":"user","objective":objective,"status":"paused"}}))?;
        let result = self.response(21, HANDSHAKE_TIMEOUT)?;
        if result.pointer("/goal/status").and_then(Value::as_str) != Some("paused") {
            return Err("No se pudo guardar el objetivo del constructor".into());
        }
        Ok(())
    }
    pub(crate) fn builder_goal_status(&mut self, status: &str) -> Result<()> {
        self.send(json!({"jsonrpc":"2.0","id":22,"method":"thread/goal/set","params":{"threadId":self.thread_id,"origin":"automatic","status":status}}))?;
        self.response(22, HANDSHAKE_TIMEOUT)?;
        Ok(())
    }
    pub(crate) fn build_turn(&mut self, text: &str) -> Result<()> {
        let mut request =
            turn_start_request(&self.thread_id, &self.cwd, &self.model, &self.effort, text);
        request["id"] = json!(9); // Activa el drenaje de turn/interrupt en errores del constructor.
        self.run_turn(request, None, &mut |_| Ok(()))
    }
    fn discover_tools(&mut self, tools: &gennie_mcp::Launch) -> Result<()> {
        self.send(
            json!({"jsonrpc":"2.0","id":13,"method":"mcpServerStatus/list","params":{
                "threadId":self.thread_id,"serverName":"gennie","detail":"toolsAndAuthOnly"
            }}),
        )?;
        tools.validate_inventory(&self.response(13, HANDSHAKE_TIMEOUT)?)
    }

    pub fn new_chat(&mut self, db: &Database) -> Result<()> {
        self.send(
            json!({"jsonrpc":"2.0","id":6,"method":"thread/start","params":{
                "cwd":self.cwd,
                "approvalPolicy":"never",
                "permissions":VOICE_PROFILE,
                "model":self.model,
                "developerInstructions":instructions(&self.settings, &self.tool_names)
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
        save_recorded_thread(
            db,
            thread_id,
            thread
                .pointer("/thread/name")
                .map(|value| value.as_str().unwrap_or("")),
            self.journal.as_deref(),
        )?;
        self.thread_id = thread_id.into();
        Ok(())
    }

    fn capture_title(&self, response: &Value) -> Result<()> {
        if response.pointer("/thread/id").and_then(Value::as_str) != Some(self.thread_id.as_str()) {
            return Err("El proveedor devolvió metadatos de otra conversación".into());
        }
        if let Some(value) = response.pointer("/thread/name")
            && let Some(title) = value.as_str().or_else(|| value.is_null().then_some(""))
            && let Some(journal) = &self.journal
        {
            journal.update_title("codex", &self.thread_id, title)?;
        }
        Ok(())
    }
    fn refresh_title(&mut self) -> Result<()> {
        if self.journal.is_none() {
            return Ok(());
        }
        self.send(json!({"jsonrpc":"2.0","id":7,"method":"thread/read","params":{"threadId":self.thread_id,"includeTurns":false}}))?;
        let response = self.response(7, Duration::from_secs(3))?;
        self.capture_title(&response)
    }

    pub fn record_error(&self, error: &str) -> Result<()> {
        if let Some(journal) = &self.journal {
            journal.append(
                "codex",
                &self.thread_id,
                "system",
                &format!("Error: {error}"),
            )?;
        }
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
        if let Some(journal) = &self.journal {
            journal.append("codex", &self.thread_id, "user", text)?;
        }
        on_message(StreamEvent::UserSaved(text.to_owned()));
        let journal = self.journal.clone();
        let thread_id = self.thread_id.clone();
        self.turn(text, &mut |event| {
            if let StreamEvent::Completed(text) = &event
                && let Some(journal) = &journal
            {
                // Mostrar la respuesta aunque falle su guardado; el error también se propaga.
                if let Err(error) = journal.append("codex", &thread_id, "assistant", text) {
                    on_message(event);
                    return Err(error);
                }
            }
            on_message(event);
            Ok(())
        })?;
        // El título es opcional: sus fallos no interrumpen ni ensucian el chat.
        let _ = self.refresh_title();
        let _ = self.generate_title();
        Ok(())
    }

    fn generate_title(&mut self) -> Result<()> {
        let Some(effort) = self.title_effort.clone() else {
            return Ok(());
        };
        let Some(journal) = self.journal.clone() else {
            return Ok(());
        };
        let Some((user, assistant)) = journal.claim_title_input("codex", &self.thread_id)? else {
            return Ok(());
        };
        self.send(crate::title::start_request(&self.cwd, &self.model))?;
        let response = self.response(8, Duration::from_secs(10))?;
        let temporary = response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .filter(|id| *id != self.thread_id)
            .ok_or("Codex no creó una conversación temporal distinta")?
            .to_owned();
        let result = (|| -> Result<String> {
            let mut output = String::new();
            self.run_turn(
                crate::title::turn_request(
                    &temporary,
                    &self.cwd,
                    &self.model,
                    &effort,
                    &user,
                    &assistant,
                ),
                Some(Duration::from_secs(10)),
                &mut |event| {
                    if let StreamEvent::Completed(text) = event {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str(&text);
                    }
                    Ok(())
                },
            )?;
            crate::title::validate(&output)
        })();
        // Liberar la suscripción incluso si la generación o validación fallan.
        self.send(json!({"jsonrpc":"2.0","id":10,"method":"thread/unsubscribe","params":{"threadId":temporary}}))?;
        self.response(10, Duration::from_secs(3))?;
        let title = result?;
        self.send(json!({"jsonrpc":"2.0","id":11,"method":"thread/name/set","params":{"threadId":self.thread_id,"name":title}}))?;
        self.response(11, Duration::from_secs(3))?;
        journal.update_title("codex", &self.thread_id, &title)
    }

    fn turn(
        &mut self,
        text: &str,
        on_message: &mut impl FnMut(StreamEvent) -> Result<()>,
    ) -> Result<()> {
        self.run_turn(
            turn_start_request(&self.thread_id, &self.cwd, &self.model, &self.effort, text),
            Some(TURN_TIMEOUT),
            on_message,
        )
    }

    fn run_turn(
        &mut self,
        request: Value,
        timeout: Option<Duration>,
        on_message: &mut impl FnMut(StreamEvent) -> Result<()>,
    ) -> Result<()> {
        let id = request["id"]
            .as_i64()
            .ok_or("Falta identificador de petición")?;
        let thread_id = request["params"]["threadId"]
            .as_str()
            .ok_or("Falta hilo de petición")?
            .to_owned();
        self.send(request)?;
        let turn = self.response(
            id,
            timeout.unwrap_or(HANDSHAKE_TIMEOUT).min(HANDSHAKE_TIMEOUT),
        )?;
        let turn_id = turn
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .ok_or("El proveedor no inició el turno")?;
        let result = self.receive_turn(turn_id, timeout, on_message);
        if result.is_err() && id == 9
            && self.send(json!({"jsonrpc":"2.0","id":12,"method":"turn/interrupt","params":{"threadId":thread_id,"turnId":turn_id}})).is_ok()
        {
            let _ = self.response(12, Duration::from_secs(3));
        }
        result
    }

    fn receive_turn(
        &mut self,
        turn_id: &str,
        timeout: Option<Duration>,
        on_message: &mut impl FnMut(StreamEvent) -> Result<()>,
    ) -> Result<()> {
        let deadline = timeout.map(|duration| Instant::now() + duration);
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
                on_message(message)?;
            }
            if method == Some("turn/completed")
                && event.pointer("/params/turn/id").and_then(Value::as_str) == Some(turn_id)
            {
                let status = event.pointer("/params/turn/status").and_then(Value::as_str);
                if status != Some("completed") {
                    if status == Some("failed")
                        && event
                            .pointer("/params/turn/error/codexErrorInfo")
                            .is_some_and(retryable_provider_info)
                    {
                        return Err(Box::new(RetryableProviderError));
                    }
                    return Err("El proveedor no completó la respuesta".into());
                }
                if let Some(message) = messages.finish() {
                    on_message(message)?;
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
            let value = self.next(Some(deadline))?;
            if value.get("id").and_then(Value::as_i64) == Some(id) {
                return response_result(&value);
            }
            if value.get("id").is_some() && value.get("method").is_some() {
                return Err("El proveedor solicitó una acción no disponible".into());
            }
        }
    }
    fn next(&self, deadline: Option<Instant>) -> Result<Value> {
        let line = match deadline {
            Some(deadline) => {
                let wait = deadline.saturating_duration_since(Instant::now());
                if wait.is_zero() {
                    return Err(Box::new(RetryableProviderError));
                }
                match self.lines.recv_timeout(wait) {
                    Ok(line) => line,
                    Err(RecvTimeoutError::Timeout) => {
                        return Err(Box::new(RetryableProviderError));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err(Box::new(RetryableProviderError));
                    }
                }
            }
            None => self.lines.recv().map_err(|_| RetryableProviderError)?,
        };
        if line.len() > 1_000_000 {
            return Err("Mensaje del proveedor demasiado grande".into());
        }
        let event: Value = serde_json::from_str(&line)?;
        if let Some(title) = notification_title(&event, &self.thread_id)
            && let Some(journal) = &self.journal
        {
            journal.update_title("codex", &self.thread_id, title)?;
        }
        Ok(event)
    }
}
fn response_result(value: &Value) -> Result<Value> {
    if let Some(error) = value.get("error") {
        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        // Clasificar solo el fallo conocido; no mostrar ni persistir detalles libres del proveedor.
        if code == -32600
            && error
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|message| message.starts_with("no rollout found for thread id "))
        {
            return Err("Codex no encuentra el historial de esta conversación y no puede reanudarla. Pulsa «Nuevo chat» para continuar. El historial local de Gennie se conserva.".into());
        }
        return Err(format!("El proveedor rechazó la operación (código {code})").into());
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "El proveedor devolvió una respuesta incompleta".into())
}

fn notification_title<'a>(event: &'a Value, thread_id: &str) -> Option<&'a str> {
    if event.get("method").and_then(Value::as_str) == Some("thread/name/updated")
        && event.pointer("/params/threadId").and_then(Value::as_str) == Some(thread_id)
    {
        event
            .pointer("/params/threadName")
            .and_then(|value| value.as_str().or_else(|| value.is_null().then_some("")))
    } else {
        None
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
        "permissions.gennie_voice.filesystem={{\":root\"=\"{root}\",\":minimal\"=\"read\",{binary_rule}=\"read\"}}"
    )
}
fn session_capabilities(settings: &Settings, tool_names: &[String]) -> Value {
    let browser_tools: Vec<_> = tool_names
        .iter()
        .filter(|name| name.starts_with("browser_"))
        .collect();
    let has_browser = ["browser_open", "browser_navigate", "browser_snapshot"]
        .iter()
        .all(|required| tool_names.iter().any(|name| name == required));
    let local_control = settings.local_actions && settings.run_commands;
    json!({
        "gennie_web": {
            "available": has_browser && local_control,
            "transport": "MCP directo a la API Playwright, sin controlar el ordenador",
            "tools": browser_tools,
            "tools_registered": has_browser,
            "permission_enabled": local_control,
            "file_read_permission_enabled":settings.read_files,
            "upload_available":has_browser && local_control && settings.read_files && tool_names.iter().any(|name| name == "browser_upload")
        },
        "web_plugins": {
            "available": local_control && ["browser_plugins", "browser_plugin_run"].iter().all(|required| tool_names.iter().any(|name| name == required)),
            "background_builder_available":local_control && tool_names.iter().any(|name| name == "plugin_build_start"),
            "scheduled_execution_available":false,
            "authentication":"solo cuando la operación requiere acceso y la sesión existente no basta"
        },
        "computer_control": {"permission_enabled":local_control,"system_permission_verified":false},
        "commands": {"permission_enabled":settings.run_commands,"network_enabled":false}
    })
}
fn instructions(settings: &Settings, tool_names: &[String]) -> String {
    let mut text = ASSISTANT_INSTRUCTIONS.to_owned();
    text.push_str(" Si el MCP local gennie ofrece herramientas assignments_*, úsalas para registrar, consultar y actualizar encargos a petición del usuario o cuando acepte claramente una propuesta tuya. No escribas directamente sus archivos ni SQLite. Hay encargos puntuales, recurrentes y objetivos. Antes de crear, consulta assignments_list para obtener fecha UTC, zona horaria del sistema y encargos existentes; usa esa referencia salvo que el usuario indique otra zona. Separa estrictamente el horario de aviso al usuario de la cadencia de preparación interna; revisiones frecuentes no implican avisos frecuentes. Elige prioridad, pasos e intervalo según el objetivo y justifica la cadencia. Usa la referencia horaria de assignments_list sin preguntar la zona salvo contradicción. Completa tú título, descripción, tipo, prioridad, pasos y seguimiento a partir de la necesidad aceptada; no conviertas los campos del registro en preguntas. Pregunta solo por el principal dato indispensable que siga faltando, como un horario obligatorio o una fuente ambigua. No inventes horarios ni cuentas. El motivo y otros campos opcionales pueden quedar vacíos. Conserva el contexto e identifica las inferencias. No inventes plugins ni acceso a cuentas. Si hay herramientas browser_*, el navegador de Gennie permite trabajo web autorizado sobre sesiones verificadas; no implica conexión nativa de esas aplicaciones ni ejecución autónoma de encargos. Registrar un encargo solo acredita su guardado. Confirma los datos acordados, sin añadir avisos de implementación o integración pendiente. No deduzcas del registro que una revisión se ejecutó o una conexión quedó activada. Confirma el guardado únicamente cuando assignments_create/assignments_update devuelva éxito; ante errores corrige datos o pide lo que falte. Antes de editar consulta assignments_get y usa su revision. Reutiliza request_key solo para repetir la misma creación; una solicitud nueva necesita otra clave. Estas herramientas registran datos propios de Gennie y son independientes de los permisos de archivos/comandos del ordenador. No hay herramientas de memoria todavía.");
    text.push_str("\n\n");
    text.push_str(include_str!("web-research-instructions.md"));
    text.push_str(include_str!("web-plugin-instructions.md"));
    text.push_str("\n\nCapacidades de esta sesión, derivadas de las herramientas registradas y los permisos vigentes; no son una autorización general para actuar:\n");
    text.push_str(&session_capabilities(settings, tool_names).to_string());
    text.push_str("\nUsa esta disponibilidad actual, aunque un mensaje anterior tuyo haya negado acceso. No confundas la ausencia de plugins o conectores nativos con la ausencia del navegador por MCP. Si gennie_web.available es true, tienes Gennie Web y debes intentar su API antes de afirmar que no puedes acceder a ese navegador. Si es false, distingue herramienta ausente de permiso desactivado y no intentes eludir esa restricción. La red de los comandos está desactivada; un fallo de curl/wget no implica que la búsqueda web o Playwright estén bloqueados. No gastes intentos equivalentes contra la misma restricción.");
    if settings.local_actions {
        text.push_str(" Puedes usar el ordenador para completar una petición del usuario que lo necesite, también como alternativa razonable dentro del mismo objetivo de lectura o navegación. No esperes una orden literal «usa el ordenador» para continuar una investigación aceptada. Prefiere primero la API de Gennie Web; si falla por una causa comprobada y queda una vía pertinente, puedes probar Safari u otro navegador disponible mediante comandos de macOS (por ejemplo osascript/System Events), con los permisos existentes. Abrir un enlace no prueba que hayas leído su contenido: obsérvalo por una vía disponible. No inventes herramientas de ordenador instaladas ni permisos del sistema concedidos. Informa brevemente de un cambio de vía cuando aporte contexto; no pidas autorización repetida para pasos de lectura ya comprendidos en la petición. El envío de mensajes, compras o cambios externos conserva el alcance autorizado por el usuario.");
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
fn gennie_directory() -> Result<PathBuf> {
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
    fn missing_rollout_explains_recovery_without_exposing_provider_details() {
        let error = super::response_result(&json!({"id":3,"error":{
            "code":-32600,"message":"no rollout found for thread id private-thread-id"
        }}))
        .unwrap_err()
        .to_string();
        assert_eq!(
            error,
            "Codex no encuentra el historial de esta conversación y no puede reanudarla. Pulsa «Nuevo chat» para continuar. El historial local de Gennie se conserva."
        );
    }

    #[test]
    fn unrelated_rejection_does_not_expose_provider_message() {
        let error = super::response_result(&json!({"id":3,"error":{
            "code":-32600,"message":"other failure with private content"
        }}))
        .unwrap_err()
        .to_string();
        assert_eq!(error, "El proveedor rechazó la operación (código -32600)");
    }

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
    fn provider_retry_classification_uses_structured_errors() {
        for info in [
            json!("serverOverloaded"),
            json!("internalServerError"),
            json!("rateLimitExceeded"),
            json!({"responseStreamDisconnected":{"httpStatusCode":null}}),
            json!({"httpConnectionFailed":{"httpStatusCode":503}}),
        ] {
            assert!(super::retryable_provider_info(&info));
        }
        for info in [
            json!("unauthorized"),
            json!("usageLimitExceeded"),
            json!("badRequest"),
            json!({"httpConnectionFailed":{"httpStatusCode":401}}),
            json!(null),
        ] {
            assert!(!super::retryable_provider_info(&info));
        }
    }

    #[cfg(unix)]
    #[test]
    fn builder_failures_cross_stdio_and_keep_the_retry_classification() {
        use super::*;

        // Proceso propio: habla el mismo protocolo, sin Codex real ni inferencia.
        for (info, disconnect, retryable) in [
            (json!("serverOverloaded"), false, true),
            (
                json!({"httpConnectionFailed":{"httpStatusCode":503}}),
                false,
                true,
            ),
            (
                json!({"responseStreamDisconnected":{"httpStatusCode":null}}),
                false,
                true,
            ),
            (json!("unauthorized"), false, false),
            (json!("usageLimitExceeded"), false, false),
            (json!(null), true, true),
        ] {
            let failure = json!({"method":"turn/completed","params":{"turn":{
                "id":"fixture-turn", "status":"failed", "error":{
                    "message":"private-provider-detail", "codexErrorInfo":info
                }
            }}});
            let mut child = Command::new("sh")
                .args(["-c", r#"
IFS= read -r request || exit 1
case "$request" in *'"id":9,'*) ;; *) exit 2 ;; esac
printf '%s\n' '{"id":9,"result":{"turn":{"id":"fixture-turn"}}}'
if [ "$DISCONNECT" = true ]; then exit 0; fi
printf '%s\n' "$FAILURE"
IFS= read -r request || exit 0
case "$request" in *'"method":"turn/interrupt"'*) printf '%s\n' '{"id":12,"result":{}}' ;; *) exit 3 ;; esac
"#])
                .env("FAILURE", failure.to_string())
                .env("DISCONNECT", disconnect.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .process_group(0)
                .spawn()
                .unwrap();
            let stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let process = ProcessHandle::default();
            *process.0.lock().unwrap() = Some(child);
            let (sender, lines) = mpsc::channel();
            let reader = std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if sender.send(line.unwrap()).is_err() {
                        break;
                    }
                }
            });
            let mut session = Session {
                stdin,
                lines,
                reader: Some(reader),
                process,
                thread_id: "fixture-thread".into(),
                journal: None,
                cwd: std::env::temp_dir(),
                model: "fixture-model".into(),
                effort: "medium".into(),
                title_effort: None,
                settings: Settings::default(),
                tool_names: Vec::new(),
            };
            let error = session
                .build_turn("Prueba sin acciones externas")
                .unwrap_err();
            assert_eq!(
                crate::builder::is_retryable(error.as_ref()),
                retryable,
                "{info}"
            );
            assert!(!error.to_string().contains("private-provider-detail"));
            assert_eq!(session.thread_id(), "fixture-thread");
        }
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
                std::path::Path::new("/tmp/gennie-test"),
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

#[cfg(test)]
mod journal_tests {
    #[test]
    fn title_notification_accepts_only_the_matching_thread_name() {
        let event = serde_json::json!({"method":"thread/name/updated","params":{"threadId":"a","threadName":"Nombre original"}});
        assert_eq!(
            super::notification_title(&event, "a"),
            Some("Nombre original")
        );
        assert_eq!(super::notification_title(&event, "b"), None);
        assert_eq!(
            super::notification_title(
                &serde_json::json!({"method":"thread/name/updated","params":{"threadId":"a","threadName":null}}),
                "a"
            ),
            Some("")
        );
    }
    use super::*;
    use gennie_core::AppModule;
    #[derive(Default)]
    struct TitleJournal {
        messages: Mutex<Vec<(String, String)>>,
        title: Mutex<Option<String>>,
        attempted: std::sync::atomic::AtomicBool,
    }
    impl ConversationJournal for TitleJournal {
        fn activate(
            &self,
            _: &gennie_db::SqliteConnection,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }
        fn append(&self, _: &str, _: &str, speaker: &str, text: &str) -> Result<()> {
            self.messages
                .lock()
                .unwrap()
                .push((speaker.into(), text.into()));
            Ok(())
        }
        fn claim_title_input(&self, _: &str, _: &str) -> Result<Option<(String, String)>> {
            if self
                .attempted
                .swap(true, std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(None);
            }
            let messages = self.messages.lock().unwrap();
            Ok(Some((messages[0].1.clone(), messages[1].1.clone())))
        }
        fn update_title(&self, _: &str, _: &str, title: &str) -> Result<()> {
            *self.title.lock().unwrap() = Some(title.into());
            Ok(())
        }
        fn committed(&self) {}
    }

    #[cfg(unix)]
    #[test]
    fn optional_title_failure_is_silent_and_uses_minimum_effort_without_changing_chat_effort() {
        for fail in [false, true] {
            // Proveedor IPC aislado: reproduce el flujo completo sin red ni modelos.
            let script = r#"
while IFS= read -r request; do
  case "$request" in
    *'"id":4,'*)
      case "$request" in *'"effort":"high"'*) ;; *) exit 1 ;; esac
      printf '%s\n' '{"id":4,"result":{"turn":{"id":"reply"}}}' '{"method":"item/completed","params":{"turnId":"reply","item":{"type":"agentMessage","text":"¡Qué onda! ¿Cómo vas?"}}}' '{"method":"turn/completed","params":{"turn":{"id":"reply","status":"completed"}}}' ;;
    *'"id":7,'*) printf '%s\n' '{"id":7,"result":{"thread":{"id":"active","name":null}}}' ;;
    *'"id":8,'*) printf '%s\n' '{"id":8,"result":{"thread":{"id":"temporary"}}}' ;;
    *'"id":9,'*)
      case "$request" in *'"effort":"none"'*) ;; *) exit 1 ;; esac
      if [ "$TITLE_FAIL" = true ]; then
        printf '%s\n' '{"id":9,"error":{"code":-1,"message":"private detail"}}'
      else
        printf '%s\n' '{"id":9,"result":{"turn":{"id":"title"}}}' '{"method":"item/completed","params":{"turnId":"title","item":{"type":"agentMessage","text":"Saludo informal"}}}' '{"method":"turn/completed","params":{"turn":{"id":"title","status":"completed"}}}'
      fi ;;
    *'"id":10,'*) printf '%s\n' '{"id":10,"result":{}}' ;;
    *'"id":11,'*) printf '%s\n' '{"id":11,"result":{}}' ;;
    *) exit 1 ;;
  esac
done
"#;
            let mut child = Command::new("sh")
                .args(["-c", script])
                .env("TITLE_FAIL", fail.to_string())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let process = ProcessHandle::default();
            *process.0.lock().unwrap() = Some(child);
            let (sender, lines) = mpsc::channel();
            let reader = std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if sender.send(line.unwrap()).is_err() {
                        break;
                    }
                }
            });
            let journal = Arc::new(TitleJournal::default());
            let mut session = Session {
                stdin,
                lines,
                reader: Some(reader),
                process,
                thread_id: "active".into(),
                journal: Some(journal.clone()),
                cwd: PathBuf::from("/tmp"),
                model: "chosen-model".into(),
                effort: "high".into(),
                title_effort: Some("none".into()),
                settings: Settings::default(),
                tool_names: Vec::new(),
            };
            let mut events = Vec::new();
            session
                .speak_stream("Qué onda pa", |event| events.push(event))
                .unwrap();
            assert_eq!(
                journal.messages.lock().unwrap().as_slice(),
                &[
                    ("user".into(), "Qué onda pa".into()),
                    ("assistant".into(), "¡Qué onda! ¿Cómo vas?".into())
                ]
            );
            assert_eq!(session.thread_id, "active");
            assert_eq!(events.len(), 2);
            assert!(matches!(&events[0], StreamEvent::UserSaved(text) if text == "Qué onda pa"));
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, StreamEvent::Completed(_)))
                    .count(),
                1
            );
            if !fail {
                assert_eq!(
                    journal.title.lock().unwrap().as_deref(),
                    Some("Saludo informal")
                );
            }
            session.generate_title().unwrap();
        }
    }

    #[test]
    #[ignore = "consume una consulta real para titular un saludo con Codex en un directorio temporal"]
    fn real_title_generation_uses_an_ephemeral_thread_and_preserves_active_thread() {
        let db = Database::memory().unwrap();
        let module = crate::LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let base =
            std::env::temp_dir().join(format!("gennie-title-provider-{}", std::process::id()));
        let journal = Arc::new(TitleJournal::default());
        journal.append("codex", "", "user", "Qué onda pa").unwrap();
        journal
            .append("codex", "", "assistant", "¡Qué onda! ¿Cómo vas?")
            .unwrap();
        let result = (|| -> Result<()> {
            let mut session = Session::connect_in_with_mode(
                &db,
                ProcessHandle::default(),
                base.clone(),
                true,
                &Settings::default(),
                Some(journal.clone()),
                None,
            )?;
            let active = session.thread_id.clone();
            session.generate_title()?;
            assert_eq!(session.thread_id, active);
            assert_eq!(load_thread(&db)?.as_deref(), Some(active.as_str()));
            assert_eq!(journal.messages.lock().unwrap().len(), 2);
            let title = journal
                .title
                .lock()
                .unwrap()
                .clone()
                .ok_or("No se guardó título")?;
            crate::title::validate(&title)?;
            eprintln!("Título real de Codex: {title}");
            session.refresh_title()?;
            assert_eq!(
                journal.title.lock().unwrap().as_deref(),
                Some(title.as_str())
            );
            session.generate_title()?;
            Ok(())
        })();
        db.close().unwrap();
        let _ = std::fs::remove_dir_all(&base);
        result.unwrap();
    }

    struct FailingJournal;
    impl ConversationJournal for FailingJournal {
        fn activate(
            &self,
            connection: &gennie_db::SqliteConnection,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<()> {
            connection.execute("INSERT INTO journal_probe VALUES('new')", [])?;
            Err("Fallo de persistencia local".into())
        }
        fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
            unreachable!()
        }
        fn committed(&self) {
            panic!("No se puede anunciar un rollback como confirmado");
        }
    }
    #[test]
    fn local_history_failure_rolls_back_provider_selection_and_catalogue() {
        let db = Database::memory().unwrap();
        let module = crate::LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        save_thread(&db, "old").unwrap();
        db.with_connection(|connection| {
            connection.execute_batch("CREATE TABLE journal_probe(value TEXT)")?;
            Ok(())
        })
        .unwrap();
        assert!(save_recorded_thread(&db, "new", None, Some(&FailingJournal)).is_err());
        assert_eq!(load_thread(&db).unwrap().as_deref(), Some("old"));
        db.with_connection(|connection| {
            assert_eq!(
                connection.query_row("SELECT count(*) FROM journal_probe", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    }
}
