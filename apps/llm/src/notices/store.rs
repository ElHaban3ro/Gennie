use super::*;
use gennie_db::params;
#[derive(Clone)]
pub(crate) struct Store(pub Database);
impl Store {
    pub(super) fn revoke_ready(&self, update: &Update) -> Result<()> {
        let reason = guidance::Decision::revoked().reason;
        self.0.with_connection(|c| {c.execute("UPDATE llm_execution_notices SET state='discarded',notice_action='record',notice_reason=?2,rendered_text=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='ready'",params![update.event_key,reason])?;Ok(())})
    }
    pub(crate) fn accept(&self, update: &Update) -> Result<()> {
        update.validate()?;
        let document = serde_json::to_string(update)?;
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let existing={let mut q=tx.prepare("SELECT document FROM llm_execution_notices WHERE event_key=?1")?;q.query_map([&update.event_key],|r|r.get::<_,String>(0))?.next().transpose()?};
            if let Some(existing)=existing {return if existing==document {Ok(())}else{Err("El aviso ya pertenece a otro resultado u origen".into())};}
            let count:i64=tx.query_row("SELECT (SELECT count(*) FROM llm_execution_notices WHERE state IN ('pending','rendering','ready'))+(SELECT count(*) FROM llm_delegation_updates WHERE state IN ('pending','rendering','ready'))",[],|r|r.get(0))?;
            if count>=512 {return Err("Hay demasiados avisos pendientes del bot".into());}
            tx.execute("INSERT INTO llm_execution_notices(event_key,bot_id,document) VALUES(?1,?2,?3)",params![update.event_key,update.origin.bot_id,document])?;
            tx.commit()?;Ok(())
        })
    }
    pub(crate) fn recover(&self) -> Result<()> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            tx.execute("UPDATE llm_execution_notices SET state='discarded',notice_action='record',notice_reason='La valoración se interrumpió. Se conserva el resultado sin repetir la consulta o enviar un aviso contextual.',guide_state=CASE WHEN guide_state='reserved' THEN 'interrupted' ELSE guide_state END,rendered_text=NULL,failure_code='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='rendering' AND json_extract(document,'$.guidance') IS NOT NULL AND json_extract(document,'$.state')='succeeded'",[])?;
            tx.execute("UPDATE llm_execution_notices SET state='ready',rendered_text=NULL,failure_code='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='rendering'",[])?;
            tx.commit()?;Ok(())
        })
    }
    pub(crate) fn next(&self) -> Result<Option<(Update, bool)>> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let row={let mut q=tx.prepare("SELECT document,state FROM llm_execution_notices WHERE state IN ('pending','ready') ORDER BY created_at,event_key LIMIT 1")?;q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.next().transpose()?};
            let result=if let Some((text,state))=row {
                let update:Update=serde_json::from_str(&text)?;
                if state=="pending" {tx.execute("UPDATE llm_execution_notices SET state='rendering',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1",[&update.event_key])?;}
                Some((update,state=="ready"))
            }else{None};
            tx.commit()?;Ok(result)
        })
    }
    pub(crate) fn reserve(&self, update: &Update, bot: &architects::Profile) -> Result<()> {
        let config=json!({"provider":bot.provider,"model":bot.model,"effort":bot.effort,"capabilities":[],"turn_timeout_seconds":delegation::TURN_TIMEOUT_SECONDS}).to_string();
        self.0.with_connection(|c| {
            if c.execute("UPDATE llm_execution_notices SET bot_revision=?2,render_config=?3,render_attempted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering' AND render_attempted_at IS NULL",params![update.event_key,bot.revision,config])?!=1 {return Err("No se pudo reservar la redacción del resultado".into());} Ok(())
        })
    }
    pub(crate) fn ready(
        &self,
        update: &Update,
        text: &str,
        revision: i64,
        failure: Option<&str>,
        duration: Option<i64>,
    ) -> Result<()> {
        self.0.with_connection(|c| {
            if c.execute("UPDATE llm_execution_notices SET state='ready',notice_action='notify',guide_state=CASE WHEN json_extract(document,'$.guidance') IS NOT NULL THEN 'bypassed' ELSE guide_state END,rendered_text=?2,bot_revision=?3,failure_code=?4,render_duration_ms=?5,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering'",params![update.event_key,text,revision,failure,duration])?!=1 {return Err("El aviso ya no está en preparación".into());}Ok(())
        })
    }
    pub(crate) fn deliver(&self, update: &Update, journal: &dyn ConversationJournal) -> Result<()> {
        let changed=self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let (state,text):(String,Option<String>)=tx.query_row("SELECT state,rendered_text FROM llm_execution_notices WHERE event_key=?1",[&update.event_key],|r|Ok((r.get(0)?,r.get(1)?)))?;
            if state!="ready" {return Ok(false);}
            let delivered=journal.deliver_execution_notice(&tx,update,text.as_deref().unwrap_or(&update.fallback()))?;
            tx.execute("UPDATE llm_execution_notices SET state=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1",params![update.event_key,if delivered{"delivered"}else{"discarded"}])?;
            tx.commit()?;Ok(delivered)
        })?;
        if changed {
            journal.committed();
        }
        Ok(())
    }
    pub(crate) fn discard(&self, update: &Update) -> Result<()> {
        self.0.with_connection(|c| {c.execute("UPDATE llm_execution_notices SET state='discarded',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state IN ('rendering','ready')",[&update.event_key])?;Ok(())})
    }
}

