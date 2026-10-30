use super::*;
use crate::AssignmentsModule;
use gennie_core::AppModule;
use gennie_db::Database;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
pub(super) fn fixture() -> Store {
    let db = Database::memory().unwrap();
    db.migrate("events", AssignmentsModule.migrations())
        .unwrap();
    Store::new(db)
}
pub(super) fn document(title: &str) -> Document {
    serde_json::from_value(json!({"title":title,"description":"Revisar cambios de esta fuente","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"}})).unwrap()
}
pub(super) fn begin(store: &Store, ids: Vec<i64>, remote: bool) -> Value {
    store
        .begin_review(Begin {
            request_key: "review-1".into(),
            assignments: ids
                .into_iter()
                .map(|id| {
                    let row = store.get(id).unwrap();
                    Reference {
                        id,
                        instance_id: row.instance_id,
                        revision: row.revision,
                    }
                })
                .collect(),
            origin: Origin {
                conversation_id: 7,
                epoch: 1,
            },
            instructions: "Propón mejoras sin duplicar avisos".into(),
            use_guide: remote,
        })
        .unwrap()
}
pub(super) fn candidate(id: i64) -> Candidate {
    let mut d = document("Resumen mejorado");
    d.context = Some("Agrupar cambios relacionados".into());
    Candidate {
        assignment_id: id,
        title: "Agrupar el resumen".into(),
        rationale: "Evitar avisos repetidos".into(),
        expected_benefit: "Una revisión más útil".into(),
        related_ids: vec![],
        questions: vec![],
        document: Some(d),
    }
}
pub(super) fn no_guide() -> GuideEvaluator {
    Arc::new(|_, _| panic!("No debe consultar la guía"))
}

#[test]
fn omitted_optional_fields_are_canonical_in_public_proposal_readback() {
    let store = fixture();
    let original = store
        .create("optional-fields", document("Resumen"))
        .unwrap();
    let review = begin(&store, vec![original.id], false);
    let review_id = review["id"].as_str().unwrap();
    let mut wire = serde_json::to_value(candidate(original.id)).unwrap();
    let mut proposed = serde_json::to_value(&original.document).unwrap();
    proposed["description"] = json!("Agrupar solo novedades");
    for key in ["context", "reason", "notify_at"] {
        proposed.as_object_mut().unwrap().remove(key);
    }
    proposed["preparation"] = json!({"steps": []});
    wire["document"] = proposed;
    let proposal: Candidate = serde_json::from_value(wire).unwrap();
    store
        .submit_proposals(
            None,
            review_id,
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![proposal],
            &no_guide(),
        )
        .unwrap();
    let readback = store.review(review_id).unwrap();
    let baseline = &readback["scope"]["assignments"][0]["document"];
    let proposed = &readback["proposals"][0]["candidate"]["document"];
    for key in ["preparation", "context", "reason", "notify_at"] {
        assert_eq!(baseline[key], proposed[key], "{key}");
    }
    assert_eq!(
        proposed["preparation"],
        json!({"interval_minutes":null,"rationale":null,"steps":[]})
    );
    let mut expected = baseline.clone();
    expected["description"] = json!("Agrupar solo novedades");
    assert_eq!(proposed, &expected);
    assert_eq!(store.get(original.id).unwrap().revision, original.revision);
    assert_eq!(store.get(original.id).unwrap().document, original.document);
}

