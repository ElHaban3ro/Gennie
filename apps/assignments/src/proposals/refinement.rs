//! Versiones de borrador ancladas a una intervención real, sin renovar consentimiento remoto.
use super::*;
#[derive(Clone, Debug)]
pub struct BotTurn {
    pub origin: Origin,
    pub message_id: i64,
    pub text: String,
    pub truncated: bool,
    pub created_at: String,
}
impl BotTurn {
    pub(super) fn validate(&self) -> Result<()> {
        text(&self.text, 4000, "Respuesta del usuario")?;
        if self.message_id <= 0
            || self.origin.conversation_id <= 0
            || self.origin.epoch <= 0
            || self.created_at.len() > 64
            || !self.created_at.ends_with('Z')
        {
            return Err("Intervención de origen no válida".into());
        }
        Ok(())
    }
}
pub type BotTurnProvider = std::sync::Arc<dyn Fn() -> Result<BotTurn> + Send + Sync>;
pub(super) fn require_leaf(c: &SqliteConnection, id: &str) -> Result<()> {
    history::require_open(c, id)?;
    let newer: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM assignment_review_refinements WHERE parent_review_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if newer {
        return Err(
            "Esta revisión tiene una versión posterior; consulta esa antes de continuar".into(),
        );
    }
    Ok(())
}
pub(super) fn bind_input(c: &SqliteConnection, id: &str, turn: &BotTurn) -> Result<()> {
    turn.validate()?;
    let earliest: Option<i64> = c.query_row(
        "SELECT max(message_id) FROM (SELECT message_id FROM assignment_review_refinements WHERE review_id=?1 UNION ALL SELECT message_id FROM assignment_chat_review_requests WHERE review_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if earliest.is_some_and(|value| turn.message_id < value) {
        return Err("La propuesta precede a su respuesta de origen".into());
    }
    c.execute("INSERT INTO assignment_review_inputs(review_id,message_id,message_text,truncated,message_created_at) VALUES(?1,?2,?3,?4,?5)",params![id,turn.message_id,turn.text,turn.truncated,turn.created_at])?;
    Ok(())
}
pub(super) fn metadata(c: &SqliteConnection, id: &str) -> Result<Value> {
    let verified: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM assignment_review_inputs WHERE review_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    let row = {
        let mut q=c.prepare("SELECT root_review_id,parent_review_id,round,message_id,message_text,truncated,message_created_at FROM assignment_review_refinements WHERE review_id=?1")?;
        q.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, bool>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?
        .next()
        .transpose()?
    };
    let next = {
        let mut q = c.prepare(
            "SELECT review_id FROM assignment_review_refinements WHERE parent_review_id=?1",
        )?;
        q.query_map([id], |r| r.get::<_, String>(0))?
            .next()
            .transpose()?
    };
    let mut previous = Vec::new();
    let mut previous_request = None;
    let (root, parent, round, response) = if let Some((
        root,
        parent,
        round,
        message,
        text,
        truncated,
        at,
    )) = row
    {
        let mut q=c.prepare("SELECT candidate,resolution FROM assignment_proposals WHERE review_id=?1 ORDER BY ordinal")?;
        for row in q.query_map([&parent], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (candidate, resolution) = row?;
            previous.push(json!({"candidate":serde_json::from_str::<Value>(&candidate)?,"resolution":resolution}));
        }
        previous_request = Some(scope(c, &parent)?.instructions);
        (
            root,
            Some(parent),
            round,
            Some(json!({"message_id":message,"text":text,"truncated":truncated,"created_at":at})),
        )
    } else {
        (id.into(), None, 1, None)
    };
    Ok(
        json!({"root_review_id":root,"parent_review_id":parent,"round":round,"next_review_id":next,"response":response,"previous_request":previous_request,"previous_proposals":previous,"input_verified":verified}),
    )
}
impl Store {
    /// Solo consulta local humana; el MCP conserva su comprobación de origen.
    pub fn review(&self, id: &str) -> Result<Value> {
        self.db.with_connection(|c| load(c, id))
    }
    pub fn refine_review(&self, parent: &str, origin: &Origin, turn: &BotTurn) -> Result<Value> {
        turn.validate()?;
        if turn.origin != *origin {
            return Err("La respuesta pertenece a otra sesión".into());
        }
        let result=self.db.with_connection(|c| {
            let tx=c.transaction()?;let s=scope(&tx,parent)?;
            if s.origin!=*origin {return Err("La revisión pertenece a otra sesión".into());}
            history::require_open(&tx,parent)?;
            let version=metadata(&tx,parent)?;let root=version["root_review_id"].as_str().ok_or("Falta la revisión original")?;
            let existing={let mut q=tx.prepare("SELECT review_id,parent_review_id,message_text,truncated,message_created_at FROM assignment_review_refinements WHERE root_review_id=?1 AND message_id=?2")?;q.query_map(params![root,turn.message_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,bool>(3)?,r.get::<_,String>(4)?)))?.next().transpose()?};
            if let Some((id,requested_parent,text,truncated,created_at))=existing {
                if requested_parent!=parent {return Err("Esta intervención ya tiene una versión; trabaja en ese borrador".into());}
                if text!=turn.text || truncated!=turn.truncated || created_at!=turn.created_at {return Err("La intervención no coincide con la respuesta guardada".into());}
                return load(&tx,&id);
            }
            require_leaf(&tx,parent)?;
            let (state,submitted):(String,bool)=tx.query_row("SELECT state,submission IS NOT NULL FROM assignment_reviews WHERE id=?1",[parent],|r|Ok((r.get(0)?,r.get(1)?)))?;
            let previous_input={let mut q=tx.prepare("SELECT message_id FROM assignment_review_inputs WHERE review_id=?1")?;q.query_map([parent],|r|r.get::<_,i64>(0))?.next().transpose()?};
            let previous_input=previous_input.ok_or("Esta revisión anterior no tiene una intervención verificable; solicita una revisión nueva desde Encargos")?;
            if turn.message_id<=previous_input {return Err("Necesito una respuesta nueva del usuario antes de cambiar el borrador".into());}
            let pending:i64=tx.query_row("SELECT count(*) FROM assignment_proposals WHERE review_id=?1 AND resolution='pending'",[parent],|r|r.get(0))?;
            if !submitted || state=="evaluating" || pending==0 || !current(&tx,parent,&s)? {return Err("La revisión terminó, sigue en valoración o sus encargos cambiaron; consulta su estado antes de continuar".into());}
            let resolved={let mut q=tx.prepare("SELECT candidate FROM assignment_proposals WHERE review_id=?1 AND resolution!='pending'")?;q.query_map([parent],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?};
            let mut editable=s.editable_ids.clone().unwrap_or_else(||s.assignments.iter().map(|a|a.id).collect());
            for text in resolved {let candidate:Candidate=serde_json::from_str(&text)?;editable.retain(|id|*id!=candidate.assignment_id);}
            if editable.is_empty() {return Err("No quedan propuestas pendientes para continuar".into());}
            let assignments=s.assignments.iter().map(|original|read(&tx,original.id).map(Baseline::from)).collect::<Result<Vec<_>>>()?;
            let scope=Scope{origin:origin.clone(),initial_instructions:Some(s.initial_instructions.unwrap_or(s.instructions)),editable_ids:Some(editable),assignments,instructions:turn.text.clone(),use_guide:false};
            let encoded=serde_json::to_string(&scope)?;if encoded.len()>256*1024 {return Err("La revisión supera el límite de contexto".into());}
            let id:String=tx.query_row("SELECT lower(hex(randomblob(16)))",[],|r|r.get(0))?;
            let key=format!("refine:{root}:{}",turn.message_id);let request=json!({"parent_review_id":parent,"origin":origin,"message_id":turn.message_id}).to_string();
            tx.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,state) VALUES(?1,?2,?3,?4,'waiting_bot')",params![id,key,request,encoded])?;
            let round=version["round"].as_i64().ok_or("Versión no válida")?.checked_add(1).ok_or("Versiones agotadas")?;
            tx.execute("INSERT INTO assignment_review_refinements(review_id,parent_review_id,root_review_id,round,message_id,message_text,truncated,message_created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,parent,root,round,turn.message_id,turn.text,turn.truncated,turn.created_at])?;
            let result=load(&tx,&id)?;tx.commit()?;Ok(result)
        })?;
        self.revision
            .fetch_add(1, std::sync::atomic::Ordering::Release);
        Ok(result)
    }
}
#[cfg(test)]
mod tests;
