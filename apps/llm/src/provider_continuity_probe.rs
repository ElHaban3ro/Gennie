//! Sonda aislada: adaptador real y lecturas ficticias, sin efectos externos.
use super::*;

fn definition(case: &str) -> Value {
    let case = canonical_case(case);
    let (description, targets) = match case {
        "polling" => (
            "Consulta el estado de un trabajo ficticio con target=job. Cada consulta obtiene el siguiente estado.",
            json!(["job"]),
        ),
        "stalled" => (
            "Consulta el estado de un trabajo ficticio con target=job. El estado puede permanecer pendiente y cada consulta solo lee datos.",
            json!(["job"]),
        ),
        "delegation" => (
            "Consulta el resultado ficticio de una delegación con target=partial. El resultado distingue lo terminado y lo pendiente.",
            json!(["partial"]),
        ),
        _ => (
            "Lee una fuente ficticia de la evaluación. Targets disponibles: alpha y beta para el primer caso, primary y backup para el segundo. No accede a cuentas ni ejecuta efectos.",
            json!(["alpha", "beta", "primary", "backup"]),
        ),
    };
    json!({"name":"probe_read","description":description,"inputSchema":{"type":"object","properties":{"target":{"type":"string","enum":targets}},"required":["target"],"additionalProperties":false},"annotations":{"readOnlyHint":true}})
}

fn canonical_case(case: &str) -> &str {
    if case.starts_with("compare_a") || case.starts_with("compare_b") {
        return "two_sources";
    }
    match case {
        "explicit_two_sources" => "two_sources",
        "explicit_polling" => "polling",
        "explicit_delegation" => "delegation",
        "correction" => "two_sources",
        other => other,
    }
}

fn read_fixture(trace: &mut Vec<Value>, case: &str, args: &Value) -> Result<Value> {
    let case = canonical_case(case);
    if trace.len() >= 64 {
        return Err("Límite de lecturas del fixture".into());
    }
    let target = args["target"].as_str().ok_or("Destino ficticio ausente")?;
    trace.push(json!({"event":"call","target":target}));
    if (case, target) == ("stalled", "job") {
        let count = trace
            .iter()
            .filter(|v| v["event"] == "call" && v["target"] == "job")
            .count();
        if count > 6 {
            return Err("Límite ficticio de consultas alcanzado".into());
        }
        trace.push(json!({"event":"observation","target":target,"state":"pending"}));
        return Ok(
            json!({"status":"pending","message":"Sigue pendiente; no hay cambios disponibles"}),
        );
    }
    if (case, target) == ("polling", "job") {
        let count = trace
            .iter()
            .filter(|v| v["event"] == "call" && v["target"] == "job")
            .count();
        if count > 3 {
            return Err("Límite ficticio de consultas alcanzado".into());
        }
        let state = if count >= 3 { "ready" } else { "pending" };
        trace.push(json!({"event":"observation","target":target,"state":state}));
        return Ok(
            json!({"status":state,"value":if state == "ready" { Some("LISTO-73") } else { None }}),
        );
    }
    if (case, target) == ("delegation", "partial") {
        trace.push(json!({"event":"delegation","state":"partial"}));
        return Ok(
            json!({"status":"partial","completed":"Revisión A terminada","pending":"PENDIENTE-B: revisión B aún no realizada"}),
        );
    }
    let value = match (case, target) {
        ("two_sources", "alpha") => Some("LIMA-42"),
        ("two_sources", "beta") => Some("SOL-71"),
        ("fallback", "backup") => Some("RIO-17"),
        ("fallback", "primary") => None,
        _ => return Err("Destino fuera del caso ficticio".into()),
    };
    match value {
        Some(value) => {
            trace.push(json!({"event":"observation","target":target,"value":value}));
            Ok(json!({"status":"ok","value":value}))
        }
        None => {
            trace.push(json!({"event":"observation","target":target,"state":"unavailable"}));
            Ok(
                json!({"status":"unavailable","message":"La fuente principal ficticia no está disponible; backup contiene el mismo dato."}),
            )
        }
    }
}