#[test]
fn rejected_batch_identifies_candidate_and_allows_explicit_repair_before_one_guide_call() {
    let store = fixture();
    let first = store
        .create("first", document("Resumen principal"))
        .unwrap();
    let second = store
        .create("second", document("Resumen duplicado"))
        .unwrap();
    let review = begin(&store, vec![first.id, second.id], true);
    let id = review["id"].as_str().unwrap();
    let origin = Origin {
        conversation_id: 7,
        epoch: 1,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let guide: GuideEvaluator = Arc::new(move |_, _| {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(result(2))
    });
    let mut batch = vec![candidate(first.id), candidate(second.id)];
    batch[1].document = None;
    let error = store
        .submit_proposals(None, id, &origin, batch.clone(), &guide)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("invalid_proposal_input:"), "{error}");
    assert!(error.contains("proposals[1].document"), "{error}");
    assert!(error.contains("questions"), "{error}");
    assert!(
        error.contains("No se guardó este lote ni se consultó la guía en este envío"),
        "{error}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let rejected = store.review(id).unwrap();
    assert_eq!(rejected["state"], "waiting_bot");
    assert!(rejected["proposals"].as_array().unwrap().is_empty());
    let wire_origin = origin.clone();
    let turn_origin = origin.clone();
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(tools(
            store.clone(),
            Arc::new(move || Ok(wire_origin.clone())),
            guide.clone(),
            Arc::new(move || {
                Ok(BotTurn {
                    origin: turn_origin.clone(),
                    message_id: 1,
                    text: "Prepara borradores".into(),
                    truncated: false,
                    created_at: "2026-10-29T12:00:00.000Z".into(),
                })
            }),
        ))
        .unwrap();
    let response = registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_propose","arguments":{"review_id":id,"proposals":batch}}})).unwrap();
    assert_eq!(response["result"]["isError"], true);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("invalid_proposal_input: proposals[1].document")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut excessive = batch.clone();
    excessive[0].questions = vec!["¿Qué fuente?".into(); 9];
    let error = store
        .submit_proposals(None, id, &origin, excessive, &guide)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("invalid_proposal_input:"));
    assert!(error.contains("proposals[0].questions"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // La reparación pertenece al consumidor: el backend no rellena el documento.
    let mut paused = second.document.clone();
    paused.status = crate::Status::Paused;
    batch[1].document = Some(paused);
    let saved = store
        .submit_proposals(None, id, &origin, batch.clone(), &guide)
        .unwrap();
    assert_eq!(saved["state"], "ready");
    assert_eq!(saved["proposals"].as_array().unwrap().len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    store
        .submit_proposals(None, id, &origin, batch, &guide)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.get(first.id).unwrap().document, first.document);
    assert_eq!(store.get(second.id).unwrap().document, second.document);
    assert_eq!(store.get(second.id).unwrap().revision, 1);
}

#[test]
fn published_candidate_union_exposes_complete_closed_objects_in_each_branch() {
    let tools = tools(
        fixture(),
        Arc::new(|| {
            Ok(Origin {
                conversation_id: 7,
                epoch: 1,
            })
        }),
        no_guide(),
        Arc::new(|| Err("No hay turno".into())),
    );
    let definition = &tools
        .iter()
        .find(|t| t.definition["name"] == "assignments_propose")
        .unwrap()
        .definition;
    let branches = definition["inputSchema"]["properties"]["proposals"]["items"]["anyOf"]
        .as_array()
        .unwrap();
    assert_eq!(branches.len(), 2);
    for branch in branches {
        assert_eq!(
            branch["type"], "object",
            "Cada alternativa debe ser autónoma"
        );
        assert_eq!(branch["additionalProperties"], false);
        let properties = branch["properties"].as_object().unwrap();
        let required = branch["required"].as_array().unwrap();
        assert_eq!(properties.len(), 7);
        assert_eq!(required.len(), properties.len());
        for field in [
            "assignment_id",
            "title",
            "rationale",
            "expected_benefit",
            "related_ids",
            "questions",
            "document",
        ] {
            assert!(properties.contains_key(field));
            assert!(required.contains(&json!(field)));
        }
    }
}

#[test]
fn published_candidate_union_keeps_metadata_and_document_question_rules() {
    let schema = jsonschema::options()
        .offline()
        .build(&super::schema::submission())
        .unwrap();
    let complete = serde_json::to_value(candidate(1)).unwrap();
    for (document_present, questions_present, valid) in [
        (true, false, true),
        (true, true, true),
        (false, true, true),
        (false, false, false),
    ] {
        let mut value = complete.clone();
        if !document_present {
            value["document"] = Value::Null;
        }
        if questions_present {
            value["questions"] = json!(["¿Qué fuente debo consultar?"]);
        }
        let batch = |candidate| json!({"review_id":"fixture", "proposals":[candidate]});
        assert_eq!(schema.is_valid(&batch(value.clone())), valid);
        for field in complete.as_object().unwrap().keys() {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(!schema.is_valid(&batch(missing)), "No omitir {field}");
        }
        value["invented_permission"] = json!(true);
        assert!(!schema.is_valid(&batch(value)));
    }
    // Formas incompletas observadas en la campaña, sin inventar los metadatos.
    for value in [
        json!({"document": complete["document"]}),
        json!({"questions":["¿Qué fuente?"]}),
    ] {
        assert!(!schema.is_valid(&json!({"review_id":"fixture", "proposals":[value]})));
    }
}

#[test]
fn published_schema_rejects_pilot_batches_before_the_store_and_accepts_explicit_repairs() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/proposal-rejections-v1.json"
    ))
    .unwrap();
    for rejected in fixtures["rejections"].as_array().unwrap() {
        let store = fixture();
        let records: Vec<_> = rejected["documents"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, value)| {
                store
                    .create(
                        &format!("source-{index}"),
                        serde_json::from_value(value.clone()).unwrap(),
                    )
                    .unwrap()
            })
            .collect();
        let review = begin(&store, records.iter().map(|r| r.id).collect(), false);
        let origin = Origin {
            conversation_id: 7,
            epoch: 1,
        };
        let tools = tools(
            store.clone(),
            Arc::new(move || Ok(origin.clone())),
            no_guide(),
            Arc::new(|| Err("No hay turno".into())),
        );
        let definition = &tools
            .iter()
            .find(|t| t.definition["name"] == "assignments_propose")
            .unwrap()
            .definition;
        let schema = jsonschema::options()
            .offline()
            .build(&definition["inputSchema"])
            .unwrap();
        let mut args = rejected["arguments"].clone();
        args["review_id"] = review["id"].clone();
        assert!(
            !schema.is_valid(&args),
            "El esquema publicado admite el lote rechazado: {}",
            rejected["case_id"]
        );
        let candidates: Vec<Candidate> = serde_json::from_value(args["proposals"].clone()).unwrap();
        assert!(
            store
                .submit_proposals(
                    None,
                    review["id"].as_str().unwrap(),
                    &Origin {
                        conversation_id: 7,
                        epoch: 1
                    },
                    candidates,
                    &no_guide()
                )
                .is_err()
        );
        assert!(
            store.review(review["id"].as_str().unwrap()).unwrap()["proposals"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        if rejected["case_id"] == "duplicate_scope" {
            // El arreglo es explícito: documento completo para pausar la copia, no descartar un elemento del lote.
            let mut document = rejected["documents"][1].clone();
            document["status"] = json!("paused");
            args["proposals"][1]["document"] = document;
        } else {
            // Una mejora del resumen no tiene que inventar un seguimiento o cadencia.
            args["proposals"][0]["document"]["follow_up"] = Value::Null;
        }
        assert!(schema.is_valid(&args));
        let candidates = serde_json::from_value(args["proposals"].clone()).unwrap();
        let saved = store
            .submit_proposals(
                None,
                review["id"].as_str().unwrap(),
                &Origin {
                    conversation_id: 7,
                    epoch: 1,
                },
                candidates,
                &no_guide(),
            )
            .unwrap();
        assert_eq!(
            saved["proposals"].as_array().unwrap().len(),
            args["proposals"].as_array().unwrap().len()
        );
        for original in records {
            assert_eq!(store.get(original.id).unwrap().document, original.document);
            assert_eq!(store.get(original.id).unwrap().revision, 1);
        }
    }
}

