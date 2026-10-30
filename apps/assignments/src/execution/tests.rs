use super::*;
use crate::AssignmentsModule;
use gennie_core::AppModule;
use gennie_db::Database;
use std::sync::atomic::{AtomicI64, AtomicUsize};
struct Fake {
    guide_ready: AtomicBool,
    effects: AtomicUsize,
    uncertain: AtomicBool,
    response: Mutex<Option<Value>>,
    accepted: Mutex<Vec<notices::Notice>>,
    notice_down: AtomicBool,
    lose_ack: AtomicBool,
    pause: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            guide_ready: AtomicBool::new(true),
            effects: AtomicUsize::new(0),
            uncertain: AtomicBool::new(false),
            response: Mutex::new(None),
            accepted: Mutex::new(Vec::new()),
            notice_down: AtomicBool::new(false),
            lose_ack: AtomicBool::new(false),
            pause: Mutex::new(None),
        }
    }
}
impl Runtime for Fake {
    fn guide_available(&self) -> Result<bool> {
        Ok(self.guide_ready.load(Ordering::SeqCst))
    }
    fn notice_origin(&self, conversation: Option<i64>) -> Result<notices::Delivery> {
        if conversation.is_some_and(|id| id != 3) {
            return Err("Destino inexistente".into());
        }
        Ok(notices::Delivery {
            conversation_id: 3,
            bot_id: "gennie".into(),
            epoch: 1,
            provider_thread: Some("origin".into()),
        })
    }
    fn accept_notice(&self, notice: &notices::Notice) -> Result<()> {
        if self.notice_down.load(Ordering::SeqCst) {
            return Err("Receptor no disponible".into());
        }
        let mut accepted = self.accepted.lock().unwrap();
        if let Some(previous) = accepted.iter().find(|n| n.event_key == notice.event_key) {
            assert_eq!(previous, notice);
        } else {
            accepted.push(notice.clone());
        }
        if self.lose_ack.swap(false, Ordering::SeqCst) {
            return Err("Respuesta de recepción perdida".into());
        }
        Ok(())
    }
    fn prepare(&self, _: &Target, _: &Value) -> Result<Route> {
        Ok(Route::Http)
    }
    fn execute(&self, _: &Target, _: &Value) -> Result<Value> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        if let Some((entered, resume)) = self.pause.lock().unwrap().take() {
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        if let Some(value) = self.response.lock().unwrap().clone() {
            return Ok(value);
        }
        if self.uncertain.load(Ordering::SeqCst) {
            Err("private transport details".into())
        } else {
            Ok(json!({"status":"ok","value":"fixture-result"}))
        }
    }
}
fn setup() -> (Controller, Arc<AtomicI64>, Arc<Fake>, Reference) {
    let db = Database::memory().unwrap();
    db.migrate("events", AssignmentsModule.migrations())
        .unwrap();
    let store = Store::new(db);
    let document=serde_json::from_value(json!({"title":"Resumen","description":"Consultar novedades","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"},"preparation":{"interval_minutes":1,"rationale":"Revisión de prueba"},"method":{"plugin_id":"fixture","functions":["read"]},"authorization":{"scope":"Consultar solo este dato","allowed_routes":["http"]}})).unwrap();
    let row = store.create("fixture", document).unwrap();
    let reference = Reference {
        id: row.id,
        instance_id: row.instance_id,
        revision: row.revision,
    };
    let clock = Arc::new(AtomicI64::new(
        schedule::epoch("2026-10-29T12:00:00Z").unwrap(),
    ));
    let c = clock.clone();
    let runtime = Arc::new(Fake::default());
    let (wake, _) = mpsc::sync_channel(1);
    let controller = Controller(Arc::new(Inner {
        store,
        runtime: runtime.clone(),
        clock: Arc::new(move || c.load(Ordering::SeqCst)),
        ingress: Mutex::new(true),
        stopping: AtomicBool::new(false),
        storage_failed: AtomicBool::new(false),
        runner_active: true,
        wake,
    }));
    (controller, clock, runtime, reference)
}
fn configuration(reference: Reference) -> Configure {
    Configure {
        contextual_notice: false,
        delivery: None,
        assignment: reference,
        expected_policy_revision: 0,
        trigger: Trigger::Preparation,
        dst: DstPolicy {
            repeated: crate::schedule::RepeatedTime::Earlier,
            missing: crate::schedule::MissingTime::Skip,
        },
        target: Target::Web {
            plugin_id: "fixture".into(),
            function: "read".into(),
            revision: 1,
        },
        arguments: json!({}),
        misfire: Misfire::RunLatest,
        max_lateness_minutes: 1,
        until: None,
        enabled: true,
    }
}
fn policies(c: &Controller, id: i64) -> Value {
    c.request(json!({"action":"list","assignment_id":id}))
        .unwrap()
}
fn runs(c: &Controller, id: &str) -> Value {
    c.request(json!({"action":"runs","policy_id":id})).unwrap()
}
#[test]
fn upcoming_times_choose_the_earliest_current_policy_without_dispatch_or_writes() {
    let (c, _, runtime, reference) = setup();
    let id = reference.id;
    let preparation = c.configure(configuration(reference.clone())).unwrap();
    let mut delivery = configuration(reference);
    delivery.trigger = Trigger::Delivery;
    let delivery = c.configure(delivery).unwrap();
    let revision = c.0.store.revision();
    let next = c.0.store.upcoming_execution_times(&[id]).unwrap();
    assert_eq!(
        next.get(&id),
        preparation["next_at"].as_str().map(String::from).as_ref()
    );
    assert_eq!(c.0.store.revision(), revision);
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
    assert!(
        c.0.store
            .upcoming_execution_times(&[id + 1])
            .unwrap()
            .is_empty()
    );
    c.request(json!({"action":"pause","policy_id":preparation["id"],"expected_revision":1}))
        .unwrap();
    let next = c.0.store.upcoming_execution_times(&[id]).unwrap();
    assert_eq!(
        next.get(&id),
        delivery["next_at"].as_str().map(String::from).as_ref()
    );
    c.request(json!({"action":"pause","policy_id":delivery["id"],"expected_revision":1}))
        .unwrap();
    assert!(
        c.0.store
            .upcoming_execution_times(&[id])
            .unwrap()
            .is_empty()
    );
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
}

