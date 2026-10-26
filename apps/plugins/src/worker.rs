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
                    let _=controller.transition(job.id,"blocked","No se pudo completar la construcción. El candidato se conserva para inspección; comprueba el acceso de Codex y las capacidades del sitio.");
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
struct BudgetWatch {
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    controller: Controller,
    job: i64,
    started: Instant,
}
impl Drop for BudgetWatch {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = self
            .controller
            .0
            .store
            .account(self.job, self.started.elapsed().as_secs(), None);
    }
}
fn build(controller: &Controller, job: &Job) -> Result<()> {
    let settings = controller.permitted()?;
    let started = Instant::now();
    let done = Arc::new(AtomicBool::new(false));
    let watching = done.clone();
    let expired = Arc::new(AtomicBool::new(false));
    let expired_watch = expired.clone();
    let watched = controller.clone();
    let id = job.id;
    let remaining = job.max_seconds.saturating_sub(job.spent_seconds);
    let watcher = std::thread::Builder::new()
        .name("plugin-budget".into())
        .spawn(move || {
            while !watching.load(Ordering::Acquire) {
                if started.elapsed().as_secs() >= remaining {
                    expired_watch.store(true, Ordering::Release);
                    if let Ok(active) = watched.0.active.lock()
                        && let Some((current, control)) = active.as_ref()
                        && *current == id
                    {
                        control.cancel();
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        })?;
    let _budget = BudgetWatch {
        done,
        thread: Some(watcher),
        controller: controller.clone(),
        job: job.id,
        started,
    };
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
                    "blocked",
                    "El plugin cambió antes de empezar; conservé la nueva versión.",
                );
            }
            controller.0.browser.automate(json!({"method":"candidate","action":"save","candidate_id":candidate_id,"manifest":published["plugin"]["manifest"],"expected_revision":0}))?;
        } else if published["revision"] != job.base_revision {
            return controller.transition(job.id,"blocked","La identidad del plugin cambió antes de empezar. Conservé el candidato sin recrear una versión eliminada.");
        }
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
    let mut transport_failures = 0;
    loop {
        let current = controller.0.store.get(job.id)?;
        if current.state != "running" || controller.0.stopping.load(Ordering::Acquire) {
            break;
        }
        let remaining = current
            .max_seconds
            .saturating_sub(current.spent_seconds + started.elapsed().as_secs());
        if remaining < 1 || !controller.0.store.begin_attempt(job.id)? {
            controller.transition(job.id,"blocked","Se agotó el presupuesto sin verificar toda la integración. El candidato se conserva y la versión publicada no cambia.")?;
            break;
        }
        controller.permitted()?;
        calls.store(0, Ordering::Release);
        ready.store(false, Ordering::Release);
        if session.is_none() {
            let control = BuilderControl::default();
            if expired.load(Ordering::Acquire) {
                control.cancel();
                return controller.transition(
                    job.id,
                    "blocked",
                    "Se agotó el tiempo antes de conectar el constructor.",
                );
            }
            *controller
                .0
                .active
                .lock()
                .map_err(|_| "Constructor inválido")? = Some((job.id, control.clone()));
            if expired.load(Ordering::Acquire) || controller.0.store.get(job.id)?.state != "running"
            {
                control.cancel();
                break;
            }
            match BuilderSession::connect(
                base.clone(),
                &settings,
                server.launch(),
                include_str!("builder-instructions.md").into(),
                current.thread_id.clone(),
                control,
            ) {
                Ok(connected) => {
                    controller
                        .0
                        .store
                        .account(job.id, 0, Some(connected.thread_id()))?;
                    session = Some(connected);
                }
                Err(_) => {
                    transport_failures += 1;
                    if transport_failures >= 2 {
                        controller.transition(job.id,"blocked","No pude conectar el constructor tras dos procesos independientes. Comprueba la sesión de Codex; el candidato se conserva.")?;
                        break;
                    }
                    controller.0.store.report(job.id,"Falló la conexión del constructor. Probaré un proceso nuevo sin cambiar la conversación principal.")?;
                    controller.notify();
                    continue;
                }
            }
        }
        let prompt=json!({"plugin_id":current.plugin_id,"origin":current.origin,"objective":current.objective,"base_revision":current.base_revision,"attempt":current.attempts+1,"remaining_seconds":remaining,"instruction":"Continúa desde el candidato guardado. Explora y verifica todas las funciones. Si el intento anterior no resolvió la integración, prueba una vía distinta y explica el cambio con plugin_build_report. Declara state=ready solo después de comprobar que todas las funciones satisfacen el objetivo."}).to_string();
        let turn = session
            .as_mut()
            .expect("sesión creada")
            .turn(&prompt, remaining.min(120));
        if controller.0.store.get(job.id)?.state != "running" {
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
                controller.transition(job.id,"blocked","El plugin cambió durante la construcción. Conservé el candidato sin sobrescribir la nueva revisión.")?;
                break;
            }
        }
        if turn.is_err() {
            transport_failures += 1;
            session = None;
            if transport_failures >= 2 {
                controller.transition(job.id,"blocked","El proveedor interrumpió dos intentos de construcción. Conservé el candidato sin declarar éxito.")?;
                break;
            }
        }
        controller.0.store.report(job.id,"El intento no completó todas las verificaciones. Voy a probar otra vía dentro del presupuesto disponible.")?;
        controller.notify();
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
                controller.transition(job.id,"waiting_user","La función necesita una sesión válida. Abre Gennie Web, inicia sesión en el sitio de esta tarea y pulsa Continuar en Plugins.")?;
            }
            Ok(response)
        })});
    }
    let controller = controller.clone();
    let job = job.clone();
    result.push(Tool {
        definition:json!({"name":"plugin_build_report","description":"Informa un hito breve al chat que disparó el trabajo. state progress/ready/waiting_user/blocked. No incluye contenido privado ni credenciales; no declara éxito por sí solo.","inputSchema":{"type":"object","properties":{"state":{"type":"string","enum":["progress","ready","waiting_user","blocked"]},"message":{"type":"string","minLength":1,"maxLength":600}},"required":["state","message"],"additionalProperties":false}}),
        call:Box::new(move|args|{
            let message=args["message"].as_str().ok_or("Falta el avance")?;
            if message.trim().is_empty()||message.len()>1200||message.contains('\0') {return Err("Avance no válido".into());}
            let count=controller.0.store.0.with_connection(|c|Ok(c.query_row("SELECT count(*) FROM plugin_build_events WHERE job_id=?1",[job.id],|r|r.get::<_,u32>(0))?))?;
            if count>24 {return Err("Agrupa los avances: se alcanzó el límite de hitos".into());}
            match args["state"].as_str() {
                Some("ready")=>{ready.store(true,Ordering::Release);controller.0.store.report(job.id,"El constructor terminó el candidato; comprobaré las verificaciones antes de publicarlo.")?;controller.notify();}
                Some("progress")=>{controller.0.store.report(job.id,message)?;controller.notify();}
                Some("waiting_user")=>controller.transition(job.id,"waiting_user",message)?,
                Some("blocked")=>controller.transition(job.id,"blocked",message)?,
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
