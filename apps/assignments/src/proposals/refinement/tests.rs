use super::*;
use crate::proposals::tests::{begin, candidate, document, fixture, no_guide};
fn turn(id: i64, text: &str) -> BotTurn {
    BotTurn {
        origin: Origin {
            conversation_id: 7,
            epoch: 1,
        },
        message_id: id,
        text: text.into(),
        truncated: false,
        created_at: "2026-10-29T13:00:00Z".into(),
    }
}
#[test]
fn a_real_answer_versions_questions_and_only_human_application_changes_the_plan() {
    let store = fixture();
    let record = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![record.id], false);
    let root_id = root["id"].as_str().unwrap();
    let mut question = candidate(record.id);
    question.document = None;
    question.questions = vec!["¿A qué hora prefieres recibirlo?".into()];
    let first = store
        .submit_proposals(
            Some(&turn(1, "Propón mejoras")),
            root_id,
            &turn(1, "").origin,
            vec![question],
            &no_guide(),
        )
        .unwrap();
    assert!(
        store
            .refine_review(
                root_id,
                &turn(1, "").origin,
                &turn(1, "Respuesta inventada en el mismo turno")
            )
            .is_err()
    );
    let response = turn(2, "A las nueve y agrupando novedades");
    let next = store
        .refine_review(root_id, &response.origin, &response)
        .unwrap();
    let id = next["id"].as_str().unwrap();
    assert_eq!(next["version"]["round"], 2);
    assert_eq!(next["scope"]["use_guide"], false);
    assert_eq!(next["version"]["response"]["text"], response.text);
    assert_eq!(
        store
            .refine_review(root_id, &response.origin, &response)
            .unwrap()["id"],
        id
    );
    let mut contradictory = response.clone();
    contradictory.text = "Otra respuesta con el mismo ID".into();
    assert!(
        store
            .refine_review(root_id, &contradictory.origin, &contradictory)
            .is_err()
    );
    assert_eq!(
        store.review_for_bot(root_id, &response.origin).unwrap()["current"],
        false
    );
    assert!(
        store
            .resolve_proposal(first["proposals"][0]["id"].as_i64().unwrap(), false)
            .is_err()
    );
    let saved = store
        .submit_proposals(
            Some(&response),
            id,
            &response.origin,
            vec![candidate(record.id)],
            &no_guide(),
        )
        .unwrap();
    assert_eq!(store.get(record.id).unwrap().revision, 1);
    let pid = saved["proposals"][0]["id"].as_i64().unwrap();
    store.resolve_proposal(pid, true).unwrap();
    store.resolve_proposal(pid, true).unwrap();
    assert_eq!(store.get(record.id).unwrap().revision, 2);
    assert!(
        store
            .create_guarded("new-plan", document("Otro"), Some(&response.origin))
            .is_ok()
    );
}
#[test]
fn settled_changes_remain_read_only_and_remote_consent_is_never_inherited() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let b = store.create("b", document("B")).unwrap();
    let review = begin(&store, vec![a.id, b.id], true);
    let id = review["id"].as_str().unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let guide: GuideEvaluator = std::sync::Arc::new(move |_, _| {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(crate::proposals::tests::result(2))
    });
    let first = store
        .submit_proposals(
            Some(&turn(10, "Pedir propuestas")),
            id,
            &turn(10, "").origin,
            vec![candidate(a.id), candidate(b.id)],
            &guide,
        )
        .unwrap();
    store
        .resolve_proposal(first["proposals"][1]["id"].as_i64().unwrap(), true)
        .unwrap();
    let answer = turn(11, "Solo ajusta el primero");
    let child = store.refine_review(id, &answer.origin, &answer).unwrap();
    let child_id = child["id"].as_str().unwrap();
    assert_eq!(child["scope"]["editable_ids"], json!([a.id]));
    assert_eq!(child["scope"]["assignments"][1]["revision"], 2);
    assert_eq!(child["scope"]["use_guide"], false);
    assert!(
        store
            .submit_proposals(
                Some(&answer),
                child_id,
                &answer.origin,
                vec![candidate(b.id)],
                &no_guide()
            )
            .is_err()
    );
    store
        .submit_proposals(
            Some(&answer),
            child_id,
            &answer.origin,
            vec![candidate(a.id)],
            &no_guide(),
        )
        .unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        store
            .refine_review(child_id, &answer.origin, &answer)
            .is_err()
    );
    let next = turn(12, "Otra aclaración del primero");
    assert_eq!(
        store.refine_review(child_id, &next.origin, &next).unwrap()["version"]["round"],
        3
    );
}
#[test]
fn changed_or_legacy_contexts_and_other_origins_cannot_create_a_version() {
    for legacy in [false, true] {
        let store = fixture();
        let row = store.create("a", document("A")).unwrap();
        let root = begin(&store, vec![row.id], false);
        let id = root["id"].as_str().unwrap();
        let initial = turn(1, "Revisar");
        store
            .submit_proposals(
                if legacy { None } else { Some(&initial) },
                id,
                &initial.origin,
                vec![candidate(row.id)],
                &no_guide(),
            )
            .unwrap();
        let mut other = turn(2, "Respuesta");
        other.origin.epoch = 2;
        assert!(store.refine_review(id, &other.origin, &other).is_err());
        if !legacy {
            store
                .update(row.id, row.revision, document("Edición ajena"))
                .unwrap();
        }
        let answer = turn(3, "Ajustar");
        assert!(store.refine_review(id, &answer.origin, &answer).is_err());
        store
            .db
            .with_connection(|c| {
                assert_eq!(
                    c.query_row(
                        "SELECT count(*) FROM assignment_review_refinements",
                        [],
                        |r| r.get::<_, i64>(0)
                    )?,
                    0
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn input_and_version_failures_roll_back_without_partial_batches_or_new_calls() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![row.id], false);
    let id = root["id"].as_str().unwrap();
    let first = turn(1, "Propuestas");
    store.db.with_connection(|c|{c.execute_batch("CREATE TRIGGER fail_input BEFORE INSERT ON assignment_review_inputs BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(
        store
            .submit_proposals(
                Some(&first),
                id,
                &first.origin,
                vec![candidate(row.id)],
                &no_guide()
            )
            .is_err()
    );
    let unchanged = store.review(id).unwrap();
    assert_eq!(unchanged["state"], "waiting_bot");
    assert!(unchanged["proposals"].as_array().unwrap().is_empty());
    store
        .db
        .with_connection(|c| {
            c.execute_batch("DROP TRIGGER fail_input")?;
            Ok(())
        })
        .unwrap();
    store
        .submit_proposals(
            Some(&first),
            id,
            &first.origin,
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    store.db.with_connection(|c|{c.execute_batch("CREATE TRIGGER fail_version BEFORE INSERT ON assignment_review_refinements BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let second = turn(2, "Cambiar");
    assert!(store.refine_review(id, &second.origin, &second).is_err());
    assert_eq!(store.review(id).unwrap()["current"], true);
    store
        .db
        .with_connection(|c| {
            assert_eq!(
                c.query_row("SELECT count(*) FROM assignment_reviews", [], |r| r
                    .get::<_, i64>(0))?,
                1
            );
            c.execute_batch("DROP TRIGGER fail_version")?;
            Ok(())
        })
        .unwrap();
    assert!(store.refine_review(id, &second.origin, &second).is_ok());
}

#[test]
fn concurrent_human_application_and_refinement_cannot_both_replace_a_pending_batch() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![row.id], false);
    let id = root["id"].as_str().unwrap().to_owned();
    let first = turn(1, "Propuestas");
    let saved = store
        .submit_proposals(
            Some(&first),
            &id,
            &first.origin,
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    let pid = saved["proposals"][0]["id"].as_i64().unwrap();
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = store.clone();
    let ready = gate.clone();
    let handle = std::thread::spawn(move || {
        ready.wait();
        other.resolve_proposal(pid, true)
    });
    gate.wait();
    let response = turn(2, "Ajusta antes de aplicar");
    let refined = store.refine_review(&id, &response.origin, &response);
    let applied = handle.join().unwrap();
    assert_ne!(refined.is_ok(), applied.is_ok());
    assert_eq!(
        store.get(row.id).unwrap().revision,
        if applied.is_ok() { 2 } else { 1 }
    );
}

#[test]
fn multiple_pending_reviews_require_selection_without_returning_an_arbitrary_scope() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![row.id], false);
    let origin = turn(1, "").origin;
    store
        .begin_review(Begin {
            request_key: "second".into(),
            assignments: vec![Reference {
                id: row.id,
                instance_id: row.instance_id,
                revision: row.revision,
            }],
            origin: origin.clone(),
            instructions: "Otra orientación".into(),
            use_guide: false,
        })
        .unwrap();
    let result = store.pending_review_for_bot(&origin).unwrap();
    assert_eq!(result["needs_clarification"], true);
    assert_eq!(result["active_reviews"].as_array().unwrap().len(), 2);
    assert!(result.get("scope").is_none());
    assert_eq!(
        store
            .review_for_bot(root["id"].as_str().unwrap(), &origin)
            .unwrap()["id"],
        root["id"]
    );
}

#[test]
fn reused_assignment_ids_do_not_inherit_a_review_or_its_input() {
    let store = fixture();
    let row = store.create("old", document("A")).unwrap();
    let root = begin(&store, vec![row.id], false);
    let id = root["id"].as_str().unwrap();
    let input = turn(1, "Propuestas");
    store
        .submit_proposals(
            Some(&input),
            id,
            &input.origin,
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    store.delete(row.id, row.revision).unwrap();
    let new = store.create("new", document("A")).unwrap();
    assert_eq!(new.id, row.id);
    assert_eq!(new.revision, row.revision);
    assert_ne!(new.instance_id, row.instance_id);
    let response = turn(2, "Ahora cambia");
    assert!(
        store
            .refine_review(id, &response.origin, &response)
            .is_err()
    );
}

#[test]
fn interrupted_guide_reopens_with_verifiable_input_and_refines_without_repeating_it() {
    use gennie_core::AppModule;
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![row.id], true);
    let id = root["id"].as_str().unwrap();
    let input = turn(1, "Propón");
    let path = std::env::temp_dir().join(format!(
        "gennie-refinement-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let copied = path.clone();
    let db = store.db.clone();
    let guide: GuideEvaluator = std::sync::Arc::new(move |_, _| {
        db.with_connection(|c| {
            c.execute("VACUUM INTO ?1", [copied.to_str().unwrap()])?;
            Ok(())
        })?;
        Err("fixture lost response".into())
    });
    store
        .submit_proposals(
            Some(&input),
            id,
            &input.origin,
            vec![candidate(row.id)],
            &guide,
        )
        .unwrap();
    let reopened = crate::Store::new(gennie_db::Database::open(&path, 50).unwrap());
    reopened
        .db
        .migrate("events", crate::AssignmentsModule.migrations())
        .unwrap();
    reopened.recover_reviews().unwrap();
    assert_eq!(reopened.review(id).unwrap()["state"], "interrupted");
    assert_eq!(
        reopened.review(id).unwrap()["version"]["input_verified"],
        true
    );
    let answer = turn(2, "Ajusta el resumen");
    let next = reopened.refine_review(id, &answer.origin, &answer).unwrap();
    reopened
        .submit_proposals(
            Some(&answer),
            next["id"].as_str().unwrap(),
            &answer.origin,
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    assert_eq!(reopened.get(row.id).unwrap().revision, 1);
    reopened.db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}
