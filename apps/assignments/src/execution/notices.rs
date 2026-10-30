//! Outbox de hechos de ejecución; el bot decide su redacción y posee la entrega al chat.
use super::*;
use gennie_db::{SqliteConnection, params};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub conversation_id: i64,
    pub bot_id: String,
    pub epoch: i64,
    pub provider_thread: Option<String>,
}
impl Delivery {
    pub fn validate(&self) -> Result<()> {
        if self.conversation_id <= 0
            || self.epoch <= 0
            || self.bot_id != "gennie"
            || self
                .provider_thread
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 200 || s.chars().any(char::is_control))
        {
            return Err("El chat de destino no es válido".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Guidance {
    pub objective: String,
    pub scope: String,
    pub signals: Vec<String>,
    pub decision_guidance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<Guidance>,
    pub event_key: String,
    pub destination: Delivery,
    pub assignment_id: i64,
    pub instance_id: String,
    pub policy_id: String,
    pub policy_revision: u32,
    pub run_id: i64,
    pub title: String,
    pub scheduled_at: String,
    pub state: String,
    pub code: String,
    pub observation: Option<String>,
    pub observation_truncated: bool,
}
impl Notice {
    pub fn validate(&self) -> Result<()> {
        self.destination.validate()?;
        if let Some(g) = &self.guidance {
            for (text, max) in [
                (&g.objective, 4000),
                (&g.scope, 2000),
                (&g.decision_guidance, 4000),
            ] {
                crate::text(text, max, "Criterio de aviso")?;
            }
            if g.signals.len() > 24 {
                return Err("Demasiados aspectos de seguimiento".into());
            }
            for signal in &g.signals {
                crate::text(signal, 500, "Aspecto de seguimiento")?;
            }
        }
        if self.assignment_id <= 0
            || self.run_id <= 0
            || self.policy_revision == 0
            || self.event_key != format!("assignment-execution:{}", self.run_id)
            || self.instance_id.len() != 32
            || !self.instance_id.bytes().all(|b| b.is_ascii_hexdigit())
            || self.policy_id.is_empty()
            || self.policy_id.len() > 100
            || self.policy_id.chars().any(char::is_control)
            || self.title.trim().is_empty()
            || self.title.chars().count() > 200
            || self.title.contains('\0')
            || !["succeeded", "rejected", "uncertain", "cancelled", "skipped"]
                .contains(&self.state.as_str())
            || self.code.is_empty()
            || self.code.len() > 100
            || !self
                .code
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'_')
            || self
                .observation
                .as_ref()
                .is_some_and(|s| s.chars().count() > 8000 || s.contains('\0'))
        {
            return Err("El hecho de ejecución no es válido".into());
        }
        schedule::epoch(&self.scheduled_at)?;
        Ok(())
    }
}
pub(super) fn queue(c: &SqliteConnection, run_id: i64) -> Result<()> {
    let (id,revision,at,state,code,output,omitted)=c.query_row("SELECT policy_id,policy_revision,scheduled_at,state,code,output,output_omitted FROM assignment_execution_runs WHERE id=?1",[run_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,bool>(6)?)))?;
    if ["reserved", "running"].contains(&state.as_str()) {
        return Ok(());
    }
    let policy: Policy = c
        .query_row(
            "SELECT document FROM assignment_execution_versions WHERE policy_id=?1 AND revision=?2",
            params![id, revision],
            |r| r.get::<_, String>(0),
        )
        .map(|s| serde_json::from_str(&s))??;
    let Some(destination) = policy.delivery else {
        return Ok(());
    };
    let guidance = if policy.contextual_notice {
        let follow = policy
            .assignment
            .document
            .follow_up
            .as_ref()
            .ok_or("Faltan los criterios de seguimiento consentidos")?;
        Some(Guidance {
            objective: policy.assignment.document.description.clone(),
            scope: follow.scope.clone(),
            signals: follow.signals.clone(),
            decision_guidance: follow.decision_guidance.clone(),
        })
    } else {
        None
    };
    let original = output
        .filter(|_| state == "succeeded")
        .map(|s| s.replace('\0', ""));
    let truncated = omitted || original.as_ref().is_some_and(|s| s.chars().count() > 8000);
    let notice = Notice {
        guidance,
        event_key: format!("assignment-execution:{run_id}"),
        destination,
        assignment_id: policy.assignment.id,
        instance_id: policy.assignment.instance_id,
        policy_id: id,
        policy_revision: revision,
        run_id,
        title: policy.assignment.document.title,
        scheduled_at: at,
        state,
        code: code.unwrap_or_else(|| "outcome_recorded".into()),
        observation: original.map(|s| s.chars().take(8000).collect()),
        observation_truncated: truncated,
    };
    notice.validate()?;
    c.execute(
        "INSERT OR IGNORE INTO assignment_execution_notices(run_id,document) VALUES(?1,?2)",
        params![run_id, serde_json::to_string(&notice)?],
    )?;
    Ok(())
}
impl Store {
    pub(super) fn next_execution_notice(&self) -> Result<Option<Notice>> {
        self.db.with_connection(|c| {
            let mut q=c.prepare("SELECT document FROM assignment_execution_notices WHERE state='pending' ORDER BY run_id LIMIT 1")?;
            let text=q.query_map([],|r|r.get::<_,String>(0))?.next().transpose()?;
            text.map(|s|Ok(serde_json::from_str(&s)?)).transpose()
        })
    }
    pub(super) fn accept_execution_notice(&self, notice: &Notice) -> Result<()> {
        self.db.with_connection(|c| {
            if c.execute("UPDATE assignment_execution_notices SET state='accepted',accepted_at=COALESCE(accepted_at,strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE run_id=?1 AND document=?2",params![notice.run_id,serde_json::to_string(notice)?])?!=1 {return Err("El recibo no corresponde al aviso de ejecución".into());}
            Ok(())
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(())
    }
}
