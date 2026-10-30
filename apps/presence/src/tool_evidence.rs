//! Evidencia local por solicitud y respuesta; no conserva argumentos, resultados ni errores libres.
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use gennie_llm::{ToolOutcome, ToolUse};
pub const MAX_CALLS_PER_TURN: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredToolUse {
    pub usage: ToolUse,
    pub created_at: String,
    pub updated_at: String,
}
impl StoredToolUse {
    pub fn characters(&self) -> usize {
        let u = &self.usage;
        [&u.call_id, &u.server, &u.tool]
            .into_iter()
            .map(|s| s.chars().count())
            .sum::<usize>()
            + [
                &u.plugin_id,
                &u.function_name,
                &u.display_name,
                &u.operation,
                &u.detail,
            ]
            .into_iter()
            .flatten()
            .map(|s| s.chars().count())
            .sum::<usize>()
    }
}
fn valid(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control)
}
fn validate(turn: &str, usage: &ToolUse) -> Result<()> {
    if !valid(turn, 256)
        || !valid(&usage.call_id, 256)
        || !valid(&usage.server, 200)
        || !valid(&usage.tool, 200)
        || usage.display_name.as_ref().is_some_and(|v| !valid(v, 200))
        || [&usage.plugin_id, &usage.function_name]
            .into_iter()
            .flatten()
            .any(|v| !valid(v, 64))
        || usage
            .operation
            .as_deref()
            .is_some_and(|v| !matches!(v, "run" | "verify"))
        || usage.detail.as_deref().is_some_and(|v| {
            !matches!(
                v,
                "result_uncertain"
                    | "timeout"
                    | "browser_closed"
                    | "response_too_large"
                    | "authentication_required"
                    | "access_denied"
                    | "site_changed"
                    | "redirect_blocked"
                    | "redirect_limit"
                    | "http_error"
                    | "execution_failed"
                    | "permission_required"
                    | "revision_conflict"
                    | "verification_required"
                    | "plugin_not_found"
                    | "function_not_found"
            )
        })
    {
        return Err("Metadatos de herramienta inválidos".into());
    }
    Ok(())
}
fn request_id(
    c: &SqliteConnection,
    provider: &str,
    thread: &str,
    turn: &str,
) -> Result<Option<i64>> {
    let mut statement = c.prepare("SELECT h.id FROM presence_history h JOIN presence_conversations c ON c.id=h.conversation_id WHERE c.provider=?1 AND c.provider_thread_id=?2 AND c.deleted=0 AND h.speaker='user' AND h.provider_turn_id=?3 ORDER BY h.id DESC LIMIT 1")?;
    let mut rows = statement.query(params![provider, thread, turn])?;
    Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
}
pub fn begin_turn(db: &Database, provider: &str, thread: &str, turn: &str) -> Result<bool> {
    if !valid(turn, 256) {
        return Err("Identificador de turno inválido".into());
    }
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let latest = {
            let mut statement = tx.prepare("SELECT h.id,h.provider_turn_id FROM presence_history h JOIN presence_conversations c ON c.id=h.conversation_id WHERE c.provider=?1 AND c.provider_thread_id=?2 AND c.deleted=0 AND h.speaker='user' ORDER BY h.id DESC LIMIT 1")?;
            let mut rows = statement.query(params![provider,thread])?;
            rows.next()?.map(|r| -> Result<_> { Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?)) }).transpose()?
        };
        let Some((id, previous)) = latest else { return Ok(false); };
        if previous.as_deref() == Some(turn) { return Ok(false); }
        if previous.is_some() { return Err("El turno no tiene una solicitud nueva en este chat".into()); }
        tx.execute("UPDATE presence_history SET provider_turn_id=?2 WHERE id=?1",params![id,turn])?;
        tx.commit()?;
        Ok(true)
    })
}
pub fn record(
    db: &Database,
    provider: &str,
    thread: &str,
    turn: &str,
    usage: &ToolUse,
) -> Result<bool> {
    validate(turn, usage)?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let Some(user) = request_id(&tx,provider,thread,turn)? else { return Ok(false); };
        let previous = {
            let mut statement = tx.prepare("SELECT id,server,tool,plugin_id,function_name,display_name,operation,outcome,detail FROM presence_tool_uses WHERE user_message_id=?1 AND turn_id=?2 AND call_id=?3")?;
            let mut rows = statement.query(params![user,turn,usage.call_id])?;
            rows.next()?.map(|r| -> Result<_> { Ok((r.get::<_,i64>(0)?, ToolUse { call_id: usage.call_id.clone(), server:r.get(1)?, tool:r.get(2)?, plugin_id:r.get(3)?,function_name:r.get(4)?,display_name:r.get(5)?,operation:r.get(6)?,outcome:ToolOutcome::parse(&r.get::<_,String>(7)?)?,detail:r.get(8)? })) }).transpose()?
        };
        let mut next = usage.clone();
        if let Some((id, old)) = previous {
            if old.server != next.server || old.tool != next.tool
                || old.plugin_id.as_ref().zip(next.plugin_id.as_ref()).is_some_and(|(a,b)|a!=b)
                || old.function_name.as_ref().zip(next.function_name.as_ref()).is_some_and(|(a,b)|a!=b) {
                return Err("Cambió la identidad de una llamada ya registrada".into());
            }
            if old.outcome != ToolOutcome::Running && next.outcome == ToolOutcome::Running
                || matches!(old.outcome,ToolOutcome::Completed|ToolOutcome::Failed) && next.outcome != old.outcome {
                return Ok(false);
            }
            next.plugin_id = next.plugin_id.or(old.plugin_id.clone());
            next.function_name = next.function_name.or(old.function_name.clone());
            next.display_name = next.display_name.or(old.display_name.clone());
            next.operation = next.operation.or(old.operation.clone());
            if next == old { return Ok(false); }
            tx.execute("UPDATE presence_tool_uses SET plugin_id=?2,function_name=?3,display_name=?4,operation=?5,outcome=?6,detail=?7,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,next.plugin_id,next.function_name,next.display_name,next.operation,next.outcome.as_str(),next.detail])?;
        } else {
            let count: i64 = tx.query_row("SELECT count(*) FROM presence_tool_uses WHERE user_message_id=?1 AND turn_id=?2",params![user,turn],|r|r.get(0))?;
            if count >= MAX_CALLS_PER_TURN as i64 { return Err("El turno excedió el límite de llamadas registrables; el resultado restante no está confirmado".into()); }
            tx.execute("INSERT INTO presence_tool_uses(user_message_id,message_id,turn_id,call_id,server,tool,plugin_id,function_name,display_name,operation,outcome,detail) VALUES(?1,?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![user,turn,next.call_id,next.server,next.tool,next.plugin_id,next.function_name,next.display_name,next.operation,next.outcome.as_str(),next.detail])?;
        }
        tx.commit()?;
        Ok(true)
    })
}
pub fn append_response(
    db: &Database,
    provider: &str,
    thread: &str,
    turn: &str,
    item: Option<&str>,
    text: &str,
) -> Result<bool> {
    append_response_state(
        db,
        provider,
        thread,
        turn,
        text,
        ResponseState {
            item,
            interruption: None,
        },
    )
}
pub fn append_interrupted_response(
    db: &Database,
    provider: &str,
    thread: &str,
    turn: &str,
    item: Option<&str>,
    text: &str,
    reason: gennie_llm::ReplyInterruption,
) -> Result<bool> {
    append_response_state(
        db,
        provider,
        thread,
        turn,
        text,
        ResponseState {
            item,
            interruption: Some(reason),
        },
    )
}
struct ResponseState<'a> {
    item: Option<&'a str>,
    interruption: Option<gennie_llm::ReplyInterruption>,
}
fn append_response_state(
    db: &Database,
    provider: &str,
    thread: &str,
    turn: &str,
    text: &str,
    state: ResponseState<'_>,
) -> Result<bool> {
    let ResponseState { item, interruption } = state;
    if !valid(turn, 256) || item.is_some_and(|v| !valid(v, 256)) || text.trim().is_empty() {
        return Err("Respuesta o identidad de proveedor inválida".into());
    }
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let Some(user) = request_id(&tx,provider,thread,turn)? else {if interruption.is_some(){return Err("El origen de la respuesta interrumpida ya no está disponible".into());}return Ok(false);};
        let conversation:i64 = tx.query_row("SELECT conversation_id FROM presence_history WHERE id=?1",[user],|r|r.get(0))?;
        if item.is_some() || interruption.is_some() {
            let mut statement = tx.prepare("SELECT text,interruption FROM presence_history WHERE conversation_id=?1 AND provider_turn_id=?2 AND ((?3 IS NOT NULL AND provider_item_id=?3) OR (?3 IS NULL AND provider_item_id IS NULL AND interruption IS NOT NULL))")?;
            let mut rows = statement.query(params![conversation,turn,item])?;
            if let Some(row) = rows.next()? {
                if row.get::<_,String>(0)? != text || row.get::<_,Option<String>>(1)?.as_deref()!=interruption.map(|v|v.as_str()) { return Err("Cambió una respuesta ya guardada para el mismo ítem".into()); }
                return Ok(false);
            }
        }
        tx.execute("INSERT INTO presence_history(conversation_id,speaker,text,provider_turn_id,provider_item_id,interruption) VALUES(?1,'assistant',?2,?3,?4,?5)",params![conversation,text,turn,item,interruption.map(|v|v.as_str())])?;
        let message = tx.last_insert_rowid();
        tx.execute("UPDATE presence_tool_uses SET message_id=?2 WHERE user_message_id=?1 AND turn_id=?3 AND message_id=?1",params![user,message,turn])?;
        tx.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[conversation])?;
        if interruption.is_some(){tx.execute("UPDATE presence_tool_uses SET outcome='unknown',detail='unconfirmed',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE user_message_id=?1 AND turn_id=?2 AND outcome='running'",params![user,turn])?;}
        tx.commit()?;
        Ok(true)
    })
}
pub fn finish_turn(db: &Database, provider: &str, thread: &str, turn: &str) -> Result<bool> {
    db.with_connection(|c| {
        let Some(user) = request_id(c,provider,thread,turn)? else { return Ok(false); };
        Ok(c.execute("UPDATE presence_tool_uses SET outcome='unknown',detail='unconfirmed',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE user_message_id=?1 AND turn_id=?2 AND outcome='running'",params![user,turn])? > 0)
    })
}
pub fn recover(db: &Database) -> Result<bool> {
    db.with_connection(|c| Ok(c.execute("UPDATE presence_tool_uses SET outcome='unknown',detail='restart',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE outcome='running'",[])? > 0))
}
pub fn load(c: &SqliteConnection, message: i64) -> Result<Vec<StoredToolUse>> {
    let mut statement = c.prepare("SELECT call_id,server,tool,plugin_id,function_name,display_name,operation,outcome,detail,created_at,updated_at FROM presence_tool_uses WHERE message_id=?1 ORDER BY id LIMIT 513")?;
    let mut rows = statement.query([message])?;
    let mut result = Vec::new();
    while let Some(r) = rows.next()? {
        if result.len() == MAX_CALLS_PER_TURN {
            return Err("La evidencia de herramientas excede el límite del lector".into());
        }
        result.push(StoredToolUse {
            usage: ToolUse {
                call_id: r.get(0)?,
                server: r.get(1)?,
                tool: r.get(2)?,
                plugin_id: r.get(3)?,
                function_name: r.get(4)?,
                display_name: r.get(5)?,
                operation: r.get(6)?,
                outcome: ToolOutcome::parse(&r.get::<_, String>(7)?)?,
                detail: r.get(8)?,
            },
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
