//! Política declarativa de seguimiento; no evalúa fuentes ni ejecuta acciones.
use crate::text;
use gennie_core::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FollowUpPolicy {
    /// Qué observar y qué cambios conservar, incluso si no justifican interrumpir.
    pub scope: String,
    /// Preguntas/aspectos propios del encargo, sin catálogo fijo de etiquetas.
    #[serde(default)]
    pub signals: Vec<String>,
    /// Cómo ponderar contexto, cambios e historial para elegir una intervención.
    pub decision_guidance: String,
    /// Alcance delegado expresamente; nunca reemplaza los permisos del ejecutor.
    pub action_scope: Option<String>,
}

impl FollowUpPolicy {
    pub(crate) fn validate(&self, interval_minutes: Option<u32>) -> Result<()> {
        if interval_minutes.is_none() {
            return Err("El seguimiento necesita una cadencia de revisión interna".into());
        }
        text(&self.scope, 2000, "Qué observar")?;
        text(&self.decision_guidance, 4000, "Criterio de intervención")?;
        if self.signals.len() > 24 {
            return Err("El seguimiento admite hasta 24 aspectos a valorar".into());
        }
        for signal in &self.signals {
            text(signal, 500, "Aspecto a valorar")?;
        }
        if let Some(scope) = &self.action_scope {
            text(scope, 2000, "Acciones delegadas")?;
        }
        Ok(())
    }
}
