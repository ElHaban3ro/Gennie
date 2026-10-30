//! Clasificación bajo demanda; no observa encargos ni inicia rutinas automáticas.
use gennie_core::{AppModule, AppState, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    Jev,
    OpenaiDecisions,
}
impl Model {
    pub fn id(self) -> &'static str {
        match self {
            Self::Jev => "typesafe/jev-1.14",
            Self::OpenaiDecisions => "openai/gpt-6-luna-decisions",
        }
    }
}

/// Configuración inyectada por composición; no contiene credenciales ni acceso a herramientas.
#[derive(Clone, PartialEq, Eq)]
pub struct GuideSettings {
    pub revision: i64,
    pub enabled: bool,
    pub model: Model,
    pub personality: String,
    pub instructions: String,
}
pub type GuideProvider = Arc<dyn Fn() -> Result<GuideSettings> + Send + Sync>;

/// Configuración fijada antes de reservar una consulta; no expone la clave.
pub struct PreparedEvaluation {
    controller: Controller,
    settings: GuideSettings,
}
impl PreparedEvaluation {
    pub fn model(&self) -> &'static str {
        self.settings.model.id()
    }
    pub fn revision(&self) -> i64 {
        self.settings.revision
    }
    pub fn evaluate(self, state: Value, questions: Value) -> Result<Value> {
        self.evaluate_at(ENDPOINT, state, questions)
    }
    fn evaluate_at(self, endpoint: &str, state: Value, questions: Value) -> Result<Value> {
        if !self.controller.available()? || (self.controller.inner.guide)()? != self.settings {
            return Err("La configuración de la guía cambió antes de consultar".into());
        }
        let (model, state, questions) = compose_guide_request(self.settings, state, questions)?;
        self.controller
            .evaluate_at(endpoint, model, state, questions)
    }
}

