//! Citas persistidas: una captura explícita del texto original, sin activar su sesión.
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredQuote {
    pub source_message_id: Option<i64>,
    pub speaker: String,
    pub text: String,
    pub created_at: String,
    pub truncated: bool,
}

pub(crate) fn append_user(
    db: &Database,
    provider: &str,
    thread: &str,
    text: &str,
    source: i64,
) -> Result<gennie_llm::QuotedMessage> {
    if text.trim().is_empty() || source <= 0 {
        return Err("Mensaje o cita inválidos".into());
    }
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let target: i64 = tx.query_row(
            "SELECT c.id FROM presence_conversations c
             JOIN presence_current_conversation active ON active.conversation_id=c.id
             WHERE c.provider=?1 AND c.provider_thread_id=?2",
            params![provider, thread],
            |r| r.get(0),
        )?;
        let (_, quote) = insert_user(&tx, target, text, source)?;
        tx.commit()?;
        Ok(quote)
    })
}

pub(crate) fn insert_user(
    c: &SqliteConnection,
    target: i64,
    text: &str,
    source: i64,
) -> Result<(i64, gennie_llm::QuotedMessage)> {
    let (speaker, excerpt, created_at, truncated): (String, String, String, bool) = {
        let mut statement = c.prepare(
            "SELECT h.speaker,substr(h.text,1,4096),h.created_at,length(h.text)>4096
                 FROM presence_history h
                 JOIN presence_conversations s ON s.id=h.conversation_id
                 JOIN presence_assistant_conversations src ON src.conversation_id=s.id
                 JOIN presence_assistant_conversations dst ON dst.assistant_id=src.assistant_id
                 WHERE h.id=?1 AND s.deleted=0 AND dst.conversation_id=?2",
        )?;
        let mut rows = statement.query(params![source, target])?;
        let row = rows
            .next()?
            .ok_or("El mensaje original ya no está disponible en este chat")?;
        (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)
    };
    c.execute(
        "INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'user',?2)",
        params![target, text],
    )?;
    let message_id = c.last_insert_rowid();
    c.execute(
            "INSERT INTO presence_message_replies(message_id,source_message_id,speaker,text,quoted_created_at,truncated)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![message_id, source, speaker, excerpt, created_at, truncated],
        )?;
    c.execute(
            "UPDATE presence_conversations SET deleted=0,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",
            [target],
        )?;
    Ok((
        message_id,
        gennie_llm::QuotedMessage {
            speaker,
            text: excerpt,
            created_at,
            truncated,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    use gennie_llm::ConversationJournal;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    fn setup(db: &Database) -> crate::history::Journal {
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        crate::history::Journal {
            db: db.clone(),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }
    fn activate(journal: &crate::history::Journal, thread: &str) -> i64 {
        journal
            .db
            .with_connection(|c| {
                let tx = c.transaction()?;
                journal.activate(&tx, "codex", thread, None)?;
                tx.commit()?;
                Ok(())
            })
            .unwrap();
        crate::history::conversation_for_thread(&journal.db, thread).unwrap()
    }
    fn latest(db: &Database) -> crate::assistants::ChatEntry {
        crate::assistants::page(db, "gennie", None, 200, 100_000)
            .unwrap()
            .messages
            .pop()
            .unwrap()
    }
    #[test]
    fn replies_capture_bounded_unicode_from_an_older_session_and_keep_literal_input() {
        let db = Database::memory().unwrap();
        let journal = setup(&db);
        let old = activate(&journal, "old");
        let original = "🦀é".repeat(3000);
        journal
            .append("codex", "old", "assistant", &original)
            .unwrap();
        let source = latest(&db).id;
        let current = activate(&journal, "current");
        let quote = journal
            .append_user("codex", "current", "Mi texto literal", Some(source))
            .unwrap()
            .unwrap();
        assert_eq!(quote.text.chars().count(), 4096);
        assert!(quote.truncated);
        assert!(original.starts_with(&quote.text));
        let saved = latest(&db);
        assert_eq!(saved.conversation_id, current);
        assert_ne!(old, current);
        assert_eq!(saved.text, "Mi texto literal");
        assert_eq!(
            saved.quote.as_ref().unwrap().source_message_id,
            Some(source)
        );
        assert_eq!(saved.quote.unwrap().text, quote.text);
        assert_eq!(
            crate::history::load(&db, None).unwrap().active_id,
            Some(current)
        );
        assert_eq!(crate::history::load(&db, None).unwrap().messages.len(), 1);
    }
    #[test]
    fn quote_failure_rolls_back_literal_input_activity_and_revision() {
        let db = Database::memory().unwrap();
        let journal = setup(&db);
        activate(&journal, "old");
        journal
            .append("codex", "old", "user", "Texto original")
            .unwrap();
        let source = latest(&db).id;
        let target = activate(&journal, "current");
        db.with_connection(|c|{c.execute("UPDATE presence_conversations SET updated_at='2020-01-01T00:00:00.000Z' WHERE id=?1",[target])?;c.execute_batch("CREATE TRIGGER reject_quote BEFORE INSERT ON presence_message_replies BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
        let revision = journal.revision.load(Ordering::Acquire);
        assert!(
            journal
                .append_user("codex", "current", "No debe persistir", Some(source))
                .is_err()
        );
        assert_eq!(journal.revision.load(Ordering::Acquire), revision);
        assert!(crate::history::load(&db, None).unwrap().messages.is_empty());
        db.with_connection(|c| {
            assert_eq!(
                c.query_row(
                    "SELECT updated_at FROM presence_conversations WHERE id=?1",
                    [target],
                    |r| r.get::<_, String>(0)
                )?,
                "2020-01-01T00:00:00.000Z"
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn missing_deleted_and_foreign_sources_are_rejected_without_a_user_message() {
        let db = Database::memory().unwrap();
        let journal = setup(&db);
        let old = activate(&journal, "old");
        journal
            .append("codex", "old", "assistant", "Privado")
            .unwrap();
        let source = latest(&db).id;
        activate(&journal, "current");
        db.with_connection(|c|{c.execute("INSERT INTO presence_assistants(id,name) VALUES('other','Otro')",[])?;c.execute("UPDATE presence_assistant_conversations SET assistant_id='other' WHERE conversation_id=?1",[old])?;Ok(())}).unwrap();
        assert!(
            journal
                .append_user("codex", "current", "No", Some(source))
                .is_err()
        );
        assert!(
            journal
                .append_user("codex", "current", "No", Some(99999))
                .is_err()
        );
        crate::history::delete(&db, old).unwrap();
        assert!(
            journal
                .append_user("codex", "current", "No", Some(source))
                .is_err()
        );
        assert!(crate::history::load(&db, None).unwrap().messages.is_empty());
    }
    #[test]
    fn deleted_original_keeps_excerpt_and_committed_ids_are_never_reused() {
        let db = Database::memory().unwrap();
        let journal = setup(&db);
        let old = activate(&journal, "old");
        journal.append("codex", "old", "user", "Original").unwrap();
        let source = latest(&db).id;
        let current = activate(&journal, "current");
        journal
            .append_user("codex", "current", "Respuesta", Some(source))
            .unwrap();
        let reply_id = latest(&db).id;
        crate::history::delete(&db, old).unwrap();
        let quote = latest(&db).quote.unwrap();
        assert_eq!(quote.text, "Original");
        assert_eq!(quote.source_message_id, None);
        crate::history::delete(&db, current).unwrap();
        activate(&journal, "new");
        journal.append("codex", "new", "user", "Nuevo").unwrap();
        assert!(latest(&db).id > reply_id);
        db.with_connection(|c| {
            assert_eq!(
                c.query_row("SELECT count(*) FROM presence_message_replies", [], |r| r
                    .get::<_, i64>(
                    0
                ))?,
                0
            );
            assert!(
                c.prepare("PRAGMA foreign_key_check")?
                    .query([])?
                    .next()?
                    .is_none()
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn migration_preserves_history_ids_receipts_and_reopen_keeps_the_quote() {
        let directory = std::env::temp_dir().join(format!(
            "gennie-quotes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("fixture.sqlite3");
        let db = Database::open(&path, 50).unwrap();
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), &module.migrations()[..12])
            .unwrap();
        let journal = crate::history::Journal {
            db: db.clone(),
            revision: Arc::default(),
        };
        let old = activate(&journal, "old");
        journal
            .append("codex", "old", "assistant", "Conservar á🦀")
            .unwrap();
        crate::history::deliver_message(&db, old, "event:1", "Avance").unwrap();
        let before = crate::history::legacy_fixture::load(&db, None).unwrap();
        db.migrate(module.name(), module.migrations()).unwrap();
        let after = crate::history::load(&db, None).unwrap();
        assert_eq!(before.messages, after.messages);
        assert_eq!(before.conversations, after.conversations);
        let source = crate::assistants::page(&db, "gennie", None, 200, 100_000)
            .unwrap()
            .messages[0]
            .id;
        let target = activate(&journal, "current");
        journal
            .append_user("codex", "current", "Continuar", Some(source))
            .unwrap();
        let persisted = latest(&db);
        db.close().unwrap();
        let reopened = Database::open(&path, 50).unwrap();
        setup(&reopened);
        assert_eq!(latest(&reopened), persisted);
        assert_eq!(
            crate::history::load(&reopened, None).unwrap().active_id,
            Some(target)
        );
        crate::history::deliver_message(&reopened, old, "event:1", "Avance").unwrap();
        assert_eq!(latest(&reopened), persisted);
        reopened.close().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn explicit_quoted_input_revives_only_the_current_target_without_restoring_deleted_content() {
        let db = Database::memory().unwrap();
        let journal = setup(&db);
        activate(&journal, "source");
        journal
            .append("codex", "source", "assistant", "Referencia")
            .unwrap();
        let source = latest(&db).id;
        let target = activate(&journal, "current");
        journal
            .append("codex", "current", "user", "Se elimina")
            .unwrap();
        crate::history::delete(&db, target).unwrap();
        assert!(crate::history::load(&db, None).unwrap().active_id.is_none());
        journal
            .append_user("codex", "current", "Entrada explícita", Some(source))
            .unwrap();
        let snapshot = crate::history::load(&db, None).unwrap();
        assert_eq!(snapshot.active_id, Some(target));
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].text, "Entrada explícita");
        assert_eq!(latest(&db).quote.unwrap().source_message_id, Some(source));
    }
}
