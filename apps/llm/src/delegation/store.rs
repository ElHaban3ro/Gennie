use super::*;
use gennie_db::params;
#[derive(Clone)]
pub(super) struct Store(pub Database);
impl Store {
    pub fn accept(&self, update: &Update) -> Result<()> {
        update.validate()?;
        let document = serde_json::to_string(update)?;
        let origin = serde_json::to_string(&update.origin)?;
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let existing={let mut s=tx.prepare("SELECT document FROM llm_delegation_updates WHERE event_key=?1")?;s.query_map([&update.event_key],|r|r.get::<_,String>(0))?.next().transpose()?};
            if let Some(existing)=existing {
                if existing!=document { return Err("El evento ya pertenece a otra actualización".into()); }
                return Ok(());
            }
            let count:i64=tx.query_row("SELECT (SELECT count(*) FROM llm_delegation_updates WHERE state IN ('pending','rendering','ready'))+(SELECT count(*) FROM llm_execution_notices WHERE state IN ('pending','rendering','ready'))",[],|r|r.get(0))?;
            if count>=512 { return Err("Hay demasiadas actualizaciones pendientes del bot".into()); }
            tx.execute("INSERT OR IGNORE INTO llm_delegations(id,request_id,bot_id,source_id,origin,job_id,plugin_id,objective) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![update.delegation_id,update.request_id,update.origin.bot_id,update.source_id,origin,update.job_id,update.plugin_id,update.objective])?;
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM llm_delegations WHERE id=?1 AND request_id=?2 AND bot_id=?3 AND source_id=?4 AND origin=?5 AND job_id=?6 AND plugin_id=?7 AND objective=?8)",params![update.delegation_id,update.request_id,update.origin.bot_id,update.source_id,origin,update.job_id,update.plugin_id,update.objective],|r|r.get(0))?;
            if !valid { return Err("La delegación ya pertenece a otro origen o solicitud".into()); }
            let newer:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM llm_delegation_updates WHERE delegation_id=?1 AND sequence>?2)",params![update.delegation_id,update.sequence],|r|r.get(0))?;
            tx.execute("UPDATE llm_delegation_updates SET state='superseded',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE delegation_id=?1 AND sequence<?2 AND state IN ('pending','ready')",params![update.delegation_id,update.sequence])?;
            tx.execute("INSERT INTO llm_delegation_updates(event_key,delegation_id,sequence,document,state) VALUES(?1,?2,?3,?4,?5)",params![update.event_key,update.delegation_id,update.sequence,document,if newer {"superseded"} else {"pending"}])?;
            tx.commit()?; Ok(())
        })
    }
    pub fn recover(&self) -> Result<()> {
        self.0.with_connection(|c| {
            c.execute("UPDATE llm_delegation_updates SET state='ready',rendered_text=NULL,failure_code='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='rendering'",[])?;
            Ok(())
        })
    }
    pub fn next(&self) -> Result<Option<(Update, bool)>> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let row={let mut s=tx.prepare("SELECT document,state FROM llm_delegation_updates WHERE state IN ('pending','ready') ORDER BY created_at,event_key LIMIT 1")?;s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.next().transpose()?};
            let result=if let Some((document,state))=row {
                let update:Update=serde_json::from_str(&document)?;
                if state=="pending" {tx.execute("UPDATE llm_delegation_updates SET state='rendering',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1",[&update.event_key])?;}
                Some((update,state=="ready"))
            } else {None};
            tx.commit()?; Ok(result)
        })
    }
    pub fn reserve_render(&self, update: &Update, bot: &architects::Profile) -> Result<()> {
        let config=serde_json::json!({"provider":bot.provider,"model":bot.model,"effort":bot.effort,"capabilities":[],"turn_timeout_seconds":TURN_TIMEOUT_SECONDS}).to_string();
        self.0.with_connection(|c| {
            if c.execute("UPDATE llm_delegation_updates SET bot_revision=?2,render_config=?3,render_attempted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering' AND render_attempted_at IS NULL",params![update.event_key,bot.revision,config])?!=1 {return Err("No se pudo reservar la redacción del bot".into());}
            Ok(())
        })
    }
    pub fn ready(
        &self,
        update: &Update,
        text: &str,
        revision: i64,
        failure: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        self.0.with_connection(|c| {
            if c.execute("UPDATE llm_delegation_updates SET state='ready',rendered_text=?2,bot_revision=?3,failure_code=?4,render_duration_ms=?5,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state='rendering'",params![update.event_key,text,revision,failure,duration_ms])?!=1 {
                return Err("La actualización del bot ya no está en preparación".into());
            }
            Ok(())
        })
    }
    /// El mensaje y el estado del inbox comparten la transacción y la conexión prestada.
    pub fn deliver(&self, update: &Update, journal: &dyn ConversationJournal) -> Result<()> {
        let changed=self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let row={let mut s=tx.prepare("SELECT state,rendered_text,failure_code FROM llm_delegation_updates WHERE event_key=?1")?;s.query_map([&update.event_key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<String>>(2)?)))?.next().transpose()?};
            let Some((state,text,failure))=row else {return Err("Actualización del bot no disponible".into());};
            if state!="ready" { return Ok(false); }
            let newer:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM llm_delegation_updates WHERE delegation_id=?1 AND sequence>?2)",params![update.delegation_id,update.sequence],|r|r.get(0))?;
            let repeated = if update.kind==Kind::Progress || update.kind==Kind::Legacy {
                let previous={let mut q=tx.prepare("SELECT document FROM llm_delegation_updates WHERE delegation_id=?1 AND state='delivered' ORDER BY sequence DESC LIMIT 1")?;q.query_map([&update.delegation_id],|r|r.get::<_,String>(0))?.next().transpose()?};
                previous.map(|document| -> Result<bool> {let previous:Update=serde_json::from_str(&document)?;Ok(previous.state==update.state && previous.kind==update.kind)}).transpose()?.unwrap_or(false)
            } else {false};
            let newer=newer || repeated;
            let delivered=if newer { false } else {
                let fallback=if failure.is_some(){update.explanation_unavailable_message()}else{update.routine_message()};
                journal.deliver_delegation(&tx,update,text.as_deref().unwrap_or(&fallback))?
            };
            tx.execute("UPDATE llm_delegation_updates SET state=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1",params![update.event_key,if newer {"superseded"} else if delivered {"delivered"} else {"discarded"}])?;
            tx.commit()?;Ok(delivered)
        })?;
        if changed {
            journal.committed();
        }
        Ok(())
    }
    pub fn discard(&self, update: &Update) -> Result<()> {
        self.0.with_connection(|c| {c.execute("UPDATE llm_delegation_updates SET state='discarded',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE event_key=?1 AND state IN ('rendering','ready')",[&update.event_key])?;Ok(())})
    }
}