fn exact_schema(inventory: &Value, case: &str) -> bool {
    let Some(rows) = inventory["data"].as_array() else {
        return false;
    };
    let matches: Vec<_> = rows.iter().filter(|v| v["name"] == "gennie").collect();
    matches.len() == 1
        && matches[0]["toolsError"].is_null()
        && matches[0]["tools"].as_object().is_some_and(|tools| {
            tools.len() == 1
                && tools.get("probe_read").and_then(|v| v.get("inputSchema"))
                    == definition(case).get("inputSchema")
        })
}

fn case_instructions(settings: &Settings, names: &[String], key: &str) -> String {
    let mut result = instructions(settings, names);
    if key.starts_with("compare_b") {
        result.push_str("\nLa herramienta MCP probe_read está registrada en esta sesión y permite consultar las fuentes ficticias alpha y beta.");
    }
    result
}

fn case(plan: &Plan, key: &str, report: &mut Value) -> Result<()> {
    report[key] = json!({"status":"started","trace":[],"response":null,"schemas_exact":false,"socket_removed":false});
    let trace = Arc::new(Mutex::new(Vec::<Value>::new()));
    let target_trace = trace.clone();
    let case_id = key.to_owned();
    let mut registry = gennie_mcp::Registry::default();
    registry.register([gennie_mcp::Tool {
        definition: definition(key),
        call: Box::new(move |args| {
            read_fixture(
                &mut *target_trace.lock().map_err(|_| "Traza inválida")?,
                &case_id,
                &args,
            )
        }),
    }])?;
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
    let mut server = gennie_mcp::Server::start(registry, &helper)?;
    let socket = server.launch().socket.clone();
    let result = (|| -> Result<()> {
        let db = Database::memory()?;
        db.migrate(
            crate::LlmModule::default().name(),
            crate::LlmModule::default().migrations(),
        )?;
        let mut settings = crate::architects::runtime_settings(&db, crate::architects::Role::Bot)?;
        settings.read_files = false;
        settings.write_files = false;
        settings.run_commands = false;
        settings.local_actions = false;
        settings.model.clone_from(&plan.model);
        settings.effort.clone_from(&plan.effort);
        if settings.instructions.trim().is_empty() {
            return Err("El perfil ficticio no contiene las instrucciones iniciales".into());
        }
        let process = ProcessHandle::default();
        let control = turn::Control::new(process.clone());
        let mut scope = Some(Scope::new(&control)?);
        let base = provider_directory(plan).join(key);
        std::fs::create_dir(&base)?;
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700))?;
        let mut session = Session::spawn(
            process,
            base,
            &settings,
            Some(&server.launch()),
            &[
                "web_search=\"disabled\"".into(),
                "features.multi_agent=false".into(),
                "features.apply_patch_freeform=false".into(),
            ],
            plan.mode == "live",
            Some(
                scope
                    .as_ref()
                    .ok_or("Falta alcance")?
                    .ticket
                    .context
                    .clone(),
            ),
        )?;
        session.bind_turn(Some(scope.as_ref().ok_or("Falta alcance")?.ticket.clone()));
        if plan.mode == "live" {
            session.authenticate_model(&settings)?;
        } else {
            session.model = plan.model.clone();
        }
        session.journal = Some(Arc::new(Journal::default()));
        let developer_instructions = case_instructions(&settings, &session.tool_names, key);
        let thread = session.open_thread(None, &developer_instructions)?;
        let id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("Hilo ausente")?;
        save_recorded_thread(&db, id, None, session.journal.as_deref())?;
        session.thread_id = id.into();
        report[key]["thread_id"] = json!(id);
        session.send(json!({"id":13,"method":"mcpServerStatus/list","params":{"threadId":id,"serverName":"gennie","detail":"toolsAndAuthOnly"}}))?;
        let inventory = session.response(13, HANDSHAKE_TIMEOUT)?;
        server.launch().validate_inventory(&inventory)?;
        report[key]["schemas_exact"] = json!(exact_schema(&inventory, key));
        if report[key]["schemas_exact"] != true {
            return Err("Esquema ficticio distinto".into());
        }
        if plan.mode == "live" {
            report["stage"] = json!(format!("{key}:turn"));
            let prompt = match key {
                "two_sources" => {
                    "Consulta las fuentes ficticias alpha y beta y dame sus dos valores exactos."
                }
                "fallback" => {
                    "Consulta el valor de la fuente ficticia primary y dame el valor exacto."
                }
                "polling" => {
                    "Comprueba el estado del trabajo ficticio job. Si aún está pendiente, consulta hasta dos veces más ahora; dime el estado final y su valor si está listo."
                }
                "delegation" => {
                    "Revisa el resultado de la delegación ficticia partial sobre las revisiones A y B. Dime qué se completó y qué queda pendiente, incluyendo el identificador de lo pendiente."
                }
                "explicit_two_sources" => {
                    "Consulta las fuentes ficticias alpha y beta con la herramienta probe_read y dame sus dos valores exactos."
                }
                "explicit_polling" => {
                    "Comprueba el estado del trabajo ficticio job con la herramienta probe_read. Si aún está pendiente, consulta hasta dos veces más ahora; dime el estado final y su valor si está listo."
                }
                "stalled" => {
                    "Consulta el estado del trabajo ficticio job con probe_read y dime si está listo. Si no cambia, explica lo observado."
                }
                "explicit_delegation" => {
                    "Consulta con probe_read el resultado de la delegación ficticia partial sobre las revisiones A y B. Dime qué se completó y qué queda pendiente, incluyendo el identificador de lo pendiente."
                }
                "correction" => {
                    "Consulta con probe_read la fuente ficticia alpha y dime su valor exacto."
                }
                key if key.starts_with("compare_a") || key.starts_with("compare_b") => {
                    "Consulta las fuentes ficticias alpha y beta y dame sus dos valores exactos."
                }
                _ => return Err("Caso ficticio desconocido".into()),
            };
            let reply = answer(plan, &mut session, prompt, None, report)?;
            trace
                .lock()
                .map_err(|_| "Traza inválida")?
                .push(json!({"event":"reply","text":reply}));
            report[key]["response"] = json!(reply);
            if key == "correction" {
                report[key]["first_response"] = json!(reply);
                drop(scope.take());
                let next_scope = Scope::new(&control)?;
                session.bind_turn(Some(next_scope.ticket.clone()));
                trace
                    .lock()
                    .map_err(|_| "Traza inválida")?
                    .push(json!({"event":"correction"}));
                report["stage"] = json!("correction:second_turn");
                let reply = answer(
                    plan,
                    &mut session,
                    "Cambio de objetivo: ahora necesito únicamente el valor de beta. Consulta beta con probe_read; no repitas la consulta de alpha.",
                    None,
                    report,
                )?;
                trace
                    .lock()
                    .map_err(|_| "Traza inválida")?
                    .push(json!({"event":"reply","text":reply}));
                report[key]["response"] = json!(reply);
                drop(next_scope);
            }
        }
        drop(scope);
        session.bind_turn(None);
        drop(session);
        db.close()?;
        Ok(())
    })();
    let stopped = server.stop();
    report[key]["trace"] = json!(*trace.lock().map_err(|_| "Traza inválida")?);
    report[key]["socket_removed"] = json!(!socket.exists());
    report[key]["status"] = json!(if result.is_ok() && stopped.is_ok() && !socket.exists() {
        "completed"
    } else {
        "failed"
    });
    result?;
    stopped?;
    if socket.exists() {
        return Err("Socket propio no retirado".into());
    }
    Ok(())
}