#[test]
fn published_cadence_conditions_agree_with_store_without_inventing_defaults() {
    let store = fixture();
    let schema = jsonschema::options()
        .offline()
        .build(&crate::tools::document_schema())
        .unwrap();
    let policy = json!({"scope":"Lista ficticia","decision_guidance":"Comparar cambios sin modificar la fuente"});
    for (follow_up, preparation, valid) in [
        (Value::Null, json!({}), true),
        (policy.clone(), json!({}), false),
        (
            policy.clone(),
            json!({"interval_minutes":null,"rationale":"Revisión acordada"}),
            false,
        ),
        (
            policy.clone(),
            json!({"interval_minutes":1440,"rationale":"Revisión diaria aceptada"}),
            true,
        ),
        (Value::Null, json!({"interval_minutes":1440}), false),
        (
            Value::Null,
            json!({"interval_minutes":null,"rationale":null}),
            true,
        ),
        (
            policy.clone(),
            json!({"interval_minutes":0,"rationale":"Revisión"}),
            false,
        ),
        (
            policy,
            json!({"interval_minutes":1440,"rationale":"  "}),
            false,
        ),
    ] {
        let mut value = serde_json::to_value(document("Fuente")).unwrap();
        value["follow_up"] = follow_up;
        value["preparation"] = preparation;
        assert_eq!(schema.is_valid(&value), valid, "Esquema: {value}");
        let mut decoded = serde_json::from_value(value).unwrap();
        assert_eq!(store.validate(&mut decoded).is_ok(), valid);
    }
    let mut incomplete = serde_json::to_value(candidate(1)).unwrap();
    incomplete["document"] = Value::Null;
    let schema = jsonschema::options()
        .offline()
        .build(&super::schema::submission())
        .unwrap();
    let mut batch = json!({"review_id":"fixture","proposals":[incomplete]});
    assert!(!schema.is_valid(&batch));
    batch["proposals"][0]["questions"] = json!(["¿Qué fuente debo consultar?"]);
    assert!(schema.is_valid(&batch));
    for question in ["", "   "] {
        batch["proposals"][0]["questions"] = json!([question]);
        assert!(!schema.is_valid(&batch));
    }
}
pub(super) fn result(count: usize) -> Value {
    let keys = questions(count);
    let answers = keys
        .as_object()
        .unwrap()
        .keys()
        .map(|key| (key.clone(), json!({"type":"noul","noul":0.75})))
        .collect::<serde_json::Map<_, _>>();
    json!({"answers":answers,"model":"fixture-guide","usage":{"input_tokens":100,"cost":0.0001},"private_extra":"NO_STORE"})
}
#[test]
fn scoped_drafts_are_not_edits_and_only_human_can_apply() {
    let store = fixture();
    let row = store.create("a", document("Antes")).unwrap();
    let review = begin(&store, vec![row.id], false);
    let id = review["id"].as_str().unwrap();
    let origin = Origin {
        conversation_id: 7,
        epoch: 1,
    };
    assert!(
        store
            .review_for_bot(
                id,
                &Origin {
                    epoch: 2,
                    ..origin.clone()
                }
            )
            .is_err()
    );
    assert!(
        store
            .review_for_bot(
                id,
                &Origin {
                    conversation_id: 8,
                    ..origin.clone()
                }
            )
            .is_err()
    );
    let saved = store
        .submit_proposals(None, id, &origin, vec![candidate(row.id)], &no_guide())
        .unwrap();
    assert_eq!(store.get(row.id).unwrap().document.title, "Antes");
    let proposal = saved["proposals"][0]["id"].as_i64().unwrap();
    let accepted = store.resolve_proposal(proposal, true).unwrap();
    assert_eq!(accepted["applied_revision"], 2);
    assert_eq!(
        store.get(row.id).unwrap().document.title,
        "Resumen mejorado"
    );
    assert_eq!(store.resolve_proposal(proposal, true).unwrap(), accepted);
    assert!(store.resolve_proposal(proposal, false).is_err());
}

