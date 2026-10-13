mod runtime;
mod settings;
use serde::{Deserialize, Serialize};
use serde_json::json;
pub use settings::{SaveResult, Settings};
use soc_core::{AppModule, AppState, Migration, Result};
use soc_db::{Database, params};
use std::{
    cell::Cell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Incomplete,
    Ambiguous,
    NoOrder,
    GodCall,
    Complete,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Prediction {
    pub route: Route,
    pub action: Option<String>,
    pub target: Option<String>,
    pub probability: Option<f64>,
    #[serde(default)]
    pub requested_action: Option<String>,
    #[serde(default)]
    pub availability: Option<Availability>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Availability {
    pub status: AvailabilityStatus,
    pub reason: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Available,
    Unavailable,
    Ambiguous,
}
impl Prediction {
    fn executable(&self) -> bool {
        self.route == Route::Complete
            && self
                .availability
                .as_ref()
                .is_none_or(|a| a.status == AvailabilityStatus::Available)
    }
    fn validate(&self) -> Result<()> {
        if self
            .requested_action
            .as_deref()
            .is_some_and(|action| !ACTIONS.contains(&action))
        {
            return Err("Intención fuera del catálogo".into());
        }
        if self
            .probability
            .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
        {
            return Err("Probabilidad inválida".into());
        }
        if self.route == Route::Complete {
            if self.probability.is_none() {
                return Err("Falta predicción de Laya".into());
            }
            let action = self.action.as_deref().ok_or("Acción ausente")?;
            if !ACTIONS.contains(&action)
                || (self.executable() && self.target.as_deref().is_none_or(str::is_empty))
            {
                return Err("Acción fuera del catálogo".into());
            }
        } else if self.action.is_some() || self.target.is_some() {
            return Err("La ruta no autoriza una acción".into());
        }
        Ok(())
    }
}
const ACTIONS: &[&str] = &[
    "open_app",
    "open_browser",
    "open_safari",
    "open_chrome",
    "open_discord",
    "open_files",
    "open_app_menu",
    "open_spotify",
    "open_music",
    "pause_music",
    "play_music",
    "next_track",
    "previous_track",
];

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub prediction: Option<Prediction>,
    pub verified: bool,
    pub message: String,
}
struct Request {
    text: String,
    execute: bool,
    generation: u64,
}
struct Reply {
    generation: u64,
    outcome: Outcome,
}
struct Worker {
    sender: SyncSender<Request>,
    replies: Option<Receiver<Reply>>,
    cancel: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    handle: JoinHandle<()>,
    warm_requested: Arc<AtomicBool>,
    release_requested: Arc<AtomicBool>,
    preferences: Option<settings::Client>,
    status: Arc<Mutex<ModelStatus>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Unloaded,
    Loading,
    Ready,
    Failed(String),
}
#[derive(Default)]
pub struct DecisionsModule {
    worker: Mutex<Option<Worker>>,
}

pub struct Controller {
    sender: SyncSender<Request>,
    replies: Receiver<Reply>,
    generation: Arc<AtomicU64>,
    status: Arc<Mutex<ModelStatus>>,
    pending: Cell<bool>,
    warm_requested: Arc<AtomicBool>,
    release_requested: Arc<AtomicBool>,
    preferences: settings::Client,
}
impl Controller {
    pub fn settings(&self) -> Settings {
        self.preferences.settings()
    }
    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        self.preferences.save(settings)
    }
    pub fn poll_settings(&self) -> Option<SaveResult> {
        self.preferences.poll()
    }
    pub fn load_model(&self) -> Result<()> {
        if self.is_pending() {
            return Err("Espera a que termine la orden antes de cargar Laya".into());
        }
        self.release_requested.store(false, Ordering::Release);
        self.warm_requested.store(true, Ordering::Release);
        Ok(())
    }
    pub fn unload_model(&self) {
        self.cancel();
        self.warm_requested.store(false, Ordering::Release);
        self.release_requested.store(true, Ordering::Release);
    }
    pub fn model_status(&self) -> ModelStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| ModelStatus::Failed("Estado de Laya inválido".into()))
    }
    pub fn submit(&self, text: &str, execute: bool) -> Result<()> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 1200 {
            return Err("Orden vacía o demasiado larga (máximo 1200 caracteres)".into());
        }
        if self.is_pending() {
            return Err("Hay una orden pendiente; espera su resultado".into());
        }
        self.sender
            .try_send(Request {
                text: text.into(),
                execute,
                generation: self.generation.load(Ordering::Acquire),
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Hay una orden pendiente; espera su resultado",
                mpsc::TrySendError::Disconnected(_) => "El control local se ha detenido",
            })?;
        self.pending.set(true);
        Ok(())
    }
    pub fn is_pending(&self) -> bool {
        self.pending.get()
    }
    pub fn poll(&self) -> Vec<Outcome> {
        let generation = self.generation.load(Ordering::Acquire);
        let mut outcomes = Vec::new();
        let disconnected = loop {
            match self.replies.try_recv() {
                Ok(reply) if reply.generation == generation => outcomes.push(reply.outcome),
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break false,
                Err(mpsc::TryRecvError::Disconnected) => break true,
            }
        };
        if !outcomes.is_empty() {
            self.pending.set(false);
        } else if disconnected && self.pending.replace(false) {
            if let Ok(mut status) = self.status.lock() {
                *status = ModelStatus::Failed("El control local se ha detenido".into());
            }
            outcomes.push(Outcome {
                prediction: None,
                verified: false,
                message: "El control local se ha detenido; no pude confirmar el resultado. Reinicia soc antes de dar otra orden.".into(),
            });
        }
        outcomes
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.pending.set(false);
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AppModule for DecisionsModule {
    fn name(&self) -> &'static str {
        "decisions"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_results.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_preferences.sql"),
            },
        ]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de decisiones inválido")?;
        if guard.is_some() {
            return Ok(());
        }
        // Los intentos interrumpidos jamás se reproducen al reiniciar.
        state.db.with_connection(|c| {
            c.execute("UPDATE decision_attempts SET status='uncertain',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE status='pending'", [])?;
            Ok(())
        })?;
        let db = state.db.clone();
        let mut settings = settings::load(&db)?;
        let (preferences, preference_requests, preference_replies) =
            settings::Client::channel(settings.clone());
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (tx, replies) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let epoch = generation.clone();
        let warm_requested = Arc::new(AtomicBool::new(false));
        let warm = warm_requested.clone();
        let release_requested = Arc::new(AtomicBool::new(false));
        let release = release_requested.clone();
        let status = Arc::new(Mutex::new(ModelStatus::Unloaded));
        let model_status = status.clone();
        let handle = std::thread::Builder::new()
            .name("local-decisions".into())
            .spawn(move || {
                let mut engine = Engine::default();
                let update_status = |value| {
                    if let Ok(mut status) = model_status.lock() {
                        *status = value;
                    }
                };
                let release_model = |engine: &mut Engine| {
                    if release.swap(false, Ordering::AcqRel) {
                        engine.process.take();
                        engine.clear_context();
                        update_status(ModelStatus::Unloaded);
                        eprintln!("event=model_unloaded module=decisions reason=manual");
                    }
                };
                loop {
                    // Persistir fuera de main y aplicar solo después del commit.
                    // Al cerrar se drena también el único guardado pendiente.
                    if let Ok(next) = preference_requests.try_recv() {
                        let result = settings::save(&db, &next)
                            .map(|()| {
                                settings = next.clone();
                                next
                            })
                            .map_err(|_| "No se pudieron guardar los ajustes de Laya".to_string());
                        let _ = preference_replies.send(result);
                    }
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    release_model(&mut engine);
                    if warm.swap(false, Ordering::AcqRel) {
                        update_status(ModelStatus::Loading);
                        let generation = epoch.load(Ordering::Acquire);
                        let cancelled = || {
                            stop.load(Ordering::Acquire)
                                || release.load(Ordering::Acquire)
                                || epoch.load(Ordering::Acquire) != generation
                        };
                        match engine.prepare(&cancelled) {
                            Ok(()) => update_status(ModelStatus::Ready),
                            Err(_) if cancelled() => {
                                engine.process.take();
                                update_status(ModelStatus::Unloaded);
                            }
                            Err(error) => {
                                engine.process.take();
                                update_status(ModelStatus::Failed(error.to_string()));
                            }
                        }
                    }
                    match requests.recv_timeout(Duration::from_millis(50)) {
                        Ok(request) => {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                            // Consumir una liberación previa antes de una orden nueva.
                            release_model(&mut engine);
                            if let Some(reply) = engine.handle_request(
                                &db,
                                request,
                                &epoch,
                                &|| stop.load(Ordering::Acquire),
                                &update_status,
                            ) && tx.send(reply).is_err()
                            {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if engine.release_if_idle(&settings, Instant::now()) {
                                update_status(ModelStatus::Unloaded);
                                eprintln!(
                                    "event=model_unloaded module=decisions reason=inactivity"
                                );
                            }
                        }
                    }
                }
                if let Ok(next) = preference_requests.try_recv() {
                    let result = settings::save(&db, &next)
                        .map(|()| next)
                        .map_err(|_| "No se pudieron guardar los ajustes de Laya".to_string());
                    let _ = preference_replies.send(result);
                }
            })?;
        *guard = Some(Worker {
            sender,
            replies: Some(replies),
            cancel,
            generation,
            handle,
            warm_requested,
            release_requested,
            preferences: Some(preferences),
            status,
        });
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        if let Some(worker) = self
            .worker
            .lock()
            .map_err(|_| "Worker de decisiones inválido")?
            .take()
        {
            worker.cancel.store(true, Ordering::Release);
            drop(worker.sender);
            worker
                .handle
                .join()
                .map_err(|_| "Worker de decisiones terminó inesperadamente")?;
        }
        Ok(())
    }
}
impl DecisionsModule {
    pub fn controller(&self) -> Result<Controller> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de decisiones inválido")?;
        let worker = guard.as_mut().ok_or("Decisiones no iniciadas")?;
        let preferences = worker
            .preferences
            .take()
            .ok_or("Decisiones ya conectadas")?;
        let replies = worker.replies.take().ok_or("Decisiones ya conectadas")?;
        if preferences.settings().preload {
            *worker
                .status
                .lock()
                .map_err(|_| "Estado de Laya inválido")? = ModelStatus::Loading;
            worker.warm_requested.store(true, Ordering::Release);
        }
        Ok(Controller {
            sender: worker.sender.clone(),
            replies,
            generation: worker.generation.clone(),
            status: worker.status.clone(),
            pending: Cell::new(false),
            warm_requested: worker.warm_requested.clone(),
            release_requested: worker.release_requested.clone(),
            preferences,
        })
    }
}