impl Store {
    pub(super) fn reserve_guide(
        &self,
        update: &Update,
        guide: &guidance::GuideInvocation,
    ) -> Result<()> {
        if guide.revision <= 0
            || guide.model.is_empty()
            || guide.model.len() > 200
            || guide.model.chars().any(char::is_control)
        {
            return Err("Configuración de guía inválida".into());
        }
        let config=json!({"model":guide.model,"revision":guide.revision,"provider":"openrouter","tools":[],"timeout_seconds":30}).to_string();
        self.0.with_connection(|c| {if c.execute("UPDATE llm_execution_notices SET guide_state='reserved',guide_config=?2,guide_attempted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering' AND guide_attempted_at IS NULL",params![update.event_key,config])?!=1 {return Err("La valoración ya fue reservada".into());}Ok(())})
    }
    pub(super) fn guide_unavailable(&self, update: &Update) -> Result<()> {
        self.0.with_connection(|c| {c.execute("UPDATE llm_execution_notices SET guide_state='unavailable' WHERE event_key=?1 AND state='rendering' AND guide_attempted_at IS NULL",[&update.event_key])?;Ok(())})
    }
    pub(super) fn finish_guide(
        &self,
        update: &Update,
        advice: Option<&serde_json::Value>,
        duration: i64,
    ) -> Result<()> {
        self.0.with_connection(|c| {if c.execute("UPDATE llm_execution_notices SET guide_state=?2,guide_advice=?3,guide_duration_ms=?4 WHERE event_key=?1 AND state='rendering' AND guide_state='reserved'",params![update.event_key,if advice.is_some(){"ready"}else{"failed"},advice.map(serde_json::to_string).transpose()?,duration])?!=1 {return Err("La valoración ya no está activa".into());}Ok(())})
    }
    pub(super) fn decide(
        &self,
        update: &Update,
        revision: i64,
        decision: &guidance::Decision,
        failure: Option<&str>,
        duration: Option<i64>,
    ) -> Result<()> {
        let action = serde_json::to_value(decision.action)?
            .as_str()
            .ok_or("Decisión no válida")?
            .to_owned();
        self.0.with_connection(|c| {if c.execute("UPDATE llm_execution_notices SET state=?2,notice_action=?3,notice_reason=?4,rendered_text=?5,bot_revision=CASE WHEN render_attempted_at IS NOT NULL THEN ?6 ELSE bot_revision END,failure_code=?7,render_duration_ms=?8,guide_state=CASE WHEN guide_state='reserved' THEN 'interrupted' ELSE guide_state END,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering'",params![update.event_key,if decision.action==guidance::Action::Record{"discarded"}else{"ready"},action,decision.reason,decision.message,revision,failure,duration])?!=1 {return Err("La decisión de aviso ya no está activa".into());}Ok(())})
    }
    pub(super) fn previous_decisions(&self, update: &Update) -> Result<Vec<serde_json::Value>> {
        self.0.with_connection(|c| {
            let mut q=c.prepare("SELECT document,notice_action,notice_reason,rendered_text FROM llm_execution_notices WHERE json_extract(document,'$.reference.policy_id')=?1 AND json_extract(document,'$.reference.policy_revision')=?2 AND json_extract(document,'$.origin.conversation_id')=?3 AND json_extract(document,'$.origin.epoch')=?4 AND json_extract(document,'$.reference.run_id')<?5 AND json_extract(document,'$.origin.provider_thread') IS ?6 AND json_extract(document,'$.origin.bot_id')=?7 AND json_extract(document,'$.guidance') IS NOT NULL AND notice_action IS NOT NULL ORDER BY json_extract(document,'$.reference.run_id') DESC LIMIT 3")?;
            let rows=q.query_map(params![update.reference.policy_id,update.reference.policy_revision,update.origin.conversation_id,update.origin.epoch,update.reference.run_id,update.origin.provider_thread,update.origin.bot_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let mut result=Vec::new();let mut remaining=8000usize;
            for (text,action,reason,message) in rows {
                let prior:Update=serde_json::from_str(&text)?;
                let observation_truncated=prior.observation_truncated||prior.observation.as_ref().is_some_and(|s|s.chars().count()>2000);
                let observation=prior.observation.map(|s|s.chars().take(2000).collect::<String>());
                let item=json!({"scheduled_at":prior.scheduled_at,"observation":observation,"observation_truncated":observation_truncated,"action":action,"reason":reason,"message":message});
                let count=item.to_string().chars().count();if count>remaining {break;}remaining-=count;result.push(item);
            }
            result.reverse();Ok(result)
        })
    }
    pub(crate) fn decision_details(&self, event: &str) -> Result<Option<serde_json::Value>> {
        self.0.with_connection(|c| {
            let mut q=c.prepare("SELECT state,guide_state,guide_config,guide_advice,guide_duration_ms,notice_action,notice_reason,failure_code,render_duration_ms,bot_revision FROM llm_execution_notices WHERE event_key=?1")?;
            q.query_map([event],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<i64>>(8)?,r.get::<_,Option<i64>>(9)?)))?.next().transpose()?.map(|(state,guide_state,config,advice,guide_duration,action,reason,failure,bot_duration,bot_revision)|{
                Ok(json!({"state":state,"guide_state":guide_state,"guide_configuration":config.map(|s|serde_json::from_str::<serde_json::Value>(&s)).transpose()?,"guide_advice":advice.map(|s|serde_json::from_str::<serde_json::Value>(&s)).transpose()?,"guide_duration_ms":guide_duration,"action":action,"reason":reason,"failure_code":failure,"bot_duration_ms":bot_duration,"bot_revision":bot_revision}))
            }).transpose()
        })
    }
}
