//! Construcción de plugins bajo demanda, independiente del chat y sin scheduler.
mod external_mcp;
mod mcp_activity;
mod mcp_attempts;
mod mcp_source;
mod mcp_store;
mod presentation;
mod store;
mod strategy;
mod tools;
mod worker;
use gennie_core::{AppModule, AppState, Migration, Result};
use gennie_db::Database;
use gennie_llm::{Settings, builder::BuilderControl};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_build_jobs.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_assignment_queue.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../migrations/0003_retry_source.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../migrations/0004_recommended_route.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../migrations/0005_retry_schedule.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../migrations/0006_mcp_sources.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("../migrations/0007_mcp_attempts.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("../migrations/0008_delegation_origin.sql"),
    },
];
pub use store::{BuildEvent, Job};
pub use tools::{contextual_tools, tools};

pub trait BuildJournal: Send + Sync {
    fn origin(&self) -> Result<i64>;
    fn exists(&self, conversation: i64) -> Result<bool>;
    fn target(&self, conversation: i64) -> Result<gennie_llm::delegation::Origin>;
    /// Acuse del inbox del bot, idempotente; nunca entrega directamente a la UI.
    fn deliver(&self, update: &gennie_llm::delegation::Update) -> Result<()>;
    /// Lectura de mensajes confirmados; ausencia no autoriza mostrar texto del helper.
    fn messages(
        &self,
        _origin: &gennie_llm::delegation::Origin,
        _job_id: i64,
        _plugin_id: &str,
        _legacy: bool,
        _through_sequence: i64,
    ) -> Result<Option<Vec<gennie_llm::delegation::BotMessage>>> {
        Ok(None)
    }
}
pub type SettingsProvider = Arc<dyn Fn() -> Result<Settings> + Send + Sync>;
/// El canal MCP conserva el límite del usuario; el constructor usa el perfil del helper.
pub struct AgentSettings {
    pub user: SettingsProvider,
    pub helper: SettingsProvider,
}
impl AgentSettings {
    pub fn shared(settings: SettingsProvider) -> Self {
        Self {
            user: settings.clone(),
            helper: settings,
        }
    }
}
pub type AssignmentSource = Arc<dyn Fn() -> Result<(Vec<Value>, Option<i64>)> + Send + Sync>;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    pub request_key: String,
    pub plugin_id: String,
    pub origin: String,
    pub objective: String,
    #[serde(default = "default_build_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_build_seconds")]
    pub max_seconds: u64,
    #[serde(default)]
    pub retry_of: Option<i64>,
}
fn default_build_attempts() -> u32 {
    8
}
fn default_build_seconds() -> u64 {
    600
}
#[derive(Clone)]
pub struct Controller(Arc<Inner>);
struct Inner {
    mcp: external_mcp::Service,
    store: store::Store,
    browser: gennie_browser::Controller,
    journal: Arc<dyn BuildJournal>,
    settings: SettingsProvider,
    helper: PathBuf,
    root: PathBuf,
    enabled: AtomicBool,
    stopping: AtomicBool,
    revision: AtomicU64,
    delivery_error: Mutex<Option<String>>,
    commit: Mutex<()>,
    agent_configuration: Mutex<()>,
    active: Mutex<Option<(i64, BuilderControl)>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    assignments: Mutex<Option<AssignmentSource>>,
}
pub struct PluginsModule {
    controller: Controller,
}
impl PluginsModule {
    pub fn new(
        db: Database,
        browser: gennie_browser::Controller,
        journal: Arc<dyn BuildJournal>,
        settings: AgentSettings,
        helper: PathBuf,
        root: PathBuf,
    ) -> Self {
        Self {
            controller: Controller(Arc::new(Inner {
                mcp: external_mcp::Service::new(
                    db.clone(),
                    root.parent().unwrap_or(&root).join("mcp"),
                    settings.user,
                ),
                store: store::Store(db),
                browser,
                journal,
                settings: settings.helper,
                helper,
                root,
                enabled: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
                revision: AtomicU64::new(0),
                delivery_error: Mutex::new(None),
                commit: Mutex::new(()),
                agent_configuration: Mutex::new(()),
                active: Mutex::new(None),
                worker: Mutex::new(None),
                assignments: Mutex::new(None),
            })),
        }
    }
    pub fn controller(&self) -> Controller {
        self.controller.clone()
    }
}
impl AppModule for PluginsModule {
    fn name(&self) -> &'static str {
        "plugins"
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        &["browser", "llm", "presence"]
    }
    fn migrations(&self) -> &'static [Migration] {
        MIGRATIONS
    }
    fn start(&self, _: &AppState) -> Result<()> {
        let mut worker = self
            .controller
            .0
            .worker
            .lock()
            .map_err(|_| "Worker de plugins inválido")?;
        if worker.is_some() {
            return Ok(());
        }
        self.controller.0.store.recover()?;
        self.controller.0.mcp.start()?;
        self.controller.0.enabled.store(true, Ordering::Release);
        self.controller.0.stopping.store(false, Ordering::Release);
        let controller = self.controller.clone();
        *worker = Some(
            std::thread::Builder::new()
                .name("plugin-builder".into())
                .spawn(move || worker::run(controller))?,
        );
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        self.controller.stop()
    }
}
impl Drop for PluginsModule {
    fn drop(&mut self) {
        let _ = self.controller.stop();
    }
}
impl Controller {
    /// La fuente es inyectada por bootstrap y se consulta en el worker propio.
    pub fn reconcile_assignments(&self, source: AssignmentSource) -> Result<()> {
        self.check()?;
        *self
            .0
            .assignments
            .lock()
            .map_err(|_| "Revisión de encargos inválida")? = Some(source);
        Ok(())
    }
    pub fn assignment_sources(&self, assignment: &Value) -> Result<Vec<Value>> {
        let id = assignment["id"].as_i64().ok_or("Encargo inválido")?;
        let revision = assignment["revision"].as_u64().ok_or("Revisión inválida")?;
        let apps = assignment["document"]["applications"]
            .as_array()
            .ok_or("Fuentes no válidas")?;
        apps.iter()
            .enumerate()
            .map(|(index, app)| {
                let status = if app["kind"] == "desktop" {
                    "desktop_unsupported".to_owned()
                } else if app["kind"] != "web" {
                    "unsupported".to_owned()
                } else if web_origin(app).is_none() {
                    "origin_required".to_owned()
                } else if let Some((job_id, state)) = self.assignment_job_for(assignment, app, id, revision, index)?
                {
                    let job = self.0.store.get(job_id)?;
                    return Ok(json!({"source":app["name"],"kind":app["kind"],"status":state,"recommended_route":job.recommended_route}));
                } else {
                    "not_investigated".to_owned()
                };
                Ok(json!({"source":app["name"],"kind":app["kind"],"status":status}))
            })
            .collect()
    }
    fn assignment_job_for(
        &self,
        assignment: &Value,
        app: &Value,
        id: i64,
        revision: u64,
        index: usize,
    ) -> Result<Option<(i64, String)>> {
        let legacy = format!("assignment_{id}_{revision}_{index}");
        let Some(instance) = assignment_instance(assignment)? else {
            return self.0.store.assignment_job(&legacy);
        };
        let key = format!("{legacy}_{instance}");
        if let Some(job) = self.0.store.assignment_job(&key)? {
            return Ok(Some(job));
        }
        if let (Some(root), Some(created), Some(origin)) = (
            self.0.store.assignment_root(&legacy)?,
            assignment["created_at"].as_str(),
            web_origin(app),
        ) {
            // Las fechas de SQLite usan el mismo formato UTC con milisegundos.
            if created.len() == 24
                && root.created_at.len() == 24
                && root.created_at.as_str() > created
                && root.origin == origin
                && root.objective == assignment_objective(&assignment["document"])
            {
                return self.0.store.assignment_job(&legacy);
            }
        }
        Ok(None)
    }
    pub fn revision(&self) -> u64 {
        self.0
            .revision
            .load(Ordering::Acquire)
            .wrapping_add(self.0.mcp.revision())
            .wrapping_add(self.0.browser.execution_revision())
    }
    fn touch(&self) {
        self.0.revision.fetch_add(1, Ordering::AcqRel);
    }
    fn check(&self) -> Result<()> {
        if !self.0.enabled.load(Ordering::Acquire) || self.0.stopping.load(Ordering::Acquire) {
            return Err("El módulo de plugins está desactivado o cerrándose".into());
        }
        Ok(())
    }
    fn permitted(&self) -> Result<Settings> {
        self.check()?;
        let settings = (self.0.settings)()?;
        if !settings.run_commands || !settings.local_actions {
            return Err("El constructor necesita el control de aplicaciones activado".into());
        }
        Ok(settings)
    }
    pub fn catalogue_changed(&self, request: &Value, response: &Value) -> Result<()> {
        if response["status"] == "deleted" {
            let id = request["plugin_id"].as_str().ok_or("Falta plugin_id")?;
            for job in self.0.store.list()?.iter().filter(|j| {
                j.plugin_id == id
                    && matches!(
                        j.state.as_str(),
                        "queued" | "running" | "waiting_user" | "interrupted" | "blocked"
                    )
            }) {
                self.cancel(job.id)?;
            }
        }
        self.touch();
        Ok(())
    }
    pub fn catalogue(&self) -> Result<Value> {
        self.catalogue_with(|| {
            self.0
                .browser
                .call(json!({"method":"plugins","action":"list"}))
        })
    }
    fn catalogue_with(&self, fetch: impl FnOnce() -> Result<Value>) -> Result<Value> {
        self.check()?;
        // Los trabajos son independientes del proceso del navegador. Una operación web
        // en curso no debe ocultarlos ni convertir un catálogo desconocido en uno vacío.
        let jobs = self.0.store.list()?;
        let mut result = match fetch() {
            Ok(mut value) => {
                value["catalogue_status"] = json!("ready");
                value
            }
            Err(error) => json!({
                "plugins":null,
                "catalogue_status":if error.is::<gennie_browser::BusyError>() {"busy"} else {"unavailable"},
                "catalogue_error":error.to_string()
            }),
        };
        result["jobs"] = serde_json::to_value(
            jobs.iter()
                .map(|job| self.present_job(job))
                .collect::<Result<Vec<_>>>()?,
        )?;
        match self.0.mcp.list() {
            Ok(sources) => {
                result["mcp_sources"] = sources;
                result["mcp_catalogue_status"] = json!("ready");
            }
            Err(error) => {
                result["mcp_sources"] = Value::Null;
                result["mcp_catalogue_status"] = json!("unavailable");
                result["mcp_catalogue_error"] = json!(error.to_string());
            }
        }
        result["delivery_error"] = serde_json::to_value(
            self.0
                .delivery_error
                .lock()
                .map_err(|_| "Estado de entrega inválido")?
                .clone(),
        )?;
        Ok(result)
    }
    /// Prepara bajo demanda las fuentes web de un encargo recién guardado.
    /// El resultado distingue el commit del encargo de la aceptación del trabajo.
    pub fn prepare_assignment(
        &self,
        assignment: &Value,
        conversation: Option<i64>,
    ) -> Result<Value> {
        self.prepare_assignment_inner(assignment, conversation, false)
    }
    fn prepare_assignment_inner(
        &self,
        assignment: &Value,
        conversation: Option<i64>,
        pending_only: bool,
    ) -> Result<Value> {
        let id = assignment["id"]
            .as_i64()
            .filter(|id| *id > 0)
            .ok_or("Encargo inválido")?;
        let document = &assignment["document"];
        if document["method"]["plugin_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("mcp:"))
        {
            return Ok(json!({"sources":[],"status":"not_needed"}));
        }
        let applications = document["applications"]
            .as_array()
            .ok_or("Fuentes no válidas")?;
        let mut sources = Vec::new();
        if !applications.iter().any(|app| app["kind"] == "web") {
            return Ok(json!({"sources":sources}));
        }
        self.check()?;
        let revision = assignment["revision"]
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or("Revisión de encargo inválida")?;
        let mut jobs = self.jobs()?;
        let catalogue = self.catalogue();
        let records = catalogue
            .as_ref()
            .ok()
            .and_then(|v| v["plugins"].as_array());
        for (index, app) in applications.iter().enumerate() {
            if app["kind"] != "web" {
                continue;
            }
            if let Some((job_id, state)) =
                self.assignment_job_for(assignment, app, id, revision, index)?
            {
                if !pending_only {
                    let status = match state.as_str() {
                        "queued" | "running" => "started",
                        "waiting_user" | "interrupted" => "existing_job",
                        _ => "previous_job",
                    };
                    sources.push(
                        json!({"source":app["name"],"status":status,"job_id":job_id,"state":state}),
                    );
                }
                continue;
            }
            let Some(url) = app["url"].as_str() else {
                sources.push(json!({"source":app["name"],"status":"origin_required"}));
                continue;
            };
            let Ok(url) = reqwest::Url::parse(url) else {
                sources.push(json!({"source":app["name"],"status":"origin_required"}));
                continue;
            };
            if !matches!(url.scheme(), "http" | "https")
                || !url.username().is_empty()
                || url.password().is_some()
            {
                sources.push(json!({"source":app["name"],"status":"origin_required"}));
                continue;
            }
            let origin = url.origin().ascii_serialization();
            let existing =
                records.and_then(|rows| rows.iter().find(|r| r["manifest"]["origin"] == origin));
            let prior = jobs.iter().find(|j| {
                j.origin == origin
                    && matches!(
                        j.state.as_str(),
                        "queued" | "running" | "waiting_user" | "interrupted"
                    )
            });
            if records.is_none() && prior.is_none() {
                sources.push(json!({"source":app["name"],"status":"not_started","message":"No se pudo consultar el catálogo de plugins; vuelve a intentarlo."}));
                continue;
            }
            let plugin_id = existing
                .and_then(|r| r["manifest"]["id"].as_str())
                .map(str::to_owned)
                .or_else(|| prior.map(|j| j.plugin_id.clone()))
                .unwrap_or_else(|| match assignment_instance(assignment) {
                    Ok(Some(instance)) => format!("assignment_{instance}_{index}"),
                    _ => format!("assignment_{id}_{index}"),
                });
            let objective = assignment_objective(document);
            let request_key = match assignment_instance(assignment)? {
                Some(instance) => format!("assignment_{id}_{revision}_{index}_{instance}"),
                None => format!("assignment_{id}_{revision}_{index}"),
            };
            let request = BuildRequest {
                request_key,
                plugin_id,
                origin,
                objective,
                max_attempts: 8,
                max_seconds: 600,
                retry_of: None,
            };
            let outcome = (|| -> Result<Job> {
                self.permitted()?;
                let chat = match conversation {
                    Some(id) => id,
                    None => self.0.journal.origin()?,
                };
                if !self.0.journal.exists(chat)? {
                    return Err("El chat de origen ya no está disponible".into());
                }
                let target = self.0.journal.target(chat)?;
                let job = self.0.store.create_target(&request, &target, 0, true)?;
                self.notify();
                Ok(job)
            })();
            match outcome {
                Ok(job) => {
                    jobs.push(job.clone());
                    let status = match job.state.as_str() {
                        "queued" | "running" => "started",
                        "waiting_user" | "interrupted" => "existing_job",
                        _ => "previous_job",
                    };
                    sources.push(json!({"source":app["name"],"status":status,"job_id":job.id,"state":job.state}));
                },
                Err(error) => sources.push(json!({"source":app["name"],"status":"not_started","message":error.to_string()})),
            }
        }
        Ok(json!({"sources":sources}))
    }
    pub fn submit(&self, request: BuildRequest, conversation: Option<i64>) -> Result<Job> {
        self.permitted()?;
        if !valid_id(&request.plugin_id)
            || request.request_key.trim().is_empty()
            || request.request_key.len() > 200
            || request.objective.trim().is_empty()
            || request.objective.len() > 8000
            || !(1..=8).contains(&request.max_attempts)
            || !(30..=1800).contains(&request.max_seconds)
        {
            return Err("Solicitud de construcción inválida".into());
        }
        let origin = reqwest::Url::parse(&request.origin).map_err(|_| "Origen web inválido")?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.origin().ascii_serialization() != request.origin
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err("Usa un origen HTTP/HTTPS sin ruta ni credenciales".into());
        }
        let conversation = match conversation {
            Some(id) => id,
            None => self.0.journal.origin()?,
        };
        if !self.0.journal.exists(conversation)? {
            return Err("El chat de origen ya no está disponible".into());
        }
        let current = self
            .0
            .browser
            .call(json!({"method":"plugins","action":"get","plugin_id":request.plugin_id}))?;
        let revision = current["plugin"]["revision"]
            .as_u64()
            .or_else(|| current["revision"].as_u64())
            .unwrap_or(0);
        let target = self.0.journal.target(conversation)?;
        let job = self
            .0
            .store
            .create_target(&request, &target, revision, false)?;
        self.notify();
        Ok(job)
    }
    pub fn cancel(&self, id: i64) -> Result<Job> {
        self.check()?;
        let _commit = self
            .0
            .commit
            .lock()
            .map_err(|_| "Estado de promoción inválido")?;
        let job = self.0.store.get(id)?;
        if job.state == "completed" || job.state == "cancelled" {
            return Ok(job);
        }
        self.0.store.transition(
            id,
            "cancelled",
            "Construcción cancelada. La versión publicada se conserva.",
        )?;
        if let Some((active, control)) = self
            .0
            .active
            .lock()
            .map_err(|_| "Constructor inválido")?
            .as_ref()
            && *active == id
        {
            control.cancel();
        }
        self.notify();
        self.0.store.get(id)
    }
    pub fn resume(&self, id: i64) -> Result<Job> {
        self.permitted()?;
        let job = self.0.store.get(id)?;
        if !matches!(job.state.as_str(), "waiting_user" | "interrupted") {
            return Err(
                "Solo se puede continuar un trabajo esperando acceso o interrumpido".into(),
            );
        }
        self.0.browser.automate(json!({"method":"background"}))?;
        self.0.store.transition(
            id,
            "queued",
            "Construcción reanudada; comprobaré el acceso y el candidato guardado.",
        )?;
        self.notify();
        self.0.store.get(id)
    }
    fn claim(&self) -> Result<Option<Job>> {
        let _configuration = self
            .0
            .agent_configuration
            .lock()
            .map_err(|_| "Configuración del helper inválida")?;
        self.check()?;
        let settings = match (self.0.settings)() {
            Err(error) if error.is::<gennie_llm::architects::HelperDisabled>() => return Ok(None),
            result => result?,
        };
        if !settings.run_commands || !settings.local_actions {
            return Ok(None);
        }
        self.0.store.claim()
    }
    /// Serializa el cambio de perfiles con la toma de un trabajo; no espera IO de red.
    pub fn while_builder_idle<T>(&self, save: impl FnOnce() -> Result<T>) -> Result<T> {
        let _configuration = self
            .0
            .agent_configuration
            .lock()
            .map_err(|_| "Configuración del helper inválida")?;
        if self
            .0
            .active
            .lock()
            .map_err(|_| "Constructor inválido")?
            .is_some()
            || self
                .0
                .store
                .list()?
                .iter()
                .any(|job| job.state == "running")
        {
            return Err("El helper está construyendo. Espera a que termine o cancela el trabajo antes de cambiar los arquitectos".into());
        }
        save()
    }
    pub fn jobs(&self) -> Result<Vec<Job>> {
        self.check()?;
        self.0.store.list()
    }
    /// Propaga el contexto del consumidor a ejecuciones web y MCP.
    pub fn request_with_context(
        &self,
        request: Value,
        context: gennie_mcp::CallContext,
    ) -> Result<Value> {
        self.check()?;
        context.check()?;
        match request["action"].as_str(){
            Some("mcp_run")=>self.0.mcp.execute_with_context(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["tool"].as_str().ok_or("Falta herramienta MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?,request["arguments"].clone(),context),
            Some("run"|"verify"|"prepare")=>self.0.browser.automate_with_context(json!({"method":"plugin_run","action":request["action"],"plugin_id":request["plugin_id"],"function_name":request["function_name"],"expected_revision":request["expected_revision"],"arguments":request["arguments"]}),context),
            _=>self.request(request),
        }
    }
    /// FRB recibe un documento pequeño; solo las operaciones del catálogo son editables.
    pub fn request(&self, request: Value) -> Result<Value> {
        self.check()?;
        match request["action"].as_str() {
            Some("web_attempts")=>self.0.browser.web_attempts(request["plugin_id"].as_str().ok_or("Falta plugin web")?,request["after"].as_i64()),
            Some("web_reconcile")=>self.0.browser.reconcile_web(request["plugin_id"].as_str().ok_or("Falta plugin web")?,request["attempt_id"].as_i64().ok_or("Falta intento web")?,request["expected_revision"].as_i64().ok_or("Falta revisión web")?,request["resolution"].as_str().ok_or("Falta comprobación web")?),
            Some("mcp_list")=>Ok(json!({"servers":self.0.mcp.list()?})),
            Some("mcp_get")=>Ok(json!({"server":self.0.mcp.get(request["server_id"].as_str().ok_or("Falta servidor MCP")?)?})),
            Some("mcp_attempts")=>self.0.mcp.pending_page(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["after"].as_i64()),
            Some("mcp_save")=>{
                let spec: mcp_source::Specification=serde_json::from_value(request["specification"].clone()).map_err(|_|"Configuración MCP inválida")?;
                Ok(json!({"server":self.0.mcp.save(spec,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?)?}))
            }
            Some("mcp_connect")=>Ok(json!({"server":self.0.mcp.connect(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?)?})),
            Some("mcp_disconnect")=>{self.0.mcp.disconnect(request["server_id"].as_str().ok_or("Falta servidor MCP")?)?;Ok(json!({"status":"disconnected"}))}
            Some("mcp_permissions")=>{
                let allowed:Vec<String>=serde_json::from_value(request["allowed_tools"].clone()).map_err(|_|"Permisos MCP inválidos")?;
                Ok(json!({"server":self.0.mcp.permissions(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?,&allowed,request["enabled"].as_bool().ok_or("Falta estado MCP")?)?}))
            }
            Some("mcp_delete")=>self.0.mcp.delete(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?),
            Some("mcp_reconcile")=>Ok(json!({"server":self.0.mcp.reconcile(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?,request["attempt_id"].as_i64().ok_or("Falta intento MCP")?,request["resolution"].as_str().ok_or("Falta resultado comprobado")?)?})),
            Some("mcp_run")=>self.0.mcp.execute(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["tool"].as_str().ok_or("Falta herramienta MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?,request["arguments"].clone()),
            Some("mcp_prepare")=>self.0.mcp.prepare_execution(request["server_id"].as_str().ok_or("Falta servidor MCP")?,request["tool"].as_str().ok_or("Falta herramienta MCP")?,request["expected_revision"].as_i64().ok_or("Falta revisión MCP")?,request["arguments"].clone()),
            Some("list")=>self.catalogue(),
            Some("save")=>{
                let result=self.0.browser.call(json!({"method":"plugins","action":"save","manifest":request["manifest"],"expected_revision":request["expected_revision"]}))?;
                self.touch();Ok(result)
            }
            Some("delete")=>{
                let id=request["plugin_id"].as_str().ok_or("Falta plugin_id")?;
                let current=self.0.browser.call(json!({"method":"plugins","action":"get","plugin_id":id}))?;
                if current["plugin"]["revision"]!=request["expected_revision"] {return Ok(json!({"status":"revision_conflict"}));}
                for job in self.0.store.list()?.iter().filter(|j|j.plugin_id==id&&matches!(j.state.as_str(),"queued"|"running"|"waiting_user"|"interrupted"|"blocked")) {self.cancel(job.id)?;}
                let result=self.0.browser.call(json!({"method":"plugins","action":"delete","plugin_id":id,"expected_revision":request["expected_revision"]}))?;
                self.touch();Ok(result)
            }
            Some("get_job")=>{
                let job=self.0.store.get(request["job_id"].as_i64().filter(|id|*id>0).ok_or("Falta un job_id válido")?)?;
                let live=self.0.store.has_live_successor(&job.plugin_id,job.id)?;
                Ok(json!({"job":self.present_job(&job)?,"has_live_successor":live}))
            }
            Some("connect")=>{
                let manifest=if let Some(id)=request["job_id"].as_i64() {
                    let job=self.0.store.get(id)?;
                    let candidate=self.0.browser.call(json!({"method":"candidate","action":"get","candidate_id":format!("job_{}",id),"plugin_id":job.plugin_id}))?;
                    if candidate["plugin"]["manifest"].is_object(){candidate["plugin"]["manifest"].clone()}else{json!({"origin":job.origin})}
                }else{
                    self.0.browser.call(json!({"method":"plugins","action":"get","plugin_id":request["plugin_id"]}))?["plugin"]["manifest"].clone()
                };
                let url=manifest["login_url"].as_str().or_else(||manifest["origin"].as_str()).ok_or("No se encuentra el origen del plugin")?;
                self.0.browser.open()?;
                self.0.browser.call(json!({"method":"navigate","url":url}))?;
                Ok(json!({"status":"opened","message":"Completa el acceso en Gennie Web y vuelve a probar o continuar el trabajo."}))
            }
            Some("run"|"verify"|"prepare")=>self.0.browser.automate(json!({"method":"plugin_run","action":request["action"],"plugin_id":request["plugin_id"],"function_name":request["function_name"],"expected_revision":request["expected_revision"],"arguments":request["arguments"]})),
            Some("build")=>{
                let build:BuildRequest=serde_json::from_value(request["request"].clone()).map_err(|_|"Datos de construcción inválidos")?;
                let origin=request["trigger_conversation_id"].as_i64();
                Ok(json!({"job":self.present_job(&self.submit(build,origin)?)?}))
            }
            Some("cancel")=>Ok(json!({"job":self.present_job(&self.cancel(request["job_id"].as_i64().ok_or("Falta job_id")?)?)?})),
            Some("resume")=>Ok(json!({"job":self.present_job(&self.resume(request["job_id"].as_i64().ok_or("Falta job_id")?)?)?})),
            _=>Err("Operación de plugins no válida".into()),
        }
    }
    pub fn cancel_if_disallowed(&self) {
        self.0.mcp.cancel_if_disallowed();
        if self.permitted().is_err() {
            let active = self.0.active.lock().ok().and_then(|active| active.clone());
            if let Some((id, control)) = active {
                if self.0.store.get(id).is_ok_and(|job| job.state == "running") {
                    let _=self.transition(id,"interrupted","La construcción se pausó al desactivar el control de aplicaciones. Puedes continuar cuando lo actives de nuevo.");
                }
                control.cancel();
            }
        }
    }

    fn notify(&self) {
        self.touch();
        let result = self.0.store.deliver(self.0.journal.as_ref());
        if let Ok(mut error) = self.0.delivery_error.lock() {
            *error=result.err().map(|_|"No se pudieron entregar algunos avances al chat de origen; se reintentará sin duplicarlos.".into());
        }
    }
    fn transition(&self, id: i64, state: &str, message: &str) -> Result<()> {
        self.0.store.transition(id, state, message)?;
        self.notify();
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        self.0.enabled.store(false, Ordering::Release);
        self.0.stopping.store(true, Ordering::Release);
        let mcp_result = self.0.mcp.stop();
        if let Some((_, control)) = self
            .0
            .active
            .lock()
            .map_err(|_| "Constructor inválido")?
            .as_ref()
        {
            control.cancel();
        }
        if let Some(worker) = self.0.worker.lock().map_err(|_| "Worker inválido")?.take() {
            worker
                .join()
                .map_err(|_| "El constructor terminó inesperadamente")?;
        }
        mcp_result
    }
}
fn assignment_instance(assignment: &Value) -> Result<Option<&str>> {
    let Some(value) = assignment.get("instance_id") else {
        return Ok(None);
    };
    let value = value.as_str().ok_or("Identidad de encargo inválida")?;
    if value.len() != 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Identidad de encargo inválida".into());
    }
    Ok(Some(value))
}
fn assignment_objective(document: &Value) -> String {
    format!(
        "Objetivo del encargo: {}\nAlcance autorizado: {}\nInvestiga las capacidades disponibles. Describe explícitamente las operaciones que no se puedan realizar; no declares completo el objetivo si faltan.",
        document["description"].as_str().unwrap_or(""),
        document["authorization"]["scope"]
            .as_str()
            .unwrap_or("Solo las operaciones expresamente descritas en el encargo")
    )
}
fn web_origin(app: &Value) -> Option<String> {
    let url = reqwest::Url::parse(app["url"].as_str()?).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}
pub(crate) fn valid_id(value: &str) -> bool {
    value.len() <= 64
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod integration_tests;
