use super::*;
use gennie_core::AppModule;
use gennie_llm::{ConversationJournal, delegation, notices};
use std::sync::{Arc, atomic::AtomicU64};

fn setup() -> crate::history::Journal {
    let db = Database::memory().unwrap();
    db.migrate("presence", crate::PresenceModule::default().migrations())
        .unwrap();
    crate::history::Journal {
        db,
        revision: Arc::new(AtomicU64::new(0)),
    }
}
fn activate(j: &crate::history::Journal, thread: &str) -> i64 {
    j.db.with_connection(|c| {
        let tx = c.transaction()?;
        j.activate(&tx, "codex", thread, None)?;
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    crate::history::conversation_for_thread(&j.db, thread).unwrap()
}
fn notice(j: &crate::history::Journal, conversation: i64, run: i64, text: &str) {
    let origin = crate::delegation::origin(&j.db, conversation)
        .unwrap()
        .unwrap();
    let update = notices::Update {
        event_key: format!("assignment-execution:{run}"),
        origin,
        reference: notices::Reference {
            assignment_id: 1,
            instance_id: "a".repeat(32),
            policy_id: "policy".into(),
            policy_revision: 1,
            run_id: run,
            state: "succeeded".into(),
        },
        title: "Lectura autorizada".into(),
        scheduled_at: "2026-10-29T13:00:00Z".into(),
        state: "succeeded".into(),
        code: "reported_success".into(),
        observation: None,
        observation_truncated: false,
        guidance: None,
    };
    j.db.with_connection(|c| {
        let tx = c.transaction()?;
        assert!(j.deliver_execution_notice(&tx, &update, text)?);
        tx.commit()?;
        Ok(())
    })
    .unwrap();
}
fn scalar(db: &Database, sql: &str) -> i64 {
    db.with_connection(|c| Ok(c.query_row(sql, [], |r| r.get(0))?))
        .unwrap()
}

#[test]
fn only_attributed_notices_from_the_active_session_are_injected_once() {
    let j = setup();
    let old = activate(&j, "old");
    notice(&j, old, 1, "PRIVATE_OLD_NOTICE");
    let current = activate(&j, "current");
    j.append("codex", "current", "assistant", "ALREADY_IN_PROVIDER")
        .unwrap();
    j.append("codex", "current", "system", "NOT_A_BOT_NOTICE")
        .unwrap();
    notice(&j, current, 2, "El resultado local del bot");
    let origin = crate::delegation::origin(&j.db, current).unwrap().unwrap();
    let update = delegation::Update {
        event_key: "plugin-build:42:1".into(),
        delegation_id: "job:42".into(),
        request_id: "request:42".into(),
        source_id: "plugin_builder".into(),
        job_id: 42,
        sequence: 1,
        plugin_id: "fixture".into(),
        display_name: "Prueba".into(),
        objective: "Comprobar".into(),
        detail: "Detalle interno".into(),
        state: delegation::State::Running,
        kind: delegation::Kind::Progress,
        origin,
    };
    j.db.with_connection(|c| {
        let tx = c.transaction()?;
        assert!(j.deliver_delegation(&tx, &update, "Plugin comprobado")?);
        tx.commit()?;
        Ok(())
    })
    .unwrap();
    let prepared = j
        .prepare_user("codex", "current", "  Texto literal\n", None)
        .unwrap();
    let ctx = prepared.context.unwrap();
    assert_eq!(ctx.updates.len(), 2);
    assert_eq!(ctx.updates[0].text, "El resultado local del bot");
    assert!(matches!(
        ctx.updates[1].reference,
        Reference::PluginBuild { job_id: 42, .. }
    ));
    assert!(!serde_json::to_string(&ctx).unwrap().contains("PRIVATE_OLD"));
    let batch = prepared.batch_id.unwrap();
    j.confirm_context("codex", "current", batch).unwrap();
    j.confirm_context("codex", "current", batch).unwrap();
    assert!(
        j.prepare_user("codex", "current", "Siguiente", None)
            .unwrap()
            .context
            .is_none()
    );
    let messages = crate::history::load(&j.db, Some(current))
        .unwrap()
        .selected_messages;
    assert!(messages.iter().any(|m| m.text == "  Texto literal\n"));
    assert_eq!(
        scalar(
            &j.db,
            "SELECT count(*) FROM presence_provider_inputs WHERE state='accepted'"
        ),
        1
    );
    activate(&j, "old");
    assert_eq!(
        j.prepare_user("codex", "old", "Ahora sí", None)
            .unwrap()
            .context
            .unwrap()
            .updates[0]
            .text,
        "PRIVATE_OLD_NOTICE"
    );
}

#[test]
fn receipt_failure_recovery_sends_references_without_repeating_bodies() {
    let j = setup();
    let id = activate(&j, "current");
    notice(&j, id, 1, "BODY_MUST_NOT_REPLAY");
    let first = j.prepare_user("codex", "current", "Uno", None).unwrap();
    let batch = first.batch_id.unwrap();
    let next = j.prepare_user("codex", "current", "Dos", None).unwrap();
    let ctx = next.context.unwrap();
    assert!(j.confirm_context("codex", "current", batch).is_err());
    assert!(ctx.updates.is_empty());
    assert_eq!(ctx.unconfirmed.len(), 1);
    assert!(
        !serde_json::to_string(&ctx)
            .unwrap()
            .contains("BODY_MUST_NOT_REPLAY")
    );
    uncertain(&j.db, "codex", "current", next.batch_id.unwrap()).unwrap();
    let retry = j.prepare_user("codex", "current", "Tres", None).unwrap();
    assert_eq!(retry.context.unwrap().unconfirmed.len(), 1);
    confirm(&j.db, "codex", "current", retry.batch_id.unwrap()).unwrap();
    assert!(
        j.prepare_user("codex", "current", "Cuatro", None)
            .unwrap()
            .context
            .is_none()
    );
    assert_eq!(
        scalar(
            &j.db,
            "SELECT count(*) FROM presence_provider_inputs WHERE state='uncertain'"
        ),
        2
    );
}

#[test]
fn quote_reservation_and_literal_message_commit_atomically() {
    let j = setup();
    let old = activate(&j, "old");
    j.append("codex", "old", "assistant", "Cita explícita")
        .unwrap();
    let source = scalar(&j.db, "SELECT max(id) FROM presence_history");
    let id = activate(&j, "current");
    notice(&j, id, 1, "Aviso pendiente");
    j.db.with_connection(|c| {c.execute_batch("CREATE TRIGGER fail_context BEFORE INSERT ON presence_provider_inputs BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let before = scalar(&j.db, "SELECT count(*) FROM presence_history");
    assert!(
        j.prepare_user("codex", "current", "No guardar", Some(source))
            .is_err()
    );
    assert_eq!(
        scalar(&j.db, "SELECT count(*) FROM presence_history"),
        before
    );
    assert_eq!(
        scalar(&j.db, "SELECT count(*) FROM presence_message_replies"),
        0
    );
    assert_eq!(
        scalar(
            &j.db,
            "SELECT count(*) FROM presence_provider_input_messages WHERE user_message_id IS NULL"
        ),
        1
    );
    j.db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER fail_context")?;
        Ok(())
    })
    .unwrap();
    let p = j
        .prepare_user("codex", "current", "Guardar con cita", Some(source))
        .unwrap();
    assert_eq!(p.quote.unwrap().text, "Cita explícita");
    assert_eq!(p.context.unwrap().updates.len(), 1);
    assert_eq!(
        crate::history::active_conversation(&j.db).unwrap(),
        Some(id)
    );
    assert_ne!(old, id);
}

#[test]
fn budgets_keep_newest_notices_and_never_replay_omitted_backlog() {
    for long in [false, true] {
        let j = setup();
        let id = activate(&j, "current");
        for run in 1..=20 {
            notice(
                &j,
                id,
                run,
                &if long {
                    "🦀".repeat(1500)
                } else {
                    format!("Aviso {run}")
                },
            );
        }
        let p = j.prepare_user("codex", "current", "Resumen", None).unwrap();
        let ctx = p.context.unwrap();
        assert!(ctx.updates.iter().map(Update::characters).sum::<usize>() <= MAX_CHARACTERS);
        assert_eq!(ctx.updates.len(), if long { 5 } else { 12 });
        assert_eq!(ctx.older_omitted, 20 - ctx.updates.len() as u64);
        assert!(
            matches!(ctx.updates.last().unwrap().reference,Reference::AssignmentExecution{ref reference} if reference.run_id==20)
        );
        confirm(&j.db, "codex", "current", p.batch_id.unwrap()).unwrap();
        assert!(
            j.prepare_user("codex", "current", "Continuar", None)
                .unwrap()
                .context
                .is_none()
        );
        assert_eq!(
            scalar(
                &j.db,
                "SELECT count(*) FROM presence_provider_input_messages WHERE included=0"
            ),
            if long { 15 } else { 8 }
        );
    }
}

#[test]
fn late_arrivals_scope_changes_and_deletion_do_not_acknowledge_the_wrong_messages() {
    let j = setup();
    let id = activate(&j, "current");
    notice(&j, id, 1, "Primero");
    let first = j.prepare_user("codex", "current", "Uno", None).unwrap();
    notice(&j, id, 2, "Llegó después de reservar");
    confirm(&j.db, "codex", "current", first.batch_id.unwrap()).unwrap();
    let second = j.prepare_user("codex", "current", "Dos", None).unwrap();
    assert_eq!(
        second.context.unwrap().updates[0].text,
        "Llegó después de reservar"
    );
    activate(&j, "other");
    assert!(confirm(&j.db, "codex", "current", second.batch_id.unwrap()).is_err());
    activate(&j, "current");
    crate::history::delete(&j.db, id).unwrap();
    assert!(confirm(&j.db, "codex", "current", second.batch_id.unwrap()).is_err());
    assert_eq!(
        scalar(&j.db, "SELECT count(*) FROM presence_provider_inputs"),
        0
    );
    assert_eq!(
        scalar(
            &j.db,
            "SELECT count(*) FROM presence_provider_input_messages"
        ),
        0
    );
    assert!(
        j.prepare_user("codex", "current", "Nueva época", None)
            .unwrap()
            .context
            .is_none()
    );
}

#[test]
fn migration_backfills_only_attributed_messages_and_file_recovery_preserves_receipts() {
    let module = crate::PresenceModule::default();
    let db = Database::memory().unwrap();
    db.migrate("presence", &module.migrations()[..17]).unwrap();
    let j = crate::history::Journal {
        db: db.clone(),
        revision: Arc::new(AtomicU64::new(0)),
    };
    let conversation = activate(&j, "current");
    j.append(
        "codex",
        "current",
        "assistant",
        "Mensaje ya conocido por el proveedor",
    )
    .unwrap();
    j.append("codex", "current", "assistant", "Aviso histórico de plugin")
        .unwrap();
    db.with_connection(|c|{c.execute("INSERT INTO presence_delegation_messages(message_id,event_key,delegation_id,source_id,job_id,source_sequence,plugin_id) VALUES(?1,'legacy:1','job:1','plugin_builder',1,1,'fixture')",[c.last_insert_rowid()])?;Ok(())}).unwrap();
    db.migrate("presence", module.migrations()).unwrap();
    assert_eq!(
        scalar(&db, "SELECT count(*) FROM presence_provider_input_messages"),
        1
    );
    let first = j
        .prepare_user("codex", "current", "Mensaje con contexto", None)
        .unwrap();
    assert_eq!(
        first.context.unwrap().updates[0].text,
        "Aviso histórico de plugin"
    );
    let batch = first.batch_id.unwrap();
    db.with_connection(|c|{assert!(c.execute("UPDATE presence_provider_input_messages SET included=NULL WHERE user_message_id=?1",[batch]).is_err());Ok(())}).unwrap();
    let path = std::env::temp_dir().join(format!(
        "gennie-provider-context-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    db.with_connection(|c| {
        c.execute("VACUUM INTO ?1", [path.to_str().unwrap()])?;
        Ok(())
    })
    .unwrap();
    let reopened = Database::open(&path, 50).unwrap();
    reopened.migrate("presence", module.migrations()).unwrap();
    recover(&reopened).unwrap();
    let p = prepare(
        &reopened,
        "codex",
        "current",
        "Continuar tras reiniciar",
        None,
    )
    .unwrap();
    assert!(p.context.as_ref().unwrap().updates.is_empty());
    assert_eq!(p.context.unwrap().unconfirmed.len(), 1);
    confirm(&reopened, "codex", "current", p.batch_id.unwrap()).unwrap();
    assert!(
        prepare(&reopened, "codex", "current", "Seguir", None)
            .unwrap()
            .context
            .is_none()
    );
    assert_eq!(
        scalar(&reopened, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(
        crate::history::active_conversation(&reopened).unwrap(),
        Some(conversation)
    );
    reopened.close().unwrap();
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn receipt_commit_failure_and_mixed_budgets_do_not_replay_or_acknowledge_bodies() {
    let j = setup();
    let id = activate(&j, "current");
    for run in 1..=12 {
        notice(&j, id, run, "Primer lote");
    }
    let first = j.prepare_user("codex", "current", "Uno", None).unwrap();
    j.db.with_connection(|c|{c.execute_batch("CREATE TRIGGER fail_context_ack BEFORE UPDATE OF state ON presence_provider_inputs WHEN NEW.state='accepted' BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(confirm(&j.db, "codex", "current", first.batch_id.unwrap()).is_err());
    assert_eq!(
        scalar(
            &j.db,
            "SELECT count(*) FROM presence_provider_inputs WHERE state='accepted'"
        ),
        0
    );
    j.db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER fail_context_ack")?;
        Ok(())
    })
    .unwrap();
    for run in 13..=17 {
        notice(&j, id, run, &"🦀".repeat(1500));
    }
    let second = j.prepare_user("codex", "current", "Dos", None).unwrap();
    let ctx = second.context.unwrap();
    assert_eq!(ctx.updates.len(), 5);
    assert!(ctx.unconfirmed.len() < 12);
    assert!(ctx.unconfirmed_omitted > 0);
    assert_eq!(ctx.unconfirmed.len() as u64 + ctx.unconfirmed_omitted, 12);
    assert!(
        ctx.updates.iter().map(Update::characters).sum::<usize>()
            + ctx
                .unconfirmed
                .iter()
                .map(Reference::characters)
                .sum::<usize>()
            <= MAX_CHARACTERS
    );
    confirm(&j.db, "codex", "current", second.batch_id.unwrap()).unwrap();
    assert!(
        j.prepare_user("codex", "current", "Tres", None)
            .unwrap()
            .context
            .is_none()
    );
}

#[test]
fn eligibility_write_failure_rolls_back_the_notice_and_its_receipt() {
    let j = setup();
    let id = activate(&j, "current");
    j.db.with_connection(|c|{c.execute_batch("CREATE TRIGGER fail_context_queue BEFORE INSERT ON presence_provider_input_messages BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let origin = crate::delegation::origin(&j.db, id).unwrap().unwrap();
    let update = delegation::Update {
        event_key: "plugin-build:9".into(),
        delegation_id: "job:9".into(),
        request_id: "request:9".into(),
        source_id: "plugin_builder".into(),
        job_id: 9,
        sequence: 1,
        plugin_id: "fixture".into(),
        display_name: "Prueba".into(),
        objective: "Comprobar".into(),
        detail: "Detalle".into(),
        state: delegation::State::Running,
        kind: delegation::Kind::Progress,
        origin,
    };
    assert!(
        j.db.with_connection(|c| {
            let tx = c.transaction()?;
            j.deliver_delegation(&tx, &update, "No guardar parcialmente")?;
            tx.commit()?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(scalar(&j.db, "SELECT count(*) FROM presence_history"), 0);
    assert_eq!(
        scalar(&j.db, "SELECT count(*) FROM presence_delegation_messages"),
        0
    );
}
