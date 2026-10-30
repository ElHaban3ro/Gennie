//! Reserva acotada de avisos del bot para la misma sesión del proveedor.
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use gennie_llm::{
    QuotedMessage,
    context::{Context, MAX_CHARACTERS, MAX_UPDATES, PreparedUser, Reference, Update},
};
const PENDING: &str = "q.conversation_id=?1 AND q.message_id<?2 AND q.user_message_id IS NULL";
const UNKNOWN: &str = "b.conversation_id=?1 AND b.delivery_epoch=?2 AND b.provider=?3 AND b.provider_thread=?4 AND b.state='uncertain' AND b.warned_by_user_id IS NULL AND m.included=1";
#[cfg(test)]
mod tests;
fn reference(c: &SqliteConnection, message: i64) -> Result<Reference> {
    let execution = crate::execution_notices::load(c, message)?;
    let delegation = crate::delegation::load(c, message)?;
    let result = match (execution, delegation) {
        (Some(reference), None) => Reference::AssignmentExecution { reference },
        (None, Some(d)) => Reference::PluginBuild {
            job_id: d.job_id,
            plugin_id: d.plugin_id,
            source_sequence: d.source_sequence,
        },
        _ => return Err("La atribución del aviso no es válida".into()),
    };
    result.validate()?;
    Ok(result)
}
pub(crate) fn prepare(
    db: &Database,
    provider: &str,
    thread: &str,
    text: &str,
    reply_to: Option<i64>,
) -> Result<PreparedUser> {
    if text.trim().is_empty() {
        return Err("El mensaje está vacío".into());
    }
    db.with_connection(|c| {
        let tx=c.transaction()?;
        let (conversation,epoch):(i64,i64)=tx.query_row("SELECT c.id,c.delivery_epoch FROM presence_conversations c JOIN presence_current_conversation a ON a.conversation_id=c.id JOIN presence_assistant_conversations s ON s.conversation_id=c.id WHERE c.provider=?1 AND c.provider_thread_id=?2 AND s.assistant_id='gennie'",params![provider,thread],|r|Ok((r.get(0)?,r.get(1)?)))?;
        // El worker de conversación es serial. Una reserva previa no confirmada
        // pertenece a un envío interrumpido, incluso si falló guardar su incertidumbre.
        tx.execute("UPDATE presence_provider_inputs SET state='uncertain' WHERE conversation_id=?1 AND delivery_epoch=?2 AND provider=?3 AND provider_thread=?4 AND state='reserved'",params![conversation,epoch,provider,thread])?;
        let (user,quote)=if let Some(source)=reply_to {
            let (id,q)=crate::replies::insert_user(&tx,conversation,text,source)?;(id,Some(q))
        }else{
            tx.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'user',?2)",params![conversation,text])?;
            let id=tx.last_insert_rowid();
            tx.execute("UPDATE presence_conversations SET deleted=0,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[conversation])?;
            (id,None::<QuotedMessage>)
        };
        let rows={let mut q=tx.prepare(&format!("SELECT h.id,h.text,h.created_at FROM presence_provider_input_messages q JOIN presence_history h ON h.id=q.message_id WHERE {PENDING} ORDER BY q.message_id DESC LIMIT ?3"))?;q.query_map(params![conversation,user,(MAX_UPDATES+1) as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<std::result::Result<Vec<_>,_>>()?};
        let mut context=Context::default();let mut characters=0;
        for (id,text,created_at) in rows.into_iter().take(MAX_UPDATES) {
            let update=Update{message_id:id,text,created_at,reference:reference(&tx,id)?};
            if characters+update.characters()>MAX_CHARACTERS {break;}
            characters+=update.characters();context.updates.push(update);
        }
        let pending:i64=tx.query_row(&format!("SELECT count(*) FROM presence_provider_input_messages q WHERE {PENDING}"),params![conversation,user],|r|r.get(0))?;
        context.older_omitted=u64::try_from(pending)?.saturating_sub(context.updates.len() as u64);
        context.updates.reverse();
        let unknown_rows={let mut q=tx.prepare(&format!("SELECT m.message_id FROM presence_provider_input_messages m JOIN presence_provider_inputs b ON b.user_message_id=m.user_message_id WHERE {UNKNOWN} ORDER BY m.message_id DESC LIMIT ?5"))?;q.query_map(params![conversation,epoch,provider,thread,MAX_UPDATES as i64],|r|r.get::<_,i64>(0))?.collect::<std::result::Result<Vec<_>,_>>()?};
        for id in unknown_rows {
            let reference=reference(&tx,id)?;
            if characters+reference.characters()>MAX_CHARACTERS {break;}
            characters+=reference.characters();context.unconfirmed.push(reference);
        }
        let (unknown_count,before):(i64,Option<i64>)=tx.query_row(&format!("SELECT count(*),max(b.user_message_id) FROM presence_provider_input_messages m JOIN presence_provider_inputs b ON b.user_message_id=m.user_message_id WHERE {UNKNOWN}"),params![conversation,epoch,provider,thread],|r|Ok((r.get(0)?,r.get(1)?)))?;
        context.unconfirmed_omitted=u64::try_from(unknown_count)?.saturating_sub(context.unconfirmed.len() as u64);
        context.validate()?;
        let batch_id=if context.is_empty(){None}else{
            tx.execute("INSERT INTO presence_provider_inputs(user_message_id,conversation_id,delivery_epoch,provider,provider_thread,older_omitted,unconfirmed_omitted,warning_before_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![user,conversation,epoch,provider,thread,i64::try_from(context.older_omitted)?,i64::try_from(context.unconfirmed_omitted)?,before])?;
            let changed=tx.execute(&format!("UPDATE presence_provider_input_messages AS q SET user_message_id=?2,included=0 WHERE {PENDING}"),params![conversation,user])?;
            if i64::try_from(changed)?!=pending {return Err("El conjunto de avisos cambió antes de reservarlo".into());}
            for update in &context.updates {tx.execute("UPDATE presence_provider_input_messages SET included=1 WHERE message_id=?1 AND user_message_id=?2",params![update.message_id,user])?;}
            Some(user)
        };
        tx.commit()?;
        Ok(PreparedUser{quote,context:batch_id.map(|_|context),batch_id})
    })
}
pub(crate) fn confirm(db: &Database, provider: &str, thread: &str, batch: i64) -> Result<()> {
    db.with_connection(|c| {
        let tx=c.transaction()?;
        let row={let mut q=tx.prepare("SELECT b.conversation_id,b.delivery_epoch,b.state,b.warning_before_id FROM presence_provider_inputs b JOIN presence_conversations c ON c.id=b.conversation_id JOIN presence_current_conversation a ON a.conversation_id=c.id WHERE b.user_message_id=?1 AND b.provider=?2 AND b.provider_thread=?3 AND c.provider=?2 AND c.provider_thread_id=?3 AND c.delivery_epoch=b.delivery_epoch AND c.deleted=0")?;q.query_map(params![batch,provider,thread],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<i64>>(3)?)))?.next().transpose()?};
        let Some((conversation,epoch,state,before))=row else{return Err("La reserva de contexto ya no pertenece a esta sesión".into());};
        if state=="accepted" {return Ok(());}
        if state!="reserved" {return Err("La recepción anterior quedó incierta; no se reenviará automáticamente".into());}
        tx.execute("UPDATE presence_provider_inputs SET state='accepted',accepted_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE user_message_id=?1",[batch])?;
        if let Some(before)=before {tx.execute("UPDATE presence_provider_inputs SET warned_by_user_id=?1 WHERE conversation_id=?2 AND delivery_epoch=?3 AND provider=?4 AND provider_thread=?5 AND state='uncertain' AND warned_by_user_id IS NULL AND user_message_id<=?6",params![batch,conversation,epoch,provider,thread,before])?;}
        tx.commit()?;Ok(())
    })
}
pub(crate) fn uncertain(db: &Database, provider: &str, thread: &str, batch: i64) -> Result<()> {
    db.with_connection(|c|{c.execute("UPDATE presence_provider_inputs SET state='uncertain' WHERE user_message_id=?1 AND provider=?2 AND provider_thread=?3 AND state='reserved'",params![batch,provider,thread])?;Ok(())})
}
pub(crate) fn recover(db: &Database) -> Result<()> {
    db.with_connection(|c| {
        c.execute(
            "UPDATE presence_provider_inputs SET state='uncertain' WHERE state='reserved'",
            [],
        )?;
        Ok(())
    })
}

/// Registrar elegibilidad junto al mensaje, sin escanear el historial en cada entrada.
pub(crate) fn track(c: &SqliteConnection, message: i64, conversation: i64) -> Result<()> {
    c.execute("INSERT OR IGNORE INTO presence_provider_input_messages(message_id,conversation_id) SELECT id,conversation_id FROM presence_history WHERE id=?1 AND conversation_id=?2 AND speaker='assistant'",params![message,conversation])?;
    let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM presence_provider_input_messages WHERE message_id=?1 AND conversation_id=?2)",params![message,conversation],|r|r.get(0))?;
    if !valid {
        return Err("El aviso no pertenece al historial indicado".into());
    }
    Ok(())
}
