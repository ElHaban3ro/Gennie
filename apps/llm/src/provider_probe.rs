//! Sonda optativa del adaptador real; su diario es un doble, no Presence.
use super::*;
use crate::{QuotedMessage, ReplyInterruption, turn};
use gennie_core::AppModule;
use serde::Deserialize;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

#[path = "provider_plugin_probe.rs"]
mod plugins;

#[path = "provider_probe_diagnostics.rs"]
mod diagnostics;

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Full,
    StopResume,
    PluginSemantics,
}
impl Scenario {
    fn turns(self) -> u32 {
        match self {
            Self::Full => 4,
            Self::StopResume => 2,
            Self::PluginSemantics => 3,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    version: u32,
    scenario: Scenario,
    mode: String,
    output_dir: PathBuf,
    nonce: String,
    model: String,
    effort: String,
    model_turns_max: u32,
    cell_timeout_seconds: u64,
}
fn provider_directory(plan: &Plan) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gennie-chat-provider-{}-{}",
        std::process::id(),
        plan.nonce
    ))
}
struct PrivateDirectory(PathBuf);
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Plan {
    fn validate(&self) -> Result<()> {
        if self.version != 2
            || !matches!(self.mode.as_str(), "protocol" | "live")
            || self.model != "gpt-6-luna"
            || self.effort != "low"
            || self.model_turns_max != self.scenario.turns()
            || self.cell_timeout_seconds != 60
            || self.nonce.len() != 32
            || !self.nonce.bytes().all(|b| b.is_ascii_hexdigit())
            || !self.output_dir.is_absolute()
        {
            return Err("Plan de sonda inválido".into());
        }
        Ok(())
    }
    fn reserve_turn(&self, report: &mut Value) -> Result<()> {
        let count = report["model_turns"].as_u64().ok_or("Contador ausente")?;
        if count >= u64::from(self.model_turns_max) {
            return Err("Presupuesto agotado".into());
        }
        report["model_turns"] = json!(count + 1);
        Ok(())
    }
}

#[derive(Default)]
struct Journal {
    quote: String,
    users: Mutex<Vec<String>>,
    interrupted: Mutex<Vec<String>>,
    tool_events: Mutex<Vec<crate::ToolUse>>,
}
impl ConversationJournal for Journal {
    fn activate(
        &self,
        _: &gennie_db::SqliteConnection,
        _: &str,
        _: &str,
        _: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
    fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn committed(&self) {}
    fn record_tool(&self, _: &str, _: &str, _: &str, usage: &crate::ToolUse) -> Result<()> {
        self.tool_events
            .lock()
            .map_err(|_| "Diario inválido")?
            .push(usage.clone());
        Ok(())
    }
    fn append_user(
        &self,
        _: &str,
        _: &str,
        text: &str,
        source: Option<i64>,
    ) -> Result<Option<QuotedMessage>> {
        self.users
            .lock()
            .map_err(|_| "Diario inválido")?
            .push(text.into());
        match source {
            None => Ok(None),
            Some(7) => Ok(Some(QuotedMessage {
                speaker: "assistant".into(),
                text: format!("La clave de este mensaje histórico es {}.", self.quote),
                created_at: "2026-10-29T00:00:00Z".into(),
                truncated: false,
            })),
            Some(_) => Err("Referencia de fixture inválida".into()),
        }
    }
    fn append_interrupted_response(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: Option<&str>,
        text: &str,
        reason: ReplyInterruption,
    ) -> Result<()> {
        if reason != ReplyInterruption::Cancelled {
            return Err("Interrupción distinta de la esperada".into());
        }
        self.interrupted
            .lock()
            .map_err(|_| "Diario inválido")?
            .push(text.into());
        Ok(())
    }
}

enum Signal {
    Finish,
    Cancel,
}
#[derive(Default)]
struct StopObservation {
    pending: String,
    completed: usize,
    delta_events: usize,
    total_delta_bytes: usize,
    max_pending_bytes: usize,
    completed_characters: usize,
    interrupted: Option<(String, bool, ReplyInterruption)>,
    requested: bool,
}
impl StopObservation {
    fn report(&self, verified: bool, cancelled: bool) -> Value {
        json!({"verified":verified,"cancelled":cancelled,"cancel_requested":self.requested,"completed_messages_before_cut":self.completed,"observed_characters":self.pending.chars().count(),"observed_prefix":self.pending,"delta_events":self.delta_events,"total_delta_bytes":self.total_delta_bytes,"max_pending_bytes":self.max_pending_bytes,"completed_characters":self.completed_characters,"journal_is_test_double":true,"server_cancel_ack":null,"remote_compute_or_cost_stopped":null})
    }