struct Engine {
    process: Option<runtime::Process>,
    pending: Option<(String, Instant)>,
    pending_action: Option<String>,
    last_target: Option<String>,
    last_confirmed: Option<Instant>,
    last_action: Option<String>,
    last_used: Instant,
    generation: u64,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            process: None,
            pending: None,
            pending_action: None,
            last_target: None,
            last_confirmed: None,
            last_action: None,
            last_used: Instant::now(),
            generation: 0,
        }
    }
}
impl Engine {
    fn release_if_idle(&mut self, settings: &Settings, now: Instant) -> bool {
        if settings.idle_expired(now.saturating_duration_since(self.last_used)) {
            return self.process.take().is_some();
        }
        false
    }
    fn clear_context(&mut self) {
        self.pending = None;
        self.pending_action = None;
        self.last_action = None;
        self.last_target = None;
        self.last_confirmed = None;
    }
    fn handle_request(
        &mut self,
        db: &Database,
        request: Request,
        epoch: &AtomicU64,
        stop: &impl Fn() -> bool,
        update_status: &impl Fn(ModelStatus),
    ) -> Option<Reply> {
        let generation = request.generation;
        let cancelled = || stop() || epoch.load(Ordering::Acquire) != generation;
        // Una orden retirada antes de empezar no invalida la precarga.
        if cancelled() {
            return None;
        }
        if self.process.is_none() {
            update_status(ModelStatus::Loading);
        }
        let result = self.run(db, request, &cancelled);
        // El trabajo de inferencia/comprobación no cuenta como inactividad.
        self.last_used = Instant::now();
        if cancelled() {
            // La respuesta IPC interrumpida ya no puede reutilizarse.
            self.process.take();
            self.clear_context();
            update_status(ModelStatus::Unloaded);
            return None;
        }
        let outcome = result.unwrap_or_else(|error| {
            self.process.take();
            self.clear_context();
            update_status(ModelStatus::Failed(error.to_string()));
            Outcome {
                prediction: None,
                verified: false,
                message: format!(
                    "Control local: {error}. La operación no se repetirá automáticamente."
                ),
            }
        });
        if self.process.is_some() {
            update_status(ModelStatus::Ready);
        }
        Some(Reply {
            generation,
            outcome,
        })
    }
    fn prepare(&mut self, cancel: &impl Fn() -> bool) -> Result<()> {
        if self.process.is_none() {
            self.process = Some(runtime::Process::start(&runtime::RuntimeConfig::load()?)?);
        }
        let reply = self
            .process
            .as_mut()
            .ok_or("Falta proceso de Laya")?
            .request(&json!({"op":"load"}), cancel)?;
        if reply.get("ready").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err("Laya no confirmó la carga".into());
        }
        self.last_used = Instant::now();
        Ok(())
    }
    fn run(
        &mut self,
        db: &Database,
        request: Request,
        cancel: &impl Fn() -> bool,
    ) -> Result<Outcome> {
        if cancel() {
            return Err("Orden cancelada".into());
        }
        if self.generation != request.generation {
            self.clear_context();
            self.generation = request.generation;
        }
        self.last_used = Instant::now();
        if self
            .last_confirmed
            .is_some_and(|time| time.elapsed() > Duration::from_secs(300))
        {
            self.last_action = None;
            self.last_target = None;
            self.last_confirmed = None;
        }
        if self.process.is_none() {
            self.prepare(cancel)?;
        }
        let pending = self
            .pending
            .as_ref()
            .filter(|(_, time)| time.elapsed() < Duration::from_secs(30))
            .map(|(text, _)| text.clone());
        if pending.is_none() {
            self.pending_action = None;
        }
        let process = self.process.as_mut().ok_or("Falta proceso de Laya")?;
        let prediction: Prediction = serde_json::from_value(process.request(&json!({"op":"decide", "text":request.text, "pending":pending, "pending_action":if pending.is_some() { self.pending_action.as_ref() } else { None }, "last_action":self.last_action, "last_target":self.last_target}), cancel)?)?;
        prediction.validate()?;
        self.pending = match prediction.route {
            Route::Incomplete | Route::Ambiguous => {
                let text = pending_context(pending.as_deref(), &request.text);
                Some((text, Instant::now()))
            }
            _ => None,
        };
        self.pending_action = if self.pending.is_some() {
            prediction
                .requested_action
                .clone()
                .or(self.pending_action.take())
        } else {
            None
        };
        let mut outcome = Outcome {
            prediction: Some(prediction.clone()),
            verified: false,
            message: match prediction.route {
                Route::Incomplete => "Continúa la frase con el atajo; estoy esperando más voz.",
                Route::Ambiguous => {
                    "Necesito que aclares la acción o la aplicación que quieres controlar."
                }
                Route::NoOrder => "No ejecutaré ninguna acción.",
                Route::GodCall => "Esta petición está fuera del catálogo. La integración para resolverla sigue pendiente.",
                Route::Complete => "Predicción disponible; sin ejecución.",
            }
            .into(),
        };
        if let Some(availability) = &prediction.availability {
            match availability.status {
                AvailabilityStatus::Unavailable => {
                    outcome.message = availability_message(
                        prediction.action.as_deref(),
                        availability.reason.as_deref(),
                    )
                }
                AvailabilityStatus::Ambiguous => {
                    outcome.message = "¿Quieres controlar Spotify o Música de Apple?".into()
                }
                AvailabilityStatus::Available => {}
            }
        }
        if !prediction.executable() || !request.execute || cancel() {
            return Ok(outcome);
        }
        let action = prediction.action.as_deref().ok_or("Falta acción")?;
        let id = begin_attempt(db, action, prediction.target.as_deref())?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Effect {
            verified: bool,
            #[serde(default)]
            uncertain: bool,
            evidence: String,
        }
        let effect = process
            .request(
                &json!({"op":"execute", "action":action, "target":prediction.target}),
                cancel,
            )
            .and_then(|value| Ok(serde_json::from_value::<Effect>(value)?));
        match effect {
            Ok(effect) => {
                finish_attempt(
                    db,
                    id,
                    if effect.verified {
                        "verified"
                    } else if effect.uncertain {
                        "uncertain"
                    } else {
                        "failed"
                    },
                    Some(&effect.evidence),
                )?;
                outcome.verified = effect.verified;
                outcome.message = if effect.verified {
                    self.last_action = Some(action.into());
                    self.last_target = prediction.target.clone();
                    self.last_confirmed = Some(Instant::now());
                    success_message(action).into()
                } else {
                    self.last_action = None;
                    self.last_target = None;
                    if effect.uncertain {
                        "La orden se envió, pero no pude comprobar el resultado. No la repetiré automáticamente."
                    } else {
                        "No pude comprobar el resultado. La operación no se repetirá automáticamente."
                    }.into()
                };
            }
            Err(error) => {
                self.last_action = None;
                self.last_target = None;
                finish_attempt(db, id, "uncertain", None)?;
                return Err(error);
            }
        }
        Ok(outcome)
    }
}
fn pending_context(previous: Option<&str>, latest: &str) -> String {
    let mut phrases = vec![latest];
    let mut length = latest.chars().count();
    for phrase in previous.into_iter().flat_map(|text| text.rsplit('\n')) {
        let additional = phrase.chars().count() + 1;
        if length + additional > 1200 {
            break;
        }
        phrases.push(phrase);
        length += additional;
    }
    phrases.reverse();
    phrases.join("\n")
}
fn availability_message(action: Option<&str>, reason: Option<&str>) -> String {
    match reason {
        Some("no_player") => "La orden está clara, pero no hay un reproductor abierto.".into(),
        Some("player_not_running") => {
            "La orden está clara, pero ese reproductor no está abierto.".into()
        }
        Some("player_unreadable") => {
            "No puedo comprobar el estado del reproductor. Revisa el permiso de Automatización."
                .into()
        }
        _ => format!(
            "{} no está disponible en este ordenador.",
            match action {
                Some("open_chrome") => "Chrome",
                Some("open_safari") => "Safari",
                Some("open_discord") => "Discord",
                Some("open_spotify") => "Spotify",
                Some("open_music") => "Música",
                Some("open_app_menu") => "El lanzador de aplicaciones",
                Some("open_browser") => "El navegador predeterminado",
                _ => "La aplicación solicitada",
            }
        ),
    }
}
fn success_message(action: &str) -> &'static str {
    match action {
        "open_app" => "La aplicación solicitada está en primer plano.",
        "open_browser" => "Tu navegador está en primer plano.",
        "open_safari" => "Safari está en primer plano.",
        "open_chrome" => "Chrome está en primer plano.",
        "open_discord" => "Discord está en primer plano.",
        "open_files" => "Finder está en primer plano.",
        "open_app_menu" => "El menú de aplicaciones está abierto.",
        "open_spotify" => "Spotify está en primer plano.",
        "open_music" => "Música está en primer plano.",
        "pause_music" => "La reproducción está detenida.",
        "play_music" => "La música está reproduciéndose.",
        "next_track" => "La pista cambió al pedir la siguiente canción.",
        "previous_track" => "La pista cambió al pedir la canción anterior.",
        _ => "Resultado comprobado.",
    }
}
fn begin_attempt(db: &Database, action: &str, target: Option<&str>) -> Result<i64> {
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO decision_attempts(action,target,status) VALUES(?1,?2,'pending')",
            params![action, target],
        )?;
        Ok(c.last_insert_rowid())
    })
}
fn finish_attempt(db: &Database, id: i64, status: &str, evidence: Option<&str>) -> Result<()> {
    db.with_connection(|c| {
        if c.execute("UPDATE decision_attempts SET status=?2,evidence=?3,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND status='pending'", params![id,status,evidence])? != 1 {
            return Err("Intento ya finalizado".into());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn model_release_respects_configured_idle_time_without_waiting() {
        let now = Instant::now();
        let mut engine = Engine {
            process: Some(runtime::Process::fake("import sys\nsys.stdin.readline()").unwrap()),
            last_used: now,
            ..Engine::default()
        };
        assert!(!engine.release_if_idle(&Settings::default(), now + Duration::from_secs(599)));
        let mut settings = Settings {
            preload: false,
            unload_seconds: Some(1800),
        };
        assert!(!engine.release_if_idle(&settings, now + Duration::from_secs(600)));
        settings.unload_seconds = None;
        assert!(!engine.release_if_idle(&settings, now + Duration::from_secs(86400)));
        assert!(engine.process.is_some());
        settings.unload_seconds = Some(1800);
        assert!(engine.release_if_idle(&settings, now + Duration::from_secs(1800)));
        assert!(engine.process.is_none());
    }

    #[test]
    fn disabled_preload_stays_unloaded_and_pending_settings_commit_on_shutdown() {
        let db = Database::memory().unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let initial = Settings {
            preload: false,
            ..Settings::default()
        };
        settings::save(&db, &initial).unwrap();
        let state = AppState {
            config: Arc::new(soc_config::Config {
                database_path: "/tmp/soc-unused-laya-settings.sqlite3".into(),
                db_busy_timeout_ms: 50,
            }),
            db: db.clone(),
            shutdown: soc_core::Shutdown::default(),
        };
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        assert_eq!(controller.model_status(), ModelStatus::Unloaded);
        assert!(!controller.warm_requested.load(Ordering::Acquire));
        let next = Settings {
            unload_seconds: None,
            ..initial
        };
        controller.save_settings(next.clone()).unwrap();
        module.stop(&state).unwrap();
        assert_eq!(controller.poll_settings().unwrap().unwrap(), next);
        assert_eq!(settings::load(&db).unwrap(), next);
    }

    fn test_controller() -> (Controller, Receiver<Request>, mpsc::Sender<Reply>) {
        let (sender, requests) = mpsc::sync_channel(1);
        let (replies, receiver) = mpsc::channel();
        (
            Controller {
                sender,
                replies: receiver,
                generation: Arc::new(AtomicU64::new(0)),
                status: Arc::new(Mutex::new(ModelStatus::Ready)),
                pending: Cell::new(false),
                warm_requested: Arc::new(AtomicBool::new(false)),
                release_requested: Arc::new(AtomicBool::new(false)),
                preferences: settings::Client::channel(Settings::default()).0,
            },
            requests,
            replies,
        )
    }
    fn reply(generation: u64) -> Reply {
        Reply {
            generation,
            outcome: Outcome {
                prediction: None,
                verified: false,
                message: "Resultado de prueba".into(),
            },
        }
    }
    #[test]
    fn cancellation_discards_queued_results_without_clearing_the_new_request() {
        let (controller, requests, replies) = test_controller();
        controller.submit("abre Safari", false).unwrap();
        let old = requests.recv().unwrap();
        replies.send(reply(old.generation)).unwrap();
        controller.cancel();
        assert!(!controller.is_pending());
        controller.submit("abre Discord", false).unwrap();
        let current = requests.recv().unwrap();
        assert!(controller.poll().is_empty());
        assert!(controller.is_pending());
        replies.send(reply(current.generation)).unwrap();
        assert_eq!(controller.poll().len(), 1);
        assert!(!controller.is_pending());
    }
    #[test]
    fn controller_allows_one_unresolved_request_and_reports_disconnection() {
        let (controller, requests, replies) = test_controller();
        controller.submit("abre Safari", false).unwrap();
        let first = requests.recv().unwrap();
        assert!(controller.submit("abre Discord", false).is_err());
        replies.send(reply(first.generation)).unwrap();
        controller.poll();
        drop(requests);
        let error = controller.submit("abre Discord", false).unwrap_err();
        assert!(error.to_string().contains("se ha detenido"));
        assert!(!controller.is_pending());
    }
    #[test]
    fn lost_worker_finishes_pending_request_once_without_overwriting_delivered_results() {
        let (controller, requests, replies) = test_controller();
        controller.submit("abre Safari", false).unwrap();
        requests.recv().unwrap();
        drop(replies);
        let outcomes = controller.poll();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].message.contains("no pude confirmar"));
        assert!(!outcomes[0].verified);
        assert!(!controller.is_pending());
        assert!(matches!(controller.model_status(), ModelStatus::Failed(_)));
        assert!(controller.poll().is_empty());

        let (controller, requests, replies) = test_controller();
        controller.submit("abre Safari", false).unwrap();
        let request = requests.recv().unwrap();
        replies.send(reply(request.generation)).unwrap();
        drop(replies);
        let outcomes = controller.poll();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].message, "Resultado de prueba");
        assert!(!controller.is_pending());
        assert!(controller.poll().is_empty());
    }
    #[test]
    fn pending_context_keeps_latest_complete_utterances_within_the_budget() {
        let old = "á".repeat(1190);
        let previous = format!("{old}\nabriendo");
        let latest = "no, mejor abre Discord";
        let context = pending_context(Some(&previous), latest);
        assert_eq!(context, format!("abriendo\n{latest}"));
        assert!(context.chars().count() <= 1200);
        let latest = "á".repeat(1200);
        assert_eq!(pending_context(Some("anterior"), &latest), latest);
    }
    #[cfg(unix)]
    #[test]
    fn cancelled_queued_request_keeps_the_preloaded_process_ready() {
        let db = Database::memory().unwrap();
        let mut engine = Engine {
            process: Some(runtime::Process::fake("import sys\nfor line in sys.stdin: raise AssertionError('cancelled request reached helper')").unwrap()),
            ..Engine::default()
        };
        let status = std::cell::RefCell::new(ModelStatus::Ready);
        let result = engine.handle_request(
            &db,
            Request {
                text: "abre Safari".into(),
                execute: true,
                generation: 0,
            },
            &AtomicU64::new(1),
            &|| false,
            &|value| *status.borrow_mut() = value,
        );
        assert!(result.is_none());
        assert!(engine.process.is_some());
        assert_eq!(*status.borrow(), ModelStatus::Ready);
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_during_request_clears_context_without_reporting_model_failure() {
        let db = Database::memory().unwrap();
        let mut engine = Engine {
            process: Some(
                runtime::Process::fake("import sys,time\nsys.stdin.readline()\ntime.sleep(30)")
                    .unwrap(),
            ),
            last_action: Some("open_app".into()),
            last_target: Some("com.apple.Safari".into()),
            ..Engine::default()
        };
        let status = std::cell::RefCell::new(ModelStatus::Ready);
        let now = Instant::now();
        let result = engine.handle_request(
            &db,
            Request {
                text: "abre Safari".into(),
                execute: true,
                generation: 0,
            },
            &AtomicU64::new(0),
            &|| now.elapsed() >= Duration::from_millis(100),
            &|value| *status.borrow_mut() = value,
        );
        assert!(result.is_none());
        assert!(engine.process.is_none());
        assert!(engine.last_action.is_none());
        assert!(engine.last_target.is_none());
        assert_eq!(*status.borrow(), ModelStatus::Unloaded);
        assert!(now.elapsed() < Duration::from_secs(3));
    }
    #[cfg(unix)]
    #[test]
    fn expired_pending_action_cannot_leak_into_the_next_clarification() {
        let db = Database::memory().unwrap();
        let script = r#"import json,sys
for line in sys.stdin:
    request=json.loads(line)
    assert request['pending'] is None
    assert request['pending_action'] is None
    print(json.dumps({'route':'ambiguous','action':None,'target':None,'probability':.8}),flush=True)
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(script).unwrap()),
            pending: Some(("pausa".into(), Instant::now() - Duration::from_secs(31))),
            pending_action: Some("pause_music".into()),
            ..Engine::default()
        };
        engine
            .run(
                &db,
                Request {
                    text: "ahora".into(),
                    execute: false,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert!(engine.pending_action.is_none());
    }
    #[cfg(unix)]
    #[test]
    fn failed_request_drops_the_helper_and_allows_a_new_explicit_request() {
        let db = Database::memory().unwrap();
        let mut engine = Engine {
            process: Some(
                runtime::Process::fake(
                    "import sys\nsys.stdin.readline()\nprint('{\"error\":\"failure\"}',flush=True)",
                )
                .unwrap(),
            ),
            pending: Some(("orden anterior".into(), Instant::now())),
            pending_action: Some("pause_music".into()),
            ..Engine::default()
        };
        let status = std::cell::RefCell::new(ModelStatus::Ready);
        let request = || Request {
            text: "nueva orden".into(),
            execute: false,
            generation: 0,
        };
        let epoch = AtomicU64::new(0);
        let result = engine
            .handle_request(&db, request(), &epoch, &|| false, &|value| {
                *status.borrow_mut() = value
            })
            .unwrap();
        assert!(result.outcome.prediction.is_none());
        assert!(matches!(*status.borrow(), ModelStatus::Failed(_)));
        assert!(engine.process.is_none());
        assert!(engine.pending.is_none());
        assert!(engine.pending_action.is_none());
        engine.process = Some(runtime::Process::fake("import sys\nsys.stdin.readline()\nprint('{\"route\":\"no_order\",\"action\":null,\"target\":null,\"probability\":0.99}',flush=True)").unwrap());
        let result = engine
            .handle_request(&db, request(), &epoch, &|| false, &|value| {
                *status.borrow_mut() = value
            })
            .unwrap();
        assert_eq!(result.outcome.prediction.unwrap().route, Route::NoOrder);
        assert_eq!(*status.borrow(), ModelStatus::Ready);
    }
    #[cfg(unix)]
    #[test]
    fn clear_unavailable_intent_never_reaches_executor_or_journal() {
        let db = Database::memory().unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let script = r#"import json,sys
for line in sys.stdin:
    request=json.loads(line)
    assert request['op']=='decide'
    print(json.dumps({'route':'complete','action':'open_chrome','target':None,'probability':.99,'requested_action':'open_chrome','availability':{'status':'unavailable','reason':'application_missing'}}),flush=True)
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(script).unwrap()),
            ..Engine::default()
        };
        let outcome = engine
            .run(
                &db,
                Request {
                    text: "abre Chrome".into(),
                    execute: true,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert!(outcome.message.contains("Chrome"));
        assert!(!outcome.verified);
        assert_eq!(outcome.prediction.unwrap().route, Route::Complete);
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT count(*) FROM decision_attempts",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .unwrap(),
            0
        );
    }
    #[cfg(unix)]
    #[test]
    fn pending_action_is_transient_and_confirmed_context_expires() {
        let db = Database::memory().unwrap();
        let script = r#"import json,sys
step=0
for line in sys.stdin:
    request=json.loads(line)
    assert request['last_action'] is None
    if step==0:
        answer={'route':'ambiguous','action':None,'target':None,'probability':.8,'requested_action':'pause_music','availability':{'status':'ambiguous','reason':'choose_player'}}
    else:
        assert request['pending_action']=='pause_music'
        answer={'route':'complete','action':'pause_music','target':'com.spotify.client','probability':.99}
    print(json.dumps(answer),flush=True)
    step+=1
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(script).unwrap()),
            last_action: Some("open_safari".into()),
            last_confirmed: Some(Instant::now() - Duration::from_secs(301)),
            ..Engine::default()
        };
        engine
            .run(
                &db,
                Request {
                    text: "pausa la música".into(),
                    execute: false,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        engine
            .run(
                &db,
                Request {
                    text: "Spotify".into(),
                    execute: false,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert!(engine.pending.is_none());
    }
    #[cfg(unix)]
    #[test]
    fn preloading_reuses_the_process_without_issuing_an_action() {
        let source = r#"import json,sys
for line in sys.stdin:
    request=json.loads(line)
    assert request['op']=='load'
    print(json.dumps({'ready':True}),flush=True)
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(source).unwrap()),
            ..Engine::default()
        };
        engine.prepare(&|| false).unwrap();
        engine.prepare(&|| false).unwrap();
        assert!(engine.process.is_some());
        assert!(engine.last_action.is_none());
        assert!(engine.pending.is_none());
    }

    #[test]
    fn non_executing_routes_and_unknown_actions_cannot_authorize_execution() {
        for route in [
            Route::Incomplete,
            Route::Ambiguous,
            Route::NoOrder,
            Route::GodCall,
        ] {
            let mut p = Prediction {
                route,
                action: None,
                target: None,
                probability: Some(0.8),
                requested_action: None,
                availability: None,
            };
            assert!(p.validate().is_ok());
            p.action = Some("open_chrome".into());
            assert!(p.validate().is_err());
        }
        let p = Prediction {
            route: Route::Complete,
            action: Some("delete_files".into()),
            target: Some("x".into()),
            probability: Some(1.0),
            requested_action: None,
            availability: None,
        };
        assert!(p.validate().is_err());
    }
    #[test]
    fn journal_keeps_verified_results_separate_and_refuses_second_finalization() {
        let db = Database::memory().unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let id = begin_attempt(&db, "next_track", Some("com.spotify.client")).unwrap();
        finish_attempt(&db, id, "failed", Some("effect_not_observed")).unwrap();
        assert!(finish_attempt(&db, id, "verified", None).is_err());
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT count(*) FROM decision_attempts WHERE status='verified'",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .unwrap(),
            0
        );
    }
    #[cfg(unix)]
    #[test]
    fn unreadable_verification_is_uncertain_and_keeps_model_ready() {
        let db = Database::memory().unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let source = r#"import json,sys
executions=0
for line in sys.stdin:
    request=json.loads(line)
    if request['op']=='decide':
        assert request['last_action'] is None
        response={'route':'complete','action':'open_app','target':'com.spotify.client','probability':.99}
    else:
        executions+=1
        assert executions==1
        response={'verified':False,'uncertain':True,'evidence':'observation_unavailable'}
    print(json.dumps(response),flush=True)
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(source).unwrap()),
            ..Engine::default()
        };
        let status = std::cell::RefCell::new(ModelStatus::Ready);
        let reply = engine
            .handle_request(
                &db,
                Request {
                    text: "Por favor, abre Spotify.".into(),
                    execute: true,
                    generation: 0,
                },
                &AtomicU64::new(0),
                &|| false,
                &|value| *status.borrow_mut() = value,
            )
            .unwrap();
        assert!(!reply.outcome.verified);
        assert!(reply.outcome.message.contains("La orden se envió"));
        assert!(reply.outcome.prediction.is_some());
        assert_eq!(*status.borrow(), ModelStatus::Ready);
        assert!(engine.process.is_some());
        assert!(engine.last_action.is_none());
        engine
            .run(
                &db,
                Request {
                    text: "otra vez".into(),
                    execute: false,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT status,evidence FROM decision_attempts",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            )?))
            .unwrap(),
            ("uncertain".into(), "observation_unavailable".into())
        );
    }
    #[cfg(unix)]
    #[test]
    fn failed_effect_is_sent_once_and_does_not_become_repeat_context() {
        let db = Database::memory().unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let source = r#"import json,sys
executions=0
for line in sys.stdin:
    request=json.loads(line)
    if request['op']=='decide':
        assert request['last_action'] is None
        response={'route':'complete','action':'next_track','target':'com.spotify.client','probability':.9}
    else:
        executions+=1
        assert executions==1
        response={'verified':False,'evidence':'effect_not_observed'}
    print(json.dumps(response),flush=True)
"#;
        let mut engine = Engine {
            process: Some(runtime::Process::fake(source).unwrap()),
            ..Engine::default()
        };
        let result = engine
            .run(
                &db,
                Request {
                    text: "siguiente canción".into(),
                    execute: true,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert!(!result.verified);
        assert!(engine.last_action.is_none());
        let preview = engine
            .run(
                &db,
                Request {
                    text: "otra vez".into(),
                    execute: false,
                    generation: 0,
                },
                &|| false,
            )
            .unwrap();
        assert!(!preview.verified);
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT count(*) FROM decision_attempts",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .unwrap(),
            1
        );
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_during_inference_prevents_any_effect_and_closes_owned_child() {
        let db = Database::memory().unwrap();
        let mut engine = Engine {
            process: Some(
                runtime::Process::fake("import sys,time\nsys.stdin.readline()\ntime.sleep(30)")
                    .unwrap(),
            ),
            ..Engine::default()
        };
        let now = Instant::now();
        assert!(
            engine
                .run(
                    &db,
                    Request {
                        text: "abre Safari".into(),
                        execute: true,
                        generation: 0
                    },
                    &|| now.elapsed() > Duration::from_millis(100)
                )
                .is_err()
        );
        engine.process.take();
        assert!(now.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn restart_marks_pending_uncertain_without_replaying_and_preserves_verified() {
        let path =
            std::env::temp_dir().join(format!("soc-decisions-{}.sqlite3", std::process::id()));
        let db = Database::open(&path, 50).unwrap();
        let module = DecisionsModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let pending = begin_attempt(&db, "next_track", None).unwrap();
        let verified = begin_attempt(&db, "open_safari", Some("com.apple.Safari")).unwrap();
        finish_attempt(&db, verified, "verified", Some("observed")).unwrap();
        db.close().unwrap();
        let db = Database::open(&path, 50).unwrap();
        let state = AppState {
            config: Arc::new(soc_config::Config {
                database_path: path.clone(),
                db_busy_timeout_ms: 50,
            }),
            db: db.clone(),
            shutdown: soc_core::Shutdown::default(),
        };
        module.start(&state).unwrap();
        module.start(&state).unwrap();
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT status FROM decision_attempts WHERE id=?1",
                [pending],
                |r| r.get::<_, String>(0)
            )?))
            .unwrap(),
            "uncertain"
        );
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT status FROM decision_attempts WHERE id=?1",
                [verified],
                |r| r.get::<_, String>(0)
            )?))
            .unwrap(),
            "verified"
        );
        module.stop(&state).unwrap();
        module.stop(&state).unwrap();
        db.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
