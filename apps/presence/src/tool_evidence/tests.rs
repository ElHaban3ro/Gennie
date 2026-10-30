use super::*;
use crate::{PresenceModule, assistants, history};
use gennie_core::AppModule;
use gennie_llm::ConversationJournal;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

fn setup(db: &Database) -> history::Journal {
    let module = PresenceModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
    history::Journal {
        db: db.clone(),
        revision: Arc::new(AtomicU64::new(0)),
    }
}
fn activate(journal: &history::Journal, thread: &str) -> i64 {
    journal
        .db
        .with_connection(|c| {
            let tx = c.transaction()?;
            journal.activate(&tx, "codex", thread, None)?;
            tx.commit()?;
            Ok(())
        })
        .unwrap();
    history::conversation_for_thread(&journal.db, thread).unwrap()
}
fn usage(id: &str, outcome: ToolOutcome) -> ToolUse {
    ToolUse {
        call_id: id.into(),
        server: "gennie".into(),
        tool: "browser_plugin_run".into(),
        plugin_id: Some("mail".into()),
        function_name: Some("list".into()),
        display_name: Some("Correo".into()),
        operation: Some("run".into()),
        outcome,
        detail: None,
    }
}
fn page(db: &Database) -> Vec<assistants::ChatEntry> {
    assistants::page(db, "gennie", None, 200, 100000)
        .unwrap()
        .messages
}

#[test]
fn interrupted_fragment_and_tool_state_commit_once_without_downgrading_complete_items() {
    use gennie_llm::ReplyInterruption::{Cancelled, Failed};
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    let conversation = activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Entrada")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    journal
        .append_response(
            "codex",
            "active",
            "turn",
            Some("complete"),
            "Mensaje completo",
        )
        .unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("running", ToolOutcome::Running),
        )
        .unwrap();
    journal
        .append_interrupted_response(
            "codex",
            "active",
            "turn",
            Some("partial"),
            "  Fragmento á🦀\n",
            Cancelled,
        )
        .unwrap();
    let expected = page(&db);
    assert_eq!(expected.len(), 3);
    assert_eq!(expected[1].interruption, None);
    assert_eq!(expected[2].text, "  Fragmento á🦀\n");
    assert_eq!(expected[2].interruption, Some(Cancelled));
    assert_eq!(expected[2].tools[0].usage.outcome, ToolOutcome::Unknown);
    let revision = journal.revision.load(Ordering::Acquire);
    journal
        .append_interrupted_response(
            "codex",
            "active",
            "turn",
            Some("partial"),
            "  Fragmento á🦀\n",
            Cancelled,
        )
        .unwrap();
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    assert!(
        journal
            .append_response(
                "codex",
                "active",
                "turn",
                Some("partial"),
                "  Fragmento á🦀\n"
            )
            .is_err()
    );
    assert!(
        journal
            .append_interrupted_response(
                "codex",
                "active",
                "turn",
                Some("complete"),
                "Mensaje completo",
                Cancelled
            )
            .is_err()
    );
    assert!(
        journal
            .append_interrupted_response(
                "codex",
                "active",
                "turn",
                Some("partial"),
                "Distinto",
                Failed
            )
            .is_err()
    );
    assert_eq!(page(&db), expected);
    journal
        .append_interrupted_response("codex", "active", "turn", None, "Sin ID", Failed)
        .unwrap();
    journal
        .append_interrupted_response("codex", "active", "turn", None, "Sin ID", Failed)
        .unwrap();
    assert_eq!(page(&db).len(), 4);
    history::delete(&db, conversation).unwrap();
    assert!(
        journal
            .append_interrupted_response("codex", "active", "turn", None, "Tardío", Cancelled)
            .is_err()
    );
    assert!(page(&db).is_empty());
}

