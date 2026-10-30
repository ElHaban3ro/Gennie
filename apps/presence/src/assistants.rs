//! Identidad del chat y lectura acotada entre sus sesiones. No cambia el contexto del proveedor.
use gennie_core::Result;
use gennie_db::{Database, params};

pub const DEFAULT_ASSISTANT_ID: &str = "gennie";
const MAX_PAGE_SESSIONS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assistant {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub avatar_revision: Option<i64>,
}

pub fn load(db: &Database, id: &str) -> Result<Assistant> {
    db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT id,name,updated_at,CASE WHEN avatar_png IS NULL THEN NULL ELSE avatar_revision END FROM presence_assistants WHERE id=?1",
            [id],
            |row| {
                Ok(Assistant {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    updated_at: row.get(2)?,
                    avatar_revision: row.get(3)?,
                })
            },
        )?)
    })
}

/// Operación bloqueante; el coordinador/worker confirma al consumidor después del commit.
pub fn rename(db: &Database, id: &str, name: &str) -> Result<Assistant> {
    let name = validate_name(name)?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        if tx.execute("UPDATE presence_assistants SET name=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1", params![id,name])? != 1 {
            return Err("El asistente ya no está disponible".into());
        }
        let result = tx.query_row(
            "SELECT id,name,updated_at,CASE WHEN avatar_png IS NULL THEN NULL ELSE avatar_revision END FROM presence_assistants WHERE id=?1",
            [id], |r| Ok(Assistant {id:r.get(0)?, name:r.get(1)?, updated_at:r.get(2)?, avatar_revision:r.get(3)?}),
        )?;
        tx.commit()?;
        Ok(result)
    })
}

pub(crate) fn validate_name(name: &str) -> Result<&str> {
    if name.contains(['\n', '\r']) {
        return Err("El nombre del asistente debe ocupar una sola línea".into());
    }
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err("Escribe un nombre de entre 1 y 200 caracteres".into());
    }
    Ok(name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub created_at: String,
    pub message_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEntry {
    pub interruption: Option<gennie_llm::ReplyInterruption>,
    pub id: i64,
    pub conversation_id: i64,
    pub session_started_at: String,
    pub starts_session: bool,
    pub quote: Option<crate::replies::StoredQuote>,
    pub tools: Vec<crate::tool_evidence::StoredToolUse>,
    pub delegation: Option<crate::delegation::MessageDelegation>,
    pub execution: Option<gennie_llm::notices::Reference>,
    pub speaker: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Orden de presentación: del más antiguo al más reciente dentro de la página.
    pub messages: Vec<ChatEntry>,
    /// Cursor exclusivo anterior al primer mensaje; None significa fin del historial.
    pub before: Option<Cursor>,
}

/// Presupuesto en caracteres Unicode, mensajes y hasta tres sesiones distintas,
/// independiente del contexto de inferencia.
/// El primer mensaje se incluye íntegro aunque exceda el presupuesto para garantizar avance.
pub fn page(
    db: &Database,
    assistant_id: &str,
    before: Option<&Cursor>,
    max_messages: usize,
    max_characters: usize,
) -> Result<Page> {
    if !(1..=200).contains(&max_messages) || !(1..=100_000).contains(&max_characters) {
        return Err("Límites de historial inválidos".into());
    }
    db.with_connection(|c| {
        let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM presence_assistants WHERE id=?1)", [assistant_id], |r| r.get(0))?;
        if !exists {return Err("El asistente ya no está disponible".into());}
        if let Some(cursor) = before {
            let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM presence_history h JOIN presence_assistant_conversations a ON a.conversation_id=h.conversation_id JOIN presence_conversations s ON s.id=h.conversation_id WHERE a.assistant_id=?1 AND s.deleted=0 AND h.id=?2 AND h.created_at=?3)",params![assistant_id,cursor.message_id,cursor.created_at],|r|r.get(0))?;
            if !valid {return Err("La referencia de historial ya no está disponible; recarga el chat".into());}
        }
        let mut statement = c.prepare("SELECT h.id,h.conversation_id,h.speaker,h.text,h.created_at,s.created_at,h.id=(SELECT min(first.id) FROM presence_history first WHERE first.conversation_id=h.conversation_id),r.source_message_id,r.speaker,r.text,r.quoted_created_at,r.truncated,h.interruption FROM presence_history h LEFT JOIN presence_message_replies r ON r.message_id=h.id JOIN presence_assistant_conversations a ON a.conversation_id=h.conversation_id JOIN presence_conversations s ON s.id=h.conversation_id WHERE a.assistant_id=?1 AND s.deleted=0 AND (?2 IS NULL OR (h.created_at,h.id)<(?2,?3)) ORDER BY h.created_at DESC,h.id DESC LIMIT ?4")?;
        let mut rows=statement.query(params![assistant_id,before.map(|v|v.created_at.as_str()),before.map(|v|v.message_id),max_messages as i64+1])?;
        let mut messages:Vec<ChatEntry>=Vec::new();
        let mut sessions=Vec::with_capacity(MAX_PAGE_SESSIONS);
        let mut characters=0usize;
        let mut has_more=false;
        while let Some(row)=rows.next()? {
            if messages.len()==max_messages {has_more=true;break;}
            let conversation_id:i64=row.get(1)?;
            let new_session=!sessions.contains(&conversation_id);
            if new_session && sessions.len()==MAX_PAGE_SESSIONS {has_more=true;break;}
            let text:String=row.get(3)?;
            let quote=match row.get::<_,Option<String>>(8)? {
                Some(speaker)=>Some(crate::replies::StoredQuote{source_message_id:row.get(7)?,speaker,text:row.get(9)?,created_at:row.get(10)?,truncated:row.get(11)?}),
                None=>None,
            };
            let tools=crate::tool_evidence::load(c,row.get(0)?)?;
            let delegation=crate::delegation::load(c,row.get(0)?)?;
            let execution=crate::execution_notices::load(c,row.get(0)?)?;
            let count=execution.as_ref().map_or(0,|e|e.characters())+text.chars().count()+quote.as_ref().map_or(0,|q|q.text.chars().count())+tools.iter().map(|u|u.characters()).sum::<usize>()+delegation.as_ref().map_or(0,|d|d.plugin_id.chars().count()+d.job_id.to_string().len()+d.source_sequence.to_string().len());
            if !messages.is_empty() && characters.saturating_add(count)>max_characters {has_more=true;break;}
            characters=characters.saturating_add(count);
            if new_session {sessions.push(conversation_id);}
            messages.push(ChatEntry{interruption:row.get::<_,Option<String>>(12)?.as_deref().map(gennie_llm::ReplyInterruption::parse).transpose()?,id:row.get(0)?,conversation_id,session_started_at:row.get(5)?,starts_session:row.get(6)?,quote,tools,delegation,execution,speaker:row.get(2)?,text,created_at:row.get(4)?});
        }
        let before=if has_more {messages.last().map(|m| Cursor {created_at:m.created_at.clone(),message_id:m.id})} else {None};
        messages.reverse();
        Ok(Page {messages,before})
    })
}

#[cfg(test)]
mod tests;
