//! Petición local del chat: procedencia real sin heredar consentimiento remoto.
use super::*;

pub(super) fn metadata(c: &SqliteConnection, id: &str) -> Result<Option<Value>> {
    let root = history::root(c, id)?;
    let mut query=c.prepare("SELECT conversation_id,epoch,message_id,message_text,truncated,message_created_at FROM assignment_chat_review_requests WHERE review_id=?1")?;
    Ok(query.query_map([root],|r|Ok(json!({"conversation_id":r.get::<_,i64>(0)?,"epoch":r.get::<_,i64>(1)?,"message_id":r.get::<_,i64>(2)?,"text":r.get::<_,String>(3)?,"truncated":r.get::<_,bool>(4)?,"created_at":r.get::<_,String>(5)?})))?.next().transpose()?)
}

pub(super) fn capture(c: &SqliteConnection, id: &str, turn: &BotTurn) -> Result<()> {
    c.execute("INSERT INTO assignment_chat_review_requests(review_id,conversation_id,epoch,message_id,message_text,truncated,message_created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,turn.origin.conversation_id,turn.origin.epoch,turn.message_id,turn.text,turn.truncated,turn.created_at])?;
    Ok(())
}

pub(super) fn same_turn(c: &SqliteConnection, id: &str, turn: &BotTurn) -> Result<()> {
    let saved = metadata(c, id)?.ok_or("La petición existente no tiene ese origen de chat")?;
    if saved
        != json!({"conversation_id":turn.origin.conversation_id,"epoch":turn.origin.epoch,"message_id":turn.message_id,"text":turn.text,"truncated":turn.truncated,"created_at":turn.created_at})
    {
        return Err("La petición no coincide con la intervención original".into());
    }
    Ok(())
}

impl Store {
    pub fn start_chat_review(
        &self,
        mut assignments: Vec<Reference>,
        turn: &BotTurn,
    ) -> Result<Value> {
        turn.validate()?;
        assignments.sort_by_key(|reference| reference.id);
        self.begin_review_captured(
            Begin {
                request_key: format!(
                    "chat:{}:{}:{}",
                    turn.origin.conversation_id, turn.origin.epoch, turn.message_id
                ),
                assignments,
                origin: turn.origin.clone(),
                instructions: turn.text.clone(),
                use_guide: false,
            },
            Some(turn),
        )
    }
}

#[cfg(test)]
mod tests;