#[test]
fn interrupted_write_failure_rolls_back_fragment_attribution_and_tool_state() {
    use gennie_llm::ReplyInterruption::Cancelled;
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Entrada")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("running", ToolOutcome::Running),
        )
        .unwrap();
    let before = page(&db);
    let revision = journal.revision.load(Ordering::Acquire);
    db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_interruption BEFORE UPDATE OF outcome ON presence_tool_uses WHEN NEW.outcome='unknown' BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(
        journal
            .append_interrupted_response(
                "codex",
                "active",
                "turn",
                Some("partial"),
                "Fragmento",
                Cancelled
            )
            .is_err()
    );
    assert_eq!(page(&db), before);
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_interruption")?;
        Ok(())
    })
    .unwrap();
    journal
        .append_interrupted_response(
            "codex",
            "active",
            "turn",
            Some("partial"),
            "Fragmento",
            Cancelled,
        )
        .unwrap();
    assert_eq!(page(&db)[1].tools[0].usage.outcome, ToolOutcome::Unknown);
}

#[test]
fn migration_0019_preserves_history_rolls_back_and_reopens_an_interrupted_fragment() {
    use gennie_llm::ReplyInterruption::Cancelled;
    let root = std::env::temp_dir().join(format!(
        "gennie-interruption-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = root.join("fixture.sqlite3");
    let db = Database::open(&path, 50).unwrap();
    let module = PresenceModule::default();
    db.migrate(module.name(), &module.migrations()[..18])
        .unwrap();
    let journal = history::Journal {
        db: db.clone(),
        revision: Arc::default(),
    };
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Conservar á🦀")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    let before=db.with_connection(|c|Ok(c.prepare("SELECT id,speaker,text,created_at,provider_turn_id FROM presence_history ORDER BY id")?.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)).unwrap();
    db.with_connection(|c| {
        c.execute_batch("ALTER TABLE presence_history ADD COLUMN interruption TEXT")?;
        Ok(())
    })
    .unwrap();
    assert!(db.migrate(module.name(), module.migrations()).is_err());
    db.with_connection(|c| {
        let version: i64 = c.query_row(
            "SELECT max(version) FROM _soc_migrations_presence",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(version, 18);
        c.execute_batch("ALTER TABLE presence_history DROP COLUMN interruption")?;
        Ok(())
    })
    .unwrap();
    setup(&db);
    assert_eq!(page(&db)[0].interruption, None);
    let after=db.with_connection(|c|Ok(c.prepare("SELECT id,speaker,text,created_at,provider_turn_id FROM presence_history ORDER BY id")?.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)).unwrap();
    assert_eq!(before, after);
    journal
        .append_interrupted_response(
            "codex",
            "active",
            "turn",
            Some("fragment"),
            "Texto recibido",
            Cancelled,
        )
        .unwrap();
    let expected = page(&db);
    db.close().unwrap();
    let db = Database::open(&path, 50).unwrap();
    setup(&db);
    assert_eq!(page(&db), expected);
    db.with_connection(|c| {
        let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        assert_eq!(integrity, "ok");
        let fk: i64 = c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
        assert_eq!(fk, 0);
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn calls_move_from_the_request_to_the_next_response_once_and_keep_late_status_updates() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Consulta")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    let first = usage("one", ToolOutcome::Running);
    journal
        .record_tool("codex", "active", "turn", &first)
        .unwrap();
    let revision = journal.revision.load(Ordering::Acquire);
    journal
        .record_tool("codex", "active", "turn", &first)
        .unwrap();
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    assert_eq!(page(&db)[0].tools.len(), 1);
    journal
        .append_response(
            "codex",
            "active",
            "turn",
            Some("partial"),
            "Primera respuesta",
        )
        .unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("one", ToolOutcome::Completed),
        )
        .unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("two", ToolOutcome::Failed),
        )
        .unwrap();
    // Un ítem repetido no captura una llamada que comenzó después de él.
    journal
        .append_response(
            "codex",
            "active",
            "turn",
            Some("partial"),
            "Primera respuesta",
        )
        .unwrap();
    journal
        .append_response("codex", "active", "turn", Some("final"), "Respuesta final")
        .unwrap();
    journal.finish_turn("codex", "active", "turn").unwrap();
    let entries = page(&db);
    assert_eq!(entries.len(), 3);
    assert!(entries[0].tools.is_empty());
    assert_eq!(
        entries[1].tools[0].usage,
        usage("one", ToolOutcome::Completed)
    );
    assert_eq!(entries[2].tools[0].usage, usage("two", ToolOutcome::Failed));
    assert!(
        journal
            .append_response("codex", "active", "turn", Some("final"), "Distinto")
            .is_err()
    );
    assert_eq!(page(&db), entries);
}

#[test]
fn independent_threads_and_deleted_sessions_do_not_mix_or_reappear() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    let old = activate(&journal, "old");
    journal.append("codex", "old", "user", "Vieja").unwrap();
    journal.begin_turn("codex", "old", "same").unwrap();
    let active = activate(&journal, "active");
    journal.append("codex", "active", "user", "Nueva").unwrap();
    journal.begin_turn("codex", "active", "same").unwrap();
    journal
        .record_tool(
            "codex",
            "old",
            "same",
            &usage("call", ToolOutcome::Completed),
        )
        .unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "same",
            &usage("call", ToolOutcome::Failed),
        )
        .unwrap();
    assert!(journal.begin_turn("codex", "active", "other").is_err());
    let entries = page(&db);
    assert_eq!(
        entries
            .iter()
            .find(|e| e.conversation_id == old)
            .unwrap()
            .tools[0]
            .usage
            .outcome,
        ToolOutcome::Completed
    );
    assert_eq!(
        entries
            .iter()
            .find(|e| e.conversation_id == active)
            .unwrap()
            .tools[0]
            .usage
            .outcome,
        ToolOutcome::Failed
    );
    history::delete(&db, old).unwrap();
    let revision = journal.revision.load(Ordering::Acquire);
    journal
        .record_tool(
            "codex",
            "old",
            "same",
            &usage("late", ToolOutcome::Completed),
        )
        .unwrap();
    journal
        .append_response("codex", "old", "same", Some("late"), "No restaurar")
        .unwrap();
    journal.finish_turn("codex", "old", "same").unwrap();
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    assert_eq!(page(&db).len(), 1);
    assert_eq!(history::load(&db, None).unwrap().active_id, Some(active));
    assert!(
        page(&db)[0]
            .tools
            .iter()
            .all(|u| u.usage.outcome == ToolOutcome::Failed)
    );
}

#[test]
fn unfinished_and_recovered_calls_remain_visible_without_inventing_a_response_or_success() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Consulta")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("pending", ToolOutcome::Running),
        )
        .unwrap();
    journal.finish_turn("codex", "active", "turn").unwrap();
    let entries = page(&db);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tools[0].usage.outcome, ToolOutcome::Unknown);
    assert_eq!(
        entries[0].tools[0].usage.detail.as_deref(),
        Some("unconfirmed")
    );
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("recovered", ToolOutcome::Running),
        )
        .unwrap();
    assert!(recover(&db).unwrap());
    assert!(!recover(&db).unwrap());
    let entries = page(&db);
    assert_eq!(entries[0].tools[1].usage.detail.as_deref(), Some("restart"));
    assert!(
        entries[0]
            .tools
            .iter()
            .all(|u| u.usage.outcome == ToolOutcome::Unknown)
    );
}

