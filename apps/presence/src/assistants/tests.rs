use super::*;
use gennie_core::AppModule;
use gennie_llm::ConversationJournal;
use std::sync::Arc;

fn migrate(db: &Database) {
    let module = crate::PresenceModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
}
fn journal(db: &Database) -> crate::history::Journal {
    crate::history::Journal {
        db: db.clone(),
        revision: Arc::default(),
    }
}
fn activate(db: &Database, thread: &str) -> i64 {
    db.with_connection(|c| {
        let tx = c.transaction()?;
        journal(db).activate(&tx, "codex", thread, None)?;
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    crate::history::conversation_for_thread(db, thread).unwrap()
}

#[test]
fn migration_preserves_sessions_messages_deliveries_and_active_thread() {
    let db = Database::memory().unwrap();
    let module = crate::PresenceModule::default();
    db.migrate(module.name(), &module.migrations()[..11])
        .unwrap();
    let first = activate(&db, "first");
    journal(&db)
        .append("codex", "first", "user", "Texto á🦀")
        .unwrap();
    crate::history::rename(&db, first, "Nombre de sesión").unwrap();
    crate::history::deliver_message(&db, first, "plugin:event:1", "Avance").unwrap();
    let second = activate(&db, "second");
    journal(&db)
        .append("codex", "second", "assistant", "Respuesta")
        .unwrap();
    let before = crate::history::legacy_fixture::load(&db, Some(first)).unwrap();
    let identities=db.with_connection(|c| Ok(c.prepare("SELECT id,conversation_id,speaker,text,created_at FROM presence_history ORDER BY id")?.query_map([], |r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)).unwrap();
    migrate(&db);
    let after = crate::history::load(&db, Some(first)).unwrap();
    assert_eq!(before.conversations, after.conversations);
    assert_eq!(before.selected_messages, after.selected_messages);
    assert_eq!(after.active_id, Some(second));
    assert_eq!(load(&db, DEFAULT_ASSISTANT_ID).unwrap().name, "Gennie");
    let all = page(&db, DEFAULT_ASSISTANT_ID, None, 200, 100_000).unwrap();
    assert_eq!(all.messages.len(), 3);
    for m in all.messages {
        assert!(identities.contains(&(m.id, m.conversation_id, m.speaker, m.text, m.created_at)));
    }
    crate::history::deliver_message(&db, first, "plugin:event:1", "Avance").unwrap();
    assert_eq!(
        page(&db, DEFAULT_ASSISTANT_ID, None, 200, 100_000)
            .unwrap()
            .messages
            .len(),
        3
    );
    db.with_connection(|c| {
        assert!(
            c.prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        activate(&db, "third"),
        crate::history::load(&db, None).unwrap().active_id.unwrap()
    );
    journal(&db)
        .append("codex", "third", "user", "Nuevo mensaje")
        .unwrap();
    assert_eq!(
        page(&db, DEFAULT_ASSISTANT_ID, None, 200, 100_000)
            .unwrap()
            .messages
            .len(),
        4
    );
}

#[test]
fn keyset_pages_handle_equal_dates_unicode_and_large_messages_without_duplicates() {
    let db = Database::memory().unwrap();
    migrate(&db);
    let first = activate(&db, "first");
    let second = activate(&db, "second");
    db.with_connection(|c| {
        for (id,text) in [(first,"á🦀".repeat(100)),(second,"uno".into()),(first,"dos".into()),(second,"tres".into())] {
            c.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(?1,'user',?2,'2026-10-28T00:00:00.000Z')",params![id,text])?;
        }
        Ok(())
    }).unwrap();
    let latest = page(&db, "gennie", None, 2, 6).unwrap();
    assert_eq!(
        latest
            .messages
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["tres"]
    );
    let middle = page(&db, "gennie", latest.before.as_ref(), 2, 6).unwrap();
    assert_eq!(
        middle
            .messages
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["uno", "dos"]
    );
    let oldest = page(&db, "gennie", middle.before.as_ref(), 2, 6).unwrap();
    assert_eq!(oldest.messages[0].text, "á🦀".repeat(100));
    assert!(oldest.before.is_none());
    assert_eq!(
        crate::history::load(&db, None).unwrap().active_id,
        Some(second)
    );
    let ids = latest
        .messages
        .iter()
        .chain(&middle.messages)
        .chain(&oldest.messages)
        .map(|m| m.id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 4);
    assert!(page(&db, "gennie", None, 0, 6).is_err());
    assert!(page(&db, "missing", None, 2, 6).is_err());
}

#[test]
fn deleted_sessions_and_foreign_cursors_do_not_leak_into_chat() {
    let db = Database::memory().unwrap();
    migrate(&db);
    activate(&db, "first");
    journal(&db)
        .append("codex", "first", "user", "Visible")
        .unwrap();
    let deleted = activate(&db, "deleted");
    journal(&db)
        .append("codex", "deleted", "user", "Oculto")
        .unwrap();
    let previous = page(&db, "gennie", None, 1, 100).unwrap();
    let cursor = previous.before.unwrap();
    crate::history::delete(&db, deleted).unwrap();
    assert!(page(&db, "gennie", Some(&cursor), 2, 100).is_err());
    assert_eq!(
        page(&db, "gennie", None, 2, 100).unwrap().messages[0].text,
        "Visible"
    );
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO presence_assistants(id,name) VALUES('other','Otro')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let foreign = page(&db, "gennie", None, 2, 100)
        .unwrap()
        .messages
        .remove(0);
    assert!(
        page(
            &db,
            "other",
            Some(&Cursor {
                created_at: foreign.created_at,
                message_id: foreign.id
            }),
            2,
            100
        )
        .is_err()
    );
}

#[test]
fn failed_migration_rolls_back_identity_and_can_retry() {
    let db = Database::memory().unwrap();
    let module = crate::PresenceModule::default();
    db.migrate(module.name(), &module.migrations()[..11])
        .unwrap();
    activate(&db, "original");
    journal(&db)
        .append("codex", "original", "user", "Preservar")
        .unwrap();
    db.with_connection(|c| {
        c.execute_batch("CREATE TABLE presence_assistant_conversations(conflict TEXT)")?;
        Ok(())
    })
    .unwrap();
    assert!(db.migrate(module.name(), module.migrations()).is_err());
    db.with_connection(|c| {
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='presence_assistants'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        assert_eq!(
            c.query_row(
                "SELECT max(version) FROM _soc_migrations_presence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            11
        );
        c.execute_batch("DROP TABLE presence_assistant_conversations")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        crate::history::legacy_fixture::load(&db, None)
            .unwrap()
            .messages[0]
            .text,
        "Preservar"
    );
    migrate(&db);
    assert_eq!(
        page(&db, "gennie", None, 2, 100).unwrap().messages[0].text,
        "Preservar"
    );
}

#[test]
fn identity_and_names_survive_reopen_without_changing_session_titles() {
    let path = std::env::temp_dir().join(format!(
        "gennie-assistant-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Database::open(&path, 50).unwrap();
    migrate(&db);
    let id = activate(&db, "current");
    crate::history::rename(&db, id, "Título histórico").unwrap();
    let profile = rename(&db, "gennie", "  Mi asistente 🦀  ").unwrap();
    for invalid in ["".into(), "a\nb".into(), "a".repeat(201)] {
        assert!(rename(&db, "gennie", &invalid).is_err());
    }
    assert!(rename(&db, "missing", "Nombre").is_err());
    db.close().unwrap();
    let db = Database::open(&path, 50).unwrap();
    migrate(&db);
    assert_eq!(load(&db, "gennie").unwrap(), profile);
    assert_eq!(
        crate::history::load(&db, None).unwrap().conversations[0].title,
        "Título histórico"
    );
    assert!(
        page(&db, "gennie", None, 10, 100)
            .unwrap()
            .messages
            .is_empty()
    );
    db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn hundreds_of_sessions_page_without_loading_context_or_repeating_recent_insertions() {
    let db = Database::memory().unwrap();
    migrate(&db);
    for index in 0..300 {
        let id = activate(&db, &format!("thread-{index}"));
        db.with_connection(|c| {
            c.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(?1,'user',?2,'2026-10-28T00:00:00.000Z')",params![id,format!("Mensaje {index} 🦀")])?;Ok(())
        }).unwrap();
    }
    let mut current = page(&db, "gennie", None, 37, 250).unwrap();
    assert_eq!(
        current.messages.len(),
        3,
        "No cargar docenas de sesiones cortas"
    );
    let active = crate::history::load(&db, None).unwrap().active_id;
    // Una entrada nueva posterior al cursor no desplaza las páginas anteriores.
    journal(&db)
        .append("codex", "thread-299", "user", "Nuevo durante la lectura")
        .unwrap();
    let mut ids = std::collections::HashSet::new();
    let mut pages = 0;
    loop {
        pages += 1;
        assert!(pages <= 300);
        assert!(current.messages.len() <= 37);
        assert!(
            current
                .messages
                .iter()
                .map(|m| m.conversation_id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                <= 3
        );
        assert!(
            current
                .messages
                .iter()
                .map(|m| m.text.chars().count())
                .sum::<usize>()
                <= 250
        );
        for message in &current.messages {
            assert!(ids.insert(message.id));
        }
        match current.before {
            Some(ref before) => current = page(&db, "gennie", Some(before), 37, 250).unwrap(),
            None => break,
        }
    }
    assert_eq!(ids.len(), 300);
    assert!(pages > 1);
    assert_eq!(crate::history::load(&db, None).unwrap().active_id, active);
    assert!(
        page(&db, "gennie", None, 37, 250)
            .unwrap()
            .messages
            .iter()
            .any(|m| m.text == "Nuevo durante la lectura")
    );
}

#[test]
fn session_budget_counts_distinct_sessions_and_preserves_interleaved_notices() {
    let db = Database::memory().unwrap();
    migrate(&db);
    let a = activate(&db, "thread-a");
    let b = activate(&db, "thread-b");
    let c = activate(&db, "thread-c");
    let d = activate(&db, "thread-d");
    for (index, session) in [a, b, a, c, b, d, c].into_iter().enumerate() {
        db.with_connection(|connection| {
            connection.execute("INSERT INTO presence_history(conversation_id,speaker,text,created_at) VALUES(?1,'assistant',?2,'2026-10-28T00:00:00.000Z')",params![session,format!("Observación {index}")])?;
            Ok(())
        }).unwrap();
    }
    let latest = page(&db, "gennie", None, 60, 24_000).unwrap();
    assert_eq!(latest.messages.len(), 4);
    assert_eq!(
        latest
            .messages
            .iter()
            .map(|m| m.conversation_id)
            .collect::<std::collections::BTreeSet<_>>(),
        [b, c, d].into_iter().collect()
    );
    let older = page(&db, "gennie", latest.before.as_ref(), 60, 24_000).unwrap();
    assert_eq!(older.messages.len(), 3);
    assert!(older.before.is_none());
    let all = older
        .messages
        .iter()
        .chain(&latest.messages)
        .map(|m| m.id)
        .collect::<Vec<_>>();
    assert_eq!(all, (1..=7).collect::<Vec<_>>());
    assert_eq!(crate::history::load(&db, None).unwrap().active_id, Some(d));
}

#[test]
fn compatibility_snapshots_are_bounded_and_session_markers_come_from_storage() {
    let db = Database::memory().unwrap();
    migrate(&db);
    let id = activate(&db, "thread");
    for index in 0..100 {
        journal(&db)
            .append("codex", "thread", "user", &format!("Entrada {index}"))
            .unwrap();
    }
    let full = crate::history::load(&db, None).unwrap();
    let recent = crate::history::load_recent(&db, None).unwrap();
    assert_eq!(full.messages.len(), 100);
    assert_eq!(recent.messages.len(), 60);
    assert_eq!(recent.messages[0].text, "Entrada 40");
    assert_eq!(recent.selected_messages.len(), 60);
    assert_eq!(recent.active_id, Some(id));
    let latest = page(&db, "gennie", None, 60, 24_000).unwrap();
    assert!(latest.messages.iter().all(|m| !m.starts_session));
    let oldest = page(&db, "gennie", latest.before.as_ref(), 60, 24_000).unwrap();
    assert!(oldest.messages[0].starts_session);
    assert_eq!(
        oldest.messages[0].session_started_at,
        crate::history::session_started_at(&db, Some(id))
            .unwrap()
            .unwrap()
    );
    assert!(oldest.messages[1..].iter().all(|m| !m.starts_session));
    journal(&db)
        .append("codex", "thread", "assistant", &"🦀".repeat(24_001))
        .unwrap();
    let recent = crate::history::load_recent(&db, None).unwrap();
    assert_eq!(recent.messages.len(), 1);
    assert_eq!(recent.messages[0].text.chars().count(), 24_001);
    assert_eq!(crate::history::load(&db, None).unwrap().messages.len(), 101);
}
