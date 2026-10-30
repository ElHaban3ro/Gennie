use super::*;
use crate::{LlmModule, delegation::Origin};
use gennie_core::AppModule;
use gennie_db::{SqliteConnection, params};
use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicUsize, Ordering},
};
pub(super) struct Journal {
    epoch: AtomicI64,
    commits: AtomicUsize,
}
impl ConversationJournal for Journal {
    fn activate(&self, _: &SqliteConnection, _: &str, _: &str, _: Option<&str>) -> Result<()> {
        Ok(())
    }
    fn append(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    fn committed(&self) {
        self.commits.fetch_add(1, Ordering::SeqCst);
    }
    fn delegation_context(
        &self,
        origin: &Origin,
    ) -> Result<Option<Vec<delegation::ContextMessage>>> {
        Ok(
            (origin.conversation_id == 3 && origin.epoch == self.epoch.load(Ordering::SeqCst))
                .then(|| {
                    vec![delegation::ContextMessage {
                        speaker: "user".into(),
                        text: "Solo la sesión de origen".into(),
                        quote: None,
                    }]
                }),
        )
    }
    fn deliver_execution_notice(
        &self,
        c: &SqliteConnection,
        update: &Update,
        text: &str,
    ) -> Result<bool> {
        if update.origin.epoch != self.epoch.load(Ordering::SeqCst) {
            return Ok(false);
        }
        c.execute(
            "INSERT INTO test_messages(event_key,text) VALUES(?1,?2)",
            params![update.event_key, text],
        )?;
        Ok(true)
    }
}
pub(super) fn fixture() -> (Database, Arc<Journal>, Update) {
    let db = Database::memory().unwrap();
    db.migrate("llm", LlmModule::default().migrations())
        .unwrap();
    db.with_connection(|c| {
        c.execute_batch("CREATE TABLE test_messages(event_key TEXT PRIMARY KEY,text TEXT)")?;
        Ok(())
    })
    .unwrap();
    let journal = Arc::new(Journal {
        epoch: AtomicI64::new(1),
        commits: AtomicUsize::new(0),
    });
    let update = Update {
        event_key: "assignment-execution:9".into(),
        origin: Origin {
            conversation_id: 3,
            bot_id: "gennie".into(),
            epoch: 1,
            provider_thread: Some("origin".into()),
        },
        reference: Reference {
            assignment_id: 4,
            instance_id: "a".repeat(32),
            policy_id: "policy-fixture".into(),
            policy_revision: 2,
            run_id: 9,
            state: "uncertain".into(),
        },
        title: "Revisión de novedades".into(),
        scheduled_at: "2026-10-29T13:00:00Z".into(),
        state: "uncertain".into(),
        code: "effect_unconfirmed".into(),
        observation: Some("OBSERVATION_NOT_A_MESSAGE".into()),
        observation_truncated: false,
        guidance: None,
    };
    (db, journal, update)
}
fn messages(db: &Database) -> Vec<String> {
    db.with_connection(|c| {
        let mut q = c.prepare("SELECT text FROM test_messages")?;
        Ok(q.query_map([], |r| r.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    })
    .unwrap()
}
#[test]
fn typed_input_is_idempotent_and_bot_renders_once_without_a_helper_identity() {
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    store.accept(&update).unwrap();
    let mut different = update.clone();
    different.origin.conversation_id = 8;
    assert!(store.accept(&different).is_err());
    let calls = AtomicUsize::new(0);
    let render = |_: &Settings, instructions: &str, input: &str| -> Result<String> {
        calls.fetch_add(1, Ordering::SeqCst);
        assert!(instructions.contains("No tienes herramientas"));
        let input: serde_json::Value = serde_json::from_str(input)?;
        assert_eq!(input["host_state"], "uncertain");
        assert_eq!(
            input["origin_context"][0]["text"],
            "Solo la sesión de origen"
        );
        assert!(input.get("helper_observation").is_none());
        Ok(json!({"message":"La función no confirmó el resultado; quedó pausada para comprobarlo."}).to_string())
    };
    assert!(step(&db, journal.as_ref(), &render).unwrap());
    assert!(!step(&db, journal.as_ref(), &render).unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(messages(&db).len(), 1);
    assert_eq!(journal.commits.load(Ordering::SeqCst), 1);
    db.with_connection(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM llm_delegations", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        let config: String =
            c.query_row("SELECT render_config FROM llm_execution_notices", [], |r| {
                r.get(0)
            })?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&config)?["capabilities"],
            json!([])
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn ordinary_notice_uses_message_contract_on_the_real_transport() {
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    let calls = AtomicUsize::new(0);
    let render = |_: &Settings, instructions: &str, input: &str, output| {
        calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(instructions, INSTRUCTIONS);
        assert_eq!(output, crate::codex::ScopedOutput::Message);
        crate::codex::scoped_output_tests::render(input, "unused", output)
    };
    assert!(step_with_guide(&db, journal.as_ref(), &render, &|| false, None).unwrap());
    assert!(!step_with_guide(&db, journal.as_ref(), &render, &|| false, None).unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(messages(&db), vec!["Mensaje exclusivo de Gennie"]);
}
#[test]
fn failed_delivery_rolls_back_message_and_reuses_ready_text_without_more_inference() {
    let (db, journal, update) = fixture();
    Store(db.clone()).accept(&update).unwrap();
    db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_notice_receipt BEFORE UPDATE ON llm_execution_notices WHEN NEW.state='delivered' BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let calls = AtomicUsize::new(0);
    let render = |_: &Settings, _: &str, _: &str| -> Result<String> {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"message":"Comprueba el efecto antes de continuar."}).to_string())
    };
    assert!(step(&db, journal.as_ref(), &render).is_err());
    assert!(messages(&db).is_empty());
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_notice_receipt")?;
        Ok(())
    })
    .unwrap();
    Store(db.clone()).recover().unwrap();
    assert!(step(&db, journal.as_ref(), &render).unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(messages(&db).len(), 1);
}
#[test]
fn interrupted_or_invalid_render_uses_fixed_bot_facts_without_copying_observation() {
    for interrupted in [false, true] {
        let (db, journal, update) = fixture();
        let store = Store(db.clone());
        store.accept(&update).unwrap();
        if interrupted {
            store.next().unwrap();
            store.recover().unwrap();
        }
        let calls = AtomicUsize::new(0);
        let render = |_: &Settings, _: &str, _: &str| -> Result<String> {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok("malformed".into())
        };
        step(&db, journal.as_ref(), &render).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(!interrupted));
        let text = messages(&db).pop().unwrap();
        assert!(text.contains("no confirmar") || text.contains("No se pudo confirmar"));
        assert!(!text.contains("OBSERVATION_NOT_A_MESSAGE"));
        assert!(!step(&db, journal.as_ref(), &render).unwrap());
    }
}
#[test]
fn closed_bot_ingress_rejects_new_notices_without_inference() {
    let (db, journal, update) = fixture();
    let (events, _) = std::sync::mpsc::channel();
    let mut worker =
        crate::delegation::Worker::start(db.clone(), Some(journal), events, None).unwrap();
    let receiver = worker.receiver.clone();
    worker.stop().unwrap();
    assert!(receiver.accept_execution_notice(&update).is_err());
    assert!(Store(db).next().unwrap().is_none());
}
#[test]
fn invalid_origin_is_discarded_before_render_and_inconsistent_facts_are_rejected() {
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    let mut invalid = update.clone();
    invalid.reference.state = "succeeded".into();
    assert!(store.accept(&invalid).is_err());
    store.accept(&update).unwrap();
    journal.epoch.store(2, Ordering::SeqCst);
    assert!(
        step(&db, journal.as_ref(), &|_, _, _| panic!(
            "no inference for deleted origin"
        ))
        .unwrap()
    );
    assert!(messages(&db).is_empty());
    assert!(!step(&db, journal.as_ref(), &|_, _, _| panic!("no retry")).unwrap());
}

#[test]
fn shutdown_retains_interrupted_render_for_delivery_without_a_second_model_call() {
    let (db, journal, update) = fixture();
    let store = Store(db.clone());
    store.accept(&update).unwrap();
    let closing = std::sync::atomic::AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let render = |_: &Settings, _: &str, _: &str| -> Result<String> {
        calls.fetch_add(1, Ordering::SeqCst);
        closing.store(true, Ordering::SeqCst);
        Err("cancelled fixture".into())
    };
    assert!(
        step_with_shutdown(&db, journal.as_ref(), &render, &|| closing
            .load(Ordering::SeqCst))
        .unwrap()
    );
    assert!(messages(&db).is_empty());
    db.with_connection(|c| {
        assert_eq!(
            c.query_row("SELECT failure_code FROM llm_execution_notices", [], |r| {
                r.get::<_, String>(0)
            })?,
            "interrupted"
        );
        Ok(())
    })
    .unwrap();
    store.recover().unwrap();
    assert!(
        step(&db, journal.as_ref(), &|_, _, _| panic!(
            "no automatic second inference"
        ))
        .unwrap()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(messages(&db).len(), 1);
}
