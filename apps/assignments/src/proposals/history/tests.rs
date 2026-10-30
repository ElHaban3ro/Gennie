use super::*;
use crate::proposals::tests::{begin, candidate, document, fixture, no_guide};

fn add(store: &Store, ids: &[i64], key: &str) -> Value {
    store
        .begin_review(Begin {
            request_key: key.into(),
            assignments: ids
                .iter()
                .map(|id| {
                    let r = store.get(*id).unwrap();
                    Reference {
                        id: *id,
                        instance_id: r.instance_id,
                        revision: r.revision,
                    }
                })
                .collect(),
            origin: Origin {
                conversation_id: 7,
                epoch: 1,
            },
            instructions: "Revisar sin efectos".into(),
            use_guide: false,
        })
        .unwrap()
}
fn turn(id: i64) -> BotTurn {
    BotTurn {
        origin: Origin {
            conversation_id: 7,
            epoch: 1,
        },
        message_id: id,
        text: format!("Respuesta {id}"),
        truncated: false,
        created_at: "2026-10-29T13:00:00Z".into(),
    }
}

#[test]
fn stable_pages_survive_ties_new_rows_and_archiving_the_cursor_row() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    for n in 0..24 {
        add(&store, &[row.id], &format!("review-{n}"));
    }
    store
        .db
        .with_connection(|c| {
            c.execute(
                "UPDATE assignment_reviews SET created_at='2026-10-29T13:00:00Z'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let first = store.reviews(row.id).unwrap();
    assert_eq!(first["total"], 24);
    assert_eq!(first["reviews"].as_array().unwrap().len(), 10);
    let mut seen = first["reviews"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    let mut cursor: Cursor = serde_json::from_value(first["next_cursor"].clone()).unwrap();
    store.archive_review(&cursor.review_id).unwrap();
    let inserted = add(&store, &[row.id], "newer");
    store
        .db
        .with_connection(|c| {
            c.execute(
                "UPDATE assignment_reviews SET created_at='2099-01-01T00:00:00Z' WHERE id=?1",
                [inserted["id"].as_str().unwrap()],
            )?;
            Ok(())
        })
        .unwrap();
    loop {
        let page = store.reviews_page(row.id, false, Some(&cursor)).unwrap();
        for r in page["reviews"].as_array().unwrap() {
            assert!(seen.insert(r["id"].as_str().unwrap().to_string()));
        }
        if page["next_cursor"].is_null() {
            break;
        }
        cursor = serde_json::from_value(page["next_cursor"].clone()).unwrap();
    }
    assert_eq!(seen.len(), 24);
    assert!(!seen.contains(inserted["id"].as_str().unwrap()));
    assert!(store.reviews_page(row.id, true, Some(&cursor)).is_err());
    assert_eq!(store.reviews_page(row.id, true, None).unwrap()["total"], 1);
}

#[test]
fn archive_preserves_family_and_receipts_but_removes_all_pending_authority() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let b = store.create("b", document("B")).unwrap();
    let root = begin(&store, vec![a.id, b.id], true);
    let id = root["id"].as_str().unwrap();
    let guide: GuideEvaluator = std::sync::Arc::new(|_, _| Ok(crate::proposals::tests::result(2)));
    let input = turn(1);
    let saved = store
        .submit_proposals(
            Some(&input),
            id,
            &input.origin,
            vec![candidate(a.id), candidate(b.id)],
            &guide,
        )
        .unwrap();
    let applied = saved["proposals"][1]["id"].as_i64().unwrap();
    store.resolve_proposal(applied, true).unwrap();
    let answer = turn(2);
    let child = store.refine_review(id, &answer.origin, &answer).unwrap();
    let child_id = child["id"].as_str().unwrap();
    let proposed = store
        .submit_proposals(
            Some(&answer),
            child_id,
            &answer.origin,
            vec![candidate(a.id)],
            &no_guide(),
        )
        .unwrap();
    assert!(
        store
            .create_guarded("denied", document("C"), Some(&answer.origin))
            .is_err()
    );
    let archive = store.archive_review(id).unwrap();
    assert_eq!(archive["archived"], true);
    assert_eq!(archive["current"], false);
    assert_eq!(
        store.archive_review(child_id).unwrap()["archived_at"],
        archive["archived_at"]
    );
    assert_eq!(store.get(a.id).unwrap().revision, 1);
    assert_eq!(store.get(b.id).unwrap().revision, 2);
    assert_eq!(
        store.review(child_id).unwrap()["proposals"][0]["resolution"],
        "pending"
    );
    assert!(store.pending_review_for_bot(&answer.origin).is_err());
    assert!(
        store
            .create_guarded("allowed", document("C"), Some(&answer.origin))
            .is_ok()
    );
    assert!(store.refine_review(id, &answer.origin, &answer).is_err());
    assert!(store.cancel_review(child_id).is_err());
    assert!(
        store
            .submit_proposals(
                Some(&answer),
                child_id,
                &answer.origin,
                vec![candidate(a.id)],
                &no_guide()
            )
            .is_err()
    );
    let pid = proposed["proposals"][0]["id"].as_i64().unwrap();
    assert!(store.resolve_proposal(pid, true).is_err());
    assert!(store.resolve_proposal(pid, false).is_err());
    assert_eq!(
        store.resolve_proposal(applied, true).unwrap()["applied_revision"],
        2
    );
    assert_eq!(store.reviews(a.id).unwrap()["total"], 0);
    assert_eq!(store.reviews_page(a.id, true, None).unwrap()["total"], 2);
    assert_eq!(store.reviews_page(b.id, true, None).unwrap()["total"], 2);
}

#[test]
fn archive_rejects_in_flight_guide_and_rolls_back_a_failed_receipt() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let root = begin(&store, vec![a.id], true);
    let id = root["id"].as_str().unwrap().to_string();
    let other = store.clone();
    let copied = id.clone();
    let guide: GuideEvaluator = std::sync::Arc::new(move |_, _| {
        assert!(other.archive_review(&copied).is_err());
        Ok(crate::proposals::tests::result(1))
    });
    store
        .submit_proposals(None, &id, &turn(1).origin, vec![candidate(a.id)], &guide)
        .unwrap();
    store.db.with_connection(|c| {c.execute_batch("CREATE TRIGGER fail_archive BEFORE INSERT ON assignment_review_archives BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(store.archive_review(&id).is_err());
    assert_eq!(store.review(&id).unwrap()["archived"], false);
    assert!(
        store
            .create_guarded("denied", document("C"), Some(&turn(1).origin))
            .is_err()
    );
    store
        .db
        .with_connection(|c| {
            c.execute_batch("DROP TRIGGER fail_archive")?;
            Ok(())
        })
        .unwrap();
    assert!(store.archive_review(&id).is_ok());
}

#[test]
fn cursor_is_scoped_to_instance_and_archived_waiting_reviews_cannot_resume() {
    let store = fixture();
    let a = store.create("old", document("A")).unwrap();
    for n in 0..11 {
        add(&store, &[a.id], &format!("r{n}"));
    }
    let first = store.reviews(a.id).unwrap();
    let cursor: Cursor = serde_json::from_value(first["next_cursor"].clone()).unwrap();
    let id = first["reviews"][0]["id"].as_str().unwrap();
    store.archive_review(id).unwrap();
    assert!(
        store
            .submit_proposals(
                None,
                id,
                &turn(1).origin,
                vec![candidate(a.id)],
                &no_guide()
            )
            .is_err()
    );
    store.delete(a.id, a.revision).unwrap();
    let b = store.create("new", document("A")).unwrap();
    assert_eq!(a.id, b.id);
    assert!(store.reviews_page(b.id, false, Some(&cursor)).is_err());
    assert_eq!(store.reviews(b.id).unwrap()["total"], 0);
    assert_eq!(store.reviews_page(b.id, true, None).unwrap()["total"], 0);
}

#[test]
fn byte_budget_paginates_large_valid_selections_without_omitting_a_review() {
    let store = fixture();
    let mut ids = vec![];
    for n in 0..20 {
        let mut d = document(&format!("A{n}"));
        d.description = "x".repeat(4000);
        ids.push(store.create(&format!("a{n}"), d).unwrap().id);
    }
    for n in 0..12 {
        add(&store, &ids, &format!("r{n}"));
    }
    let first = store.reviews(ids[0]).unwrap();
    let count = first["reviews"].as_array().unwrap().len();
    assert!(count > 0 && count < 10);
    assert!(serde_json::to_vec(&first["reviews"]).unwrap().len() < 512 * 1024);
    let mut page = first;
    let mut seen = BTreeSet::new();
    loop {
        for row in page["reviews"].as_array().unwrap() {
            assert!(seen.insert(row["id"].as_str().unwrap().to_string()));
        }
        if page["next_cursor"].is_null() {
            break;
        }
        let cursor: Cursor = serde_json::from_value(page["next_cursor"].clone()).unwrap();
        page = store.reviews_page(ids[0], false, Some(&cursor)).unwrap();
    }
    assert_eq!(seen.len(), 12);
}

#[test]
fn upgrading_eight_migrations_and_reopening_preserves_archived_source_and_both_checksums() {
    use gennie_core::AppModule;
    let db = gennie_db::Database::memory().unwrap();
    let migrations = crate::AssignmentsModule.migrations();
    db.migrate("events", &migrations[..8]).unwrap();
    let store = Store::new(db);
    let a = store.create("a", document("A")).unwrap();
    let id = "legacy-review";
    let scope = Scope {
        initial_instructions: None,
        editable_ids: None,
        origin: turn(1).origin,
        assignments: vec![Baseline::from(a.clone())],
        instructions: "Orientación histórica".into(),
        use_guide: false,
    };
    let checksums = store.db.with_connection(|c| {
        c.execute("INSERT INTO assignment_reviews(id,request_key,request,scope,state) VALUES(?1,'legacy','{}',?2,'waiting_bot')",params![id,serde_json::to_string(&scope)?])?;
        let mut q=c.prepare("SELECT version,checksum FROM _soc_migrations_events ORDER BY version")?;
        Ok(q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)
    }).unwrap();
    store.db.migrate("events", migrations).unwrap();
    store.archive_review(id).unwrap();
    let path = std::env::temp_dir().join(format!(
        "gennie-archive-{}-{}.sqlite",
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
    assert_eq!(reopened.review(id).unwrap()["archived"], true);
    assert_eq!(reopened.get(a.id).unwrap().document, a.document);
    reopened
        .db
        .with_connection(|c| {
            let count: i64 =
                c.query_row("SELECT count(*) FROM _soc_migrations_events", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(count, 10);
            let issues: i64 =
                c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(issues, 0);
            Ok(())
        })
        .unwrap();
    reopened.db.with_connection(|c| {let mut q=c.prepare("SELECT version,checksum FROM _soc_migrations_events WHERE version<=8 ORDER BY version")?;let after=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;assert_eq!(checksums,after);Ok(())}).unwrap();
    reopened.db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}