#[test]
fn bot_mutation_is_blocked_transactionally_until_human_resolution() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![row.id], false);
    let id = review["id"].as_str().unwrap();
    let origin = Origin {
        conversation_id: 7,
        epoch: 1,
    };
    assert!(
        store
            .create_guarded("copy", document("Copia"), Some(&origin))
            .is_err()
    );
    assert!(
        store
            .update_guarded(row.id, 1, document("Cambio"), Some(&origin))
            .is_err()
    );
    assert_eq!(store.get(row.id).unwrap().revision, 1);
    store.cancel_review(id).unwrap();
    assert!(store.pending_review_for_bot(&origin).is_err());
    store
        .update_guarded(row.id, 1, document("Cambio aceptado aparte"), Some(&origin))
        .unwrap();
    assert_eq!(store.get(row.id).unwrap().revision, 2);
}

#[test]
fn mcp_exposes_read_and_drafts_but_no_human_approval() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![row.id], false);
    let mut registry = gennie_mcp::Registry::default();
    registry
        .register(crate::tools_with_origin(
            store.clone(),
            Arc::new(|| {
                Ok(Some(Origin {
                    conversation_id: 7,
                    epoch: 1,
                }))
            }),
        ))
        .unwrap();
    registry
        .register(tools(
            store.clone(),
            Arc::new(|| {
                Ok(Origin {
                    conversation_id: 7,
                    epoch: 1,
                })
            }),
            no_guide(),
            Arc::new(|| {
                Ok(BotTurn {
                    origin: Origin {
                        conversation_id: 7,
                        epoch: 1,
                    },
                    message_id: 1,
                    text: "Petición de prueba".into(),
                    truncated: false,
                    created_at: "2026-10-29T12:00:00Z".into(),
                })
            }),
        ))
        .unwrap();
    let list = registry
        .respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .unwrap();
    let names = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        !names
            .iter()
            .any(|n| n.contains("resolve") || n.contains("begin") || n.contains("cancel"))
    );
    let get=registry.respond(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"assignments_review_get","arguments":{}}})).unwrap();
    assert_eq!(get["result"]["isError"], false);
    let update=registry.respond(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"assignments_update","arguments":{"id":row.id,"expected_revision":1,"document":document("Cambio no revisado")}}})).unwrap();
    assert_eq!(update["result"]["isError"], true);
    let save=registry.respond(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"assignments_propose","arguments":{"review_id":review["id"],"proposals":[candidate(row.id)]}}})).unwrap();
    assert_eq!(save["result"]["isError"], false);
    assert_eq!(store.get(row.id).unwrap().revision, 1);
}

