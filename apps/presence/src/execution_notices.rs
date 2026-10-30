//! Mensajes del bot con referencia tipada a la ejecución, sin atribución a un helper.
use gennie_core::Result;
use gennie_db::{SqliteConnection, params};
use gennie_llm::notices::{Reference, Update};
pub(crate) fn deliver(c: &SqliteConnection, update: &Update, text: &str) -> Result<bool> {
    update.validate()?;
    if text.trim().is_empty() || text.chars().count() > 1600 {
        return Err("Mensaje del bot inválido".into());
    }
    if crate::delegation::origin_in(c, update.origin.conversation_id)?.as_ref()
        != Some(&update.origin)
    {
        return Ok(false);
    }
    let reference = serde_json::to_string(&update.reference)?;
    let existing = {
        let mut q=c.prepare("SELECT h.conversation_id,h.text,e.reference FROM presence_execution_messages e JOIN presence_history h ON h.id=e.message_id WHERE e.event_key=?1")?;
        q.query_map([&update.event_key], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .next()
        .transpose()?
    };
    if let Some((id, saved, saved_reference)) = existing {
        if id != update.origin.conversation_id || saved != text || saved_reference != reference {
            return Err("El recibo del bot pertenece a otra ejecución o mensaje".into());
        }
        return Ok(true);
    }
    c.execute(
        "INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'assistant',?2)",
        params![update.origin.conversation_id, text],
    )?;
    let message = c.last_insert_rowid();
    c.execute(
        "INSERT INTO presence_execution_messages(message_id,event_key,reference) VALUES(?1,?2,?3)",
        params![message, update.event_key, reference],
    )?;
    crate::provider_context::track(c, message, update.origin.conversation_id)?;
    c.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[update.origin.conversation_id])?;
    Ok(true)
}
pub(crate) fn load(c: &SqliteConnection, message: i64) -> Result<Option<Reference>> {
    let mut q =
        c.prepare("SELECT reference FROM presence_execution_messages WHERE message_id=?1")?;
    let text = q
        .query_map([message], |r| r.get::<_, String>(0))?
        .next()
        .transpose()?;
    text.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    use gennie_db::Database;
    use gennie_llm::{ConversationJournal, delegation::Origin};
    fn fixture() -> (Database, std::sync::Arc<dyn ConversationJournal>, Update) {
        let db = Database::memory().unwrap();
        let module = crate::PresenceModule::default();
        db.migrate("presence", module.migrations()).unwrap();
        let journal = module.conversation_journal(db.clone());
        db.with_connection(|c| {
            let tx = c.transaction()?;
            journal.activate(&tx, "codex", "origin", None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        journal
            .append("codex", "origin", "user", "Solo el contexto autorizado")
            .unwrap();
        let id = crate::history::conversation_for_thread(&db, "origin").unwrap();
        let origin = crate::delegation::origin(&db, id).unwrap().unwrap();
        let update = Update {
            event_key: "assignment-execution:12".into(),
            origin,
            reference: gennie_llm::notices::Reference {
                assignment_id: 7,
                instance_id: "a".repeat(32),
                policy_id: "policy-fixture".into(),
                policy_revision: 4,
                run_id: 12,
                state: "succeeded".into(),
            },
            title: "Novedades".into(),
            scheduled_at: "2026-10-29T13:00:00Z".into(),
            state: "succeeded".into(),
            code: "reported_success".into(),
            observation: None,
            observation_truncated: false,
            guidance: None,
        };
        (db, journal, update)
    }
    #[test]
    fn receipt_is_idempotent_typed_and_does_not_change_active_session() {
        let (db, journal, update) = fixture();
        db.with_connection(|c| {
            let tx = c.transaction()?;
            journal.activate(&tx, "codex", "other", None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        journal
            .append("codex", "other", "user", "OTHER_PRIVATE_SESSION")
            .unwrap();
        let active = crate::history::active_conversation(&db).unwrap();
        let context = journal.delegation_context(&update.origin).unwrap().unwrap();
        assert_eq!(context.len(), 1);
        assert!(!context[0].text.contains("OTHER_PRIVATE_SESSION"));
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(deliver(&tx, &update, "La función informó novedades.")?);
            assert!(deliver(&tx, &update, "La función informó novedades.")?);
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        assert_eq!(crate::history::active_conversation(&db).unwrap(), active);
        let page = crate::assistants::page(&db, "gennie", None, 60, 24000).unwrap();
        let matches = page
            .messages
            .iter()
            .filter(|m| m.execution.is_some())
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].execution.as_ref().unwrap(), &update.reference);
        assert!(matches[0].delegation.is_none());
        assert_eq!(matches[0].speaker, "assistant");
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(deliver(&tx, &update, "Otro texto incompatible").is_err());
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn deletion_epoch_and_receipt_failure_never_recreate_or_leave_partial_messages() {
        let (db, journal, update) = fixture();
        db.with_connection(|c|{c.execute_batch("CREATE TRIGGER fail_execution_receipt BEFORE INSERT ON presence_execution_messages BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
        assert!(
            db.with_connection(|c| {
                let tx = c.transaction()?;
                deliver(&tx, &update, "Resultado del bot")?;
                tx.commit()?;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(
            crate::history::load(&db, Some(update.origin.conversation_id))
                .unwrap()
                .selected_messages
                .len(),
            1
        );
        db.with_connection(|c| {
            c.execute_batch("DROP TRIGGER fail_execution_receipt")?;
            Ok(())
        })
        .unwrap();
        crate::history::delete(&db, update.origin.conversation_id).unwrap();
        journal
            .append_user("codex", "origin", "Nuevo contenido", None)
            .unwrap();
        let new_origin = crate::delegation::origin(&db, update.origin.conversation_id)
            .unwrap()
            .unwrap();
        assert_ne!(new_origin, update.origin);
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(!deliver(&tx, &update, "Resultado tardío")?);
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        let mut current = update.clone();
        current.origin = Origin {
            epoch: new_origin.epoch,
            ..update.origin
        };
        db.with_connection(|c| {
            let tx = c.transaction()?;
            assert!(deliver(&tx, &current, "Resultado vigente")?);
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        crate::history::delete(&db, current.origin.conversation_id).unwrap();
        db.with_connection(|c| {
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM presence_execution_messages",
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
    fn execution_state_and_reference_share_the_history_character_budget() {
        let (db, _, update) = fixture();
        let mut later = update.clone();
        later.reference.run_id = 13;
        later.event_key = "assignment-execution:13".into();
        db.with_connection(|c| {
            let tx = c.transaction()?;
            deliver(&tx, &update, "Aviso")?;
            deliver(&tx, &later, "Aviso")?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
        // Dos mensajes más todos los valores de referencia, incluidos ambos estados, menos un carácter.
        let budget = 2 * (5 + 32 + "policy-fixture".len() + 1 + 2 + 1 + "succeeded".len()) - 1;
        let page = crate::assistants::page(&db, "gennie", None, 60, budget).unwrap();
        assert_eq!(page.messages.len(), 1);
        assert_eq!(page.messages[0].execution.as_ref().unwrap().run_id, 13);
        assert!(page.before.is_some());
    }
}