#[test]
fn failures_roll_back_evidence_response_activity_and_commit_counter() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Conservar")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    db.with_connection(|c| {c.execute_batch("CREATE TRIGGER reject_tool BEFORE INSERT ON presence_tool_uses BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let revision = journal.revision.load(Ordering::Acquire);
    assert!(
        journal
            .record_tool(
                "codex",
                "active",
                "turn",
                &usage("one", ToolOutcome::Completed)
            )
            .is_err()
    );
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    assert!(page(&db)[0].tools.is_empty());
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_tool;")?;
        Ok(())
    })
    .unwrap();
    journal
        .record_tool(
            "codex",
            "active",
            "turn",
            &usage("one", ToolOutcome::Completed),
        )
        .unwrap();
    db.with_connection(|c| {c.execute_batch("UPDATE presence_conversations SET updated_at='2000-01-01T00:00:00.000Z'; CREATE TRIGGER reject_binding BEFORE UPDATE OF message_id ON presence_tool_uses BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let before = page(&db);
    let history = history::load(&db, None).unwrap();
    let revision = journal.revision.load(Ordering::Acquire);
    assert!(
        journal
            .append_response(
                "codex",
                "active",
                "turn",
                Some("reply"),
                "No guardar a medias"
            )
            .is_err()
    );
    assert_eq!(page(&db), before);
    assert_eq!(
        history::load(&db, None).unwrap().conversations,
        history.conversations
    );
    assert_eq!(journal.revision.load(Ordering::Acquire), revision);
    let mut invalid = usage("bad", ToolOutcome::Completed);
    invalid.detail = Some("PRIVATE_ERROR".into());
    assert!(
        journal
            .record_tool("codex", "active", "turn", &invalid)
            .is_err()
    );
}

#[test]
fn migration_and_reopen_preserve_history_and_receipts_and_retain_bound_evidence() {
    let root = std::env::temp_dir().join(format!(
        "gennie-tools-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = root.join("fixture.sqlite3");
    let db = Database::open(&path, 50).unwrap();
    let module = PresenceModule::default();
    db.migrate(module.name(), &module.migrations()[..14])
        .unwrap();
    let journal = history::Journal {
        db: db.clone(),
        revision: Arc::default(),
    };
    let conversation = activate(&journal, "old");
    journal
        .append("codex", "old", "assistant", "Historial íntegro á🦀")
        .unwrap();
    history::deliver_message(&db, conversation, "event:old", "Avance").unwrap();
    let before = history::legacy_fixture::load(&db, None).unwrap();
    let journal = setup(&db);
    assert_eq!(history::load(&db, None).unwrap().messages, before.messages);
    journal
        .append("codex", "old", "user", "Consulta nueva")
        .unwrap();
    journal.begin_turn("codex", "old", "turn").unwrap();
    journal
        .record_tool(
            "codex",
            "old",
            "turn",
            &usage("one", ToolOutcome::Completed),
        )
        .unwrap();
    journal
        .append_response("codex", "old", "turn", Some("reply"), "Respuesta")
        .unwrap();
    let expected = page(&db);
    db.close().unwrap();
    let db = Database::open(&path, 50).unwrap();
    setup(&db);
    assert_eq!(page(&db), expected);
    history::deliver_message(&db, conversation, "event:old", "Avance").unwrap();
    assert_eq!(page(&db), expected);
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
    db.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn call_budget_is_explicit_and_metadata_counts_toward_page_character_budget() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "active");
    journal
        .append("codex", "active", "user", "Consulta")
        .unwrap();
    journal.begin_turn("codex", "active", "turn").unwrap();
    for i in 0..MAX_CALLS_PER_TURN {
        journal
            .record_tool(
                "codex",
                "active",
                "turn",
                &usage(&format!("call-{i}"), ToolOutcome::Completed),
            )
            .unwrap();
    }
    assert!(
        journal
            .record_tool(
                "codex",
                "active",
                "turn",
                &usage("overflow", ToolOutcome::Running)
            )
            .is_err()
    );
    journal
        .append_response("codex", "active", "turn", Some("reply"), "Respuesta")
        .unwrap();
    let latest = assistants::page(&db, "gennie", None, 60, 100).unwrap();
    assert_eq!(latest.messages.len(), 1);
    assert_eq!(latest.messages[0].tools.len(), MAX_CALLS_PER_TURN);
    let older = assistants::page(&db, "gennie", latest.before.as_ref(), 60, 100).unwrap();
    assert_eq!(older.messages[0].speaker, "user");
    assert!(older.messages[0].tools.is_empty());
    assert!(older.before.is_none());
}

#[test]
fn uncertain_mcp_result_has_a_fixed_code_and_free_error_text_remains_rejected() {
    let db = Database::memory().unwrap();
    let journal = setup(&db);
    activate(&journal, "review");
    journal
        .append("codex", "review", "user", "Consulta")
        .unwrap();
    journal.begin_turn("codex", "review", "turn").unwrap();
    let mut pending = usage("pending", ToolOutcome::Unknown);
    pending.server = "mcp:fixture".into();
    pending.detail = Some("result_uncertain".into());
    journal
        .record_tool("codex", "review", "turn", &pending)
        .unwrap();
    assert_eq!(
        page(&db)[0].tools[0].usage.detail.as_deref(),
        Some("result_uncertain")
    );
    pending.call_id = "invalid".into();
    pending.detail = Some("PRIVATE_ERROR".into());
    assert!(
        journal
            .record_tool("codex", "review", "turn", &pending)
            .is_err()
    );
    assert_eq!(page(&db)[0].tools.len(), 1);
    db.close().unwrap();
}
