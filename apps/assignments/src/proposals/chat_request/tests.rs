use super::*;
use crate::proposals::tests::{candidate, document, fixture, no_guide};
fn turn(id: i64) -> BotTurn {
    BotTurn {
        origin: Origin {
            conversation_id: 7,
            epoch: 1,
        },
        message_id: id,
        text: "Genera propuestas para A y B, evitando avisos repetidos".into(),
        truncated: false,
        created_at: "2026-10-29T13:00:00Z".into(),
    }
}
fn reference(row: &Record) -> Reference {
    Reference {
        id: row.id,
        instance_id: row.instance_id.clone(),
        revision: row.revision,
    }
}

#[test]
fn actual_request_is_atomic_idempotent_local_and_preserved_across_versions() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let b = store.create("b", document("B")).unwrap();
    let input = turn(10);
    let before = store.revision();
    let first = store
        .start_chat_review(vec![reference(&b), reference(&a)], &input)
        .unwrap();
    let id = first["id"].as_str().unwrap();
    assert_eq!(store.revision(), before + 1);
    assert_eq!(first["chat_request"]["text"], input.text);
    assert_eq!(first["scope"]["instructions"], input.text);
    assert_eq!(first["scope"]["use_guide"], false);
    assert_eq!(
        store
            .start_chat_review(vec![reference(&a), reference(&b)], &input)
            .unwrap()["id"],
        id
    );
    assert_eq!(store.revision(), before + 1);
    for field in ["text", "date", "truncated"] {
        let mut changed = input.clone();
        match field {
            "text" => changed.text.push('!'),
            "date" => changed.created_at = "2026-10-29T13:00:01Z".into(),
            _ => changed.truncated = true,
        }
        assert!(
            store
                .start_chat_review(vec![reference(&a), reference(&b)], &changed)
                .is_err()
        );
    }
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &input)
            .is_err()
    );
    assert!(
        store
            .submit_proposals(
                Some(&turn(9)),
                id,
                &input.origin,
                vec![candidate(a.id)],
                &no_guide()
            )
            .is_err()
    );
    assert!(
        store.review(id).unwrap()["proposals"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    store
        .submit_proposals(
            Some(&input),
            id,
            &input.origin,
            vec![candidate(a.id)],
            &no_guide(),
        )
        .unwrap();
    let answer = turn(11);
    let child = store.refine_review(id, &answer.origin, &answer).unwrap();
    assert_eq!(child["chat_request"], first["chat_request"]);
    assert_eq!(child["scope"]["use_guide"], false);
    assert_eq!(store.get(a.id).unwrap().revision, 1);
    assert_eq!(store.get(b.id).unwrap().revision, 1);
    store.archive_review(id).unwrap();
    assert_eq!(
        store
            .start_chat_review(vec![reference(&a), reference(&b)], &input)
            .unwrap()["archived"],
        true
    );
    let next = store
        .start_chat_review(vec![reference(&a)], &turn(12))
        .unwrap();
    assert_ne!(next["id"], id);
    assert_eq!(next["scope"]["use_guide"], false);
}

#[test]
fn invalid_selection_changed_revision_and_reused_ids_leave_no_source_receipt() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    for refs in [
        vec![],
        vec![reference(&a), reference(&a)],
        vec![Reference {
            revision: 0,
            ..reference(&a)
        }],
    ] {
        assert!(store.start_chat_review(refs, &turn(1)).is_err());
    }
    store.update(a.id, a.revision, document("Edited")).unwrap();
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &turn(1))
            .is_err()
    );
    store.delete(a.id, 2).unwrap();
    let new = store.create("new", document("A")).unwrap();
    assert_eq!(new.id, a.id);
    assert_ne!(new.instance_id, a.instance_id);
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &turn(1))
            .is_err()
    );
    let mut invalid = turn(0);
    assert!(
        store
            .start_chat_review(vec![reference(&new)], &invalid)
            .is_err()
    );
    invalid.message_id = 1;
    invalid.origin.epoch = 0;
    assert!(
        store
            .start_chat_review(vec![reference(&new)], &invalid)
            .is_err()
    );
    store
        .db
        .with_connection(|c| {
            for table in ["assignment_reviews", "assignment_chat_review_requests"] {
                let count: i64 =
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
                assert_eq!(count, 0);
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn receipt_failure_rolls_back_review_and_public_revision_before_any_proposal() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let revision = store.revision();
    store.db.with_connection(|c| {c.execute_batch("CREATE TRIGGER fail_chat_request BEFORE INSERT ON assignment_chat_review_requests BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &turn(1))
            .is_err()
    );
    assert_eq!(store.revision(), revision);
    store
        .db
        .with_connection(|c| {
            let count: i64 =
                c.query_row("SELECT count(*) FROM assignment_reviews", [], |r| r.get(0))?;
            assert_eq!(count, 0);
            c.execute_batch("DROP TRIGGER fail_chat_request")?;
            Ok(())
        })
        .unwrap();
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &turn(1))
            .is_ok()
    );
}

#[test]
fn competing_selections_share_a_user_turn_but_cannot_both_create_a_scope() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let b = store.create("b", document("B")).unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = store.clone();
    let ready = gate.clone();
    let a_ref = reference(&a);
    let handle = std::thread::spawn(move || {
        ready.wait();
        other.start_chat_review(vec![a_ref], &turn(1))
    });
    gate.wait();
    let second = store.start_chat_review(vec![reference(&b)], &turn(1));
    assert_ne!(second.is_ok(), handle.join().unwrap().is_ok());
    let before = store.revision();
    let refs = vec![reference(&a), reference(&b)];
    let copy = refs.clone();
    let other = store.clone();
    let handle = std::thread::spawn(move || other.start_chat_review(copy, &turn(2)));
    let first = store.start_chat_review(refs, &turn(2)).unwrap();
    assert_eq!(first["id"], handle.join().unwrap().unwrap()["id"]);
    assert_eq!(store.revision(), before + 1);
}

#[test]
fn historical_human_key_is_not_relabelled_as_a_chat_request_or_remote_consent() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let input = turn(1);
    let request = Begin {
        request_key: "chat:7:1:1".into(),
        assignments: vec![reference(&a)],
        origin: input.origin.clone(),
        instructions: input.text.clone(),
        use_guide: false,
    };
    let legacy = store.begin_review(request.clone()).unwrap();
    assert!(
        store
            .start_chat_review(vec![reference(&a)], &input)
            .is_err()
    );
    assert!(store.review(legacy["id"].as_str().unwrap()).unwrap()["chat_request"].is_null());
    let mut remote = request;
    remote.request_key = "remote".into();
    remote.use_guide = true;
    assert!(store.begin_review_captured(remote, Some(&input)).is_err());
}

