//! Sonda optativa del recorrido real; protocolo sin clave antes de cualquier modelo.
use super::*;
use gennie_llm::architects::{self, Role};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Write, os::unix::fs::OpenOptionsExt, time::Instant};

const CORPUS: &str = include_str!("../../tests/fixtures/contextual-notices-v1.json");
const OTHER: &str = "OTHER_SESSION_SENTINEL_DO_NOT_SHARE";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u32,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    request: String,
    observation: String,
    guidance: String,
    expected_action: String,
    baseline_changed: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    version: u32,
    run_id: String,
    mode: String,
    output_dir: PathBuf,
    model: String,
    effort: String,
    guide_model: String,
    guide_environment: PathBuf,
    maximum_cases: usize,
    case_timeout_seconds: u64,
    total_timeout_seconds: u64,
}
fn write(path: &std::path::Path, value: &Value) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn seed(backend: &Backend, case: &Case) -> Result<(i64, i64, String, Value)> {
    let origin_id = append_conversation(backend, "notice-origin", &case.request).parse::<i64>()?;
    let origin = gennie_presence::delegation::origin(&backend.state.db, origin_id)?
        .ok_or("Falta origen del fixture")?;
    let active = append_conversation(backend, "notice-other", OTHER).parse::<i64>()?;
    let document = json!({"title":"Solicitud ficticia","description":case.request,"kind":"goal","priority":"normal","status":"active","timezone":"America/Bogota","preparation":{"interval_minutes":30,"rationale":"Revisar solo la fuente ficticia"},"follow_up":{"scope":"Datos ficticios de esta solicitud","signals":[],"decision_guidance":case.guidance},"method":{"plugin_id":"fixture","functions":["read"]},"authorization":{"scope":"Solo lectura ficticia; sin enviar ni redactar","allowed_routes":["http"]}});
    let assignment = backend
        .assignments
        .create("notice-probe", serde_json::from_value(document)?)?;
    let destination = gennie_assignments::execution::notices::Delivery {
        conversation_id: origin_id,
        bot_id: origin.bot_id,
        epoch: origin.epoch,
        provider_thread: origin.provider_thread,
    };
    let policy = json!({"contextual_notice":true,"assignment":{"id":assignment.id,"instance_id":assignment.instance_id,"revision":assignment.revision,"document":assignment.document},"trigger":"preparation","calendar":{"kind":"interval","anchor":1793278800,"seconds":1800},"target":{"kind":"web","plugin_id":"fixture","function":"read","revision":1},"arguments":{},"route":"http","misfire":"skip_missed","max_lateness_minutes":5,"delivery":destination});
    let notice = gennie_assignments::execution::notices::Notice {
        event_key: "assignment-execution:1".into(),
        destination: destination.clone(),
        assignment_id: assignment.id,
        instance_id: assignment.instance_id.clone(),
        policy_id: "notice-policy".into(),
        policy_revision: 1,
        run_id: 1,
        title: assignment.document.title.clone(),
        scheduled_at: "2026-10-30T13:00:00Z".into(),
        state: "succeeded".into(),
        code: "reported_success".into(),
        observation: Some(case.observation.clone()),
        observation_truncated: false,
        guidance: Some(gennie_assignments::execution::notices::Guidance {
            objective: case.request.clone(),
            scope: "Datos ficticios de esta solicitud".into(),
            signals: vec![],
            decision_guidance: case.guidance.clone(),
        }),
    };
    notice.validate()?;
    let policy_text = policy.to_string();
    backend.state.db.with_connection(|c| {
        let tx=c.transaction()?;
        tx.execute("INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state) VALUES('notice-policy',?1,?2,'preparation',1,0,'paused')",gennie_db::params![assignment.id,assignment.instance_id])?;
        tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES('notice-policy',1,?1)",[&policy_text])?;
        tx.execute("INSERT INTO assignment_execution_runs(id,policy_id,policy_revision,scheduled_at,state,code,output) VALUES(1,'notice-policy',1,'2026-10-30T13:00:00Z','succeeded','reported_success','{\"fictitious\":true}')",[])?;
        tx.execute("INSERT INTO assignment_execution_notices(run_id,document) VALUES(1,?1)",[serde_json::to_string(&notice)?])?;
        tx.execute("UPDATE modules SET active=1 WHERE name IN ('assignments','llm','proactivity')",[])?;
        tx.commit()?; Ok(())
    })?;
    if !backend
        .assignments
        .contextual_notice_permitted(1, &destination)?
    {
        return Err("El fixture no conserva consentimiento válido".into());
    }
    let mut bot = architects::load(&backend.state.db, Role::Bot)?;
    bot.model = "gpt-6-luna".into();
    bot.effort = Some("low".into());
    bot.read_files = false;
    bot.write_files = false;
    bot.run_commands = false;
    bot.local_actions = false;
    architects::save(&backend.state.db, &bot)?;
    let mut guide = architects::load(&backend.state.db, Role::Guide)?;
    guide.enabled = true;
    guide.model = "openai/gpt-6-luna-decisions".into();
    architects::save(&backend.state.db, &guide)?;
    Ok((
        origin_id,
        active,
        policy_text,
        serde_json::to_value(&assignment.document)?,
    ))
}
fn observe(
    backend: &Backend,
    origin: i64,
    active: i64,
    policy: &str,
    document: &Value,
) -> Result<Value> {
    let decision = gennie_llm::notices::decision_details(&backend.state.db, 1)?;
    let page = gennie_presence::assistants::page(&backend.state.db, "gennie", None, 60, 24000)?;
    let messages: Vec<_> = page
        .messages
        .iter()
        .filter(|m| m.execution.is_some())
        .map(|m| json!({"conversation_id":m.conversation_id,"text":m.text,"speaker":m.speaker}))
        .collect();
    let state=backend.state.db.with_connection(|c| {
        let counts=c.query_row("SELECT (SELECT count(*) FROM llm_execution_notices WHERE render_attempted_at IS NOT NULL),(SELECT count(*) FROM llm_execution_notices WHERE guide_attempted_at IS NOT NULL),(SELECT count(*) FROM assignment_execution_notices WHERE state='accepted'),(SELECT count(*) FROM plugin_build_jobs)+(SELECT count(*) FROM plugin_mcp_attempts)+(SELECT count(*) FROM browser_web_attempts),(SELECT count(*) FROM codex_conversation)",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?)))?;
        let (saved,enabled,state):(String,bool,String)=c.query_row("SELECT v.document,p.enabled,p.state FROM assignment_execution_versions v JOIN assignment_execution_policies p ON v.policy_id=p.id WHERE v.policy_id='notice-policy' AND v.revision=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let integrity:String=c.query_row("PRAGMA integrity_check",[],|r|r.get(0))?;
        let foreign_keys=c.prepare("PRAGMA foreign_key_check")?.query([])?.next()?.is_none();
        Ok(json!({"bot_attempts":counts.0,"guide_attempts":counts.1,"outbox_accepted":counts.2,"effect_attempts":counts.3,"conversation_provider_rows":counts.4,"policy_unchanged":saved==policy&&!enabled&&state=="paused","integrity":integrity,"foreign_keys_ok":foreign_keys}))
    })?;
    let assignment = backend.assignments.get(1)?;
    let source_ok = messages.iter().all(|m| {
        m["conversation_id"] == origin
            && m["speaker"] == "assistant"
            && !m["text"].as_str().unwrap_or_default().contains(OTHER)
    });
    Ok(
        json!({"decision":decision,"messages":messages,"state":state,"origin_ok":source_ok,"selection_unchanged":history::active_conversation(&backend.state.db)?==Some(active),"assignment_unchanged":assignment.revision==1&&serde_json::to_value(assignment.document)?==*document}),
    )
}
fn one(plan: &Plan, case: &Case) -> Result<Value> {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let (origin, active, policy, document) = seed(&backend, case)?;
    backend.close()?;
    let secrets = if plan.mode == "live" {
        gennie_config::Secrets::load(&plan.guide_environment)?
    } else {
        gennie_config::Secrets::default()
    };
    if plan.mode == "live" && secrets.openrouter_api_key.is_none() {
        return Err("Falta clave de guía; no se inicia el caso".into());
    }
    let helper = std::env::current_exe()?;
    let began = Instant::now();
    let mut backend = Backend::open_config_with_secrets(
        fixture.config.clone(),
        helper.to_str().ok_or("Ruta no UTF-8")?,
        secrets,
    )?;
    let socket = backend.mcp.as_ref().map(|m| m.launch().socket);
    let mut row;
    loop {
        row = observe(&backend, origin, active, &policy, &document)?;
        if row["decision"]["action"].is_string()
            && matches!(
                row["decision"]["state"].as_str(),
                Some("delivered" | "discarded")
            )
        {
            break;
        }
        if began.elapsed() > Duration::from_secs(plan.case_timeout_seconds) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let elapsed = began.elapsed().as_millis();
    backend.close()?;
    let socket_removed = socket.is_none_or(|p| !p.exists());
    // Reabrir la misma DB no debe repetir guía, redacción o entrega.
    let mut reopened = fixture.open();
    let after = observe(&reopened, origin, active, &policy, &document)?;
    reopened.close()?;
    let reopen_same = after == row;
    let expected = if plan.mode == "protocol" {
        "record"
    } else {
        &case.expected_action
    };
    let action = row["decision"]["action"]
        .as_str()
        .unwrap_or("missing")
        .to_owned();
    let delivered = row["messages"].as_array().map_or(0, Vec::len);
    let phase_ok = if plan.mode == "protocol" {
        row["decision"]["guide_state"] == "unavailable"
            && row["state"]["bot_attempts"] == 0
            && row["state"]["guide_attempts"] == 0
            && delivered == 0
    } else {
        row["decision"]["guide_state"] == "ready"
            && row["decision"]["failure_code"].is_null()
            && row["state"]["bot_attempts"] == 1
            && row["state"]["guide_attempts"] == 1
            && delivered == usize::from(action != "record")
    };
    let checks_ok = phase_ok
        && row["origin_ok"] == true
        && row["selection_unchanged"] == true
        && row["assignment_unchanged"] == true
        && row["state"]["policy_unchanged"] == true
        && row["state"]["effect_attempts"] == 0
        && row["state"]["conversation_provider_rows"] == 0
        && row["state"]["outbox_accepted"] == 1
        && row["state"]["integrity"] == "ok"
        && row["state"]["foreign_keys_ok"] == true
        && reopen_same
        && socket_removed;
    row["case_id"] = json!(case.id);
    row["elapsed_ms"] = json!(elapsed);
    row["checks_ok"] = json!(checks_ok);
    row["expected_action"] = json!(expected);
    row["action_matches"] = json!(action == expected);
    row["reopen_same"] = json!(reopen_same);
    row["socket_removed"] = json!(socket_removed);
    row["baseline_action"] = json!(if case.baseline_changed {
        "notify"
    } else {
        "record"
    });
    row["complete_monetary_cost"] = Value::Null;
    drop(reopened);
    drop(backend);
    let directory = fixture.directory.clone();
    drop(fixture);
    row["fixture_removed"] = json!(!directory.exists());
    Ok(row)
}
fn run() -> Result<()> {
    let path = std::env::var_os("GENNIE_NOTICE_PROBE_PLAN").ok_or("Falta plan explícito")?;
    let plan: Plan = serde_json::from_slice(&std::fs::read(path)?)?;
    let corpus: Corpus = serde_json::from_str(CORPUS)?;
    let expected_ids = [
        "unchanged",
        "useful_change",
        "essential_question",
        "concrete_help",
    ];
    if plan.version != 1
        || corpus.version != 1
        || plan.maximum_cases != 4
        || corpus.cases.len() != 4
        || plan.model != "gpt-6-luna"
        || plan.effort != "low"
        || plan.guide_model != "openai/gpt-6-luna-decisions"
        || plan.case_timeout_seconds != 180
        || plan.total_timeout_seconds != 720
        || !["protocol", "live"].contains(&plan.mode.as_str())
        || corpus.cases.iter().map(|c| c.id.as_str()).ne(expected_ids)
    {
        return Err("Plan fuera de los límites fijados".into());
    }
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("Falta raíz")?
        .join(".verification/contextual-notice-probe")
        .canonicalize()?;
    let output = plan.output_dir.canonicalize()?;
    if output.parent() != Some(base.as_path())
        || output.file_name().and_then(|n| n.to_str()) != Some(plan.run_id.as_str())
        || std::env::var("GENNIE_NOTICE_PROBE_ACK").ok().as_deref() != Some(&plan.run_id)
    {
        return Err("Identidad/acuse del plan no válidos".into());
    }
    if plan.mode == "live" {
        let protocol: Value =
            serde_json::from_slice(&std::fs::read(output.join("protocol-summary.json"))?)?;
        if protocol["status"] != "completed_protocol" {
            return Err("El protocolo previo no pasó".into());
        }
    }
    write(
        &output.join(format!("{}-claim.json", plan.mode)),
        &json!({"mode":plan.mode,"run_id":plan.run_id,"maximum_cases":4,"case_timeout_seconds":180,"total_timeout_seconds":720}),
    )?;
    let began = Instant::now();
    let mut rows = Vec::new();
    let mut completed = true;
    for case in &corpus.cases {
        if began.elapsed().as_secs() + plan.case_timeout_seconds > plan.total_timeout_seconds {
            completed = false;
            break;
        }
        let row = one(&plan, case)?;
        write(
            &output.join(format!("{}-{}.json", plan.mode, case.id)),
            &row,
        )?;
        let ok = row["checks_ok"] == true && row["fixture_removed"] == true;
        rows.push(row);
        if !ok {
            completed = false;
            break;
        }
    }
    write(
        &output.join(format!("{}-summary.json", plan.mode)),
        &json!({"status":if completed&&rows.len()==4 {format!("completed_{}",plan.mode)} else {"stopped_technical_failure".into()},"cases":rows,"elapsed_ms":began.elapsed().as_millis(),"test_binary":std::env::current_exe()?,"full_monetary_cost":null,"human_ratings":null}),
    )?;
    if !completed {
        return Err("La sonda se detuvo; revisar resultados guardados sin reintentar".into());
    }
    Ok(())
}
#[test]
#[ignore = "Sonda optativa: protocolo sin cuenta primero; live consume hasta4 turnos y4 guía con plan/acuse explícitos"]
fn planned_contextual_notices_reach_the_origin_without_effects() {
    run().expect("Fallo de la sonda acotada de avisos ficticios");
}
