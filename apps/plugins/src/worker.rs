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
    message: &str,
    route: Option<&str>,
) -> Result<()> {
    if store.get(job_id)?.state != "running" {
        return Err("El trabajo no está en ejecución".into());
    }
    store.transition_with_route(job_id, "blocked", message, route)?;
    Ok(())
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
    let mut registry = Registry::default();
    registry.register(scoped_tools(
        controller,
        job,
        &task.id,
        calls.clone(),
        ready.clone(),
    ))?;
    let mut server = Server::start(registry, &controller.0.helper)?;
    let base = controller.0.root.join(&candidate_id);
    let mut session: Option<BuilderSession> = None;
    loop {
        let current = controller.0.store.get(job.id)?;
        if current.state != "running" || controller.0.stopping.load(Ordering::Acquire) {
            break;
        }
        if !controller.0.store.begin_attempt(job.id)? {
            break;
        }
        controller.permitted()?;
        calls.store(0, Ordering::Release);
        ready.store(false, Ordering::Release);
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
            "Revisa el historial y el candidato guardado. Tras una interrupción, comprueba los efectos anteriores antes de actuar y no repitas acciones de resultado incierto. Investiga una alternativa distinta y pertinente para las partes que aún puedan resolverse. Mantén el objetivo activo y busca vías diferentes; documenta lo que sí puedes comprobar. Si no quedan alternativas pertinentes, comunica el bloqueo concreto. No repitas acciones con resultado incierto; modifica datos solo cuando el objetivo lo autorice. Declara ready solo con todas las funciones verificadas."
        } else {
            "Continúa desde el candidato guardado. Explora y verifica todas las funciones. Declara state=ready solo después de comprobar que todas las funciones satisfacen el objetivo."
        };
        let prompt=json!({"plugin_id":current.plugin_id,"origin":current.origin,"objective":current.objective,"base_revision":current.base_revision,"construction_scope":"Construye y verifica capacidades reutilizables. La ejecución periódica futura es independiente y su ausencia no bloquea por sí sola el plugin; no declares verificadas funciones que aún falten.","instruction":instruction}).to_string();
        let turn = session.as_mut().expect("sesión creada").turn(&prompt);
        if controller.0.store.get(job.id)?.state != "running"
            || controller.0.stopping.load(Ordering::Acquire)
        {
            break;
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
        if let Err(error) = turn {
            if gennie_llm::builder::is_retryable(error.as_ref()) {
                controller.0.store.schedule_provider_retry(job.id)?;
                controller.notify();
                break;
            }
            return Err(error);
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
fn scoped_tools(
    controller: &Controller,
    job: &Job,
    page: &str,
    calls: Arc<AtomicU64>,
    ready: Arc<AtomicBool>,
) -> Vec<Tool> {
    let mut result = Vec::new();
    for tool in gennie_browser::tools(controller.0.browser.clone()) {
        let name = tool.definition["name"].as_str().unwrap_or("");
        if !matches!(
            name,
            "browser_snapshot"
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
        let page = page.to_owned();
        let calls = calls.clone();
        let definition = tool.definition;
        let name = definition["name"].as_str().unwrap().to_owned();
        let schema = definition["inputSchema"].clone();
        result.push(Tool {definition,call:Box::new(move|mut args|{
            gate(&controller,&job,&calls)?;
            gennie_browser::validate_arguments(&schema,&args)?;
            if let Some(url)=args["url"].as_str() {
                let url=reqwest::Url::parse(url).map_err(|_|"URL no válida")?;
                if url.origin().ascii_serialization()!=job.origin {return Err("La construcción solo puede navegar o consultar el origen delegado. El usuario inicia sesión en la ventana cuando hace falta".into());}
            }
            if name=="browser_network" && args["action"]=="start" {args["origin"]=json!(job.origin);}
            args["page_id"]=json!(page);args["background_scope"]=json!(true);
            args["method"]=json!(name.strip_prefix("browser_").unwrap());
            let mut value=controller.0.browser.automate(args)?;
            if let Some(object)=value.as_object_mut(){object.remove("tabs");}
            Ok(value)
        })});
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
        result.push(Tool{definition,call:Box::new(move|mut args|{
            gate(&controller,&job,&calls)?;
            gennie_browser::validate_arguments(&schema,&args)?;
            if name=="plugin_candidate" {
                if !matches!(args["action"].as_str(),Some("get"|"save")){return Err("Solo se permite leer/guardar el candidato".into());}
                if args["action"]=="save" && (args["manifest"]["id"]!=job.plugin_id||args["manifest"]["origin"]!=job.origin){return Err("El candidato debe conservar plugin_id y origin del trabajo".into());}
            }
            args["method"]=json!("candidate");args["candidate_id"]=json!(format!("job_{}",job.id));args["plugin_id"]=json!(job.plugin_id);
            let response=controller.0.browser.automate(args)?;
            if response["status"]=="authentication_required" {
                controller.transition(job.id,"waiting_user","La función necesita una sesión válida. Pulsa «Abrir Gennie Web» en este mensaje e inicia sesión en el sitio. Después pulsa «Continuar» aquí para retomar la comprobación.")?;
            }
            Ok(response)
        })});
    }
    let controller = controller.clone();
    let job = job.clone();
    result.push(Tool {
        definition:json!({"name":"plugin_build_report","description":"Informa un hito breve al chat que disparó el trabajo. Si está bloqueado, puede proponer recommended_route http/web/computer_use como método pendiente de comprobación, sin afirmar que esté autorizado ni disponible.","inputSchema":{"type":"object","properties":{"state":{"type":"string","enum":["progress","ready","waiting_user","blocked"]},"message":{"type":"string","minLength":1,"maxLength":600},"recommended_route":{"type":"string","enum":["http","web","computer_use"]}},"required":["state","message"],"additionalProperties":false}}),
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
                Some("waiting_user")=>controller.transition(job.id,"waiting_user",message)?,
                Some("blocked")=>{
                    report_block(&controller.0.store,job.id,message,args["recommended_route"].as_str())?;
                    controller.notify();
                },
                _=>return Err("Estado de avance no válido".into()),
            }
            Ok(json!({"status":"recorded","job_id":job.id}))
        }),
    });
    result
}
fn gate(controller: &Controller, job: &Job, calls: &AtomicU64) -> Result<()> {
    controller.permitted()?;
    if controller.0.store.get(job.id)?.state != "running" {
        return Err("El trabajo está pausado, cancelado o terminado".into());
    }
    if calls.fetch_add(1, Ordering::AcqRel) >= 80 {
        return Err("Se agotó el presupuesto de herramientas de este intento; cambia de vía o informa el bloqueo".into());
    }
    Ok(())
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn block_stops_without_waiting_for_a_deadline() {
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
        report_block(&store, job.id, "No hay otra vía", Some("computer_use")).unwrap();
        let result = store.get(job.id).unwrap();
        assert_eq!(result.state, "blocked");
        assert_eq!(result.recommended_route.as_deref(), Some("computer_use"));
        let waiting = BuildRequest {
            request_key: "waiting".into(),
            plugin_id: "other".into(),
            ..request
        };
        let waiting = store.create(&waiting, 7, 0).unwrap();
        store.claim().unwrap();
        store
            .transition(waiting.id, "waiting_user", "Necesita acceso")
            .unwrap();
        assert!(report_block(&store, waiting.id, "Tardío", None).is_err());
        assert_eq!(store.get(waiting.id).unwrap().state, "waiting_user");
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