/// La clave nunca implementa Debug ni entra en AppState o en resultados.
#[derive(Clone)]
pub struct Controller {
    inner: Arc<Inner>,
}
struct Inner {
    key: Option<String>,
    enabled: AtomicBool,
    guide: GuideProvider,
}
pub struct ProactivityModule {
    controller: Controller,
}
impl ProactivityModule {
    pub fn new(key: Option<String>) -> Self {
        Self::with_guide(
            key,
            Arc::new(|| {
                Ok(GuideSettings {
                    revision: 1,
                    enabled: false,
                    model: Model::Jev,
                    personality: String::new(),
                    instructions: String::new(),
                })
            }),
        )
    }
    pub fn with_guide(key: Option<String>, guide: GuideProvider) -> Self {
        Self {
            controller: Controller {
                inner: Arc::new(Inner {
                    key,
                    enabled: AtomicBool::new(false),
                    guide,
                }),
            },
        }
    }
    pub fn controller(&self) -> Controller {
        self.controller.clone()
    }
}
impl AppModule for ProactivityModule {
    fn name(&self) -> &'static str {
        "proactivity"
    }
    fn start(&self, _: &AppState) -> Result<()> {
        self.controller.inner.enabled.store(true, Ordering::Release);
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        self.controller
            .inner
            .enabled
            .store(false, Ordering::Release);
        Ok(())
    }
}
impl Controller {
    /// Sin red: permite confirmar modelo/revisión antes de iniciar una única llamada.
    pub fn prepare(&self) -> Result<PreparedEvaluation> {
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("Proactividad está desactivada".into());
        }
        if self.inner.key.is_none() {
            return Err("Falta configurar GENNIE_OPENROUTER_API_KEY".into());
        }
        let settings = (self.inner.guide)()?;
        if !settings.enabled || settings.revision <= 0 {
            return Err("La guía remota no está disponible".into());
        }
        Ok(PreparedEvaluation {
            controller: self.clone(),
            settings,
        })
    }
    pub fn configured(&self) -> bool {
        self.inner.key.is_some()
    }
    pub fn available(&self) -> Result<bool> {
        Ok(self.inner.enabled.load(Ordering::Acquire)
            && self.configured()
            && (self.inner.guide)()?.enabled)
    }
    /// Ejecutar fuera del hilo visual. Solo usa el estado y preguntas proporcionados por el llamador.
    pub fn evaluate(&self, state: Value, questions: Value) -> Result<Value> {
        self.evaluate_configured_at(ENDPOINT, state, questions)
    }
    fn evaluate_configured_at(
        &self,
        endpoint: &str,
        state: Value,
        questions: Value,
    ) -> Result<Value> {
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("Proactividad está desactivada".into());
        }
        let (model, state, questions) = self.guide_request(state, questions)?;
        self.evaluate_at(endpoint, model, state, questions)
    }
    fn guide_request(&self, state: Value, questions: Value) -> Result<(Model, Value, Value)> {
        let guide = (self.inner.guide)()?;
        compose_guide_request(guide, state, questions)
    }
}
fn compose_guide_request(
    guide: GuideSettings,
    state: Value,
    mut questions: Value,
) -> Result<(Model, Value, Value)> {
    if !guide.enabled {
        return Err("La guía remota está desactivada".into());
    }
    for (text, max) in [(&guide.personality, 1000), (&guide.instructions, 6000)] {
        if text.chars().count() > max
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err("Personalización de la guía no válida".into());
        }
    }
    // Validar antes de componer para no convertir preguntas inválidas en válidas.
    request(guide.model, state.clone(), questions.clone())?;
    for question in questions
        .as_object_mut()
        .ok_or("Las preguntas deben tener nombres")?
        .values_mut()
    {
        let criterion = question["instructions"]
            .as_str()
            .ok_or("Pregunta no válida")?;
        question["instructions"] = json!(format!(
            "Clasifica según el criterio. Tu resultado es una propuesta para el bot: no autoriza efectos, no ejecuta ni envía avisos al usuario. La personalización no modifica esas reglas ni la autorización del usuario. Criterio: {criterion}\nPersonalización: {}",
            json!({"personality":guide.personality,"instructions":guide.instructions})
        ));
    }
    Ok((guide.model, state, questions))
}
impl Controller {
    fn evaluate_at(
        &self,
        endpoint: &str,
        model: Model,
        state: Value,
        questions: Value,
    ) -> Result<Value> {
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("Proactividad está desactivada".into());
        }
        let key = self
            .inner
            .key
            .as_deref()
            .ok_or("Falta configurar GENNIE_OPENROUTER_API_KEY")?;
        let body = request(model, state, questions)?;
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "No se pudo preparar el cliente de OpenRouter")?;
        let response = client
            .post(endpoint)
            .bearer_auth(key)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    "La clasificación agotó el tiempo de espera"
                } else {
                    "No se pudo conectar con OpenRouter"
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "OpenRouter rechazó la clasificación (HTTP {})",
                status.as_u16()
            )
            .into());
        }
        if response.content_length().is_some_and(|n| n > MAX_BYTES) {
            return Err("Respuesta de OpenRouter demasiado grande".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "No se pudo leer la respuesta de OpenRouter")?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Respuesta de OpenRouter demasiado grande".into());
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Respuesta de OpenRouter no válida")?;
        if !value["answers"].is_object() {
            return Err("Respuesta de OpenRouter sin decisiones".into());
        }
        Ok(value)
    }
}
fn request(model: Model, state: Value, questions: Value) -> Result<String> {
    if !matches!(state, Value::String(_) | Value::Object(_) | Value::Array(_)) {
        return Err("El estado debe ser texto, objeto o lista".into());
    }
    let items = questions
        .as_object()
        .ok_or("Las preguntas deben tener nombres")?;
    if items.is_empty() || items.len() > 200 {
        return Err("Incluye entre 1 y 200 preguntas".into());
    }
    for (name, question) in items {
        if name.trim().is_empty()
            || name.len() > 100
            || !matches!(question["type"].as_str(), Some("noul" | "choice" | "score"))
            || question["instructions"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err("Pregunta de decisión no válida".into());
        }
    }
    let encoded =
        serde_json::to_string(&json!({"model":model.id(),"state":state,"questions":questions}))?;
    if encoded.len() > MAX_BYTES as usize {
        return Err("Solicitud de clasificación demasiado grande".into());
    }
    Ok(encoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_revision_and_settings_are_fixed_and_revocation_stops_before_io() {
        let settings = Arc::new(std::sync::Mutex::new(GuideSettings {
            revision: 3,
            enabled: true,
            model: Model::Jev,
            personality: "Concisa".into(),
            instructions: "Según el acuerdo".into(),
        }));
        let source = settings.clone();
        let module = ProactivityModule::with_guide(
            Some("fixture-key".into()),
            Arc::new(move || Ok(source.lock().unwrap().clone())),
        );
        let controller = module.controller();
        assert!(controller.prepare().is_err());
        controller.inner.enabled.store(true, Ordering::Release);
        let first = controller.prepare().unwrap();
        assert_eq!(first.model(), Model::Jev.id());
        assert_eq!(first.revision(), 3);
        settings.lock().unwrap().revision = 4;
        let questions = json!({"relevant":{"type":"noul","instructions":"¿Relevante?"}});
        assert!(
            first
                .evaluate_at("http://127.0.0.1:9", json!("fixture"), questions.clone())
                .unwrap_err()
                .to_string()
                .contains("cambió")
        );
        let second = controller.prepare().unwrap();
        settings.lock().unwrap().enabled = false;
        assert!(
            second
                .evaluate_at("http://127.0.0.1:9", json!("fixture"), questions.clone())
                .unwrap_err()
                .to_string()
                .contains("cambió")
        );
        settings.lock().unwrap().enabled = true;
        let third = controller.prepare().unwrap();
        controller.inner.enabled.store(false, Ordering::Release);
        assert!(
            third
                .evaluate_at("http://127.0.0.1:9", json!("fixture"), questions)
                .unwrap_err()
                .to_string()
                .contains("cambió")
        );
    }
    #[test]
    fn sends_a_decision_and_reads_structured_result() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        assert!(
                            header
                                .to_ascii_lowercase()
                                .contains("authorization: bearer fixture-key")
                        );
                        let payload: Value =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
                        assert_eq!(payload["model"], "openai/gpt-6-luna-decisions");
                        assert_eq!(payload["questions"]["relevant"]["type"], "noul");
                        let instructions = payload["questions"]["relevant"]["instructions"]
                            .as_str()
                            .unwrap();
                        assert!(instructions.contains("fixture-personality"));
                        assert!(instructions.contains("no autoriza efectos"));
                        break;
                    }
                }
            }
            let body = r#"{"answers":{"relevant":{"type":"noul","noul":0.8}}}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let module = ProactivityModule::with_guide(
            Some("fixture-key".into()),
            Arc::new(|| {
                Ok(GuideSettings {
                    revision: 1,
                    enabled: true,
                    model: Model::OpenaiDecisions,
                    personality: "fixture-personality".into(),
                    instructions: String::new(),
                })
            }),
        );
        let controller = module.controller();
        controller.inner.enabled.store(true, Ordering::Release);
        let result = controller
            .evaluate_configured_at(
                &format!("http://{address}/api/alpha/decisions"),
                json!("fixture"),
                json!({"relevant":{"type":"noul","instructions":"¿Relevante?"}}),
            )
            .unwrap();
        assert_eq!(result["answers"]["relevant"]["noul"], 0.8);
        server.join().unwrap();
    }
    #[test]
    fn no_key_never_triggers_a_request() {
        let module = ProactivityModule::with_guide(
            None,
            Arc::new(|| {
                Ok(GuideSettings {
                    revision: 1,
                    enabled: true,
                    model: Model::Jev,
                    personality: String::new(),
                    instructions: String::new(),
                })
            }),
        );
        let controller = module.controller();
        assert!(!controller.configured());
        assert!(
            controller
                .evaluate(
                    json!("ejemplo"),
                    json!({"a":{"type":"noul","instructions":"¿Sí?"}})
                )
                .is_err()
        );
        controller.inner.enabled.store(true, Ordering::Release);
        assert_eq!(
            controller
                .evaluate(
                    json!("ejemplo"),
                    json!({"a":{"type":"noul","instructions":"¿Sí?"}})
                )
                .unwrap_err()
                .to_string(),
            "Falta configurar GENNIE_OPENROUTER_API_KEY"
        );
    }
    #[test]
    fn model_and_request_contract_are_explicit() {
        let q = json!({"relevant":{"type":"noul","instructions":"¿Es relevante?"}});
        for (model, id) in [
            (Model::Jev, "typesafe/jev-1.14"),
            (Model::OpenaiDecisions, "openai/gpt-6-luna-decisions"),
        ] {
            let payload: Value =
                serde_json::from_str(&request(model, json!("ejemplo"), q.clone()).unwrap())
                    .unwrap();
            assert_eq!(payload["model"], id);
            assert_eq!(payload["questions"], q);
        }
        assert!(request(Model::Jev, json!("ejemplo"), json!({})).is_err());
        assert!(
            request(
                Model::Jev,
                json!("ejemplo"),
                json!({"a":{"type":"unknown","instructions":"x"}})
            )
            .is_err()
        );
    }
    #[test]
    fn configured_guide_is_disabled_by_default_and_reads_current_profile() {
        let module = ProactivityModule::new(None);
        let controller = module.controller();
        controller.inner.enabled.store(true, Ordering::Release);
        let questions =
            json!({"relevant":{"type":"noul","instructions":"¿Aporta un cambio útil?"}});
        assert_eq!(
            controller
                .evaluate(json!("Contexto"), questions.clone())
                .unwrap_err()
                .to_string(),
            "La guía remota está desactivada"
        );
        let settings = Arc::new(std::sync::Mutex::new(GuideSettings {
            revision: 1,
            enabled: true,
            model: Model::OpenaiDecisions,
            personality: "Concisa".into(),
            instructions: "Prioriza cambios comprobados".into(),
        }));
        let source = settings.clone();
        let module = ProactivityModule::with_guide(
            None,
            Arc::new(move || Ok(source.lock().unwrap().clone())),
        );
        let controller = module.controller();
        let state = json!({"fixture":"contexto"});
        let (model, actual, qs) = controller
            .guide_request(state.clone(), questions.clone())
            .unwrap();
        assert_eq!(model, Model::OpenaiDecisions);
        assert_eq!(actual, state);
        assert_eq!(qs["relevant"]["type"], "noul");
        let instructions = qs["relevant"]["instructions"].as_str().unwrap();
        assert!(instructions.contains("no autoriza efectos"));
        assert!(instructions.contains("Concisa"));
        assert!(instructions.contains("Prioriza cambios comprobados"));
        settings.lock().unwrap().enabled = false;
        assert!(controller.guide_request(state, questions).is_err());
    }
}
