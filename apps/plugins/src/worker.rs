use super::*;
use gennie_llm::builder::BuilderSession;
use gennie_mcp::{Registry, Server, Tool};
use std::time::{Duration, Instant};

pub(crate) fn run(controller: Controller) {
    controller.notify();
    let mut next_reconciliation = Instant::now();
    while !controller.0.stopping.load(Ordering::Acquire) {
        if Instant::now() >= next_reconciliation {
            next_reconciliation = Instant::now() + Duration::from_secs(5);
            let source = controller.0.assignments.lock().ok().and_then(|s| s.clone());
            if let Some(source) = source
                && let Ok((assignments, Some(chat))) = source()
            {
                let pending = reconcile_pending(&controller, assignments, |assignment| {
                    controller
                        .prepare_assignment_inner(assignment, Some(chat), true)
                        .map(|_| ())
                });
                if !pending && let Ok(mut source) = controller.0.assignments.lock() {
                    *source = None;
                }
            }
        }
        let next = controller.0.store.claim();
        match next {
            Ok(Some(mut job)) => {
                let result = (|| -> Result<()> {
                    if job.refresh_base {
                        let current = controller.0.browser.call(
                            json!({"method":"plugins","action":"get","plugin_id":job.plugin_id}),
                        )?;
                        let revision = current["plugin"]["revision"]
                            .as_u64()
                            .or_else(|| current["revision"].as_u64())
                            .ok_or("Revisión de plugin no disponible")?;
                        controller.0.store.set_initial_base(job.id, revision)?;
                        job = controller.0.store.get(job.id)?;
                    }
                    build(&controller, &job)
                })();
                if result.is_err()
                    && controller
                        .0
                        .store
                        .get(job.id)
                        .is_ok_and(|j| j.state == "running")
                {
                    let _=controller.transition(job.id,"interrupted","La construcción se interrumpió por un fallo técnico. El candidato se conserva; comprueba el acceso de Codex y vuelve a continuar.");
                }
                if let Ok(mut active) = controller.0.active.lock() {
                    active.take();
                }
                controller.notify();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
        if controller
            .0
            .delivery_error
            .lock()
            .is_ok_and(|e| e.is_some())
        {
            controller.notify();
        }
    }
    let _ = controller.0.store.recover();
    controller.notify();
}
pub(crate) fn reconcile_pending(
    controller: &Controller,
    assignments: Vec<Value>,
    mut prepare: impl FnMut(&Value) -> Result<()>,
) -> bool {
    let mut pending = false;
    for assignment in assignments {
        if controller.0.stopping.load(Ordering::Acquire) {
            break;
        }
        if assignment["document"]["status"] != "active" {
            continue;
        }
        if controller
            .assignment_sources(&assignment)
            .is_ok_and(|sources| sources.iter().any(|s| s["status"] == "not_investigated"))
        {
            pending = true;
            let _ = prepare(&assignment);
        }
    }
    pending
}
struct TaskPage {
    browser: gennie_browser::Controller,
    id: String,
}
impl Drop for TaskPage {
    fn drop(&mut self) {
        let _ = self
            .browser
            .call(json!({"method":"task_page","action":"close","page_id":self.id}));
    }
}
fn copy_retry_candidate(
    job: &Job,
    source_id: i64,
    mut call: impl FnMut(Value) -> Result<Value>,
) -> Result<bool> {
    let source = call(
        json!({"method":"candidate","action":"get","candidate_id":format!("job_{source_id}"),"plugin_id":job.plugin_id}),
    )?;
    let Some(manifest) = source["plugin"]["manifest"].as_object() else {
        return Ok(false);
    };
    if manifest.get("id") != Some(&json!(job.plugin_id))
        || manifest.get("origin") != Some(&json!(job.origin))
    {
        return Err("El candidato anterior no corresponde al plugin o al origen".into());
    }
    let result = call(
        json!({"method":"candidate","action":"save","candidate_id":format!("job_{}",job.id),"manifest":manifest,"expected_revision":0}),
    )?;
    if result["status"] != "saved" {
        return Err("No se pudo copiar el candidato anterior".into());
    }
    Ok(true)
}

fn report_block(
    store: &store::Store,
    job_id: i64,
    args: &Value,
    strategy: &mut strategy::BuildStrategy,
    turn_budget_exhausted: bool,
) -> Result<Value> {
    if store.get(job_id)?.state != "running" {
        return Err("El trabajo no está en ejecución".into());
    }
    if args["state"] == "waiting_user"
        && !matches!(
            args["blocker"]["kind"].as_str(),
            Some("user_action_required" | "permission_required")
        )
    {
        return Ok(
            json!({"status":"continue","instruction":"La espera necesita una intervención humana concreta: acceso, permiso o decisión del usuario. El trabajo técnico pendiente debe continuar."}),
        );
    }
    match strategy.assess(&args["blocker"], turn_budget_exhausted) {
        strategy::BlockDecision::Continue(instruction) => {
            Ok(json!({"status":"continue","instruction":instruction}))
        }
        strategy::BlockDecision::Review => {
            // No publicar como bloqueo una propuesta que todavía se está revisando.
            Ok(
                json!({"status":"review_required","instruction":"No se ha detenido el trabajo. Termina este turno para que el host abra una revisión con presupuesto nuevo. Reconsidera el impedimento, haz una comprobación nueva y continúa si queda una alternativa viable."}),
            )
        }
        strategy::BlockDecision::Stop => {
            let state = if args["state"] == "waiting_user" {
                "waiting_user"
            } else {
                "blocked"
            };
            store.transition_with_route(
                job_id,
                state,
                args["message"].as_str().ok_or("Falta el avance")?,
                args["recommended_route"].as_str(),
            )?;
            Ok(json!({"status":"recorded","job_id":job_id}))
        }
    }
}
fn tool_transport_available(store: &store::Store, job_id: i64, server: &Server) -> Result<bool> {
    if server.has_clients() {
        return Ok(true);
    }
    store.schedule_tool_retry(job_id)?;
    Ok(false)
}
fn build(controller: &Controller, job: &Job) -> Result<()> {
    let settings = controller.permitted()?;
    controller.0.store.report(job.id,"Estoy explorando la web y preparando funciones reutilizables; conservaré la versión publicada hasta verificar el candidato.")?;
    controller.notify();
    let page = controller
        .0
        .browser
        .automate(json!({"method":"task_page","action":"create"}))?;
    let task = TaskPage {
        browser: controller.0.browser.clone(),
        id: page["page_id"]
            .as_str()
            .ok_or("No se pudo crear la pestaña de trabajo")?
            .into(),
    };
    let candidate_id = format!("job_{}", job.id);
    let candidate=controller.0.browser.automate(json!({"method":"candidate","action":"get","plugin_id":job.plugin_id,"candidate_id":candidate_id}))?;
    if candidate["plugin"].is_null() && job.base_revision > 0 {
        let published = controller
            .0
            .browser
            .call(json!({"method":"plugins","action":"get","plugin_id":job.plugin_id}))?;
        if published["plugin"].is_object() {
            if published["plugin"]["revision"] != job.base_revision {
                return controller.transition(
                    job.id,
                    "interrupted",
                    "El plugin cambió antes de empezar; conservé la nueva versión.",
                );
            }
            if job.retry_of.is_none() {
                controller.0.browser.automate(json!({"method":"candidate","action":"save","candidate_id":candidate_id,"manifest":published["plugin"]["manifest"],"expected_revision":0}))?;
            }
        } else if published["revision"] != job.base_revision {
            return controller.transition(job.id,"interrupted","La identidad del plugin cambió antes de empezar. Conservé el candidato sin recrear una versión eliminada.");
        }
    }
    if candidate["plugin"].is_null()
        && let Some(source_id) = job.retry_of
    {
        if copy_retry_candidate(job, source_id, |request| {
            controller.0.browser.automate(request)
        })? {
            controller.0.store.report(job.id, "Retomé el candidato anterior; volveré a comprobar sus funciones antes de publicar cualquier cambio.")?;
        } else {
            controller.0.store.report(
                job.id,
                "El trabajo anterior no tenía un candidato guardado; investigaré desde el inicio.",
            )?;
        }
        controller.notify();
    }

    let calls = Arc::new(AtomicU64::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    let strategy = Arc::new(Mutex::new(strategy::BuildStrategy::default()));
    let mut registry = Registry::default();
    registry.register(scoped_tools(
        controller,
        job,
        &task.id,
        calls.clone(),
        ready.clone(),
        strategy.clone(),
    ))?;
    let mut server = Server::start(registry, &controller.0.helper)?;
    let base = controller.0.root.join(&candidate_id);
    let mut session: Option<BuilderSession> = None;
    loop {
        let current = controller.0.store.get(job.id)?;
        if current.state != "running" || controller.0.stopping.load(Ordering::Acquire) {
            break;
        }
        if session.is_some() && !tool_transport_available(&controller.0.store, job.id, &server)? {
            controller.notify();
            break;
        }
        if !controller.0.store.begin_attempt(job.id)? {
            break;
        }
        controller.permitted()?;
        calls.store(0, Ordering::Release);
        ready.store(false, Ordering::Release);
        strategy
            .lock()
            .map_err(|_| "Estrategia de construcción inválida")?
            .begin_turn();
        if session.is_none() {
            let control = BuilderControl::default();
            *controller
                .0
                .active
                .lock()
                .map_err(|_| "Constructor inválido")? = Some((job.id, control.clone()));
            match BuilderSession::connect(
                base.clone(),
                &settings,
                server.launch(),
                include_str!("builder-instructions.md").into(),
                current.thread_id.clone(),
                control,
            ) {
                Ok(mut connected) => {
                    controller
                        .0
                        .store
                        .account(job.id, 0, Some(connected.thread_id()))?;
                    let goal = format!(
                        "Prepara y verifica una integración web para este objetivo autorizado: {}. Solo declara completo el objetivo cuando la cobertura esté comprobada; conserva los permisos y comunica bloqueos reales.",
                        current.objective
                    );
                    if let Err(error) = connected.set_goal(&goal) {
                        if gennie_llm::builder::is_retryable(error.as_ref()) {
                            controller.0.store.schedule_provider_retry(job.id)?;
                            controller.notify();
                            break;
                        }
                        return Err(error);
                    }
                    session = Some(connected);
                }
                Err(error) => {
                    if gennie_llm::builder::is_retryable(error.as_ref()) {
                        controller.0.store.schedule_provider_retry(job.id)?;
                        controller.notify();
                        break;
                    }
                    return Err(error);
                }
            }
        }
        let instruction = if current.attempts > 0 {
            "Retoma el candidato y el registro de lo probado: qué aprendiste, qué sigue pendiente y qué experimento útil ejecutarás ahora. Contrasta las conclusiones anteriores con la petición; no las conviertas en requisitos nuevos. Tras una interrupción, comprueba los efectos antes de actuar y no repitas escrituras inciertas. Aplica el ciclo de recuperación y el criterio de finalización de tus instrucciones."
        } else {
            "Contrasta la petición, el resultado acordado y los hallazgos previos; elige la primera incertidumbre útil y ejecuta el ciclo de implementación y comprobación desde el candidato guardado."
        };
        let review = strategy
            .lock()
            .map_err(|_| "Estrategia de construcción inválida")?
            .review_context();
        let prompt=json!({"plugin_id":current.plugin_id,"origin":current.origin,"objective":current.objective,"base_revision":current.base_revision,"construction_scope":"Construye y verifica capacidades reutilizables. La ejecución periódica futura es independiente y su ausencia no bloquea por sí sola el plugin; no declares verificadas funciones que aún falten.","instruction":instruction,"stop_review":review}).to_string();
        let turn = session.as_mut().expect("sesión creada").turn(&prompt);
        if controller.0.store.get(job.id)?.state != "running"
            || controller.0.stopping.load(Ordering::Acquire)
        {
            break;
        }
        if !tool_transport_available(&controller.0.store, job.id, &server)? {
            controller.notify();
            break;
        }
        if let Err(error) = turn {
            if gennie_llm::builder::is_retryable(error.as_ref()) {
                controller.0.store.schedule_provider_retry(job.id)?;
                controller.notify();
                break;
            }
            return Err(error);
        }
        if ready.load(Ordering::Acquire) {
            let _guard = controller
                .0
                .commit
                .lock()
                .map_err(|_| "Estado de promoción inválido")?;
            if controller.0.store.get(job.id)?.state != "running" {
                break;
            }
            let result=controller.0.browser.automate(json!({"method":"plugins","action":"promote","plugin_id":job.plugin_id,"candidate_id":candidate_id,"expected_revision":job.base_revision}))?;
            if result["status"] == "promoted" {
                controller.transition(job.id,"completed","El plugin quedó disponible: todas las funciones del candidato se ejecutaron y verificaron antes de publicarlo.")?;
                break;
            }
            if result["status"] == "revision_conflict" {
                controller.transition(job.id,"interrupted","El plugin cambió durante la construcción. Conservé el candidato sin sobrescribir la nueva revisión.")?;
                break;
            }
        }
    }
    if let Some(session) = session.as_mut() {
        let state = controller.0.store.get(job.id)?.state;
        let status = if state == "completed" {
            "complete"
        } else {
            "paused"
        };
        let _ = session.set_goal_status(status);
    }
    drop(session);
    server.stop()?;
    Ok(())
}
fn prior_candidate(
    job: &Job,
    source: &Job,
    mut fetch: impl FnMut(Value) -> Result<Value>,
) -> Result<Value> {
    if source.id >= job.id
        || source.plugin_id != job.plugin_id
        || source.origin != job.origin
        || source.trigger_conversation_id != job.trigger_conversation_id
    {
        return Err(
            "Los hallazgos anteriores deben corresponder al mismo plugin, origen y chat".into(),
        );
    }
    let mut candidate = fetch(
        json!({"method":"candidate","action":"get","candidate_id":format!("job_{}",source.id),"plugin_id":job.plugin_id}),
    )?;
    if let Some(plugin) = candidate["plugin"].as_object_mut() {
        plugin.remove("verified");
    }
    Ok(
        json!({"job_id":source.id,"candidate":candidate,"verification_current":false,"instruction":"Son hallazgos de referencia, no evidencia vigente ni autorización de efectos. Copia solo lo pertinente al candidato actual y vuelve a verificarlo dentro del objetivo y autorización actuales."}),
    )
}
fn scoped_tools(
    controller: &Controller,
    job: &Job,
    page: &str,
    calls: Arc<AtomicU64>,
    ready: Arc<AtomicBool>,
    strategy: Arc<Mutex<strategy::BuildStrategy>>,
) -> Vec<Tool> {
    let mut result = Vec::new();
    for tool in gennie_browser::tools(controller.0.browser.clone()) {
        let name = tool.definition["name"].as_str().unwrap_or("");
        if !matches!(
            name,
            "browser_snapshot"
                | "browser_screenshot"
                | "browser_navigate"
                | "browser_find"
                | "browser_read"
                | "browser_wait_for"
                | "browser_click"
                | "browser_fill"
                | "browser_press"
                | "browser_hover"
                | "browser_scroll"
                | "browser_select_option"
                | "browser_check"
                | "browser_dialog"
                | "browser_request"
                | "browser_network"
        ) {
            continue;
        }
        let controller = controller.clone();
        let job = job.clone();
        let strategy = strategy.clone();
        let page = page.to_owned();
        let calls = calls.clone();
        let mut definition = tool.definition;
        let name = definition["name"].as_str().unwrap().to_owned();
        if name == "browser_screenshot" {
            definition["description"] = json!(
                "Imagen real y transitoria de la pestaña propia del constructor, sin exportar un archivo. Complementa el DOM para investigar la interfaz oficial. Si orquestas mediante exec, emite el bloque content de type=image con image(bloque), no lo imprimas como JSON/base64. full_page y target/selector son excluyentes."
            );
        }
        if name == "browser_request" {
            definition["description"] = json!(
                "Consulta GET/HEAD con la sesión existente. En el origen delegado usa url/http_method. Para una API o servicio auxiliar documentado necesario para el mismo encargo, añade service_purpose (relación y necesidad concretas) y documentation_url (fuente pública realmente consultada). Solo HTTPS sin credenciales en URLs auxiliares; no amplía la autorización ni da acceso OAuth. No sustituye navegación ni permite consultar cuentas o servicios ajenos al encargo. Un 401/403 real se distingue de una restricción de esta herramienta."
            );
            definition["inputSchema"]["properties"]["service_purpose"] =
                json!({"type":"string","minLength":1,"maxLength":600});
            definition["inputSchema"]["properties"]["documentation_url"] =
                json!({"type":"string","minLength":1,"maxLength":4000});
        }
        let schema = definition["inputSchema"].clone();
        result.push(Tool {
            definition,
            call: Box::new(move |mut args| {
                gate(&controller, &job, &calls)?;
                gennie_browser::validate_arguments(&schema, &args)?;
                if name == "browser_screenshot"
                    && args["full_page"] == true
                    && (args.get("target").is_some() || args.get("selector").is_some())
                {
                    return Err("La captura completa no acepta target o selector".into());
                }
                validate_task_url(&name, &job.origin, &args)?;
                if let Some(object) = args.as_object_mut() {
                    object.remove("service_purpose");
                    object.remove("documentation_url");
                }
                if name == "browser_network" && args["action"] == "start" {
                    args["origin"] = json!(job.origin);
                }
                args["page_id"] = json!(page);
                args["background_scope"] = json!(true);
                args["method"] = json!(name.strip_prefix("browser_").unwrap());
                let observation_args = args.clone();
                let result = controller.0.browser.automate(args);
                let busy = result
                    .as_ref()
                    .is_err_and(|e| e.is::<gennie_browser::BusyError>());
                let output = result.and_then(|mut value| {
                    if let Some(object) = value.as_object_mut() {
                        object.remove("tabs");
                    }
                    gennie_browser::tool_output(name.strip_prefix("browser_").unwrap(), value)
                });
                let observation = if busy {
                    None
                } else {
                    strategy
                        .lock()
                        .map_err(|_| "Estrategia de construcción inválida")?
                        .observe(&name, &observation_args, output.as_ref().ok())
                };
                match output {
                    Ok(mut value) => {
                        if let Some(observation) = observation {
                            let data = if value.get("__gennie_mcp_output_v1").is_some() {
                                &mut value["__gennie_mcp_output_v1"]["data"]
                            } else {
                                &mut value
                            };
                            if let Some(object) = data.as_object_mut() {
                                object.insert("build_observation".into(), observation);
                            }
                        }
                        Ok(value)
                    }
                    Err(error) => {
                        if let Some(observation) = observation {
                            return Err(format!(
                                "{error}; observación fallida {}",
                                observation["id"].as_str().unwrap_or_default()
                            )
                            .into());
                        }
                        Err(error)
                    }
                }
            }),
        });
    }
    let source = gennie_browser::tools(controller.0.browser.clone());
    for (name, original) in [
        ("plugin_candidate", "browser_plugins"),
        ("plugin_candidate_test", "browser_plugin_run"),
    ] {
        let mut definition = source
            .iter()
            .find(|t| t.definition["name"] == original)
            .unwrap()
            .definition
            .clone();
        definition["name"] = json!(name);
        definition["description"] = json!(if name == "plugin_candidate" {
            "Lee/guarda únicamente el candidato de este trabajo (action get/save). Incluye cobertura en description, capacidades en capabilities y límites en limitations. No modifica el plugin publicado; expected_revision=0 si no existe candidato."
        } else {
            "Ejecuta y verifica una función del candidato (action verify/run). Solo verify satisfactorio deja evidencia. Usa plugin_id y revisión del candidato. authentication_required necesita acceso humano; no lo confundas con 403 o fallo de transporte."
        });
        let schema = definition["inputSchema"].clone();
        let controller = controller.clone();
        let job = job.clone();
        let calls = calls.clone();
        let strategy = strategy.clone();
        let candidate_ready = ready.clone();
        result.push(Tool{definition,call:Box::new(move|mut args|{
            gate(&controller,&job,&calls)?;
            gennie_browser::validate_arguments(&schema,&args)?;
            if name=="plugin_candidate" {
                if !matches!(args["action"].as_str(),Some("get"|"save")){return Err("Solo se permite leer/guardar el candidato".into());}
                if args["action"]=="save" && (args["manifest"]["id"]!=job.plugin_id||args["manifest"]["origin"]!=job.origin){return Err("El candidato debe conservar plugin_id y origin del trabajo".into());}
            }
            args["method"]=json!("candidate");args["candidate_id"]=json!(format!("job_{}",job.id));args["plugin_id"]=json!(job.plugin_id);
            let saving = name == "plugin_candidate" && args["action"] == "save";
            let mut response=controller.0.browser.automate(args)?;
            if saving && response["status"] == "saved" {
                strategy.lock().map_err(|_| "Estrategia de construcción inválida")?.candidate_changed();
                candidate_ready.store(false, Ordering::Release);
            }
            if response["status"]=="authentication_required" {
                response["instruction"] = json!("Esta vía necesita acceso, pero no pausa por sí sola la construcción. Comprueba si la interfaz oficial existente ofrece otra vía autorizada. Si el acceso del usuario es indispensable para el objetivo, propone waiting_user con la evidencia observada y la instrucción de abrir Gennie Web, iniciar sesión y continuar desde el mismo mensaje. No pidas credenciales ni confundas OAuth de una API opcional con acceso a la interfaz.");
            }
            Ok(response)
        })});
    }
    let investigation_controller = controller.clone();
    let investigation_job = job.clone();
    let investigation_calls = calls.clone();
    let investigation_strategy = strategy.clone();
    result.push(Tool {
        definition:json!({"name":"plugin_build_investigate","description":"Antes de explorar una capacidad, fija su identificador estable y una hipótesis técnica. Las herramientas UI devolverán build_observation con IDs del host ligados a ese criterio y candidato. Cambia la hipótesis cuando pruebes otra vía; no cambies el ID de capacidad por una paráfrasis ni incluyas datos personales.","inputSchema":{"type":"object","properties":{"capability":{"type":"string","minLength":1,"maxLength":120},"hypothesis":{"type":"string","minLength":1,"maxLength":120}},"required":["capability","hypothesis"],"additionalProperties":false}}),
        call:Box::new(move |args| {
            gate(&investigation_controller,&investigation_job,&investigation_calls)?;
            investigation_strategy.lock().map_err(|_| "Estrategia de construcción inválida")?
                .investigate(args["capability"].as_str().ok_or("Falta la capacidad")?,args["hypothesis"].as_str().ok_or("Falta la hipótesis")?)?;
            Ok(json!({"status":"investigating","instruction":"Explora la UI de esa capacidad y conserva los build_observation.id reales. La API alternativa no genera evidencia de UI. Un fallo no equivale a una observación entregada."}))
        })
    });
    let findings_controller = controller.clone();
    let findings_job = job.clone();
    let findings_calls = calls.clone();
    result.push(Tool {
        definition:json!({"name":"plugin_build_findings","description":"Recupera avances y candidatos anteriores del mismo plugin, origen y chat para aprovechar descubrimientos de la interfaz sin repetir la investigación. action=list enumera trabajos previos; action=get requiere job_id y devuelve su candidato sin verificaciones heredadas. Solo lectura de metadatos/código; no ejecuta sus funciones ni amplía la autorización actual.","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["list","get"]},"job_id":{"type":"integer","minimum":1}},"required":["action"],"additionalProperties":false}}),
        call:Box::new(move |args| {
            gate(&findings_controller,&findings_job,&findings_calls)?;
            match args["action"].as_str() {
                Some("list") => {
                    let jobs:Vec<_> = findings_controller.0.store.list()?.into_iter().filter(|j| j.id < findings_job.id && j.plugin_id == findings_job.plugin_id && j.origin == findings_job.origin && j.trigger_conversation_id == findings_job.trigger_conversation_id)
                        .map(|j| json!({"job_id":j.id,"state":j.state,"summary":j.summary,"updated_at":j.updated_at})).collect();
                    Ok(json!({"jobs":jobs,"verification_current":false}))
                }
                Some("get") => {
                    let source = findings_controller.0.store.get(args["job_id"].as_i64().ok_or("Falta el trabajo anterior")?)?;
                    prior_candidate(&findings_job,&source,|request|findings_controller.0.browser.call(request))
                }
                _ => Err("Operación de hallazgos no válida".into())
            }
        })
    });
    let controller = controller.clone();
    let job = job.clone();
    result.push(Tool {
        definition: strategy::report_definition(),
        call:Box::new(move|args|{
            let message=args["message"].as_str().ok_or("Falta el avance")?;
            if message.trim().is_empty()||message.len()>1200||message.contains('\0') {return Err("Avance no válido".into());}
            if args.get("recommended_route").is_some() && args["state"] != "blocked" { return Err("La vía propuesta solo se admite al comunicar un bloqueo".into()); }
            if args.get("recommended_route").is_some() && !matches!(args["recommended_route"].as_str(),Some("http"|"web"|"computer_use")) { return Err("Vía propuesta inválida".into()); }
            let count=controller.0.store.0.with_connection(|c|Ok(c.query_row("SELECT count(*) FROM plugin_build_events WHERE job_id=?1",[job.id],|r|r.get::<_,u32>(0))?))?;
            if count>200 && args["state"] == "progress" {return Err("Agrupa los avances: se alcanzó el límite de hitos".into());}
            match args["state"].as_str() {
                Some("ready")=>{ready.store(true,Ordering::Release);controller.0.store.report(job.id,"El constructor terminó el candidato; comprobaré las verificaciones antes de publicarlo.")?;controller.notify();}
                Some("progress")=>{controller.0.store.report(job.id,message)?;controller.notify();}
                Some("blocked" | "waiting_user")=>{
                    let mut strategy = strategy.lock().map_err(|_| "Estrategia de construcción inválida")?;
                    let response = report_block(&controller.0.store,job.id,&args,&mut strategy,calls.load(Ordering::Acquire) >= 80)?;
                    drop(strategy);
                    controller.notify();
                    return Ok(response);
                },
                _=>return Err("Estado de avance no válido".into()),
            }
            Ok(json!({"status":"recorded","job_id":job.id}))
        }),
    });
    result
}
fn validate_task_url(name: &str, origin: &str, args: &Value) -> Result<()> {
    let Some(raw) = args["url"].as_str() else {
        return Ok(());
    };
    let url = reqwest::Url::parse(raw).map_err(|_| "URL no válida")?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("No se admiten credenciales en la URL".into());
    }
    if url.origin().ascii_serialization() == origin {
        return Ok(());
    }
    if name != "browser_request" {
        return Err("La pestaña de construcción solo puede navegar al origen delegado; investiga documentación pública mediante búsqueda web y consulta servicios auxiliares documentados con browser_request".into());
    }
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err("Las consultas a servicios auxiliares requieren HTTPS".into());
    }
    let purpose = args["service_purpose"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty());
    let documented = args["documentation_url"]
        .as_str()
        .and_then(|s| reqwest::Url::parse(s).ok())
        .is_some_and(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        });
    if !purpose || !documented {
        return Err("Para consultar un servicio auxiliar del mismo encargo, indica service_purpose y documentation_url de una fuente pública consultada. Este requisito de la herramienta no demuestra que falten permisos ni que la API sea inaccesible".into());
    }
    Ok(())
}
fn gate(controller: &Controller, job: &Job, calls: &AtomicU64) -> Result<()> {
    controller.permitted()?;
    if controller.0.store.get(job.id)?.state != "running" {
        return Err("El trabajo está pausado, cancelado o terminado".into());
    }
    if calls.fetch_add(1, Ordering::AcqRel) >= 80 {
        return Err("Se agotó solo el presupuesto de herramientas de este turno. No es un bloqueo del trabajo: termina el turno y el host continuará automáticamente con presupuesto nuevo".into());
    }
    Ok(())
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn disconnected_tool_channel_schedules_recovery_instead_of_another_turn() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        let (store, _, job) = running_build();
        store.account(job.id, 0, Some("preserved-thread")).unwrap();
        let mut server =
            Server::start(Registry::default(), &std::env::current_exe().unwrap()).unwrap();
        let mut stream = UnixStream::connect(server.launch().socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
            .unwrap();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut String::new())
            .unwrap();
        assert!(tool_transport_available(&store, job.id, &server).unwrap());
        assert_eq!(store.get(job.id).unwrap().state, "running");
        drop(stream);
        let deadline = Instant::now() + Duration::from_secs(1);
        while server.has_clients() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!tool_transport_available(&store, job.id, &server).unwrap());
        assert!(!tool_transport_available(&store, job.id, &server).unwrap());
        let saved = store.get(job.id).unwrap();
        assert_eq!(saved.state, "queued");
        assert_eq!(saved.thread_id.as_deref(), Some("preserved-thread"));
        assert_eq!(saved.attempts, 1);
        assert!(!store.begin_attempt(job.id).unwrap());
        assert!(store.claim().unwrap().is_none());
        assert_eq!(
            saved
                .events
                .iter()
                .filter(|e| e.message.contains("conexión con las herramientas"))
                .count(),
            1
        );
        assert!(!saved.summary.contains("conexión con Codex"));
        store.recover().unwrap();
        assert_eq!(store.claim().unwrap().unwrap().id, job.id);
        server.stop().unwrap();
    }
    fn running_build() -> (store::Store, BuildRequest, Job) {
        let db = gennie_db::Database::memory().unwrap();
        db.migrate(
            "plugins",
            &[
                gennie_core::Migration {
                    version: 1,
                    sql: include_str!("../migrations/0001_build_jobs.sql"),
                },
                gennie_core::Migration {
                    version: 2,
                    sql: include_str!("../migrations/0002_assignment_queue.sql"),
                },
                gennie_core::Migration {
                    version: 3,
                    sql: include_str!("../migrations/0003_retry_source.sql"),
                },
                gennie_core::Migration {
                    version: 4,
                    sql: include_str!("../migrations/0004_recommended_route.sql"),
                },
                gennie_core::Migration {
                    version: 5,
                    sql: include_str!("../migrations/0005_retry_schedule.sql"),
                },
            ],
        )
        .unwrap();
        let store = store::Store(db);
        let request = BuildRequest {
            request_key: "review".into(),
            plugin_id: "mail".into(),
            origin: "https://example.com".into(),
            objective: "Revisar".into(),
            max_attempts: 4,
            max_seconds: 600,
            retry_of: None,
        };
        let job = store.create(&request, 7, 0).unwrap();
        store.claim().unwrap();
        assert!(store.begin_attempt(job.id).unwrap());
        (store, request, job)
    }
    fn block_args(capability: &str) -> Value {
        json!({"state":"blocked","message":"Falta una capacidad comprobada", "blocker":{"kind":"exhausted_alternatives", "capability":capability, "required_outcome":"Resultado solicitado", "outcome_status":"unmet", "evidence":"Impedimento observado", "next_step":"Investigar otra vía", "attempted_approaches":["Controles de la vista", "Vista de detalle"], "research":{"sources":["https://example.com/docs"], "findings":"Ayuda de la interfaz consultada"},"observation_ids":[]}})
    }
    fn observation(
        strategy: &mut strategy::BuildStrategy,
        args: &mut Value,
        tool: &str,
        response: Option<&Value>,
    ) -> Option<Value> {
        let result = strategy.observe(tool, &json!({}), response);
        if let Some(record) = &result {
            args["blocker"]["observation_ids"]
                .as_array_mut()
                .unwrap()
                .push(record["id"].clone());
        }
        result
    }
    fn prepare_interface(strategy: &mut strategy::BuildStrategy, args: &mut Value) {
        let capability = args["blocker"]["capability"].as_str().unwrap().to_owned();
        strategy
            .investigate(&capability, "inspect_controls")
            .unwrap();
        args["blocker"]["observation_ids"] = json!([]);
        observation(
            strategy,
            args,
            "browser_snapshot",
            Some(&json!({"open":true,"snapshot":"Vista de prueba"})),
        );
        observation(
            strategy,
            args,
            "browser_screenshot",
            Some(&json!({"__gennie_mcp_output_v1":{"image":{"data":"fixture_pixels"}}})),
        );
        observation(
            strategy,
            args,
            "browser_find",
            Some(&json!({"targets":[{"label":"Filtro"}]})),
        );
    }
    fn review_probe(strategy: &mut strategy::BuildStrategy, args: &mut Value, result: &str) {
        strategy
            .investigate(
                args["blocker"]["capability"].as_str().unwrap(),
                "inspect_detail",
            )
            .unwrap();
        observation(
            strategy,
            args,
            "browser_read",
            Some(&json!({"text":result})),
        );
    }
    #[test]
    fn incomplete_candidate_keeps_the_build_running() {
        let (store, request, job) = running_build();
        let mut strategy = strategy::BuildStrategy::default();
        strategy.begin_turn();
        let mut args =
            json!({"state":"blocked","message":"Falta ampliar y comprobar la lectura completa"});
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        args = block_args("read_thread");
        for kind in ["work_remaining", "recoverable_failure"] {
            args["blocker"]["kind"] = json!(kind);
            assert_eq!(
                report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
                "continue"
            );
        }
        args["blocker"]["kind"] = json!("exhausted_alternatives");
        prepare_interface(&mut strategy, &mut args);
        let events = store.get(job.id).unwrap().events.len();
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        review_probe(&mut strategy, &mut args, "Inspección del mismo turno");
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        assert_eq!(store.get(job.id).unwrap().events.len(), events);
        review_probe(
            &mut strategy,
            &mut args,
            "Comprobación distinta del turno nuevo",
        );
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, true).unwrap()["status"],
            "continue"
        );
        assert_eq!(store.get(job.id).unwrap().state, "running");
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "recorded"
        );
        assert_eq!(store.get(job.id).unwrap().events.len(), events + 1);
        let other = store
            .create(
                &BuildRequest {
                    request_key: "waiting".into(),
                    plugin_id: "other".into(),
                    ..request
                },
                7,
                0,
            )
            .unwrap();
        store.claim().unwrap();
        store
            .transition(other.id, "waiting_user", "Necesita acceso")
            .unwrap();
        assert!(report_block(&store, other.id, &args, &mut strategy, false).is_err());
    }
    #[test]
    fn changed_blocker_requires_its_own_review_but_paraphrases_keep_identity() {
        let (store, _, job) = running_build();
        let mut strategy = strategy::BuildStrategy::default();
        strategy.begin_turn();
        let mut args = block_args("paginate_results");
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        args = block_args("archive_message");
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        args["blocker"]["required_outcome"] = json!("Paráfrasis del mismo resultado solicitado");
        review_probe(
            &mut strategy,
            &mut args,
            "Otra comprobación del impedimento",
        );
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "recorded"
        );
    }
    #[test]
    fn candidate_change_invalidates_previous_block_review() {
        let (store, _, job) = running_build();
        let mut strategy = strategy::BuildStrategy::default();
        strategy.begin_turn();
        let mut args = block_args("archive_item");
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        strategy.candidate_changed();
        assert!(strategy.review_context().is_null());
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        review_probe(&mut strategy, &mut args, "Candidato nuevo comprobado");
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "recorded"
        );
    }
    #[test]
    fn satisfied_unrequested_or_ambiguous_effect_does_not_exhaust_alternatives() {
        let (store, _, job) = running_build();
        let mut strategy = strategy::BuildStrategy::default();
        strategy.begin_turn();
        let mut args = block_args("archive_item");
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        for status in ["already_satisfied", "not_required", "unclear"] {
            args["blocker"]["outcome_status"] = json!(status);
            assert_eq!(
                report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
                "continue"
            );
            assert_eq!(store.get(job.id).unwrap().state, "running");
            assert!(strategy.review_context().is_null());
        }
    }
    #[test]
    fn api_failure_errors_and_other_capabilities_do_not_exhaust_the_interface() {
        let (store, _, job) = running_build();
        let mut strategy = strategy::BuildStrategy::default();
        strategy.begin_turn();
        let mut args = block_args("list_unread");
        strategy.investigate("list_unread", "try_api").unwrap();
        assert!(
            observation(
                &mut strategy,
                &mut args,
                "browser_request",
                Some(&json!({"status":401}))
            )
            .is_none()
        );
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        for tool in ["browser_snapshot", "browser_screenshot", "browser_find"] {
            observation(&mut strategy, &mut args, tool, None);
        }
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        strategy
            .investigate("other_capability", "inspect_controls")
            .unwrap();
        observation(
            &mut strategy,
            &mut args,
            "browser_snapshot",
            Some(&json!({"open":true,"snapshot":"Otra capacidad"})),
        );
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        prepare_interface(&mut strategy, &mut args);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "review_required"
        );
        strategy.begin_turn();
        strategy
            .investigate("list_unread", "rename_same_hypothesis")
            .unwrap();
        let duplicate = observation(&mut strategy,&mut args,"browser_snapshot",Some(&json!({"open":true,"snapshot":"Vista de prueba","snapshot_id":999,"page_id":"different_ephemeral_id"}))).unwrap();
        assert_eq!(duplicate["novel"], false);
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "continue"
        );
        review_probe(
            &mut strategy,
            &mut args,
            "Resultado de otra lectura pertinente",
        );
        assert_eq!(
            report_block(&store, job.id, &args, &mut strategy, false).unwrap()["status"],
            "recorded"
        );
    }
    #[test]
    fn historical_findings_reuse_code_but_not_verification_or_other_chats() {
        let (store, request, source) = running_build();
        store
            .transition(source.id, "blocked", "Candidato parcial")
            .unwrap();
        let request = BuildRequest {
            request_key: "next-feature".into(),
            objective: "Añadir otra capacidad de lectura".into(),
            ..request
        };
        let job = store
            .create(
                &request,
                source.trigger_conversation_id,
                source.base_revision,
            )
            .unwrap();
        let result = prior_candidate(&job,&source,|args| {
            assert_eq!(args["action"],"get");
            assert_eq!(args["candidate_id"], format!("job_{}",source.id));
            Ok(json!({"plugin":{"manifest":{"id":source.plugin_id,"origin":source.origin,"functions":[]},"revision":7,"verified":{"old":{"revision":7}}}}))
        }).unwrap();
        assert_eq!(result["verification_current"], false);
        assert!(result["candidate"]["plugin"]["manifest"].is_object());
        assert!(result["candidate"]["plugin"]["verified"].is_null());
        for field in ["chat", "plugin", "origin", "future"] {
            let mut other = source.clone();
            match field {
                "chat" => other.trigger_conversation_id += 1,
                "plugin" => other.plugin_id = "another_plugin".into(),
                "origin" => other.origin = "https://other.example".into(),
                _ => other.id = job.id,
            }
            assert!(
                prior_candidate(&job, &other, |_| panic!(
                    "No debe consultar un candidato fuera de alcance"
                ))
                .is_err()
            );
        }
    }
    struct TestJournal;
    impl BuildJournal for TestJournal {
        fn origin(&self) -> Result<i64> {
            Ok(7)
        }
        fn exists(&self, _: i64) -> Result<bool> {
            Ok(true)
        }
        fn deliver(&self, _: i64, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
    }
    #[test]
    fn catalogue_keeps_jobs_when_browser_metadata_is_busy_or_unavailable() {
        let (store, _, job) = running_build();
        let root = std::env::temp_dir().join("gennie-catalogue-no-runtime");
        let browser = gennie_browser::BrowserModule::new(root.clone());
        let module = PluginsModule::new(
            store.0.clone(),
            browser.controller(),
            Arc::new(TestJournal),
            Arc::new(|| Ok(Settings::default())),
            root.join("unused-helper"),
            root,
        );
        let controller = module.controller();
        controller.0.enabled.store(true, Ordering::Release);
        let busy = controller
            .catalogue_with(|| Err(Box::new(gennie_browser::BusyError)))
            .unwrap();
        assert_eq!(busy["jobs"][0]["id"], job.id);
        assert_eq!(busy["jobs"][0]["state"], "running");
        assert_eq!(busy["catalogue_status"], "busy");
        assert!(busy["plugins"].is_null());
        let failed = controller
            .catalogue_with(|| Err("Falta preparar el navegador".into()))
            .unwrap();
        assert_eq!(failed["jobs"][0]["id"], job.id);
        assert_eq!(failed["catalogue_status"], "unavailable");
        assert!(failed["catalogue_error"].is_string());
        let recovered = controller
            .catalogue_with(|| Ok(json!({"plugins":[]})))
            .unwrap();
        assert_eq!(recovered["catalogue_status"], "ready");
        assert_eq!(recovered["plugins"], json!([]));
        assert!(recovered["catalogue_error"].is_null());
    }
    #[test]
    fn scoped_service_request_reaches_browser_without_expanding_page_navigation() {
        let (store, _, job) = running_build();
        let root = std::env::temp_dir().join("gennie-service-no-runtime");
        let browser = gennie_browser::BrowserModule::new(root.clone());
        browser.controller().set_automation_allowed(true);
        let module = PluginsModule::new(
            store.0.clone(),
            browser.controller(),
            Arc::new(TestJournal),
            Arc::new(|| Ok(Settings::default())),
            root.join("unused-helper"),
            root,
        );
        let controller = module.controller();
        controller.0.enabled.store(true, Ordering::Release);
        let tools = scoped_tools(
            &controller,
            &job,
            "unused-page",
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(strategy::BuildStrategy::default())),
        );
        assert!(
            tools
                .iter()
                .any(|t| t.definition["name"] == "browser_screenshot"),
            "El constructor necesita capturas visuales de su propia pestaña"
        );
        assert!(
            tools
                .iter()
                .any(|t| t.definition["name"] == "plugin_build_findings"),
            "El constructor debe poder recuperar hallazgos de candidatos anteriores"
        );
        let findings = tools
            .iter()
            .find(|t| t.definition["name"] == "plugin_build_findings")
            .unwrap();
        assert_eq!(
            (findings.call)(json!({"action":"list"})).unwrap()["jobs"],
            json!([])
        );
        assert!((findings.call)(json!({"action":"get","job_id":job.id})).is_err());
        let request = tools
            .iter()
            .find(|t| t.definition["name"] == "browser_request")
            .unwrap();
        let mut args = json!({"url":"https://api.example.com/items", "http_method":"GET"});
        assert!(
            (request.call)(args.clone())
                .unwrap_err()
                .to_string()
                .contains("service_purpose")
        );
        args["service_purpose"] =
            json!("Consultar la API documentada para el mismo objetivo de example.com");
        args["documentation_url"] = json!("https://docs.example.com/items");
        gennie_browser::validate_arguments(&request.definition["inputSchema"], &args).unwrap();
        // Se supera la política del constructor y llega al navegador desactivado de este fixture.
        // No se inicia Node, no hay red y no se presupone autenticación real en la API.
        assert!(
            (request.call)(args.clone())
                .unwrap_err()
                .to_string()
                .contains("módulo de navegador está desactivado")
        );
        let navigate = tools
            .iter()
            .find(|t| t.definition["name"] == "browser_navigate")
            .unwrap();
        assert!(
            (navigate.call)(json!({"url":"https://api.example.com/items"}))
                .unwrap_err()
                .to_string()
                .contains("solo puede navegar")
        );
        args["http_method"] = json!("POST");
        assert!((request.call)(args.clone()).is_err());
        args["http_method"] = json!("GET");
        for url in [
            "http://api.example.com/items",
            "https://user:secret@api.example.com/items",
        ] {
            args["url"] = json!(url);
            assert!((request.call)(args.clone()).is_err());
        }
        assert_eq!(store.get(job.id).unwrap().state, "running");
    }
    #[test]
    fn report_tool_reviews_human_intervention_and_does_not_pause_for_pending_work() {
        let (store, _, job) = running_build();
        let root = std::env::temp_dir().join("gennie-report-no-runtime");
        let browser = gennie_browser::BrowserModule::new(root.clone());
        let module = PluginsModule::new(
            store.0.clone(),
            browser.controller(),
            Arc::new(TestJournal),
            Arc::new(|| Ok(Settings::default())),
            root.join("unused-helper"),
            root,
        );
        let controller = module.controller();
        let calls = Arc::new(AtomicU64::new(0));
        let strategy = Arc::new(Mutex::new(strategy::BuildStrategy::default()));
        strategy.lock().unwrap().begin_turn();
        let tools = scoped_tools(
            &controller,
            &job,
            "unused-page",
            calls.clone(),
            Arc::new(AtomicBool::new(false)),
            strategy.clone(),
        );
        let report = tools
            .iter()
            .find(|t| t.definition["name"] == "plugin_build_report")
            .unwrap();
        let mut args = json!({"state":"waiting_user","message":"Necesito intervención"});
        assert_eq!((report.call)(args.clone()).unwrap()["status"], "continue");
        args["blocker"] = json!({"kind":"work_remaining","capability":"read_thread","evidence":"Hay casos sin probar","next_step":"Ampliar el código"});
        assert_eq!((report.call)(args.clone()).unwrap()["status"], "continue");
        assert_eq!(store.get(job.id).unwrap().state, "running");
        prepare_interface(&mut strategy.lock().unwrap(), &mut args);
        for kind in ["permission_required", "user_action_required"] {
            args["blocker"]["kind"] = json!(kind);
            gennie_browser::validate_arguments(&report.definition["inputSchema"], &args).unwrap();
            assert_eq!(
                (report.call)(args.clone()).unwrap()["status"],
                "review_required"
            );
        }
        {
            let mut review = strategy.lock().unwrap();
            review.begin_turn();
            review_probe(
                &mut review,
                &mut args,
                "Confirmación nueva del acceso requerido",
            );
        }
        calls.store(80, Ordering::Release);
        assert_eq!((report.call)(args.clone()).unwrap()["status"], "continue");
        assert_eq!(store.get(job.id).unwrap().state, "running");
        calls.store(0, Ordering::Release);
        assert_eq!((report.call)(args).unwrap()["status"], "recorded");
        assert_eq!(store.get(job.id).unwrap().state, "waiting_user");
        // Un reporte tardío nunca reactiva una cancelación.
        store.transition(job.id, "cancelled", "Cancelado").unwrap();
        assert!((report.call)(json!({"state":"blocked","message":"Tardío"})).is_err());
        assert_eq!(store.get(job.id).unwrap().state, "cancelled");
    }
    #[test]
    fn copies_only_manifest_to_new_candidate_and_rejects_other_origins() {
        let job: Job = serde_json::from_value(json!({
            "id": 8, "plugin_id":"mail", "origin":"https://example.com", "objective":"Leer", "trigger_conversation_id":7,
            "base_revision":0, "refresh_base":false, "retry_of":3, "state":"running", "attempts":0, "max_attempts":4,
            "max_seconds":600, "spent_seconds":0, "thread_id":null, "summary":"", "created_at":"", "updated_at":"", "events":[]
        })).unwrap();
        let mut requests = Vec::new();
        assert!(copy_retry_candidate(&job, 3, |request| {
            requests.push(request.clone());
            Ok(if request["action"] == "get" { json!({"plugin":{"manifest":{"id":"mail","origin":"https://example.com"},"verified":{"list":{"revision":1}}}}) } else { json!({"status":"saved"}) })
        }).unwrap());
        assert_eq!(requests[0]["candidate_id"], "job_3");
        assert_eq!(requests[1]["candidate_id"], "job_8");
        assert_eq!(requests[1]["expected_revision"], 0);
        assert!(requests[1].get("verified").is_none());
        assert!(
            copy_retry_candidate(&job, 3, |_| Ok(
                json!({"plugin":{"manifest":{"id":"mail","origin":"https://other.example"}}})
            ))
            .is_err()
        );
        assert!(
            !copy_retry_candidate(&job, 3, |_| Ok(json!({"status":"plugin_not_found"}))).unwrap()
        );
    }
}