#[test]
fn human_selection_rejects_changed_revision_and_reused_identity_before_io() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let request = Begin {
        request_key: "stale".into(),
        assignments: vec![Reference {
            id: row.id,
            instance_id: row.instance_id.clone(),
            revision: 1,
        }],
        origin: Origin {
            conversation_id: 7,
            epoch: 1,
        },
        instructions: "Mejorar".into(),
        use_guide: true,
    };
    store
        .update(row.id, 1, document("Edición reciente"))
        .unwrap();
    assert!(store.begin_review(request.clone()).is_err());
    store.delete(row.id, 2).unwrap();
    let replacement = store.create("a", document("A")).unwrap();
    assert_eq!(row.id, replacement.id);
    assert!(store.begin_review(request).is_err());
    assert!(
        store.reviews(replacement.id).unwrap()["reviews"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn a_batch_can_apply_distinct_changes_but_external_edits_invalidate_the_context() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let b = store.create("b", document("B")).unwrap();
    let review = begin(&store, vec![a.id, b.id], false);
    let id = review["id"].as_str().unwrap();
    let origin = Origin {
        conversation_id: 7,
        epoch: 1,
    };
    let saved = store
        .submit_proposals(
            None,
            id,
            &origin,
            vec![candidate(a.id), candidate(b.id)],
            &no_guide(),
        )
        .unwrap();
    store
        .resolve_proposal(saved["proposals"][0]["id"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(store.review_for_bot(id, &origin).unwrap()["current"], true);
    store
        .resolve_proposal(saved["proposals"][1]["id"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(store.get(b.id).unwrap().revision, 2);
    store.update(a.id, 2, document("Edición ajena")).unwrap();
    assert_eq!(store.review_for_bot(id, &origin).unwrap()["current"], false);
}
#[test]
fn stale_revision_and_reused_identity_cannot_receive_a_proposal() {
    let store = fixture();
    let a = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![a.id], false);
    let id = review["id"].as_str().unwrap();
    let saved = store
        .submit_proposals(
            None,
            id,
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![candidate(a.id)],
            &no_guide(),
        )
        .unwrap();
    store.delete(a.id, a.revision).unwrap();
    let replacement = store.create("a", document("A")).unwrap();
    assert_eq!(replacement.id, a.id);
    assert!(
        store
            .resolve_proposal(saved["proposals"][0]["id"].as_i64().unwrap(), true)
            .is_err()
    );
    assert_eq!(store.get(a.id).unwrap().revision, 1);
}
#[test]
fn guide_is_reserved_once_and_its_advice_never_applies_changes() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![row.id], true);
    let id = review["id"].as_str().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let guide: GuideEvaluator = Arc::new(move |state, questions| {
        count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(state["assignments"].as_array().unwrap().len(), 1);
        assert_eq!(questions.as_object().unwrap().len(), 3);
        Ok(result(1))
    });
    let origin = Origin {
        conversation_id: 7,
        epoch: 1,
    };
    let saved = store
        .submit_proposals(None, id, &origin, vec![candidate(row.id)], &guide)
        .unwrap();
    assert_eq!(saved["guide"]["probabilities"]["p0_useful"], 0.75);
    assert!(!saved.to_string().contains("NO_STORE"));
    store
        .submit_proposals(None, id, &origin, vec![candidate(row.id)], &guide)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.get(row.id).unwrap().revision, 1);
    let mut changed = candidate(row.id);
    changed.title = "Otro lote".into();
    assert!(
        store
            .submit_proposals(None, id, &origin, vec![changed], &guide)
            .is_err()
    );
}
#[test]
fn invalid_or_failed_guide_keeps_drafts_and_does_not_retry() {
    for answer in [Err("falló"), Ok(json!({"answers":{"other":{"noul":2}}}))] {
        let store = fixture();
        let row = store.create("a", document("A")).unwrap();
        let review = begin(&store, vec![row.id], true);
        let id = review["id"].as_str().unwrap();
        let guide: GuideEvaluator = Arc::new(move |_, _| answer.clone().map_err(Into::into));
        let origin = Origin {
            conversation_id: 7,
            epoch: 1,
        };
        let saved = store
            .submit_proposals(None, id, &origin, vec![candidate(row.id)], &guide)
            .unwrap();
        assert_eq!(saved["state"], "guide_failed");
        assert_eq!(saved["proposals"].as_array().unwrap().len(), 1);
        store
            .submit_proposals(None, id, &origin, vec![candidate(row.id)], &no_guide())
            .unwrap();
    }
}
#[test]
fn interrupted_evaluation_is_recovered_from_disk_without_another_request() {
    let file = std::env::temp_dir().join(format!(
        "gennie-review-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Database::open(&file, 1000).unwrap();
    db.migrate("events", AssignmentsModule.migrations())
        .unwrap();
    let store = Store::new(db.clone());
    let row = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![row.id], true);
    let id = review["id"].as_str().unwrap().to_owned();
    store
        .submit_proposals(
            None,
            &id,
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![candidate(row.id)],
            &({
                let guide: GuideEvaluator = Arc::new(|_, _| Err("sin red".into()));
                guide
            }),
        )
        .unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE assignment_reviews SET state='evaluating'", [])?;
        Ok(())
    })
    .unwrap();
    drop(store);
    db.close().unwrap();
    drop(db);
    let db = Database::open(&file, 1000).unwrap();
    let store = Store::new(db.clone());
    store.recover_reviews().unwrap();
    let saved = store
        .submit_proposals(
            None,
            &id,
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    assert_eq!(saved["state"], "interrupted");
    drop(store);
    db.close().unwrap();
    drop(db);
    std::fs::remove_file(file).unwrap();
}
#[test]
fn apply_and_receipt_roll_back_together_and_questions_cannot_be_applied() {
    let store = fixture();
    let row = store.create("a", document("A")).unwrap();
    let review = begin(&store, vec![row.id], false);
    let id = review["id"].as_str().unwrap();
    let saved = store
        .submit_proposals(
            None,
            id,
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![candidate(row.id)],
            &no_guide(),
        )
        .unwrap();
    let pid = saved["proposals"][0]["id"].as_i64().unwrap();
    store.db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_receipt BEFORE UPDATE ON assignment_proposals BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(store.resolve_proposal(pid, true).is_err());
    assert_eq!(store.get(row.id).unwrap().revision, 1);
    store
        .db
        .with_connection(|c| {
            c.execute_batch("DROP TRIGGER reject_receipt")?;
            Ok(())
        })
        .unwrap();
    store.resolve_proposal(pid, false).unwrap();
    assert_eq!(store.get(row.id).unwrap().revision, 1);
    let other = store.create("b", document("B")).unwrap();
    let mut request:Begin=serde_json::from_value(json!({"request_key":"q","assignments":[{"id":other.id,"instance_id":other.instance_id,"revision":other.revision}],"origin":{"conversation_id":7,"epoch":1},"instructions":"Aclarar","use_guide":false})).unwrap();
    request.request_key = "q".into();
    let review = store.begin_review(request).unwrap();
    let mut p = candidate(other.id);
    p.document = None;
    p.questions = vec!["¿Qué cuenta quieres revisar?".into()];
    let saved = store
        .submit_proposals(
            None,
            review["id"].as_str().unwrap(),
            &Origin {
                conversation_id: 7,
                epoch: 1,
            },
            vec![p],
            &no_guide(),
        )
        .unwrap();
    assert!(
        store
            .resolve_proposal(saved["proposals"][0]["id"].as_i64().unwrap(), true)
            .is_err()
    );
}