    fn observe(&mut self, event: StreamEvent, cancel: &mpsc::Sender<Signal>) {
        match event {
            StreamEvent::Delta(text) => {
                self.delta_events += 1;
                self.total_delta_bytes += text.len();
                self.pending.push_str(&text);
                self.max_pending_bytes = self.max_pending_bytes.max(self.pending.len());
                if self.pending.len() >= 64 && !self.requested {
                    self.requested = cancel.send(Signal::Cancel).is_ok();
                }
            }
            StreamEvent::Completed(text) => {
                self.completed += 1;
                self.completed_characters += text.chars().count();
                self.pending.clear();
            }
            StreamEvent::Interrupted {
                text,
                saved,
                reason,
            } => self.interrupted = Some((text, saved, reason)),
            _ => {}
        }
    }
    fn verified(&self, cancelled: bool, timed_out: bool, saved_fragments: usize) -> bool {
        self.requested
            && cancelled
            && !timed_out
            && !self.pending.trim().is_empty()
            && saved_fragments == 1
            && self
                .interrupted
                .as_ref()
                .is_some_and(|(text, saved, reason)| {
                    text == &self.pending && *saved && *reason == ReplyInterruption::Cancelled
                })
    }
}
struct Scope {
    ticket: turn::Ticket,
    sender: mpsc::Sender<Signal>,
    handle: Option<JoinHandle<()>>,
    control: turn::Control,
    timed_out: Arc<AtomicBool>,
}
impl Scope {
    fn new(control: &turn::Control) -> Result<Self> {
        let ticket = control.begin()?;
        let (sender, receiver) = mpsc::channel();
        let control = control.clone();
        let worker = control.clone();
        let id = ticket.id.clone();
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout = timed_out.clone();
        let handle =
            std::thread::spawn(
                move || match receiver.recv_timeout(Duration::from_secs(60)) {
                    Ok(Signal::Cancel) => {
                        let _ = worker.cancel(&id);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        timeout.store(true, Ordering::Release);
                        let _ = worker.cancel(&id);
                    }
                    _ => {}
                },
            );
        Ok(Self {
            ticket,
            sender,
            handle: Some(handle),
            control,
            timed_out,
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let _ = self.sender.send(Signal::Finish);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.control.finish(&self.ticket.id);
    }
}

fn open(
    plan: &Plan,
    process: ProcessHandle,
    scope: &Scope,
    journal: Arc<Journal>,
    db: &Database,
    saved: Option<&str>,
    report: &mut Value,
) -> Result<Session> {
    let settings = Settings {
        read_files: false,
        write_files: false,
        run_commands: false,
        local_actions: false,
        model: plan.model.clone(),
        effort: plan.effort.clone(),
        ..Settings::default()
    };
    report["stage"] = json!("spawn_and_handshake");
    let mut session = Session::spawn(
        process,
        provider_directory(plan),
        &settings,
        None,
        &[
            "web_search=\"disabled\"".into(),
            "features.multi_agent=false".into(),
            "features.apply_patch_freeform=false".into(),
        ],
        plan.mode == "live",
        Some(scope.ticket.context.clone()),
    )?;
    session.bind_turn(Some(scope.ticket.clone()));
    if plan.mode == "live" {
        report["stage"] = json!("authenticate_selected_model");
        session.authenticate_model(&settings)?;
    } else {
        session.model = settings.model.clone();
    }
    session.journal = Some(journal);
    report["stage"] = json!("create_or_resume_thread");
    let thread = session.open_thread(saved, &instructions(&settings, &[]))?;
    let id = thread
        .pointer("/thread/id")
        .and_then(Value::as_str)
        .ok_or("Hilo ausente")?;
    if saved.is_some_and(|old| old != id) {
        return Err("Hilo reanudado distinto".into());
    }
    report["stage"] = json!("record_active_thread");
    save_recorded_thread(db, id, None, session.journal.as_deref())?;
    session.thread_id = id.into();
    Ok(session)
}

fn read(session: &mut Session, id: &str) -> Result<Value> {
    session.send(
        json!({"id":26,"method":"thread/read","params":{"threadId":id,"includeTurns":true}}),
    )?;
    let value = session.response(26, HANDSHAKE_TIMEOUT)?;
    if value.pointer("/thread/id").and_then(Value::as_str) != Some(id)
        || !value.pointer("/thread/turns").is_some_and(Value::is_array)
    {
        return Err("Historial no comprobable".into());
    }
    Ok(value)
}

fn answer(
    plan: &Plan,
    session: &mut Session,
    prompt: &str,
    quote: Option<i64>,
    report: &mut Value,
) -> Result<String> {
    plan.reserve_turn(report)?;
    let mut completed = Vec::new();
    let (result, trace) = super::turn_observation::capture(|| {
        session.speak_stream_reply(prompt, quote, |event| {
            if let StreamEvent::Completed(text) = event {
                completed.push(text);
            }
        })
    });
    if report["turn_protocol"].is_null() {
        report["turn_protocol"] = json!([]);
    }
    let turn_index = report["model_turns"].clone();
    report["turn_protocol"]
        .as_array_mut()
        .ok_or("Traza de sonda inválida")?
        .push(json!({"turn_index":turn_index,"received":trace}));
    result?;
    Ok(completed.join("\n\n"))
}

#[test]
fn turn_observation_survives_probe_answer_error_and_finalization() {
    let plan = Plan {
        version: 2,
        scenario: Scenario::PluginSemantics,
        mode: "live".into(),
        output_dir: std::env::temp_dir(),
        nonce: "a".repeat(32),
        model: "gpt-6-luna".into(),
        effort: "low".into(),
        model_turns_max: 3,
        cell_timeout_seconds: 60,
    };
    let mut report = json!({"model_turns":0,"guide_calls":0});
    let mut session = super::tool_evidence_transport_tests::failed_server_request_session();
    let result = answer(&plan, &mut session, "PRIVATE_INPUT", None, &mut report).map(|_| ());
    assert!(result.is_err());
    diagnostics::finish(&mut report, &result, 1, true);
    assert_eq!(report["failure"]["code"], "unavailable_native_action");
    assert_eq!(report["model_turns"], 1);
    assert_eq!(report["turn_protocol"][0]["turn_index"], 1);
    assert_eq!(
        report["turn_protocol"][0]["received"]["events"][3]["kind"],
        "server_request"
    );
    assert!(!report.to_string().contains("PRIVATE"));
}

#[test]
fn plugin_probe_journal_accepts_tools_through_actual_turn_receiver() {
    let plan = Plan {
        version: 2,
        scenario: Scenario::PluginSemantics,
        mode: "live".into(),
        output_dir: std::env::temp_dir(),
        nonce: "a".repeat(32),
        model: "gpt-6-luna".into(),
        effort: "low".into(),
        model_turns_max: 3,
        cell_timeout_seconds: 60,
    };
    let mut session = super::tool_evidence_transport_tests::successful_tool_session_for_probe();
    let journal = Arc::new(Journal::default());
    session.journal = Some(journal.clone());
    let mut report = json!({"model_turns":0,"guide_calls":0});
    let result = answer(&plan, &mut session, "Consulta ficticia", None, &mut report);
    assert!(
        result.is_ok(),
        "{:?}",
        result.as_ref().err().map(ToString::to_string)
    );
    assert_eq!(result.unwrap(), "Resultado literal");
    let events = journal.tool_events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].outcome, crate::ToolOutcome::Running);
    assert_eq!(events[1].outcome, crate::ToolOutcome::Completed);
    assert!(!format!("{events:?}").contains("PRIVATE"));
    assert!(!report.to_string().contains("PRIVATE"));
}

fn fixture_context(marker: &str) -> crate::context::Context {
    crate::context::Context {
        updates: vec![crate::context::Update {
            message_id: 1,
            text: marker.into(),
            created_at: "2026-10-29T00:00:00Z".into(),
            reference: crate::context::Reference::PluginBuild {
                job_id: 1,
                plugin_id: "probe".into(),
                source_sequence: 1,
            },
        }],
        ..Default::default()
    }
}

fn rollout_text(directory: &Path, id: &str) -> Result<String> {
    fn visit(directory: &Path, id: &str, found: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(&entry.path(), id, found)?;
            } else if kind.is_file()
                && entry.path().extension().is_some_and(|e| e == "jsonl")
                && entry.file_name().to_string_lossy().contains(id)
            {
                found.push(entry.path());
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    visit(directory, id, &mut found)?;
    if found.len() != 1 || std::fs::metadata(&found[0])?.len() > 1024 * 1024 {
        return Err("Rollout privado no comprobable".into());
    }
    Ok(std::fs::read_to_string(&found[0])?)
}

fn exercise(plan: &Plan, report: &mut Value) -> Result<()> {
    if plan.scenario == Scenario::PluginSemantics {
        return plugins::exercise(plan, report);
    }
    let db = Database::memory()?;
    let module = crate::LlmModule::default();
    db.migrate(module.name(), module.migrations())?;
    let journal = Arc::new(Journal {
        quote: format!("Q_{}", plan.nonce),
        ..Journal::default()
    });
    let process = ProcessHandle::default();
    let control = turn::Control::new(process.clone());
    let scope = Scope::new(&control)?;
    report["stage"] = json!("open_isolated_session");
    let mut session = open(
        plan,
        process.clone(),
        &scope,
        journal.clone(),
        &db,
        None,
        report,
    )?;
    let first = session.thread_id.clone();
    if plan.mode == "protocol" {
        let first_marker = format!("QA_FIRST_{}", plan.nonce);
        let second_marker = format!("QA_SECOND_{}", plan.nonce);
        report["stage"] = json!("inject_first_fixture_context");
        session.inject_context(&fixture_context(&first_marker))?;
        report["stage"] = json!("protocol_new_thread");
        session.new_chat(&db)?;
        let second = session.thread_id.clone();
        report["stage"] = json!("inject_second_fixture_context");
        session.inject_context(&fixture_context(&second_marker))?;
        report["stage"] = json!("read_materialized_contexts");
        let one = read(&mut session, &first)?;
        let two = read(&mut session, &second)?;
        if first == second
            || one["thread"]["id"] == two["thread"]["id"]
            || load_thread(&db)?.as_deref() != Some(second.as_str())
        {
            return Err("Protocolo no confirmado".into());
        }
        for (request, id) in [(27, &first), (28, &second)] {
            session.send(
                json!({"id":request,"method":"thread/unsubscribe","params":{"threadId":id}}),
            )?;
            session.response(request, HANDSHAKE_TIMEOUT)?;
        }
        drop(session);
        report["stage"] = json!("verify_separate_private_rollouts");
        let records = provider_directory(plan).join("cli-home/sessions");
        let one = rollout_text(&records, &first)?;
        let two = rollout_text(&records, &second)?;
        if !one.contains(&first_marker)
            || one.contains(&second_marker)
            || !two.contains(&second_marker)
            || two.contains(&first_marker)
        {
            return Err("Contextos de fixture mezclados o no persistidos".into());
        }
        drop(scope);
        let scope = Scope::new(&control)?;
        let mut resumed = open(
            plan,
            process.clone(),
            &scope,
            journal.clone(),
            &db,
            Some(&second),
            report,
        )?;
        report["stage"] = json!("read_resumed_private_thread");
        read(&mut resumed, &second)?;
        resumed
            .send(json!({"id":29,"method":"thread/unsubscribe","params":{"threadId":second}}))?;
        resumed.response(29, HANDSHAKE_TIMEOUT)?;
        drop(resumed);
        drop(scope);
        report["protocol"] = json!({"distinct_threads":true,"generation_turns":0,"local_fixture_contexts_isolated":true,"recorded_active_matches":true,"cold_process_resume_same_thread":true,"account_linked":false});
        db.close()?;
        return Ok(());
    }
    let (second, scope) = if plan.scenario == Scenario::Full {
        report["stage"] = json!("quoted_context");
        let quoted = answer(
            plan,
            &mut session,
            "Devuelve únicamente la clave del mensaje al que estoy respondiendo.",
            Some(7),
            report,
        )?;
        if !quoted.contains(&journal.quote) {
            return Err("La cita no se comprendió".into());
        }
        report["quote"] = json!({"matched":true,"response":quoted});
        drop(scope);
        let scope = Scope::new(&control)?;
        session.bind_turn(Some(scope.ticket.clone()));
        report["stage"] = json!("new_thread_context");
        session.new_chat(&db)?;
        let second = session.thread_id.clone();
        let literal =
            "  Si no hay una clave en ESTA conversación, responde solamente NO_DISPONIBLE.\n";
        let reply = answer(plan, &mut session, literal, None, report)?;
        let history = read(&mut session, &second)?;
        if first == second
            || reply.trim() != "NO_DISPONIBLE"
            || history.to_string().contains(&journal.quote)
            || !journal
                .users
                .lock()
                .map_err(|_| "Diario inválido")?
                .iter()
                .any(|s| s == literal)
        {
            return Err("Aislamiento no comprobado".into());
        }
        report["new_thread"] = json!({"distinct":true,"response":reply,"quote_absent_from_provider_history":true,"literal_journal_input_preserved":true});
        drop(scope);
        (second, Scope::new(&control)?)
    } else {
        (first, scope)
    };
    session.bind_turn(Some(scope.ticket.clone()));
    let timeout = scope.timed_out.clone();
    let cancel = scope.sender.clone();
    report["stage"] = json!("stop_stream");
    plan.reserve_turn(report)?;
    let mut observation = StopObservation::default();
    let result = session.speak_stream_reply("Escribe un relato ficticio de unas 400 palabras sobre una biblioteca. Empieza directamente con el relato y no uses herramientas ni planes.", None, |event| {
        // El callback mantiene el gate; otro hilo procesa la señal de corte.
        observation.observe(event, &cancel);
    });
    drop(scope);
    let cancelled = result
        .as_ref()
        .is_err_and(|e| e.is::<gennie_mcp::Cancelled>());
    let verified = observation.verified(
        cancelled,
        timeout.load(Ordering::Acquire),
        journal
            .interrupted
            .lock()
            .map_err(|_| "Diario inválido")?
            .len(),
    );
    report["stop"] = observation.report(verified, cancelled);
    if !verified {
        return Err("Cancelación no comprobada; no repetir muestra".into());
    }
    drop(session);
    let scope = Scope::new(&control)?;
    report["stage"] = json!("resume_after_stop");
    let mut session = open(plan, process, &scope, journal, &db, Some(&second), report)?;
    let reply = answer(
        plan,
        &mut session,
        "Responde solamente LISTO.",
        None,
        report,
    )?;
    if reply.trim() != "LISTO" {
        return Err("Reanudación no comprobada".into());
    }
    report["resume"] = json!({"same_thread":true,"response":reply});
    drop(session);
    drop(scope);
    db.close()?;
    Ok(())
}

#[test]
#[ignore = "Optativa: protocolo sin cuenta; live requiere plan y aprobación nuevos"]
fn chat_provider_probe() {
    let path = std::env::var_os("GENNIE_TEST_CHAT_PROVIDER_PLAN").expect("Falta plan explícito");
    let plan: Plan = serde_json::from_slice(&std::fs::read(path).expect("Plan ilegible"))
        .expect("Plan inválido");
    plan.validate().expect("Alcance de plan inválido");
    let base = provider_directory(&plan);
    assert!(!base.exists(), "No reutilizar un perfil");
    std::fs::create_dir(&base).unwrap();
    let _directory = PrivateDirectory(base.clone());
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
    let began = Instant::now();
    let mut report = json!({"mode":plan.mode,"scenario":plan.scenario,"model_turns":0,"guide_calls":0,"journal":"in_memory_test_double_not_presence","semantic_quality":null,"usage_reported":null,"effective_monetary_cost":null,"provider_internal_retries":null});
    let result = exercise(&plan, &mut report);
    let profile_removed = std::fs::remove_dir_all(&base).is_ok() && !base.exists();
    diagnostics::finish(
        &mut report,
        &result,
        began.elapsed().as_millis(),
        profile_removed,
    );
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(plan.output_dir.join(format!("{}-result.json", plan.mode)))
        .unwrap();
    serde_json::to_writer_pretty(file, &report).unwrap();
    assert!(
        result.is_ok() && report["profile_removed"] == true,
        "La sonda falló; resultado conservado sin repetir"
    );
}

#[test]
fn stop_resume_probe_requires_its_explicit_two_turn_budget() {
    let value = json!({"version":2,"scenario":"stop_resume","mode":"protocol","output_dir":"/tmp/gennie-fixture","nonce":"a".repeat(32),"model":"gpt-6-luna","effort":"low","model_turns_max":2,"cell_timeout_seconds":60});
    let plan: Plan = serde_json::from_value(value.clone()).unwrap();
    assert!(plan.validate().is_ok());
    for budget in [0, 1, 3, 4] {
        let mut changed = value.clone();
        changed["model_turns_max"] = json!(budget);
        let plan: Plan = serde_json::from_value(changed).unwrap();
        assert!(plan.validate().is_err());
    }
}

#[test]
fn provider_probe_plan_rejects_unbounded_or_unknown_modes() {
    let value = json!({"version":2,"scenario":"full","mode":"protocol","output_dir":"/tmp/gennie-fixture","nonce":"a".repeat(32),"model":"gpt-6-luna","effort":"low","model_turns_max":4,"cell_timeout_seconds":60});
    assert!(
        serde_json::from_value::<Plan>(value.clone())
            .unwrap()
            .validate()
            .is_ok()
    );
    for (key, bad) in [
        ("version", json!(1)),
        ("mode", json!("auto")),
        ("model_turns_max", json!(5)),
        ("cell_timeout_seconds", json!(180)),
        ("nonce", json!("invalid")),
    ] {
        let mut changed = value.clone();
        changed[key] = bad;
        assert!(
            serde_json::from_value::<Plan>(changed)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut changed = value;
    changed["scenario"] = json!("automatic");
    assert!(serde_json::from_value::<Plan>(changed).is_err());
}

#[test]
fn provider_probe_turn_counter_never_exceeds_the_selected_scenario() {
    for (scenario, turns) in [("stop_resume", 2), ("full", 4), ("plugin_semantics", 3)] {
        let plan: Plan = serde_json::from_value(json!({"version":2,"scenario":scenario,"mode":"protocol","output_dir":"/tmp/gennie-fixture","nonce":"c".repeat(32),"model":"gpt-6-luna","effort":"low","model_turns_max":turns,"cell_timeout_seconds":60})).unwrap();
        plan.validate().unwrap();
        let mut report = json!({"model_turns":0});
        for expected in 1..=turns {
            plan.reserve_turn(&mut report).unwrap();
            assert_eq!(report["model_turns"], expected);
        }
        assert!(plan.reserve_turn(&mut report).is_err());
        assert_eq!(report["model_turns"], turns);
        assert!(
            plan.reserve_turn(&mut json!({"model_turns":false}))
                .is_err()
        );
    }
}

#[test]
fn provider_workspace_cannot_inherit_the_report_repository() {
    let plan: Plan = serde_json::from_value(json!({"version":2,"scenario":"full","mode":"protocol","output_dir":std::env::current_dir().unwrap().join(".verification"),"nonce":"b".repeat(32),"model":"gpt-6-luna","effort":"low","model_turns_max":4,"cell_timeout_seconds":60})).unwrap();
    let workspace = provider_directory(&plan);
    assert!(!workspace.starts_with(&plan.output_dir));
    assert_eq!(workspace.parent(), Some(std::env::temp_dir().as_path()));
}

#[test]
fn stop_probe_reports_short_stream_completed_without_stream_and_completion_race_separately() {
    let (send, receive) = mpsc::channel();
    let mut short = StopObservation::default();
    short.observe(StreamEvent::Delta("Breve".into()), &send);
    short.observe(StreamEvent::Completed("Breve".into()), &send);
    let report = short.report(false, false);
    assert_eq!(report["delta_events"], 1);
    assert_eq!(report["total_delta_bytes"], 5);
    assert_eq!(report["max_pending_bytes"], 5);
    assert_eq!(report["completed_characters"], 5);
    assert_eq!(report["observed_characters"], 0);
    assert_eq!(report["cancel_requested"], false);
    assert!(!short.verified(false, false, 0));
    assert!(receive.try_recv().is_err());

    let mut final_only = StopObservation::default();
    final_only.observe(StreamEvent::Completed("Breve".into()), &send);
    let report = final_only.report(false, false);
    assert_eq!(report["delta_events"], 0);
    assert_eq!(report["max_pending_bytes"], 0);
    assert_eq!(report["completed_characters"], 5);
    assert!(!final_only.verified(false, false, 0));

    let mut race = StopObservation::default();
    race.observe(StreamEvent::Delta("x".repeat(64)), &send);
    assert!(matches!(receive.recv().unwrap(), Signal::Cancel));
    race.observe(StreamEvent::Completed("x".repeat(64)), &send);
    let report = race.report(false, false);
    assert_eq!(report["delta_events"], 1);
    assert_eq!(report["total_delta_bytes"], 64);
    assert_eq!(report["max_pending_bytes"], 64);
    assert_eq!(report["completed_characters"], 64);
    assert_eq!(report["cancel_requested"], true);
    assert_eq!(report["observed_characters"], 0);
    assert!(!race.verified(false, false, 0));
}

#[test]
fn stop_probe_preserves_only_the_pending_fragment_after_an_earlier_message() {
    let (send, receive) = mpsc::channel();
    let mut state = StopObservation::default();
    let earlier = "x".repeat(64);
    state.observe(StreamEvent::Delta(earlier.clone()), &send);
    assert!(matches!(receive.recv().unwrap(), Signal::Cancel));
    state.observe(StreamEvent::Completed(earlier), &send);
    state.observe(StreamEvent::Delta("Fragmento actual".into()), &send);
    state.observe(
        StreamEvent::Interrupted {
            text: "Fragmento actual".into(),
            saved: true,
            reason: ReplyInterruption::Cancelled,
        },
        &send,
    );
    assert!(state.verified(true, false, 1));
    assert_eq!(state.completed, 1);
    assert!(receive.try_recv().is_err());
}

#[test]
fn stop_probe_does_not_accept_completed_unsaved_or_unobserved_cancellation() {
    let empty = StopObservation {
        requested: true,
        interrupted: Some((String::new(), true, ReplyInterruption::Cancelled)),
        ..Default::default()
    };
    assert!(!empty.verified(true, false, 1));
    let (send, _) = mpsc::channel();
    let mut state = StopObservation::default();
    state.observe(StreamEvent::Delta("x".repeat(64)), &send);
    assert!(!state.verified(false, false, 0));
    state.requested = true;
    state.observe(
        StreamEvent::Interrupted {
            text: "x".repeat(64),
            saved: false,
            reason: ReplyInterruption::Cancelled,
        },
        &send,
    );
    assert!(!state.verified(true, false, 0));
    state.observe(
        StreamEvent::Interrupted {
            text: "x".repeat(64),
            saved: true,
            reason: ReplyInterruption::Cancelled,
        },
        &send,
    );
    assert!(!state.verified(true, true, 1));
    assert!(!state.verified(false, false, 1));
}
