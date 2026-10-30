//! Lectura del esquema 0011–0018 solo para comparar fixtures anteriores a migrar.
use super::*;
pub(crate) fn load(db: &Database, selected: Option<i64>) -> Result<Snapshot> {
    db.with_connection(|c|{
        let conversations=c.prepare("SELECT c.id,coalesce(c.local_title,c.provider_title,CASE WHEN c.provider IS NULL THEN c.title ELSE 'Nuevo chat' END),c.updated_at,(SELECT substr(h.text,1,512) FROM presence_history h WHERE h.conversation_id=c.id ORDER BY h.id DESC LIMIT 1) FROM presence_conversations c WHERE c.deleted=0 ORDER BY c.updated_at DESC,c.id DESC")?.query_map([],|r|Ok(Conversation{id:r.get(0)?,title:r.get(1)?,updated_at:r.get(2)?,last_message:r.get(3)?}))?.collect::<std::result::Result<Vec<_>,_>>()?;
        let active=c.prepare("SELECT c.id FROM presence_current_conversation a JOIN presence_conversations c ON c.id=a.conversation_id WHERE a.id=1 AND c.deleted=0")?.query_map([],|r|r.get::<_,i64>(0))?.next().transpose()?;
        let selected_id=selected.filter(|id|conversations.iter().any(|r|r.id==*id)).or(active).or_else(||conversations.first().map(|r|r.id));
        let messages=|id|->Result<Vec<Message>>{Ok(c.prepare("SELECT speaker,text,created_at FROM presence_history WHERE conversation_id=?1 ORDER BY id")?.query_map([id],|r|Ok(Message{speaker:r.get(0)?,text:r.get(1)?,created_at:r.get(2)?,interruption:None}))?.collect::<std::result::Result<Vec<_>,_>>()?)};
        Ok(Snapshot{messages:messages(active)?,selected_messages:messages(selected_id)?,conversations,selected_id,active_id:active})
    })
}