pub(super) fn exercise(plan: &Plan, report: &mut Value) -> Result<()> {
    if matches!(
        plan.scenario,
        Scenario::ContinuityCompare | Scenario::ContinuityCompareFollowup
    ) {
        let keys = if plan.scenario == Scenario::ContinuityCompareFollowup {
            ["compare_b3", "compare_a3", "compare_a4", "compare_b4"]
        } else {
            ["compare_a1", "compare_b1", "compare_b2", "compare_a2"]
        };
        for key in keys {
            report[key] = json!({"status":"not_run"});
        }
        let mut threads = std::collections::HashSet::new();
        for key in keys {
            case(plan, key, report)?;
            let id = report[key]["thread_id"]
                .as_str()
                .ok_or("Hilo ficticio ausente")?;
            if !threads.insert(id.to_owned()) {
                return Err("Hilos ficticios no aislados".into());
            }
        }
        if plan.mode == "protocol" {
            report["protocol"] =
                json!({"account_linked":false,"generation_turns":0,"schemas_exact":true});
        }
        return Ok(());
    }
    if matches!(
        plan.scenario,
        Scenario::ContinuityCorrection | Scenario::ContinuityCorrectionModel
    ) {
        report["correction"] = json!({"status":"not_run"});
        case(plan, "correction", report)?;
        if plan.mode == "protocol" {
            report["protocol"] =
                json!({"account_linked":false,"generation_turns":0,"schemas_exact":true});
        }
        return Ok(());
    }
    let keys = if plan.scenario == Scenario::ContinuityExtended {
        ["polling", "delegation"]
    } else if plan.scenario == Scenario::ContinuityModelCompare {
        ["compare_a1", "compare_a2"]
    } else if plan.scenario == Scenario::ContinuityModelFollowup {
        ["compare_a3", "compare_a4"]
    } else if plan.scenario == Scenario::ContinuityExplicit {
        ["explicit_two_sources", "explicit_polling"]
    } else if plan.scenario == Scenario::ContinuityStall {
        ["stalled", "explicit_delegation"]
    } else {
        ["two_sources", "fallback"]
    };
    for key in keys {
        report[key] = json!({"status":"not_run"});
    }
    case(plan, keys[0], report)?;
    case(plan, keys[1], report)?;
    if report[keys[0]]["thread_id"] == report[keys[1]]["thread_id"] {
        return Err("Hilos ficticios no aislados".into());
    }
    if plan.mode == "protocol" {
        report["protocol"] =
            json!({"account_linked":false,"generation_turns":0,"schemas_exact":true});
    }
    Ok(())
}