#[test]
fn upcoming_times_omit_stale_documents_and_reused_assignment_instances() {
    let (c, _, runtime, reference) = setup();
    let id = reference.id;
    c.configure(configuration(reference)).unwrap();
    let mut row = c.0.store.get(id).unwrap();
    row.document.description = "Objetivo cambiado".into();
    let updated = c.0.store.update(id, row.revision, row.document).unwrap();
    assert!(
        c.0.store
            .upcoming_execution_times(&[id])
            .unwrap()
            .is_empty()
    );
    let mut cfg = configuration(Reference {
        id,
        instance_id: updated.instance_id.clone(),
        revision: updated.revision,
    });
    cfg.expected_policy_revision = 1;
    c.configure(cfg).unwrap();
    assert!(
        c.0.store
            .upcoming_execution_times(&[id])
            .unwrap()
            .contains_key(&id)
    );
    c.0.store.delete(id, updated.revision).unwrap();
    let replacement = c.0.store.create("replacement", updated.document).unwrap();
    assert_eq!(replacement.id, id);
    assert_ne!(replacement.instance_id, updated.instance_id);
    assert!(
        c.0.store
            .upcoming_execution_times(&[id])
            .unwrap()
            .is_empty()
    );
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
}

#[test]
fn upcoming_times_are_bounded_and_reject_invalid_persisted_dates_without_dispatch() {
    let (c, _, runtime, reference) = setup();
    let id = reference.id;
    let policy = c.configure(configuration(reference)).unwrap();
    assert!(c.0.store.upcoming_execution_times(&[]).unwrap().is_empty());
    assert!(c.0.store.upcoming_execution_times(&[0]).is_err());
    assert!(c.0.store.upcoming_execution_times(&[id; 101]).is_err());
    c.0.store
        .db
        .with_connection(|connection| {
            connection.execute(
                "UPDATE assignment_execution_policies SET next_at='invalid' WHERE id=?1",
                [policy["id"].as_str().unwrap()],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(c.0.store.upcoming_execution_times(&[id]).is_err());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
}

#[test]
fn contextual_notices_require_explicit_scope_origin_and_available_guide_without_consulting_it() {
    let (c, clock, runtime, reference) = setup();
    let id = reference.id;
    let mut cfg = configuration(reference);
    cfg.contextual_notice = true;
    assert!(c.configure(cfg.clone()).is_err());
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    assert!(c.configure(cfg.clone()).is_err());
    let mut row = c.0.store.get(id).unwrap();
    row.document.follow_up = Some(crate::FollowUpPolicy {
        scope: "Cambios de la fuente acordada".into(),
        signals: vec!["¿Hay un bloqueo nuevo?".into()],
        decision_guidance: "Evita repetir preguntas mientras espero".into(),
        action_scope: None,
    });
    let updated = c.0.store.update(id, row.revision, row.document).unwrap();
    cfg.assignment.revision = updated.revision;
    runtime.guide_ready.store(false, Ordering::SeqCst);
    assert!(c.configure(cfg.clone()).is_err());
    runtime.guide_ready.store(true, Ordering::SeqCst);
    let mut wrong = cfg.clone();
    wrong.trigger = Trigger::Delivery;
    assert!(c.configure(wrong).is_err());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
    let policy = c.configure(cfg).unwrap();
    assert_eq!(policy["configuration"]["contextual_notice"], true);
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().unwrap());
    let notice = c.0.store.next_execution_notice().unwrap().unwrap();
    let criteria = notice.guidance.unwrap();
    assert_eq!(criteria.scope, "Cambios de la fuente acordada");
    assert_eq!(criteria.signals.len(), 1);
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    let run = notice.run_id;
    let origin = runtime.notice_origin(None).unwrap();
    assert!(c.0.store.contextual_notice_permitted(run, &origin).unwrap());
    let mut other = origin.clone();
    other.epoch += 1;
    assert!(!c.0.store.contextual_notice_permitted(run, &other).unwrap());
    let paused = c
        .request(json!({"action":"pause","policy_id":policy["id"],"expected_revision":1}))
        .unwrap();
    assert!(c.0.store.contextual_notice_permitted(run, &origin).unwrap());
    runtime.guide_ready.store(false, Ordering::SeqCst);
    let revoked=c.request(json!({"action":"revoke_guide","policy_id":policy["id"],"expected_revision":paused["revision"]})).unwrap();
    assert_eq!(revoked["configuration"]["contextual_notice"], false);
    assert!(revoked["configuration"]["delivery"].is_null());
    assert_eq!(revoked["enabled"], false);
    assert!(!c.0.store.contextual_notice_permitted(run, &origin).unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    assert_eq!(
        c.0.store.execution_run_details(run).unwrap()["configuration"]["contextual_notice"],
        true
    );
}
#[test]
fn occurrence_is_reserved_once_and_rollback_of_clock_never_repeats_it() {
    let (c, clock, runtime, reference) = setup();
    let id = reference.id;
    let policy = c.configure(configuration(reference)).unwrap();
    let pid = policy["id"].as_str().unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().unwrap());
    assert!(!c.step().unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    clock.fetch_sub(30, Ordering::SeqCst);
    assert!(!c.step().unwrap());
    clock.fetch_add(90, Ordering::SeqCst);
    assert!(c.step().unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 2);
    assert_eq!(runs(&c, pid)["runs"].as_array().unwrap().len(), 2);
    assert_eq!(policies(&c, id)["policies"][0]["enabled"], true);
}
#[test]
fn missed_policy_coalesces_or_skips_without_a_backlog() {
    for latest in [false, true] {
        let (c, clock, runtime, reference) = setup();
        let mut cfg = configuration(reference);
        if !latest {
            cfg.misfire = Misfire::SkipMissed;
        }
        let policy = c.configure(cfg).unwrap();
        let pid = policy["id"].as_str().unwrap();
        clock.fetch_add(190, Ordering::SeqCst);
        assert!(c.step().unwrap());
        assert!(!c.step().unwrap());
        assert_eq!(runtime.effects.load(Ordering::SeqCst), usize::from(latest));
        assert_eq!(
            runs(&c, pid)["runs"][0]["state"],
            if latest { "succeeded" } else { "skipped" }
        );
    }
}
#[test]
fn changed_document_identity_or_authorization_prevents_dispatch() {
    let (c, clock, runtime, reference) = setup();
    let id = reference.id;
    c.configure(configuration(reference)).unwrap();
    let mut row = c.0.store.get(id).unwrap();
    row.document.authorization = None;
    row.document.method = None;
    c.0.store.update(id, row.revision, row.document).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
    assert_eq!(policies(&c, id)["policies"][0]["state"], "stale");
}
#[test]
fn result_uncertainty_blocks_other_arguments_and_requires_human_review() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference.clone());
    let policy = c.configure(cfg.clone()).unwrap();
    let pid = policy["id"].as_str().unwrap();
    runtime.uncertain.store(true, Ordering::SeqCst);
    clock.fetch_add(60, Ordering::SeqCst);
    c.step().unwrap();
    clock.fetch_add(600, Ordering::SeqCst);
    assert!(!c.step().unwrap());
    let page = runs(&c, pid);
    assert_eq!(page["runs"][0]["state"], "uncertain");
    assert!(!page.to_string().contains("private transport"));
    cfg.expected_policy_revision = 1;
    cfg.arguments = json!({"other":"arguments"});
    assert!(c.configure(cfg.clone()).is_err());
    c.request(json!({"action":"reconcile","run_id":page["runs"][0]["id"],"resolution":"applied"}))
        .unwrap();
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    assert_eq!(policies(&c, reference.id)["policies"][0]["enabled"], false);
    runtime.uncertain.store(false, Ordering::SeqCst);
    c.configure(cfg).unwrap();
    assert!(!c.step().unwrap());
}
#[test]
fn web_and_mcp_review_receipts_pause_without_retaining_private_results() {
    for (key, response) in [
        (
            "web_attempt_id",
            json!({"status":"ok","review_required":true,"executed":true,"web_attempt_id":7,"attempt":{"private":"PRIVATE_RESULT"},"data":"PRIVATE_RESULT"}),
        ),
        (
            "mcp_attempt_id",
            json!({"status":"tool_failed","review_required":true,"executed":true,"attempt_id":7,"data":"PRIVATE_RESULT"}),
        ),
    ] {
        let (controller, clock, runtime, reference) = setup();
        let policy = controller
            .configure(configuration(reference.clone()))
            .unwrap();
        *runtime.response.lock().unwrap() = Some(response);
        clock.fetch_add(60, Ordering::SeqCst);
        assert!(controller.step().unwrap());
        let page = runs(&controller, policy["id"].as_str().unwrap());
        let run = page["runs"][0]["id"].as_i64().unwrap();
        let details = controller.0.store.execution_run_details(run).unwrap();
        assert_eq!(details["state"], "uncertain");
        assert_eq!(details["output"], json!({key:7}));
        assert!(!details.to_string().contains("PRIVATE_RESULT"));
        assert_eq!(
            policies(&controller, reference.id)["policies"][0]["enabled"],
            false
        );
        clock.fetch_add(600, Ordering::SeqCst);
        assert!(!controller.step().unwrap());
        assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn completion_write_failure_does_not_allow_another_effect() {
    let (c, clock, runtime, reference) = setup();
    let policy = c.configure(configuration(reference)).unwrap();
    let pid = policy["id"].as_str().unwrap();
    c.0.store.db.with_connection(|db|{db.execute_batch("CREATE TRIGGER refuse_completed BEFORE UPDATE ON assignment_execution_runs WHEN NEW.state='succeeded' BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().is_err());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    c.0.store.recover_executions().unwrap();
    clock.fetch_add(120, Ordering::SeqCst);
    assert!(!c.step().unwrap());
    assert_eq!(runs(&c, pid)["runs"][0]["state"], "uncertain");
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
}
#[test]
fn pausing_during_io_does_not_block_or_reenable_the_policy_after_completion() {
    let (c, clock, runtime, reference) = setup();
    let policy = c.configure(configuration(reference.clone())).unwrap();
    let pid = policy["id"].as_str().unwrap();
    let (entered, wait) = mpsc::channel();
    let (resume, proceed) = mpsc::channel();
    *runtime.pause.lock().unwrap() = Some((entered, proceed));
    clock.fetch_add(60, Ordering::SeqCst);
    let worker = c.clone();
    let thread = std::thread::spawn(move || worker.step());
    wait.recv_timeout(Duration::from_secs(2)).unwrap();
    c.request(json!({"action":"pause","policy_id":pid,"expected_revision":1}))
        .unwrap();
    resume.send(()).unwrap();
    thread.join().unwrap().unwrap();
    assert_eq!(policies(&c, reference.id)["policies"][0]["enabled"], false);
    clock.fetch_add(600, Ordering::SeqCst);
    assert!(!c.step().unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
}
#[test]
fn reservation_failure_sends_nothing_and_expiration_is_enforced() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference);
    cfg.until = Some(schedule::utc(clock.load(Ordering::SeqCst) + 90).unwrap());
    let policy = c.configure(cfg).unwrap();
    let pid = policy["id"].as_str().unwrap();
    c.0.store.db.with_connection(|db|{db.execute_batch("CREATE TRIGGER refuse_reservation BEFORE INSERT ON assignment_execution_runs BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().is_err());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
    c.0.store
        .db
        .with_connection(|db| {
            db.execute_batch("DROP TRIGGER refuse_reservation")?;
            Ok(())
        })
        .unwrap();
    clock.fetch_add(40, Ordering::SeqCst);
    assert!(c.step().unwrap());
    assert_eq!(runs(&c, pid)["runs"][0]["state"], "skipped");
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
}
#[test]
fn runner_has_an_exclusive_lifecycle_and_rejects_late_requests() {
    let (c, _, runtime, _) = setup();
    let path = std::env::temp_dir().join(format!(
        "gennie-scheduler-{}-{}.lock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut runner = Runner::start(c.0.store.clone(), runtime.clone(), &path).unwrap();
    assert!(Runner::start(c.0.store.clone(), runtime.clone(), &path).is_err());
    let controller = runner.controller();
    runner.close_ingress().unwrap();
    assert!(
        controller
            .request(json!({"action":"list","assignment_id":1}))
            .is_err()
    );
    runner.join().unwrap();
    drop(runner);
    std::fs::remove_file(path).unwrap();
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
}

#[test]
fn reopening_a_dispatched_run_requires_review_and_preserves_its_original_version() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference.clone());
    cfg.arguments = json!({"query":"original"});
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    let policy = c.configure(cfg).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    let store::Due::Run(run_id, _) =
        c.0.store
            .reserve_execution(clock.load(Ordering::SeqCst))
            .unwrap()
    else {
        panic!("expected reservation")
    };
    assert!(
        c.0.store
            .dispatch_execution(run_id, clock.load(Ordering::SeqCst))
            .unwrap()
    );
    let path = std::env::temp_dir().join(format!(
        "gennie-execution-recovery-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    c.0.store
        .db
        .with_connection(|db| {
            db.execute("VACUUM INTO ?1", [path.to_str().unwrap()])?;
            Ok(())
        })
        .unwrap();
    let db = Database::open(&path, 50).unwrap();
    db.migrate("events", AssignmentsModule.migrations())
        .unwrap();
    let reopened = Store::new(db.clone());
    let before = reopened.revision();
    reopened.recover_executions().unwrap();
    assert_ne!(before, reopened.revision());
    let detail = reopened.execution_run_details(run_id).unwrap();
    assert_eq!(detail["state"], "uncertain");
    assert_eq!(
        detail["configuration"]["arguments"],
        json!({"query":"original"})
    );
    assert!(matches!(
        reopened
            .reserve_execution(clock.load(Ordering::SeqCst) + 600)
            .unwrap(),
        store::Due::Idle
    ));
    assert_eq!(
        reopened.execution_policies(reference.id).unwrap()["policies"][0]["state"],
        "review_required"
    );
    assert!(
        reopened
            .execution_runs(policy["id"].as_str().unwrap(), None)
            .unwrap()["runs"][0]
            .get("output")
            .is_none()
    );
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 0);
    let pending = reopened.next_execution_notice().unwrap().unwrap();
    assert_eq!(pending.state, "uncertain");
    db.close().unwrap();
    let file_db = Database::open(&path, 50).unwrap();
    file_db
        .migrate("events", AssignmentsModule.migrations())
        .unwrap();
    assert_eq!(
        Store::new(file_db.clone())
            .next_execution_notice()
            .unwrap()
            .unwrap(),
        pending
    );
    file_db.close().unwrap();
    for file in [
        path.clone(),
        path.with_extension("sqlite-wal"),
        path.with_extension("sqlite-shm"),
    ] {
        let _ = std::fs::remove_file(file);
    }
}

#[test]
fn bot_reads_only_consented_results_in_its_exact_origin_without_an_effect() {
    for consent in [false, true] {
        let (c, clock, runtime, reference) = setup();
        let id = reference.id;
        let mut cfg = configuration(reference);
        cfg.arguments = json!({"private_argument":"DO_NOT_EXPOSE_ARGUMENTS"});
        if consent {
            cfg.delivery = Some(runtime.notice_origin(None).unwrap());
        }
        let policy = c.configure(cfg).unwrap();
        clock.fetch_add(60, Ordering::SeqCst);
        c.step().unwrap();
        let run = runs(&c, policy["id"].as_str().unwrap())["runs"][0]["id"]
            .as_i64()
            .unwrap();
        let origin = crate::proposals::Origin {
            conversation_id: 3,
            epoch: 1,
        };
        let result = c.0.store.execution_for_origin(run, &origin);
        if consent {
            let result = result.unwrap();
            assert_eq!(result["read_only"], true);
            assert_eq!(result["execution"]["state"], "succeeded");
            assert!(
                result["execution"]["output"]
                    .to_string()
                    .contains("fixture-result")
            );
            assert!(!result.to_string().contains("DO_NOT_EXPOSE_ARGUMENTS"));
        } else {
            assert!(result.is_err());
        }
        for wrong in [
            crate::proposals::Origin {
                conversation_id: 4,
                epoch: 1,
            },
            crate::proposals::Origin {
                conversation_id: 3,
                epoch: 2,
            },
        ] {
            assert!(c.0.store.execution_for_origin(run, &wrong).is_err());
        }
        let mut registry = gennie_mcp::Registry::default();
        let stored = origin.clone();
        registry
            .register(crate::tools::tools_with_origin(
                c.0.store.clone(),
                Arc::new(move || Ok(Some(stored.clone()))),
            ))
            .unwrap();
        let response=registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_execution_get","arguments":{"run_id":run}}})).unwrap();
        assert_eq!(response["result"]["isError"], !consent);
        assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
        assert!(!c.step().unwrap());
        assert_eq!(policies(&c, id)["policies"][0]["enabled"], true);
    }
}

#[test]
fn scoped_activity_discovers_quiet_results_with_paging_without_bodies_or_reused_id_leaks() {
    let (c, clock, runtime, reference) = setup();
    let id = reference.id;
    let mut cfg = configuration(reference);
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    c.configure(cfg).unwrap();
    for _ in 0..3 {
        clock.fetch_add(60, Ordering::SeqCst);
        c.step().unwrap();
    }
    let origin = crate::proposals::Origin {
        conversation_id: 3,
        epoch: 1,
    };
    let mut before = None;
    for expected in (1..=3).rev() {
        let page =
            c.0.store
                .execution_activity_for_origin(id, &origin, before, 1)
                .unwrap();
        assert_eq!(page["executions"].as_array().unwrap().len(), 1);
        assert_eq!(page["executions"][0]["run_id"], expected);
        assert_eq!(page["executions"][0]["has_output"], true);
        assert!(!page.to_string().contains("fixture-result"));
        assert!(page["executions"][0].get("arguments").is_none());
        before = page["next_before"].as_i64();
        assert_eq!(before.is_some(), expected > 1);
    }
    for wrong in [
        crate::proposals::Origin {
            conversation_id: 4,
            epoch: 1,
        },
        crate::proposals::Origin {
            conversation_id: 3,
            epoch: 2,
        },
    ] {
        assert!(
            c.0.store
                .execution_activity_for_origin(id, &wrong, None, 20)
                .unwrap()["executions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        c.0.store
            .execution_activity_for_origin(id, &origin, Some(0), 20)
            .is_err()
    );
    assert!(
        c.0.store
            .execution_activity_for_origin(id, &origin, None, 51)
            .is_err()
    );
    let mut registry = gennie_mcp::Registry::default();
    let saved = origin.clone();
    registry
        .register(crate::tools::tools_with_origin(
            c.0.store.clone(),
            Arc::new(move || Ok(Some(saved.clone()))),
        ))
        .unwrap();
    let result=registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_execution_list","arguments":{"assignment_id":id,"limit":2}}})).unwrap();
    assert_eq!(result["result"]["isError"], false);
    let body: Value =
        serde_json::from_str(result["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["executions"].as_array().unwrap().len(), 2);
    let old = c.0.store.get(id).unwrap();
    c.0.store.delete(id, old.revision).unwrap();
    let fresh =
        c.0.store
            .create("replacement-activity", old.document)
            .unwrap();
    assert_eq!(fresh.id, id);
    assert_ne!(fresh.instance_id, old.instance_id);
    assert!(
        c.0.store
            .execution_activity_for_origin(fresh.id, &origin, None, 20)
            .unwrap()["executions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 3);
}
#[test]
fn notices_are_opt_in_and_lost_receipts_do_not_repeat_execution() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference);
    let mut wrong = runtime.notice_origin(None).unwrap();
    wrong.epoch = 2;
    cfg.delivery = Some(wrong);
    assert!(c.configure(cfg.clone()).is_err());
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    let policy = c.configure(cfg).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    c.step().unwrap();
    let notice = c.0.store.next_execution_notice().unwrap().unwrap();
    assert_eq!(notice.state, "succeeded");
    assert_eq!(notice.policy_id, policy["id"]);
    assert!(
        notice
            .observation
            .as_deref()
            .unwrap()
            .contains("fixture-result")
    );
    runtime.lose_ack.store(true, Ordering::SeqCst);
    assert!(c.flush_notice().is_err());
    assert!(c.0.store.next_execution_notice().unwrap().is_some());
    assert!(c.flush_notice().unwrap());
    assert!(!c.flush_notice().unwrap());
    assert_eq!(runtime.accepted.lock().unwrap().len(), 1);
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    // Un fallo de entrega no es motivo para volver a enviar el efecto.
    assert!(!c.step().unwrap());
    let (silent, clock, _, reference) = setup();
    silent.configure(configuration(reference)).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    silent.step().unwrap();
    assert!(silent.0.store.next_execution_notice().unwrap().is_none());
}
#[test]
fn notice_and_result_confirm_together_and_recovery_reports_uncertainty_once() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference);
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    c.configure(cfg).unwrap();
    c.0.store.db.with_connection(|db| {db.execute_batch("CREATE TRIGGER refuse_notice BEFORE INSERT ON assignment_execution_notices BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    clock.fetch_add(60, Ordering::SeqCst);
    assert!(c.step().is_err());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
    assert!(c.0.store.next_execution_notice().unwrap().is_none());
    c.0.store
        .db
        .with_connection(|db| {
            db.execute_batch("DROP TRIGGER refuse_notice")?;
            Ok(())
        })
        .unwrap();
    c.0.store.recover_executions().unwrap();
    c.0.store.recover_executions().unwrap();
    let notice = c.0.store.next_execution_notice().unwrap().unwrap();
    assert_eq!(notice.state, "uncertain");
    assert_eq!(notice.code, "interrupted_after_dispatch");
    assert!(c.flush_notice().unwrap());
    assert!(!c.flush_notice().unwrap());
    clock.fetch_add(600, Ordering::SeqCst);
    assert!(!c.step().unwrap());
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 1);
}
#[test]
fn bot_unavailability_retains_results_without_stopping_authorized_future_occurrences() {
    let (c, clock, runtime, reference) = setup();
    let mut cfg = configuration(reference);
    cfg.delivery = Some(runtime.notice_origin(None).unwrap());
    c.configure(cfg).unwrap();
    runtime.notice_down.store(true, Ordering::SeqCst);
    for _ in 0..2 {
        clock.fetch_add(60, Ordering::SeqCst);
        c.step().unwrap();
        assert!(c.flush_notice().is_err());
    }
    assert_eq!(runtime.effects.load(Ordering::SeqCst), 2);
    runtime.notice_down.store(false, Ordering::SeqCst);
    assert!(c.flush_notice().unwrap());
    assert!(c.flush_notice().unwrap());
    assert!(!c.flush_notice().unwrap());
    assert_eq!(runtime.accepted.lock().unwrap().len(), 2);
}
