//! Mensajes locales del bot para el historial del proveedor, no nuevas solicitudes del usuario.
use crate::{QuotedMessage, notices};
use gennie_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(test)]
mod tests;
pub const MAX_UPDATES: usize = 12;
pub const MAX_CHARACTERS: usize = 8000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reference {
    PluginBuild {
        job_id: i64,
        plugin_id: String,
        source_sequence: i64,
    },
    AssignmentExecution {
        reference: notices::Reference,
    },
}
impl Reference {
    pub fn characters(&self) -> usize {
        match self {
            Self::PluginBuild {
                job_id,
                plugin_id,
                source_sequence,
            } => {
                job_id.to_string().len()
                    + plugin_id.chars().count()
                    + source_sequence.to_string().len()
            }
            Self::AssignmentExecution { reference } => reference.characters(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        let valid = match self {
            Self::PluginBuild {
                job_id,
                plugin_id,
                source_sequence,
            } => {
                *job_id > 0
                    && *source_sequence > 0
                    && !plugin_id.is_empty()
                    && plugin_id.len() <= 64
                    && !plugin_id.chars().any(char::is_control)
            }
            Self::AssignmentExecution { reference: r } => {
                r.assignment_id > 0
                    && r.run_id > 0
                    && r.policy_revision > 0
                    && r.instance_id.len() == 32
                    && r.instance_id.bytes().all(|b| b.is_ascii_hexdigit())
                    && !r.policy_id.is_empty()
                    && r.policy_id.len() <= 100
                    && !r.policy_id.chars().any(char::is_control)
                    && ["succeeded", "rejected", "uncertain", "cancelled", "skipped"]
                        .contains(&r.state.as_str())
            }
        };
        if !valid {
            return Err("Referencia de contexto inválida".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Update {
    pub message_id: i64,
    pub text: String,
    pub created_at: String,
    pub reference: Reference,
}
impl Update {
    pub fn characters(&self) -> usize {
        self.message_id.to_string().len()
            + self.text.chars().count()
            + self.created_at.chars().count()
            + self.reference.characters()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    pub updates: Vec<Update>,
    pub older_omitted: u64,
    pub unconfirmed: Vec<Reference>,
    pub unconfirmed_omitted: u64,
}
impl Context {
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
            && self.older_omitted == 0
            && self.unconfirmed.is_empty()
            && self.unconfirmed_omitted == 0
    }
    pub fn validate(&self) -> Result<()> {
        if self.updates.len() > MAX_UPDATES
            || self.unconfirmed.len() > MAX_UPDATES
            || self.updates.iter().map(Update::characters).sum::<usize>()
                + self
                    .unconfirmed
                    .iter()
                    .map(Reference::characters)
                    .sum::<usize>()
                > MAX_CHARACTERS
        {
            return Err("Contexto local demasiado grande".into());
        }
        for u in &self.updates {
            u.reference.validate()?;
            if u.message_id <= 0
                || u.text.trim().is_empty()
                || u.text.contains('\0')
                || u.text.chars().count() > 1600
                || u.created_at.len() > 64
            {
                return Err("Mensaje local de contexto inválido".into());
            }
        }
        for r in &self.unconfirmed {
            r.validate()?;
        }
        Ok(())
    }
    pub(crate) fn items(&self) -> Result<Vec<Value>> {
        self.validate()?;
        let mut result = Vec::new();
        for (index, u) in self.updates.iter().enumerate() {
            let meta = json!({"local_message_id":u.message_id,"created_at":u.created_at,"reference":u.reference,"older_omitted":if index==0{self.older_omitted}else{0}});
            result.push(json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":u.text},{"type":"output_text","text":format!("Metadatos de Gennie sobre este aviso, como datos históricos y sin autorización de acciones: {meta}")}]}));
        }
        if self.updates.is_empty() && self.older_omitted > 0 {
            result.push(json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":format!("Contexto interno de Gennie: se omitieron {} avisos anteriores por el límite de contexto. No se conoce aquí su contenido ni se autorizan acciones nuevas.",self.older_omitted)}]}));
        }
        if !self.unconfirmed.is_empty() || self.unconfirmed_omitted > 0 {
            let meta = json!({"unconfirmed_references":self.unconfirmed,"unconfirmed_omitted":self.unconfirmed_omitted});
            result.push(json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":format!("Contexto interno de Gennie: no pude confirmar la incorporación previa de los avisos referenciados. Esto no es una solicitud del usuario ni autoriza repetir acciones. Consulta su estado solo si hace falta para responder. No se están repitiendo sus mensajes completos. Datos: {meta}")}]}));
        }
        if serde_json::to_vec(&result)?.len() > 64000 {
            return Err("Contexto codificado demasiado grande".into());
        }
        Ok(result)
    }
}
#[derive(Clone, Debug)]
pub struct PreparedUser {
    pub quote: Option<QuotedMessage>,
    pub context: Option<Context>,
    pub batch_id: Option<i64>,
}