#[test]
fn fixture_records_failure_and_backup_without_effects() {
    let mut trace = vec![];
    assert_eq!(
        read_fixture(&mut trace, "fallback", &json!({"target":"primary"})).unwrap()["status"],
        "unavailable"
    );
    assert_eq!(
        read_fixture(&mut trace, "fallback", &json!({"target":"backup"})).unwrap()["value"],
        "RIO-17"
    );
    assert_eq!(trace.len(), 4);
    assert!(read_fixture(&mut trace, "fallback", &json!({"target":"alpha"})).is_err());
}

#[test]
fn fixture_wait_changes_on_third_read_and_partial_delegation_is_explicit() {
    let mut trace = vec![];
    for state in ["pending", "pending", "ready"] {
        assert_eq!(
            read_fixture(&mut trace, "polling", &json!({"target":"job"})).unwrap()["status"],
            state
        );
    }
    assert!(read_fixture(&mut trace, "polling", &json!({"target":"job"})).is_err());
    let mut trace = vec![];
    assert_eq!(
        read_fixture(&mut trace, "delegation", &json!({"target":"partial"})).unwrap()["status"],
        "partial"
    );
    assert_eq!(trace[1]["state"], "partial");
}

#[test]
fn unchanged_fixture_records_six_reads_and_caps_further_attempts() {
    let mut trace = vec![];
    for _ in 0..6 {
        assert_eq!(
            read_fixture(&mut trace, "stalled", &json!({"target":"job"})).unwrap()["status"],
            "pending"
        );
    }
    assert!(read_fixture(&mut trace, "stalled", &json!({"target":"job"})).is_err());
    assert_eq!(trace.iter().filter(|v| v["event"] == "call").count(), 7);
}

#[test]
fn comparison_variants_differ_only_by_the_explicit_capability_sentence() {
    let settings = Settings::default();
    let names = vec!["probe_read".to_owned()];
    let a = case_instructions(&settings, &names, "compare_a1");
    let b = case_instructions(&settings, &names, "compare_b1");
    assert_eq!(a, case_instructions(&settings, &names, "compare_a2"));
    assert_eq!(b, case_instructions(&settings, &names, "compare_b2"));
    assert_eq!(
        b.strip_prefix(&a),
        Some(
            "\nLa herramienta MCP probe_read está registrada en esta sesión y permite consultar las fuentes ficticias alpha y beta."
        )
    );
    for key in ["compare_a1", "compare_b1", "compare_b2", "compare_a2"] {
        assert_eq!(definition(key), definition("two_sources"));
    }
}
