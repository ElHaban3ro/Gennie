//! Runner optativo; offline por defecto, proveedor aislado solo con un plan live explícito.
use super::*;
use std::io::Write;
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    version: u32,
    mode: String,
    output_dir: PathBuf,
    cases: Vec<String>,
    strategies: Vec<Strategy>,
    representations: Vec<Representation>,
    repetitions: u32,
    model: String,
    effort: String,
    cell_timeout_seconds: u64,
    total_timeout_seconds: u64,
    helper: PathBuf,
    worker: PathBuf,
    guide_environment: Option<PathBuf>,
    metadata: Value,
}
fn output(path: &std::path::Path, value: &Value) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}
fn guide(
    plan: &Plan,
    cell: &Cell,
) -> Result<(
    Option<gennie_proactivity::ProactivityModule>,
    Option<gennie_proactivity::Controller>,
)> {
    let path = plan
        .guide_environment
        .clone()
        .unwrap_or_else(|| plan.output_dir.join("no-env-file"));
    let secrets = gennie_config::Secrets::load(&path)?;
    if secrets.openrouter_api_key.is_none() {
        return Ok((None, None));
    }
    let module = gennie_proactivity::ProactivityModule::with_guide(
        secrets.openrouter_api_key,
        Arc::new(|| {
            Ok(gennie_proactivity::GuideSettings {
                revision: 1,
                enabled: true,
                model: gennie_proactivity::Model::OpenaiDecisions,
                personality: String::new(),
                instructions: String::new(),
            })
        }),
    );
    let state = AppState {
        config: Arc::new(Config {
            database_path: plan.output_dir.join("unused.sqlite"),
            db_busy_timeout_ms: 500,
        }),
        db: cell.db.clone(),
        shutdown: Default::default(),
    };
    module.start(&state)?;
    let controller = module.controller();
    Ok((Some(module), Some(controller)))
}

