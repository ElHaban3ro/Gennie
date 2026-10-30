//! Ejecución periódica explícita. El calendario no concede permisos ni repite efectos inciertos.
pub mod notices;
mod store;
use crate::{
    Route, Status, Store,
    proposals::{Baseline, Reference},
    schedule::{self, Calendar, DstPolicy, Trigger},
};
use gennie_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::{thread::JoinHandle, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Web {
        plugin_id: String,
        function: String,
        revision: u64,
    },
    Mcp {
        server_id: String,
        tool: String,
        revision: i64,
    },
}
impl Target {
    pub fn plugin_id(&self) -> String {
        match self {
            Self::Web { plugin_id, .. } => plugin_id.clone(),
            Self::Mcp { server_id, .. } => format!("mcp:{server_id}"),
        }
    }
    pub fn function(&self) -> &str {
        match self {
            Self::Web { function, .. } => function,
            Self::Mcp { tool, .. } => tool,
        }
    }
    fn validate(&self) -> Result<()> {
        match self {
            Self::Web {
                plugin_id,
                function,
                revision,
            } if crate::method::identifier(plugin_id)
                && crate::method::identifier(function)
                && *revision > 0 =>
            {
                Ok(())
            }
            Self::Mcp {
                server_id,
                tool,
                revision,
            } if crate::method::identifier(server_id)
                && !tool.is_empty()
                && tool.len() <= 256
                && !tool.chars().any(char::is_control)
                && *revision > 0 =>
            {
                Ok(())
            }
            _ => Err("Función o revisión de ejecución inválida".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Misfire {
    SkipMissed,
    RunLatest,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configure {
    #[serde(default)]
    pub contextual_notice: bool,
    pub assignment: Reference,
    pub expected_policy_revision: u32,
    pub trigger: Trigger,
    pub dst: DstPolicy,
    pub target: Target,
    pub arguments: Value,
    pub misfire: Misfire,
    pub max_lateness_minutes: u32,
    pub until: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub delivery: Option<notices::Delivery>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub contextual_notice: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<notices::Delivery>,
    pub assignment: Baseline,
    pub trigger: Trigger,
    pub calendar: Calendar,
    pub target: Target,
    pub arguments: Value,
    pub route: Route,
    pub misfire: Misfire,
    pub max_lateness_minutes: u32,
    pub until: Option<String>,
}
/// Preparar valida sin efectos; ejecutar usa exactamente el destino/revisión reservados.
pub trait Runtime: Send + Sync {
    fn guide_available(&self) -> Result<bool> {
        Ok(false)
    }
    fn notice_details(&self, _run_id: i64) -> Result<Option<Value>> {
        Ok(None)
    }
    fn execution_available(&self) -> bool {
        true
    }
    fn prepare(&self, target: &Target, arguments: &Value) -> Result<Route>;
    fn execute(&self, target: &Target, arguments: &Value) -> Result<Value>;
    fn notice_origin(&self, _conversation: Option<i64>) -> Result<notices::Delivery> {
        Err("No hay un bot disponible para recibir resultados".into())
    }
    fn accept_notice(&self, _notice: &notices::Notice) -> Result<()> {
        Err("El bot no está disponible; el aviso sigue guardado".into())
    }
}
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;
#[derive(Clone)]
pub struct Controller(Arc<Inner>);
struct Inner {
    store: Store,
    runtime: Arc<dyn Runtime>,
    clock: Clock,
    ingress: Mutex<bool>,
    stopping: AtomicBool,
    storage_failed: AtomicBool,
    runner_active: bool,
    wake: mpsc::SyncSender<()>,
}
pub struct Runner {
    controller: Controller,
    handle: Option<JoinHandle<()>>,
    // El lock del archivo permanece retenido durante toda la vida del runner.
    _lock: std::fs::File,
}
impl Runner {
    pub fn start(
        store: Store,
        runtime: Arc<dyn Runtime>,
        lock_path: &std::path::Path,
    ) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(lock_path)?;
        lock.try_lock()
            .map_err(|_| "Otra instancia controla las ejecuciones de esta base")?;
        let (wake, signal) = mpsc::sync_channel(1);
        let execution_available = runtime.execution_available();
        let controller = Controller(Arc::new(Inner {
            store,
            runtime,
            clock: Arc::new(|| jiff::Timestamp::now().as_second()),
            ingress: Mutex::new(true),
            stopping: AtomicBool::new(false),
            storage_failed: AtomicBool::new(false),
            runner_active: execution_available,
            wake,
        }));
        controller.0.store.recover_executions()?;
        let worker = controller.clone();
        let handle = std::thread::Builder::new()
            .name("assignment-executions".into())
            .spawn(move || {
                let mut recovering = false;
                while !worker.0.stopping.load(Ordering::Acquire) {
                    if recovering {
                        if worker.0.store.recover_executions().is_err() {
                            let _ = signal.recv_timeout(Duration::from_secs(1));
                            continue;
                        }
                        recovering = false;
                        worker.0.storage_failed.store(false, Ordering::Release);
                        worker.0.store.revision.fetch_add(1, Ordering::Release);
                    }
                    let notice_pending = worker.flush_notice().unwrap_or(true);
                    match worker.step() {
                        Ok(true) => continue,
                        Err(_) => {
                            recovering = true;
                            worker.0.storage_failed.store(true, Ordering::Release);
                            worker.0.store.revision.fetch_add(1, Ordering::Release);
                            let _ = signal.recv_timeout(Duration::from_secs(1));
                            continue;
                        }
                        Ok(false) => {}
                    }
                    if !worker.0.runner_active {
                        if notice_pending {
                            let _ = signal.recv_timeout(Duration::from_secs(30));
                        } else if signal.recv().is_err() {
                            break;
                        }
                        continue;
                    }
                    let now = (worker.0.clock)();
                    match worker.0.store.next_execution() {
                        Ok(Some(at)) => {
                            let wait = at.saturating_sub(now).clamp(1, 60) as u64;
                            let _ = signal.recv_timeout(Duration::from_secs(wait));
                        }
                        Ok(None) if notice_pending => {
                            let _ = signal.recv_timeout(Duration::from_secs(30));
                        }
                        Ok(None) => {
                            if signal.recv().is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = signal.recv_timeout(Duration::from_secs(1));
                        }
                    }
                }
            })?;
        Ok(Self {
            controller,
            handle: Some(handle),
            _lock: lock,
        })
    }
    pub fn controller(&self) -> Controller {
        self.controller.clone()
    }
    pub fn request_stop(&self) {
        self.controller.0.stopping.store(true, Ordering::Release);
        let _ = self.controller.0.wake.try_send(());
    }
    pub fn close_ingress(&self) -> Result<()> {
        self.request_stop();
        let mut accepting = self
            .controller
            .0
            .ingress
            .lock()
            .map_err(|_| "Entrada de ejecuciones inválida")?;
        *accepting = false;
        Ok(())
    }
    /// Cerrar/cancelar primero transportes propios en composición; después unir antes de DB.
    pub fn join(&mut self) -> Result<()> {
        let ingress = self.close_ingress();
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| "El ejecutor terminó inesperadamente")?;
        }
        ingress
    }
}
impl Drop for Runner {
    fn drop(&mut self) {
        let _ = self.join();
    }
}
impl Controller {
    pub fn view(store: Store, runtime: Arc<dyn Runtime>) -> Self {
        let (wake, _) = mpsc::sync_channel(1);
        Self(Arc::new(Inner {
            store,
            runtime,
            clock: Arc::new(|| jiff::Timestamp::now().as_second()),
            ingress: Mutex::new(true),
            stopping: AtomicBool::new(false),
            storage_failed: AtomicBool::new(false),
            runner_active: false,
            wake,
        }))
    }
    pub fn request_stop(&self) {
        self.0.stopping.store(true, Ordering::Release);
        let _ = self.0.wake.try_send(());
    }
    pub fn close_ingress(&self) -> Result<()> {
        self.request_stop();
        *self
            .0
            .ingress
            .lock()
            .map_err(|_| "Entrada de ejecuciones inválida")? = false;
        Ok(())
    }
    pub fn request(&self, value: Value) -> Result<Value> {
        if self.0.stopping.load(Ordering::Acquire) {
            return Err("El ejecutor se está cerrando".into());
        }
        let accepting = self
            .0
            .ingress
            .lock()
            .map_err(|_| "Entrada de ejecuciones inválida")?;
        if !*accepting || self.0.stopping.load(Ordering::Acquire) {
            return Err("El ejecutor se está cerrando".into());
        }
        #[derive(Deserialize)]
        #[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
        enum Request {
            NoticeOrigin,
            List {
                assignment_id: i64,
            },
            Configure {
                configuration: Configure,
            },
            Pause {
                policy_id: String,
                expected_revision: u32,
            },
            RevokeGuide {
                policy_id: String,
                expected_revision: u32,
            },
            Runs {
                policy_id: String,
                before: Option<i64>,
            },
            RunDetails {
                run_id: i64,
            },
            Reconcile {
                run_id: i64,
                resolution: String,
            },
        }
        let result = match serde_json::from_value(value)? {
            Request::NoticeOrigin => {
                serde_json::to_value(self.0.runtime.notice_origin(None)?).map_err(Into::into)
            }
            Request::List { assignment_id } => {
                self.0
                    .store
                    .execution_policies(assignment_id)
                    .map(|mut value| {
                        value["runner_active"] = json!(self.0.runner_active);
                        value["storage_failed"] =
                            json!(self.0.storage_failed.load(Ordering::Acquire));
                        value
                    })
            }
            Request::Configure { configuration } => self.configure(configuration),
            Request::Pause {
                policy_id,
                expected_revision,
            } => self.0.store.pause_execution(&policy_id, expected_revision),
            Request::RevokeGuide {
                policy_id,
                expected_revision,
            } => self
                .0
                .store
                .revoke_contextual_notice(&policy_id, expected_revision),
            Request::Runs { policy_id, before } => self.0.store.execution_runs(&policy_id, before),
            Request::RunDetails { run_id } => {
                let mut details = self.0.store.execution_run_details(run_id)?;
                details["notification"] =
                    serde_json::to_value(self.0.runtime.notice_details(run_id)?)?;
                Ok(details)
            }
            Request::Reconcile { run_id, resolution } => {
                self.0.store.reconcile_execution(run_id, &resolution)
            }
        }?;
        let _ = self.0.wake.try_send(());
        Ok(result)
    }
    fn configure(&self, request: Configure) -> Result<Value> {
        if request.enabled && !self.0.runner_active {
            return Err("Activa Plugins para habilitar el ejecutor periódico".into());
        }
        request.target.validate()?;
        if !request.arguments.is_object()
            || serde_json::to_vec(&request.arguments)?.len() > 60000
            || !(1..=1440).contains(&request.max_lateness_minutes)
        {
            return Err("Parámetros o tolerancia de retraso inválidos".into());
        }
        let now = (self.0.clock)();
        if request
            .until
            .as_deref()
            .map(schedule::epoch)
            .transpose()?
            .is_some_and(|at| at <= now)
        {
            return Err("El vencimiento debe ser futuro".into());
        }
        let record = self.0.store.execution_assignment(&request.assignment)?;
        if request.contextual_notice {
            if request.trigger != Trigger::Preparation
                || request.delivery.is_none()
                || record.document.follow_up.is_none()
            {
                return Err("La valoración contextual necesita preparación interna, criterios de seguimiento y un chat autorizado".into());
            }
            if !self.0.runtime.guide_available()? {
                return Err("Configura y habilita la guía remota antes de autorizar valoraciones periódicas".into());
            }
        }
        if record.document.status != Status::Active {
            return Err("El encargo debe estar activo para configurar su ejecución".into());
        }
        let method = record
            .document
            .method
            .as_ref()
            .ok_or("Define primero el método del encargo")?;
        if method.plugin_id != request.target.plugin_id()
            || !method
                .functions
                .iter()
                .any(|f| f == request.target.function())
        {
            return Err("La función no pertenece al método acordado".into());
        }
        let route = self
            .0
            .runtime
            .prepare(&request.target, &request.arguments)?;
        let authorization = record
            .document
            .authorization
            .as_ref()
            .ok_or("Falta el alcance autorizado del encargo")?;
        if !authorization.allowed_routes.contains(&route) {
            return Err("La vía no está autorizada en el encargo".into());
        }
        let calendar =
            Calendar::from_document(&record.document, request.trigger, request.dst, now)?;
        let next = calendar
            .next_after(now)?
            .ok_or("No queda una ocurrencia futura para este horario")?;
        if request
            .until
            .as_deref()
            .map(schedule::epoch)
            .transpose()?
            .is_some_and(|at| next > at)
        {
            return Err("El horario queda después del vencimiento".into());
        }
        if let Some(destination) = &request.delivery {
            destination.validate()?;
            if self
                .0
                .runtime
                .notice_origin(Some(destination.conversation_id))?
                != *destination
            {
                return Err("El chat de destino cambió; vuelve a abrir la configuración".into());
            }
        }
        let policy = Policy {
            contextual_notice: request.contextual_notice,
            delivery: request.delivery,
            assignment: record.into(),
            trigger: request.trigger,
            calendar,
            target: request.target,
            arguments: request.arguments,
            route,
            misfire: request.misfire,
            max_lateness_minutes: request.max_lateness_minutes,
            until: request
                .until
                .as_deref()
                .map(schedule::epoch)
                .transpose()?
                .map(schedule::utc)
                .transpose()?,
        };
        self.0.store.save_execution(
            policy,
            request.expected_policy_revision,
            request.enabled,
            next,
        )
    }
    fn flush_notice(&self) -> Result<bool> {
        let Some(notice) = self.0.store.next_execution_notice()? else {
            return Ok(false);
        };
        self.0.runtime.accept_notice(&notice)?;
        self.0.store.accept_execution_notice(&notice)?;
        Ok(true)
    }
    fn step(&self) -> Result<bool> {
        if !self.0.runner_active {
            return Ok(false);
        }
        let now = (self.0.clock)();
        let (run_id, policy) = match self.0.store.reserve_execution(now)? {
            store::Due::Idle => return Ok(false),
            store::Due::Advanced => return Ok(true),
            store::Due::Run(id, policy) => (id, policy),
        };
        let prepare = self.0.runtime.prepare(&policy.target, &policy.arguments);
        if prepare.as_ref().ok() != Some(&policy.route) {
            self.0
                .store
                .finish_execution(run_id, "rejected", "preflight_failed", None, true)?;
            return Ok(true);
        }
        {
            let accepting = self
                .0
                .ingress
                .lock()
                .map_err(|_| "Entrada de ejecuciones inválida")?;
            if !*accepting
                || self.0.stopping.load(Ordering::Acquire)
                || !self.0.store.dispatch_execution(run_id, (self.0.clock)())?
            {
                self.0.store.finish_execution(
                    run_id,
                    "cancelled",
                    "dispatch_not_permitted",
                    None,
                    false,
                )?;
                return Ok(true);
            }
        }
        let result = self.0.runtime.execute(&policy.target, &policy.arguments);
        let (state, code, output, pause) = match result {
            Ok(value) if value["status"] == "ok" && value["review_required"] != true => {
                ("succeeded", "reported_success", Some(value), false)
            }
            Ok(value)
                if value["executed"] == false
                    || matches!(
                        value["status"].as_str(),
                        Some(
                            "revision_conflict"
                                | "permission_required"
                                | "verification_required"
                                | "function_not_found"
                                | "input_required"
                                | "invalid_arguments"
                        )
                    ) =>
            {
                ("rejected", "rejected_before_effect", None, true)
            }
            Ok(value) => {
                let reference = value["web_attempt_id"]
                    .as_i64()
                    .filter(|id| *id > 0)
                    .map(|id| json!({"web_attempt_id":id}))
                    .or_else(|| {
                        value["attempt_id"]
                            .as_i64()
                            .filter(|id| *id > 0)
                            .map(|id| json!({"mcp_attempt_id":id}))
                    });
                ("uncertain", "effect_unconfirmed", reference, true)
            }
            Err(_) => ("uncertain", "effect_unconfirmed", None, true),
        };
        self.0
            .store
            .finish_execution(run_id, state, code, output, pause)?;
        Ok(true)
    }
}
#[cfg(test)]
mod tests;
