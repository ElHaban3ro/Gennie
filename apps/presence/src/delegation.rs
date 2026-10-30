//! El origen y la época impiden entregar respuestas tardías tras borrar una sesión.
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use gennie_llm::delegation::{ContextMessage, Origin, Update};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageDelegation {
    pub job_id: i64,
    pub source_sequence: i64,
    pub plugin_id: String,
}
pub(crate) fn origin_in(c: &SqliteConnection, id: i64) -> Result<Option<Origin>> {
    let mut s=c.prepare("SELECT c.id,a.assistant_id,c.delivery_epoch,c.provider_thread_id FROM presence_conversations c JOIN presence_assistant_conversations a ON a.conversation_id=c.id WHERE c.id=?1 AND c.deleted=0")?;
    Ok(s.query_map([id], |r| {
        Ok(Origin {
            conversation_id: r.get(0)?,
            bot_id: r.get(1)?,
            epoch: r.get(2)?,
            provider_thread: r.get(3)?,
        })
    })?
    .next()
    .transpose()?)
}
pub fn origin(db: &Database, id: i64) -> Result<Option<Origin>> {
    db.with_connection(|c| origin_in(c, id))
}

/// Solo mensajes assistant confirmados para la identidad y época originales del trabajo.
/// Últimos doce, hasta 8.000 caracteres; no activa conversaciones ni publica revisiones.
pub fn build_messages(
    db: &Database,
    origin: &Origin,
    job_id: i64,
    plugin_id: &str,
    legacy: bool,
    through_sequence: i64,
) -> Result<Option<Vec<gennie_llm::delegation::BotMessage>>> {
    origin.validate()?;
    if job_id <= 0 || through_sequence <= 0 || plugin_id.is_empty() || plugin_id.len() > 64 {
        return Err("Referencia de construcción inválida".into());
    }
    let delegation = format!(
        "plugin-build:{}{job_id}",
        if legacy { "legacy:" } else { "" }
    );
    db.with_connection(|c| {
        if origin_in(c, origin.conversation_id)?.as_ref() != Some(origin) { return Ok(None); }
        let mut query = c.prepare("SELECT d.message_id,d.source_sequence,h.text,h.created_at FROM presence_delegation_messages d JOIN presence_history h ON h.id=d.message_id JOIN presence_conversations s ON s.id=h.conversation_id JOIN presence_assistant_conversations a ON a.conversation_id=s.id WHERE s.id=?1 AND s.deleted=0 AND s.delivery_epoch=?2 AND a.assistant_id=?3 AND s.provider_thread_id IS ?4 AND h.speaker='assistant' AND d.source_id='plugin_builder' AND d.delegation_id=?5 AND d.job_id=?6 AND d.plugin_id=?7 AND d.source_sequence<=?8 ORDER BY d.source_sequence DESC,d.message_id DESC LIMIT 13")?;
        let rows = query.query_map(params![origin.conversation_id,origin.epoch,origin.bot_id,origin.provider_thread,delegation,job_id,plugin_id,through_sequence], |r| {
            Ok(gennie_llm::delegation::BotMessage { message_id:r.get(0)?,sequence:r.get(1)?,text:r.get(2)?,created_at:r.get(3)? })
        })?;
        let mut messages = Vec::new();
        let mut characters = 0usize;
        for row in rows {
            let message = row?;
            message.validate()?;
            if messages.len() == 12 || characters + message.text.chars().count() > 8000 { break; }
            characters += message.text.chars().count();
            messages.push(message);
        }
        messages.reverse();
        Ok(Some(messages))
    })
}
pub(crate) fn context(db: &Database, origin: &Origin) -> Result<Option<Vec<ContextMessage>>> {
    origin.validate()?;
    db.with_connection(|c| {
        if origin_in(c, origin.conversation_id)?.as_ref() != Some(origin) { return Ok(None); }
        let mut query = c.prepare("SELECT h.speaker,h.text,q.speaker,q.text,q.quoted_created_at,q.truncated FROM presence_history h LEFT JOIN presence_message_replies q ON q.message_id=h.id WHERE h.conversation_id=?1 ORDER BY h.id DESC LIMIT 12")?;
        let rows = query.query_map([origin.conversation_id], |r| {
            let speaker: Option<String> = r.get(2)?;
            let quote = if let Some(speaker) = speaker {
                Some(gennie_llm::QuotedMessage { speaker, text:r.get(3)?, created_at:r.get(4)?, truncated:r.get(5)? })
            } else { None };
            Ok(ContextMessage { speaker:r.get(0)?, text:r.get(1)?, quote })
        })?;
        let mut result = Vec::new();
        let mut remaining = 8000;
        for row in rows {
            if remaining == 0 { break; }
            let mut message = row?;
            message.text = message.text.chars().take(remaining).collect();
            remaining -= message.text.chars().count();
            if remaining == 0 { message.quote = None; }
            if let Some(quote) = message.quote.as_mut() {
                let original = quote.text.chars().count();
                quote.text = quote.text.chars().take(remaining).collect();
                let count = quote.text.chars().count();
                quote.truncated |= count < original;
                remaining -= count;
            }
            result.push(message);
        }
        result.reverse();
        Ok(Some(result))
    })
}
pub(crate) fn deliver(c: &SqliteConnection, update: &Update, text: &str) -> Result<bool> {
    update.validate()?;
    if text.trim().is_empty() || text.chars().count() > 1600 {
        return Err("Mensaje del bot inválido".into());
    }
    if origin_in(c, update.origin.conversation_id)?.as_ref() != Some(&update.origin) {
        return Ok(false);
    }
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM presence_delegation_messages WHERE event_key=?1)",
        [&update.event_key],
        |r| r.get(0),
    )?;
    if exists {
        let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM presence_delegation_messages d JOIN presence_history h ON h.id=d.message_id WHERE d.event_key=?1 AND h.conversation_id=?2 AND d.delegation_id=?3 AND d.source_id=?4 AND d.job_id=?5 AND d.source_sequence=?6 AND d.plugin_id=?7 AND h.text=?8)",params![update.event_key,update.origin.conversation_id,update.delegation_id,update.source_id,update.job_id,update.sequence,update.plugin_id,text],|r|r.get(0))?;
        if !valid {
            return Err("El recibo del bot pertenece a otra entrega".into());
        }
        return Ok(true);
    }
    c.execute(
        "INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'assistant',?2)",
        params![update.origin.conversation_id, text],
    )?;
    let message = c.last_insert_rowid();
    c.execute("INSERT INTO presence_delegation_messages(message_id,event_key,delegation_id,source_id,job_id,source_sequence,plugin_id) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![message,update.event_key,update.delegation_id,update.source_id,update.job_id,update.sequence,update.plugin_id])?;
    crate::provider_context::track(c, message, update.origin.conversation_id)?;
    c.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[update.origin.conversation_id])?;
    Ok(true)
}
pub(crate) fn load(c: &SqliteConnection, message: i64) -> Result<Option<MessageDelegation>> {
    let mut query=c.prepare("SELECT job_id,source_sequence,plugin_id FROM presence_delegation_messages WHERE message_id=?1")?;
    Ok(query
        .query_map([message], |r| {
            Ok(MessageDelegation {
                job_id: r.get(0)?,
                source_sequence: r.get(1)?,
                plugin_id: r.get(2)?,
            })
        })?
        .next()
        .transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    use gennie_llm::delegation::{Kind, State};
    fn fixture() -> (
        Database,
        std::sync::Arc<dyn gennie_llm::ConversationJournal>,
        Origin,
    ) {
        let db = Database::memory().unwrap();
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let journal = module.conversation_journal(db.clone());
        db.with_connection(|c| {
            let tx = c.transaction()?;
            journal.activate(&tx, "codex", "origin", None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        journal
            .append(
                "codex",
                "origin",
                "user",
                "Contexto autorizado de la primera sesión",
            )
            .unwrap();
        let id = crate::history::conversation_for_thread(&db, "origin").unwrap();
        let origin = origin(&db, id).unwrap().unwrap();
        (db, journal, origin)
    }
    fn update(origin: Origin) -> Update {
        Update {
            event_key: "plugin-build:1".into(),
            delegation_id: "job:7".into(),
            request_id: "request:7".into(),
            origin,
            source_id: "plugin_builder".into(),
            job_id: 7,
            plugin_id: "fixture".into(),
            display_name: "Plugin de prueba".into(),
            objective: "Preparar plugin".into(),
            sequence: 1,
            state: State::Running,
            kind: Kind::Progress,
            detail: "Texto interno del helper".into(),
        }
    }
    #[test]
    fn build_presentation_reads_only_committed_bot_messages_for_exact_origin() {
        let (db, _, origin) = fixture();
        let mut update = update(origin.clone());
        update.delegation_id = "plugin-build:7".into();
        let failed: Result<()> = db.with_connection(|c| {
            let tx = c.transaction()?;
            deliver(&tx, &update, "BOT_ROLLED_BACK")?;
            Err("Rollback deliberado".into())
        });
        assert!(failed.is_err());
        assert!(
            build_messages(&db, &origin, 7, "fixture", false, 1)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        db.with_connection(|c| {
            let tx = c.transaction()?;
            deliver(&tx, &update, "Texto confirmado por Gennie")?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        let messages = build_messages(&db, &origin, 7, "fixture", false, 1)
            .unwrap()
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Texto confirmado por Gennie");
        assert!(!messages[0].text.contains("helper"));
        assert!(
            build_messages(&db, &origin, 8, "fixture", false, 1)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        assert!(
            build_messages(&db, &origin, 7, "other", false, 1)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        assert!(
            build_messages(&db, &origin, 7, "fixture", true, 1)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        let mut changed = origin.clone();
        changed.epoch += 1;
        assert!(
            build_messages(&db, &changed, 7, "fixture", false, 1)
                .unwrap()
                .is_none()
        );
        changed = origin.clone();
        changed.provider_thread = Some("foreign-thread".into());
        assert!(
            build_messages(&db, &changed, 7, "fixture", false, 1)
                .unwrap()
                .is_none()
        );
        crate::history::delete(&db, origin.conversation_id).unwrap();
        assert!(
            build_messages(&db, &origin, 7, "fixture", false, 1)
                .unwrap()
                .is_none()
        );
        db.close().unwrap();
    }

    #[test]
    fn build_presentation_limits_unicode_content_and_preserves_source_sequence() {
        let (db, _, origin) = fixture();
        for sequence in 1..=20 {
            let mut update = update(origin.clone());
            update.delegation_id = "plugin-build:7".into();
            update.event_key = format!("plugin-build:{sequence}");
            update.sequence = sequence;
            db.with_connection(|c| {
                let tx = c.transaction()?;
                deliver(&tx, &update, &"á".repeat(1400))?;
                tx.commit()?;
                Ok(())
            })
            .unwrap();
        }
        let messages = build_messages(&db, &origin, 7, "fixture", false, 18)
            .unwrap()
            .unwrap();
        assert_eq!(
            messages.iter().map(|m| m.sequence).collect::<Vec<_>>(),
            vec![14, 15, 16, 17, 18]
        );
        assert_eq!(
            messages
                .iter()
                .map(|m| m.text.chars().count())
                .sum::<usize>(),
            7000
        );
        assert!(messages.iter().all(|m| m.validate().is_ok()));
        db.close().unwrap();
        assert!(build_messages(&db, &origin, 7, "fixture", false, 18).is_err());
    }

    #[test]
    fn origin_context_and_bot_message_stay_in_their_session_with_trusted_metadata() {
        let (db, journal, origin) = fixture();
        db.with_connection(|c| {
            let tx = c.transaction()?;
            journal.activate(&tx, "codex", "other", None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        journal
            .append("codex", "other", "user", "OTHER_SESSION_PRIVATE")
            .unwrap();
        let context = context(&db, &origin).unwrap().unwrap();
        assert_eq!(context.len(), 1);
        assert!(!context[0].text.contains("OTHER_SESSION_PRIVATE"));
        let update = update(origin.clone());
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(deliver(&tx, &update, "Avance preparado por el bot")?);
            assert!(deliver(&tx, &update, "Avance preparado por el bot")?);
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        let history = crate::history::load(&db, Some(origin.conversation_id)).unwrap();
        assert_eq!(history.selected_messages.len(), 2);
        assert_eq!(history.selected_messages[1].speaker, "assistant");
        assert_eq!(
            history.active_id.unwrap(),
            crate::history::conversation_for_thread(&db, "other").unwrap()
        );
        let page = crate::assistants::page(&db, "gennie", None, 60, 24000).unwrap();
        let message = page
            .messages
            .iter()
            .find(|m| m.text == "Avance preparado por el bot")
            .unwrap();
        assert_eq!(message.delegation.as_ref().unwrap().job_id, 7);
        assert_eq!(message.delegation.as_ref().unwrap().source_sequence, 1);
    }
    #[test]
    fn deleting_and_reopening_a_session_invalidates_old_delegation_epoch() {
        let (db, journal, old) = fixture();
        crate::history::delete(&db, old.conversation_id).unwrap();
        assert!(context(&db, &old).unwrap().is_none());
        journal
            .append_user("codex", "origin", "Entrada nueva después de borrar", None)
            .unwrap();
        let new = origin(&db, old.conversation_id).unwrap().unwrap();
        assert_ne!(new.epoch, old.epoch);
        assert!(context(&db, &old).unwrap().is_none());
        let update = update(old.clone());
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(!deliver(
                &tx,
                &update,
                "Resultado tardío que no debe reaparecer"
            )?);
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        let history = crate::history::load(&db, Some(old.conversation_id)).unwrap();
        assert_eq!(history.selected_messages.len(), 1);
        assert_eq!(
            history.selected_messages[0].text,
            "Entrada nueva después de borrar"
        );
    }
    #[test]
    fn context_budget_is_bounded_and_deleting_message_removes_the_reference() {
        let (db, journal, origin) = fixture();
        for _ in 0..20 {
            journal
                .append("codex", "origin", "user", &"á".repeat(2000))
                .unwrap();
        }
        let values = context(&db, &origin).unwrap().unwrap();
        assert!(values.len() <= 12);
        assert_eq!(
            values.iter().map(|m| m.text.chars().count()).sum::<usize>(),
            8000
        );
        let update = update(origin.clone());
        db.with_connection(|c| {
            let tx = c.transaction()?;
            deliver(&tx, &update, "Un mensaje del bot")?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        crate::history::delete(&db, origin.conversation_id).unwrap();
        db.with_connection(|c| {
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM presence_delegation_messages",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                0
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn explicit_quotes_are_context_data_and_share_the_content_budget() {
        let (db, journal, origin) = fixture();
        journal
            .append(
                "codex",
                "origin",
                "assistant",
                "Código autorizado de referencia",
            )
            .unwrap();
        let id: i64 = db
            .with_connection(|c| {
                Ok(c.query_row("SELECT max(id) FROM presence_history", [], |r| r.get(0))?)
            })
            .unwrap();
        journal
            .append_user("codex", "origin", "Revisa el mensaje citado", Some(id))
            .unwrap();
        let messages = context(&db, &origin).unwrap().unwrap();
        assert_eq!(
            messages.last().unwrap().quote.as_ref().unwrap().text,
            "Código autorizado de referencia"
        );
        assert_eq!(
            messages.last().unwrap().quote.as_ref().unwrap().speaker,
            "assistant"
        );
        assert!(
            messages
                .iter()
                .map(|m| m.text.chars().count()
                    + m.quote.as_ref().map_or(0, |q| q.text.chars().count()))
                .sum::<usize>()
                <= 8000
        );
    }
}