#[test]
fn migration_from_nine_and_reopen_preserve_legacy_rows_and_request_receipts() {
    use gennie_core::AppModule;
    let db = gennie_db::Database::memory().unwrap();
    let migrations = crate::AssignmentsModule.migrations();
    db.migrate("events", &migrations[..9]).unwrap();
    let store = Store::new(db);
    let a = store.create("a", document("A")).unwrap();
    let scope = Scope {
        initial_instructions: None,
        editable_ids: None,
        origin: turn(1).origin,
        assignments: vec![Baseline::from(a.clone())],
        instructions: "Legacy request".into(),
        use_guide: false,
    };
    let old=store.db.with_connection(|c| {c.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,state) VALUES('legacy','legacy','{}',?1,'waiting_bot')",[serde_json::to_string(&scope)?])?;let mut q=c.prepare("SELECT version,checksum FROM _soc_migrations_events ORDER BY version")?;Ok(q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)}).unwrap();
    store.db.migrate("events", migrations).unwrap();
    assert!(store.review("legacy").unwrap()["chat_request"].is_null());
    let created = store
        .start_chat_review(vec![reference(&a)], &turn(2))
        .unwrap();
    let path = std::env::temp_dir().join(format!(
        "gennie-chat-review-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    store
        .db
        .with_connection(|c| {
            c.execute("VACUUM INTO ?1", [path.to_str().unwrap()])?;
            Ok(())
        })
        .unwrap();
    let reopened = Store::new(gennie_db::Database::open(&path, 50).unwrap());
    reopened.db.migrate("events", migrations).unwrap();
    assert_eq!(
        reopened
            .start_chat_review(vec![reference(&a)], &turn(2))
            .unwrap()["chat_request"],
        created["chat_request"]
    );
    reopened.db.with_connection(|c| {let mut q=c.prepare("SELECT version,checksum FROM _soc_migrations_events WHERE version<=9 ORDER BY version")?;let after=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;assert_eq!(old,after);let issues:i64=c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get(0))?;assert_eq!(issues,0);Ok(())}).unwrap();
    reopened.db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}
