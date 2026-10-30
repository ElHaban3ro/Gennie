use super::*;
use crate::notices::{decision_details, step_with_guide};
use std::sync::atomic::{AtomicUsize, Ordering};
fn fixture() -> (Database, Arc<crate::notices::tests::Journal>, Update) {
    let (db, journal, mut update) = crate::notices::tests::fixture();
    update.state = "succeeded".into();
    update.reference.state = "succeeded".into();
    update.code = "reported_success".into();
    update.guidance = Some(Guidance {
        objective: "Detectar cambios útiles".into(),
        scope: "Solo novedades de esta fuente".into(),
        signals: vec!["¿Cambió algo que requiera ayuda?".into()],
        decision_guidance: "No repitas preguntas pendientes; respeta aplazamientos".into(),
    });
    (db, journal, update)
}
fn response() -> Value {
    json!({"answers":{"record":{"type":"noul","noul":0.7},"notify":{"type":"noul","noul":0.2},"ask":{"type":"noul","noul":0.1},"offer_help":{"type":"noul","noul":0.05}},"usage":{"cost":0.002,"input_tokens":100,"output_tokens":20}})
}
type Permission = Box<dyn Fn(&Update) -> Result<bool> + Send + Sync>;
type Preparation = Box<dyn Fn(&Update) -> Result<GuideInvocation> + Send + Sync>;
struct TestGuide {
    permitted: Permission,
    prepare: Preparation,
}
impl Guide for TestGuide {
    fn permitted(&self, update: &Update) -> Result<bool> {
        (self.permitted)(update)
    }
    fn prepare(&self, update: &Update) -> Result<GuideInvocation> {
        (self.prepare)(update)
    }
}
fn factory(calls: Arc<AtomicUsize>, output: Option<Value>) -> GuideFactory {
    Arc::new(TestGuide {
        permitted: Box::new(|_| Ok(true)),
        prepare: Box::new(move |_| {
            let calls = calls.clone();
            let output = output.clone();
            Ok(GuideInvocation {
                model: "typesafe/jev-1.14".into(),
                revision: 2,
                evaluate: Box::new(move |state, questions| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(questions.as_object().unwrap().len(), 4);
                    assert_eq!(
                        state["origin_context"][0]["text"],
                        "Solo la sesión de origen"
                    );
                    output.ok_or_else(|| "FREE_PRIVATE_GUIDE_ERROR".into())
                }),
            })
        }),
    })
}
fn messages(db: &Database) -> i64 {
    db.with_connection(|c| Ok(c.query_row("SELECT count(*) FROM test_messages", [], |r| r.get(0))?))
        .unwrap()
}
#[test]
fn bot_decision_does_not_turn_a_record_or_proposal_into_an_effect() {
    let record =
        Decision::parse(r#"{"action":"record","reason":"Sin cambios pertinentes","message":null}"#)
            .unwrap();
    assert_eq!(record.action, Action::Record);
    assert!(record.message.is_none());
    for invalid in [
        r#"{"action":"execute","reason":"nuevo efecto","message":"Hecho"}"#,
        r#"{"action":"record","reason":"sin cambios","message":"No enviar"}"#,
        r#"{"action":"ask","reason":"falta dato","message":null}"#,
    ] {
        assert!(Decision::parse(invalid).is_err());
    }
}

#[test]
fn guide_advice_goes_to_the_bot_and_only_bot_actions_reach_the_journal_once() {
    for action in ["record", "notify", "ask", "offer_help"] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let guide = factory(calls.clone(), Some(response()));
        let renders = AtomicUsize::new(0);
        let render = |_: &Settings,
                      instructions: &str,
                      input: &str,
                      output: crate::codex::ScopedOutput|
         -> Result<String> {
            renders.fetch_add(1, Ordering::SeqCst);
            assert!(instructions.contains("no son autorización"));
            assert_eq!(output, crate::codex::ScopedOutput::ContextualDecision);
            assert!(instructions.contains("No tienes herramientas"));
            let input: Value = serde_json::from_str(input)?;
            assert_eq!(input["guide_advice"]["probabilities"]["record"], 0.7);
            // El bot puede elegir preguntar aunque record tenga mayor probabilidad: no hay corte universal.
            Ok(json!({"action":action,"reason":"Necesidad concreta según el acuerdo","message":if action=="record"{None}else{Some("Mensaje exclusivo de Gennie")}}).to_string())
        };
        assert!(step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap());
        assert!(!step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(renders.load(Ordering::SeqCst), 1);
        assert_eq!(messages(&db), i64::from(action != "record"));
        let detail = decision_details(&db, 9).unwrap().unwrap();
        assert_eq!(detail["action"], action);
        assert_eq!(detail["guide_state"], "ready");
        assert_eq!(detail["guide_configuration"]["revision"], 2);
        assert_eq!(detail["guide_advice"]["cost_reported"], 0.002);
        assert!(!detail.to_string().contains("OBSERVATION_NOT_A_MESSAGE"));
        store.recover().unwrap();
        store.accept(&update).unwrap();
        assert!(store.next().unwrap().is_none());
    }
}

#[test]
fn contextual_decisions_survive_the_real_transport_output_contract() {
    let mut actual = Vec::new();
    for action in ["record", "notify", "ask", "offer_help"] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let guide = factory(calls.clone(), Some(response()));
        let renders = AtomicUsize::new(0);
        let render =
            |_: &Settings, instructions: &str, input: &str, output: crate::codex::ScopedOutput| {
                renders.fetch_add(1, Ordering::SeqCst);
                assert_eq!(instructions, INSTRUCTIONS);
                crate::codex::scoped_output_tests::render(input, action, output)
            };
        assert!(step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap());
        assert!(!step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(renders.load(Ordering::SeqCst), 1);
        let detail = decision_details(&db, 9).unwrap().unwrap();
        actual.push((
            detail["action"].clone(),
            messages(&db),
            detail["reason"].clone(),
        ));
    }
    let expected: Vec<_> = ["record", "notify", "ask", "offer_help"]
        .into_iter()
        .map(|action| {
            (
                json!(action),
                i64::from(action != "record"),
                json!("Necesidad concreta según el acuerdo"),
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn unavailable_invalid_failed_or_interrupted_guidance_keeps_results_without_retry() {
    for kind in [
        "unavailable",
        "failed",
        "invalid",
        "bot_failed",
        "interrupted",
    ] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let output = match kind {
            "failed" => None,
            "invalid" => Some(json!({"answers":{"unknown":{},"wrong":{},"keys":{},"only":{}}})),
            _ => Some(response()),
        };
        let guide = factory(calls.clone(), output);
        let renders = AtomicUsize::new(0);
        let render =
            |_: &Settings, _: &str, _: &str, _: crate::codex::ScopedOutput| -> Result<String> {
                renders.fetch_add(1, Ordering::SeqCst);
                Err("FREE_PRIVATE_BOT_ERROR".into())
            };
        if kind == "interrupted" {
            store.next().unwrap();
            let invocation = guide.prepare(&update).unwrap();
            store.reserve_guide(&update, &invocation).unwrap();
            store.recover().unwrap();
        } else {
            step_with_guide(
                &db,
                journal.as_ref(),
                &render,
                &|| false,
                if kind == "unavailable" {
                    None
                } else {
                    Some(&guide)
                },
            )
            .unwrap();
        }
        assert_eq!(messages(&db), 0);
        assert_eq!(
            renders.load(Ordering::SeqCst),
            usize::from(kind == "bot_failed")
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            usize::from(!["unavailable", "interrupted"].contains(&kind))
        );
        store.recover().unwrap();
        assert!(!step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap());
        let detail = decision_details(&db, 9).unwrap().unwrap();
        assert_eq!(detail["action"], "record");
        assert!(!detail.to_string().contains("FREE_PRIVATE"));
    }
}

#[test]
fn operational_uncertainty_bypasses_the_contextual_filter_and_never_calls_the_guide() {
    let (db, journal, mut update) = fixture();
    update.state = "uncertain".into();
    update.reference.state = "uncertain".into();
    update.code = "effect_unconfirmed".into();
    Store(db.clone()).accept(&update).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let guide = factory(calls.clone(), Some(response()));
    step_with_guide(
        &db,
        journal.as_ref(),
        &|_, _, _, _| {
            Ok(
                json!({"message":"El efecto no se confirmó. Compruébalo antes de reactivar."})
                    .to_string(),
            )
        },
        &|| false,
        Some(&guide),
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(messages(&db), 1);
    assert_eq!(
        decision_details(&db, 9).unwrap().unwrap()["guide_state"],
        "bypassed"
    );
}

#[test]
fn reservations_and_decision_commit_failures_never_repeat_a_remote_call() {
    for fail_reservation in [false, true] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        db.with_connection(|c| {c.execute_batch(if fail_reservation{"CREATE TRIGGER fail_guide BEFORE UPDATE OF guide_attempted_at ON llm_execution_notices BEGIN SELECT RAISE(ABORT,'fixture'); END;"}else{"CREATE TRIGGER fail_guide BEFORE UPDATE OF notice_action ON llm_execution_notices BEGIN SELECT RAISE(ABORT,'fixture'); END;"})?;Ok(())}).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let guide = factory(calls.clone(), Some(response()));
        let _ = step_with_guide(
            &db,
            journal.as_ref(),
            &|_, _, _, _| {
                Ok(
                    json!({"action":"notify","reason":"Novedad útil","message":"Solo Gennie"})
                        .to_string(),
                )
            },
            &|| false,
            Some(&guide),
        );
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(!fail_reservation));
        assert_eq!(messages(&db), 0);
        db.with_connection(|c| {
            c.execute_batch("DROP TRIGGER fail_guide")?;
            Ok(())
        })
        .unwrap();
        store.recover().unwrap();
        assert!(
            !step_with_guide(
                &db,
                journal.as_ref(),
                &|_, _, _, _| panic!("No repetir"),
                &|| false,
                Some(&guide)
            )
            .unwrap()
        );
    }
}

#[test]
fn previous_context_is_bounded_to_consented_same_policy_revision_and_origin() {
    let (db, _, mut update) = fixture();
    let store = Store(db.clone());
    for run in 1..=8 {
        update.reference.run_id = run;
        update.event_key = format!("assignment-execution:{run}");
        update.observation = Some("🦀".repeat(8000));
        if run == 7 {
            update.reference.policy_revision = 3;
        } else {
            update.reference.policy_revision = 2;
        }
        if run == 8 {
            update.origin.epoch = 2;
        } else {
            update.origin.epoch = 1;
        }
        store.accept(&update).unwrap();
        store.next().unwrap();
        let bot = architects::load(&db, architects::Role::Bot).unwrap();
        store
            .decide(
                &update,
                bot.revision,
                &Decision {
                    action: Action::Record,
                    reason: format!("Resultado {run}"),
                    message: None,
                },
                None,
                None,
            )
            .unwrap();
    }
    update.reference.run_id = 9;
    update.origin.epoch = 1;
    update.reference.policy_revision = 2;
    let history = store.previous_decisions(&update).unwrap();
    assert_eq!(history.len(), 3);
    let encoded = serde_json::to_string(&history).unwrap();
    assert!(!encoded.contains("Resultado 7"));
    assert!(!encoded.contains("Resultado 8"));
    assert!(history.iter().all(
        |v| v["observation"].as_str().unwrap().chars().count() <= 2000
            && v["observation_truncated"] == true
    ));
    assert!(
        history
            .iter()
            .map(|v| v.to_string().chars().count())
            .sum::<usize>()
            <= 8000
    );
}

#[test]
fn revocation_before_or_during_io_prevents_further_queries_and_messages() {
    use std::sync::atomic::AtomicBool;
    for phase in ["before_guide", "after_guide", "after_bot"] {
        let (db, journal, update) = fixture();
        Store(db.clone()).accept(&update).unwrap();
        let allowed = Arc::new(AtomicBool::new(phase != "before_guide"));
        let checks = allowed.clone();
        let after_guide = allowed.clone();
        let guide_calls = Arc::new(AtomicUsize::new(0));
        let count = guide_calls.clone();
        let guide: GuideFactory = Arc::new(TestGuide {
            permitted: Box::new(move |_| Ok(checks.load(Ordering::SeqCst))),
            prepare: Box::new(move |_| {
                let allowed = after_guide.clone();
                let count = count.clone();
                Ok(GuideInvocation {
                    model: "typesafe/jev-1.14".into(),
                    revision: 1,
                    evaluate: Box::new(move |_, _| {
                        count.fetch_add(1, Ordering::SeqCst);
                        if phase == "after_guide" {
                            allowed.store(false, Ordering::SeqCst);
                        }
                        Ok(response())
                    }),
                })
            }),
        });
        let renders = AtomicUsize::new(0);
        let render = |_: &Settings,
                      _: &str,
                      _: &str,
                      _: crate::codex::ScopedOutput|
         -> Result<String> {
            renders.fetch_add(1, Ordering::SeqCst);
            allowed.store(false, Ordering::SeqCst);
            Ok(json!({"action":"notify","reason":"Novedad","message":"No se entrega tras retirar permiso"}).to_string())
        };
        step_with_guide(&db, journal.as_ref(), &render, &|| false, Some(&guide)).unwrap();
        assert_eq!(
            guide_calls.load(Ordering::SeqCst),
            usize::from(phase != "before_guide")
        );
        assert_eq!(
            renders.load(Ordering::SeqCst),
            usize::from(phase == "after_bot")
        );
        assert_eq!(messages(&db), 0);
        assert_eq!(
            decision_details(&db, 9).unwrap().unwrap()["action"],
            "record"
        );
    }
}

#[test]
fn legacy_receipts_migrate_without_becoming_contextual_or_repeating_prepared_output() {
    use gennie_core::AppModule;
    let (_, journal, mut update) = fixture();
    update.guidance = None;
    let db = Database::memory().unwrap();
    let module = crate::LlmModule::default();
    db.migrate("llm", &module.migrations()[..8]).unwrap();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    db.with_connection(|c|{c.execute_batch("CREATE TABLE test_messages(event_key TEXT PRIMARY KEY,text TEXT)")?;c.execute("UPDATE llm_execution_notices SET state='ready',rendered_text='Salida preparada anterior'",[])?;Ok(())}).unwrap();
    db.migrate("llm", module.migrations()).unwrap();
    store.recover().unwrap();
    step_with_guide(
        &db,
        journal.as_ref(),
        &|_, _, _, _| panic!("No inferir"),
        &|| false,
        None,
    )
    .unwrap();
    assert_eq!(messages(&db), 1);
    let detail = decision_details(&db, 9).unwrap().unwrap();
    assert_eq!(detail["guide_state"], "not_requested");
    assert!(!serde_json::to_string(&update).unwrap().contains("guidance"));
    assert!(
        !step_with_guide(
            &db,
            journal.as_ref(),
            &|_, _, _, _| panic!("No repetir"),
            &|| false,
            None
        )
        .unwrap()
    );
}

#[test]
fn reopening_an_interrupted_valuation_keeps_advice_without_another_inference() {
    use gennie_core::AppModule;
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    store.next().unwrap();
    let invocation = GuideInvocation {
        model: "typesafe/jev-1.14".into(),
        revision: 4,
        evaluate: Box::new(|_, _| panic!("No red")),
    };
    store.reserve_guide(&update, &invocation).unwrap();
    store
        .finish_guide(&update, Some(&advice(response()).unwrap()), 12)
        .unwrap();
    let root = std::env::temp_dir().join(format!(
        "gennie-contextual-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("fixture.sqlite");
    db.with_connection(|c| {
        c.execute("VACUUM INTO ?1", [path.to_str().unwrap()])?;
        Ok(())
    })
    .unwrap();
    let reopened = Database::open(&path, 50).unwrap();
    reopened
        .migrate("llm", crate::LlmModule::default().migrations())
        .unwrap();
    Store(reopened.clone()).recover().unwrap();
    assert!(
        !step_with_guide(
            &reopened,
            journal.as_ref(),
            &|_, _, _, _| panic!("No repetir bot"),
            &|| false,
            None
        )
        .unwrap()
    );
    let detail = decision_details(&reopened, 9).unwrap().unwrap();
    assert_eq!(detail["action"], "record");
    assert_eq!(detail["guide_state"], "ready");
    assert_eq!(detail["failure_code"], "interrupted");
    assert_eq!(detail["guide_duration_ms"], 12);
    assert!(detail["bot_revision"].is_null());
    assert_eq!(messages(&reopened), 0);
    reopened.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn prepared_messages_revalidate_policy_without_preparing_a_model_even_after_recovery() {
    for permitted in [false, true] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        store.next().unwrap();
        let bot = architects::load(&db, architects::Role::Bot).unwrap();
        store
            .decide(
                &update,
                bot.revision,
                &Decision {
                    action: Action::Notify,
                    reason: "Cambio pertinente".into(),
                    message: Some("Mensaje preparado por Gennie".into()),
                },
                None,
                None,
            )
            .unwrap();
        store.recover().unwrap();
        let guide: GuideFactory = Arc::new(TestGuide {
            permitted: Box::new(move |_| Ok(permitted)),
            prepare: Box::new(|_| panic!("No preparar un modelo para entregar")),
        });
        step_with_guide(
            &db,
            journal.as_ref(),
            &|_, _, _, _| panic!("No inferir"),
            &|| false,
            Some(&guide),
        )
        .unwrap();
        assert_eq!(messages(&db), i64::from(permitted));
        let detail = decision_details(&db, 9).unwrap().unwrap();
        assert_eq!(
            detail["state"],
            if permitted { "delivered" } else { "discarded" }
        );
        assert_eq!(
            detail["action"],
            if permitted { "notify" } else { "record" }
        );
        assert!(
            !step_with_guide(
                &db,
                journal.as_ref(),
                &|_, _, _, _| panic!("No repetir"),
                &|| false,
                Some(&guide)
            )
            .unwrap()
        );
    }
}

#[test]
fn permission_read_failure_keeps_prepared_text_for_a_later_check_without_inference() {
    use std::sync::atomic::AtomicBool;
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    store.next().unwrap();
    let bot = architects::load(&db, architects::Role::Bot).unwrap();
    store
        .decide(
            &update,
            bot.revision,
            &Decision {
                action: Action::Notify,
                reason: "Cambio pertinente".into(),
                message: Some("Mensaje que debe conservarse".into()),
            },
            None,
            None,
        )
        .unwrap();
    let unavailable = Arc::new(AtomicBool::new(true));
    let flag = unavailable.clone();
    let guide: GuideFactory = Arc::new(TestGuide {
        permitted: Box::new(move |_| {
            if flag.load(Ordering::SeqCst) {
                Err("fixture read failure".into())
            } else {
                Ok(true)
            }
        }),
        prepare: Box::new(|_| panic!("No preparar")),
    });
    assert!(
        step_with_guide(
            &db,
            journal.as_ref(),
            &|_, _, _, _| panic!("No inferir"),
            &|| false,
            Some(&guide)
        )
        .is_err()
    );
    assert_eq!(messages(&db), 0);
    store.recover().unwrap();
    assert_eq!(decision_details(&db, 9).unwrap().unwrap()["state"], "ready");
    unavailable.store(false, Ordering::SeqCst);
    step_with_guide(
        &db,
        journal.as_ref(),
        &|_, _, _, _| panic!("No inferir"),
        &|| false,
        Some(&guide),
    )
    .unwrap();
    assert_eq!(messages(&db), 1);
}