fn model_row(
    corpus: &Corpus,
    case: &Case,
    representation: Representation,
    strategy: Strategy,
    plan: &Plan,
    sequence: usize,
    budget_seconds: u64,
) -> Result<Value> {
    let protocol = plan.mode == "protocol";
    let started = Instant::now();
    let fixture = Fixture::new();
    let with_guide = strategy == Strategy::BotGuide && !protocol;
    let cell = setup(corpus, case, with_guide)?;
    let (guide_module, controller) = if with_guide {
        guide(plan, &cell)?
    } else {
        (None, None)
    };
    if with_guide && controller.is_none() {
        cell.db.close()?;
        return Ok(
            json!({"case_id":case.id,"strategy":strategy,"representation":representation,"status":"guide_unavailable","model_turns":0,"guide_calls":0,"cost_reported":null,"fixture_removed":remove_fixture(&fixture)}),
        );
    }
    let guide_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = guide_calls.clone();
    let advice = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let advice_log = advice.clone();
    let evaluator: gennie_assignments::proposals::GuideEvaluator = Arc::new(
        move |mut state, questions| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            project_payload(&mut state, representation)?;
            let question_keys = questions
                .as_object()
                .ok_or("Preguntas no válidas")?
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            let began = Instant::now();
            let result = controller
                .as_ref()
                .ok_or("Guía no consentida")?
                .evaluate(state, questions);
            let reported = result.as_ref().ok();
            let probabilities = question_keys
                .iter()
                .map(|name| {
                    (
                        name.clone(),
                        reported
                            .and_then(|r| r["answers"][name]["noul"].as_f64())
                            .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
                            .map_or(Value::Null, |v| json!(v)),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            advice_log.lock().map_err(|_|"Diario inválido")?.push(json!({"elapsed_ms":began.elapsed().as_millis(),"status":if result.is_ok(){"completed"}else{"failed"},"probabilities":probabilities,"cost_reported":reported.and_then(|r|r["usage"]["cost"].as_f64()).filter(|n|n.is_finite() && *n>=0.0),"model_requested":"openai/gpt-6-luna-decisions"}));
            result
        },
    );
    let origin = cell.origin.clone();
    let turn = case.request.clone();
    let clock = corpus.clock.clone();
    let turn_origin = origin.clone();
    let mut tools = gennie_assignments::proposals::tools(
        cell.store.clone(),
        Arc::new(move || Ok(origin.clone())),
        evaluator,
        Arc::new(move || {
            Ok(gennie_assignments::proposals::BotTurn {
                origin: turn_origin.clone(),
                message_id: 1,
                text: turn.clone(),
                truncated: false,
                created_at: clock.clone(),
            })
        }),
    );
    tools.retain(|tool| {
        matches!(
            tool.definition["name"].as_str(),
            Some("assignments_review_get" | "assignments_propose")
        )
    });
    let trace = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    for tool in &mut tools {
        let prior = std::mem::replace(&mut tool.call, Box::new(|_| Err("No preparado".into())));
        let name = tool.definition["name"]
            .as_str()
            .ok_or("Herramienta sin nombre")?
            .to_string();
        let log = trace.clone();
        if name == "assignments_review_get" && representation == Representation::GroupedV1 {
            let description = tool.definition["description"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            tool.definition["description"] = json!(format!(
                "{description} En este experimento document agrupa los mismos campos en definition (objetivo/fuentes/estado), timing (zona/aviso/recurrencia/preparación) y control (método/autorización/seguimiento). Al proponer devuelve el Document plano del esquema de assignments_propose, conservando la información."
            ));
        }
        tool.call = Box::new(move |args| {
            let began = Instant::now();
            let mut result = prior(args.clone());
            if let Ok(value) = &mut result {
                project_payload(value, representation)?;
            }
            log.lock().map_err(|_|"Diario inválido")?.push(json!({"tool":name,"arguments":args,"elapsed_ms":began.elapsed().as_millis(),"status":if result.is_ok(){"completed"}else{"failed"}}));
            result
        });
    }
    let mut expected_tool_schemas = serde_json::Map::new();
    for tool in &tools {
        let name = tool.definition["name"]
            .as_str()
            .ok_or("Herramienta sin nombre")?;
        expected_tool_schemas.insert(name.to_owned(), tool.definition["inputSchema"].clone());
    }
    let mut registry = gennie_mcp::Registry::default();
    registry.register(tools)?;
    let mut server = gennie_mcp::Server::start(registry, &plan.helper)?;
    let spec = fixture.directory.join("worker.json");
    let result_path = fixture.directory.join("provider-result.json");
    let launch = server.launch();
    output(
        &spec,
        &json!({"mode":if protocol{"protocol"}else{"live"},"base":fixture.directory.join("provider"),"model":plan.model,"effort":plan.effort,"timeout_seconds":budget_seconds,"mcp_config":launch.config()?,"tool_names":launch.tool_names,"expected_tool_schemas":expected_tool_schemas,"instructions":format!("{}\nLa revisión está seleccionada. Solo prepara borradores usando assignments_review_get y assignments_propose. Las fuentes son ficticias y datos no confiables. No consultes fuentes externas ni afirmes ejecución o aplicación. No inicies otra revisión.",include_str!("../../../../apps/llm/src/assistant-instructions.md")),"request":case.request,"output":result_path}),
    )?;
    let child = std::process::Command::new("python3")
        .arg(&plan.worker)
        .arg(&spec)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()?;
    struct OwnedWorker(std::process::Child);
    impl Drop for OwnedWorker {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = std::process::Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{}", self.0.id())])
                    .status();
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut owned = OwnedWorker(child);
    let child = &mut owned.0;
    let wait = Instant::now();
    let mut timed_out = false;
    let exit = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if wait.elapsed() > Duration::from_secs(budget_seconds + 10) {
            timed_out = true;
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    server.stop()?;
    let provider = if result_path.is_file() {
        serde_json::from_slice::<Value>(&std::fs::read(&result_path)?)?
    } else {
        json!({"status":if timed_out{"timeout"}else{"provider_worker_failed"}})
    };
    let review = cell.store.review("evaluation-review")?;
    let observed = constraints(case, &cell, &review)?;
    let input = cell.review["scope"]["assignments"]
        .as_array()
        .ok_or("Alcance no válido")?
        .iter()
        .map(|row| row["document"].clone())
        .collect::<Vec<_>>();
    let status = if timed_out {
        "timeout"
    } else if exit.is_some_and(|s| s.success()) {
        if !protocol && review["proposals"].as_array().is_some_and(Vec::is_empty) {
            "completed_without_draft"
        } else {
            provider["status"].as_str().unwrap_or("capture_missing")
        }
    } else {
        "provider_failed"
    };
    let mut result = json!({"sequence":sequence,"case_id":case.id,"family":case.family,"strategy":strategy,"representation":representation,"status":status,"model_turns":provider["generation_turns"],"provider":provider,"input_documents":input,"proposals":review["proposals"],"review_state":review["state"],"guide":review["guide"],"guide_trace":advice.lock().map_err(|_|"Diario inválido")?.clone(),"tool_trace":trace.lock().map_err(|_|"Diario inválido")?.clone(),"machine":observed,"cell_elapsed_ms":started.elapsed().as_millis(),"guide_calls":guide_calls.load(std::sync::atomic::Ordering::SeqCst),"cost_reported":null,"human_review":"pending"});
    if let Some(module) = guide_module {
        module.stop(&AppState {
            config: Arc::new(fixture.config.clone()),
            db: cell.db.clone(),
            shutdown: Default::default(),
        })?;
    }
    cell.db.close()?;
    result["fixture_removed"] = json!(remove_fixture(&fixture));
    Ok(result)
}

fn remove_fixture(fixture: &Fixture) -> bool {
    std::fs::remove_dir_all(&fixture.directory).is_ok() && !fixture.directory.exists()
}

fn model_cell_verified(row: &Value, strategy: Strategy) -> bool {
    row["status"] == "completed"
        && row["model_turns"] == 1
        && row["review_state"] == "ready"
        && row["proposals"].as_array().is_some_and(|v| !v.is_empty())
        && row["machine"]["checks"]
            .as_object()
            .is_some_and(|checks| !checks.is_empty() && checks.values().all(|v| v == &json!(true)))
        && match strategy {
            Strategy::Bot => row["guide_calls"] == 0,
            Strategy::BotGuide => {
                row["guide_calls"] == 1
                    && row["guide_trace"].as_array().is_some_and(|v| v.len() == 1)
                    && row["guide_trace"][0]["status"] == "completed"
            }
            Strategy::LocalRules => false,
        }
}

// Una abstención/rechazo semántico completo sigue siendo una muestra; una celda
// técnica incompleta o con consumo incierto no permite iniciar otra.
fn model_cell_completed(row: &Value, strategy: Strategy) -> bool {
    matches!(
        row["status"].as_str(),
        Some("completed" | "completed_without_draft")
    ) && row["provider"]["status"] == "completed"
        && row["provider"]["generation_turns"] == 1
        && row["model_turns"] == 1
        && row["fixture_removed"] == true
        && row["machine"]["checks"]["plans_unchanged"] == true
        && row["guide_calls"].as_u64().is_some_and(|count| {
            count <= u64::from(strategy == Strategy::BotGuide)
                && row["guide_trace"].as_array().is_some_and(|trace| {
                    trace.len() as u64 == count
                        && trace.iter().all(|entry| entry["status"] == "completed")
                })
        })
}

// Punto único que decide si se permite iniciar otra celda.
struct CellSequence {
    live: bool,
    stop_on_failure: bool,
    halted: Option<&'static str>,
}
impl CellSequence {
    fn next(&mut self, strategy: Strategy, execute: impl FnOnce() -> Value) -> Value {
        if let Some(status) = self.halted {
            return json!({"status":status,"model_turns":0,"guide_calls":0});
        }
        let row = execute();
        if self.live
            && strategy != Strategy::LocalRules
            && row["status"] != "not_run_budget"
            && !model_cell_completed(&row, strategy)
        {
            self.halted = Some("not_run_after_technical_failure");
        } else if self.stop_on_failure
            && strategy != Strategy::LocalRules
            && !model_cell_verified(&row, strategy)
        {
            self.halted = Some("not_run_after_model_failure");
        }
        row
    }
}

#[test]
fn wide_evaluation_stops_before_later_calls_on_technical_failure() {
    for failure in [
        json!({"status":"provider_failed","model_turns":0,"provider":{"status":"model_or_effort_unavailable"}}),
        json!({"status":"provider_failed","model_turns":null,"provider":{"status":"provider_worker_failed"}}),
        json!({"status":"completed_without_draft","model_turns":1,"provider":{"status":"completed","generation_turns":1},"guide_calls":0,"guide_trace":[],"fixture_removed":false}),
    ] {
        let mut sequence = CellSequence {
            live: true,
            stop_on_failure: false,
            halted: None,
        };
        let mut calls = 0;
        let rows: Vec<_> = [Strategy::Bot, Strategy::BotGuide, Strategy::Bot]
            .into_iter()
            .map(|strategy| {
                sequence.next(strategy, || {
                    calls += 1;
                    failure.clone()
                })
            })
            .collect();
        assert_eq!(calls, 1, "No debe iniciarse otra celda tras fallo técnico");
        assert_eq!(rows[0], failure);
        for row in &rows[1..] {
            assert_eq!(row["status"], "not_run_after_technical_failure");
            assert_eq!(row["model_turns"], 0);
            assert_eq!(row["guide_calls"], 0);
        }
    }
}

#[test]
fn wide_evaluation_retains_semantic_failure_and_continues() {
    let failure = json!({"status":"completed_without_draft","model_turns":1,
        "provider":{"status":"completed","generation_turns":1},"guide_calls":0,
        "guide_trace":[],"fixture_removed":true,"proposals":[],
        "machine":{"checks":{"plans_unchanged":true,"questions_required":null}}});
    for brief in [false, true] {
        let mut sequence = CellSequence {
            live: true,
            stop_on_failure: brief,
            halted: None,
        };
        let mut calls = 0;
        let rows: Vec<_> = [Strategy::Bot, Strategy::BotGuide]
            .into_iter()
            .map(|strategy| {
                sequence.next(strategy, || {
                    calls += 1;
                    failure.clone()
                })
            })
            .collect();
        assert_eq!(rows[0], failure);
        assert_eq!(calls, if brief { 1 } else { 2 });
        if brief {
            assert_eq!(rows[1]["status"], "not_run_after_model_failure");
        }
    }
}

#[test]
fn wide_evaluation_rejects_unknown_counts_failed_guide_or_changed_inputs() {
    let complete = json!({"status":"completed","model_turns":1,
        "provider":{"status":"completed","generation_turns":1},"fixture_removed":true,
        "guide_calls":1,"guide_trace":[{"status":"completed"}],
        "machine":{"checks":{"plans_unchanged":true}}});
    assert!(model_cell_completed(&complete, Strategy::BotGuide));
    for (pointer, value) in [
        ("/model_turns", Value::Null),
        ("/provider/generation_turns", json!(0)),
        ("/provider/status", json!("generation_not_completed")),
        ("/guide_calls", Value::Null),
        ("/guide_calls", json!(2)),
        ("/guide_trace", json!([])),
        ("/guide_trace/0/status", json!("failed")),
        ("/machine/checks/plans_unchanged", json!(false)),
        ("/fixture_removed", Value::Null),
    ] {
        let mut failure = complete.clone();
        *failure.pointer_mut(pointer).unwrap() = value;
        let mut sequence = CellSequence {
            live: true,
            stop_on_failure: false,
            halted: None,
        };
        assert_eq!(
            sequence.next(Strategy::BotGuide, || failure.clone()),
            failure
        );
        assert_eq!(
            sequence.next(Strategy::Bot, || panic!("No debe iniciar proveedor"))["status"],
            "not_run_after_technical_failure"
        );
    }
    assert!(!model_cell_completed(&complete, Strategy::Bot));
}

#[test]
fn evaluation_protocol_and_local_controls_do_not_require_generation() {
    for (live, strategy) in [(false, Strategy::Bot), (true, Strategy::LocalRules)] {
        let mut sequence = CellSequence {
            live,
            stop_on_failure: false,
            halted: None,
        };
        let mut calls = 0;
        for _ in 0..2 {
            sequence.next(strategy, || {
                calls += 1;
                json!({"status":"protocol_verified","model_turns":0})
            });
        }
        assert_eq!(calls, 2);
    }
    let fixture = Fixture::new();
    assert!(remove_fixture(&fixture));
    assert!(!fixture.directory.exists());
}

#[test]
fn brief_evaluation_stops_on_missing_drafts_unknown_checks_or_unconfirmed_guide() {
    let row = json!({"status":"completed","model_turns":1,"review_state":"ready",
        "proposals":[{}],"machine":{"checks":{"plans_unchanged":true}},"guide_calls":0});
    assert!(model_cell_verified(&row, Strategy::Bot));
    for patch in [
        json!({"status":"completed_without_draft"}),
        json!({"proposals":[]}),
        json!({"machine":{"checks":{"plans_unchanged":null}}}),
        json!({"machine":{"checks":{"plans_unchanged":false}}}),
        json!({"machine":{"checks":{}}}),
        json!({"model_turns":null}),
    ] {
        let mut invalid = row.clone();
        invalid
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(!model_cell_verified(&invalid, Strategy::Bot));
    }
    let mut guided = row;
    guided["guide_calls"] = json!(1);
    guided["guide_trace"] = json!([{"status":"completed"}]);
    assert!(model_cell_verified(&guided, Strategy::BotGuide));
    guided["review_state"] = json!("guide_failed");
    assert!(!model_cell_verified(&guided, Strategy::BotGuide));
    guided["review_state"] = json!("ready");
    guided["guide_trace"][0]["status"] = json!("failed");
    assert!(!model_cell_verified(&guided, Strategy::BotGuide));
}

#[test]
#[ignore = "Runner manual: sin modelos en offline/protocol; live requiere plan concreto aprobado"]
fn assignment_evaluation_run() {
    let path = std::env::var_os("GENNIE_TEST_EVALUATION_PLAN")
        .expect("Falta plan de evaluación explícito");
    let plan: Plan = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(plan.version, 1);
    assert!(["offline", "protocol", "live"].contains(&plan.mode.as_str()));
    assert!((1..=3).contains(&plan.repetitions));
    assert!((10..=120).contains(&plan.cell_timeout_seconds));
    assert!((30..=3600).contains(&plan.total_timeout_seconds));
    assert!(
        !plan.cases.is_empty() && !plan.strategies.is_empty() && !plan.representations.is_empty()
    );
    assert!(
        plan.strategies
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<BTreeSet<_>>()
            .len()
            == plan.strategies.len()
    );
    assert!(
        plan.representations
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<BTreeSet<_>>()
            .len()
            == plan.representations.len()
    );
    if plan.mode == "offline" {
        assert_eq!(plan.strategies, vec![Strategy::LocalRules]);
    }
    let corpus: Corpus = serde_json::from_str(CORPUS).unwrap();
    let selected: Vec<_> = plan
        .cases
        .iter()
        .map(|id| {
            corpus
                .cases
                .iter()
                .find(|case| &case.id == id)
                .expect("Caso desconocido")
        })
        .collect();
    assert!(plan.cases.iter().collect::<BTreeSet<_>>().len() == plan.cases.len());
    for case in &corpus.cases {
        let cell = setup(&corpus, case, false).unwrap();
        constraints(case, &cell, &cell.review).unwrap();
        for record in &cell.records {
            let flat = serde_json::to_value(&record.document).unwrap();
            assert_eq!(
                ungroup(&project(&flat, Representation::GroupedV1).unwrap()).unwrap(),
                flat
            );
        }
        cell.db.close().unwrap();
    }
    let filename = match plan.mode.as_str() {
        "offline" => "baseline.jsonl",
        "protocol" => "protocol.jsonl",
        _ => "live.jsonl",
    };
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(plan.output_dir.join(filename))
        .unwrap();
    let mut append = |value: &Value| {
        serde_json::to_writer(&mut file, value).unwrap();
        file.write_all(b"\n").unwrap();
        file.flush().unwrap();
        file.sync_data().unwrap();
    };
    let started = Instant::now();
    let mut sequence = 0;
    let stop_on_failure = plan.mode == "live" && plan.metadata["stop_on_failed_model_cell"] == true;
    let mut cells = CellSequence {
        live: plan.mode == "live",
        stop_on_failure,
        halted: None,
    };
    for repetition in 0..plan.repetitions {
        for (index, case) in selected.iter().enumerate() {
            if case.family == "contract" {
                append(&contract_row(&corpus, case).unwrap());
                continue;
            }
            let mut representations = plan.representations.clone();
            if (index + repetition as usize) % 2 == 1 {
                representations.reverse();
            }
            for representation in representations {
                for strategy in &plan.strategies {
                    sequence += 1;
                    let mut row = cells.next(*strategy, || { if started.elapsed() > Duration::from_secs(plan.total_timeout_seconds) {
                        json!({"case_id":case.id,"strategy":strategy,"representation":representation,"status":"not_run_budget","model_turns":0,"guide_calls":0})
                    } else if plan.mode == "offline" || *strategy == Strategy::LocalRules {
                        local_row(&corpus, case, representation).unwrap()
                    } else {
                        let remaining = plan
                            .total_timeout_seconds
                            .saturating_sub(started.elapsed().as_secs())
                            .max(1);
                        model_row(&corpus,case,representation,*strategy,&plan,sequence,plan.cell_timeout_seconds.min(remaining)).unwrap_or_else(|_|json!({"case_id":case.id,"strategy":strategy,"representation":representation,"status":"harness_failed","model_turns":null,"cost_reported":null}))
                    }});
                    row["case_id"] = json!(case.id);
                    row["strategy"] = json!(strategy);
                    row["representation"] = json!(representation);
                    row["repetition"] = json!(repetition + 1);
                    append(&row);
                }
            }
        }
    }
    output(&plan.output_dir.join(format!("{}-manifest.json",plan.mode)),&json!({"version":1,"mode":plan.mode,"elapsed_ms":started.elapsed().as_millis(),"sequence_count":sequence,"metadata":plan.metadata})).unwrap();
}
