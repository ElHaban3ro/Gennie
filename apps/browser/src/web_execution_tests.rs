use super::*;
use gennie_db::Database;

fn db() -> Database {
    let db = Database::memory().unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    db
}
fn metadata() -> Value {
    json!({"plugin_id":"fixture","plugin_name":"Prueba","function_name":"send","revision":1})
}
#[test]
fn journal_recovers_gates_exact_reviews_and_preserves_audit_without_payloads() {
    let db = db();
    let store = attempts::Store(db.clone());
    let first = store.reserve(&metadata(), "verify", Some("job_1")).unwrap();
    assert!(store.reserve(&metadata(), "run", None).is_err());
    store.recover().unwrap();
    assert_eq!(
        store.page("fixture", None).unwrap()["attempts"][0]["reason"],
        "interrupted"
    );
    for (plugin, id, revision, resolution) in [
        ("other", 1, 1, "applied"),
        ("fixture", 2, 1, "applied"),
        ("fixture", 1, 2, "applied"),
        ("fixture", 1, 1, "unknown"),
    ] {
        assert!(store.reconcile(plugin, id, revision, resolution).is_err());
    }
    store
        .reconcile("fixture", first["id"].as_i64().unwrap(), 1, "applied")
        .unwrap();
    assert!(store.reconcile("fixture", 1, 1, "not_applied").is_err());
    assert_eq!(store.page("fixture", None).unwrap()["total"], 0);
    assert_eq!(
        store.reviews("fixture").unwrap()[0]["resolution"],
        "applied"
    );
    let second = store.reserve(&metadata(), "run", None).unwrap();
    assert!(second["id"].as_i64() > first["id"].as_i64());
    store
        .finish(second["id"].as_i64().unwrap(), "reported_success")
        .unwrap();
    assert_eq!(store.page("fixture", None).unwrap()["total"], 0);
    db.close().unwrap();
}
#[test]
fn another_function_can_run_while_mutations_keep_the_pending_guard() {
    let db = db();
    let store = attempts::Store(db.clone());
    let first = store.reserve(&metadata(), "run", None).unwrap();
    store
        .finish(first["id"].as_i64().unwrap(), "reported_failure")
        .unwrap();
    let mut other = metadata();
    other["function_name"] = json!("read_other");
    let second = store.reserve(&other, "run", None).unwrap();
    store
        .finish(second["id"].as_i64().unwrap(), "reported_success")
        .unwrap();
    assert!(store.pending("fixture", Some("send")).unwrap().is_some());
    assert!(
        store
            .pending("fixture", Some("read_other"))
            .unwrap()
            .is_none()
    );
    assert!(store.pending("fixture", None).unwrap().is_some());
    assert!(store.reserve(&metadata(), "verify", Some("job_2")).is_err());
    db.close().unwrap();
}

#[test]
fn catalogue_mutation_and_review_cannot_cross_an_active_web_execution() {
    let (controller, db, _) = fixture();
    let execution = controller.inner.web_execution_gate.lock().unwrap();
    assert!(
        controller
            .reconcile_web("fixture", 1, 1, "not_applied")
            .is_err()
    );
    let result =
        controller.call(json!({"method":"plugins","action":"save","manifest":{"id":"fixture"}}));
    assert!(result.unwrap_err().is::<BusyError>());
    drop(execution);
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn journal_reservation_completion_and_review_failures_keep_a_safe_gate() {
    let db = db();
    let store = attempts::Store(db.clone());
    db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_reserve BEFORE INSERT ON browser_web_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(store.reserve(&metadata(), "run", None).is_err());
    assert_eq!(store.page("fixture", None).unwrap()["total"], 0);
    db.with_connection(|c|{c.execute_batch("DROP TRIGGER reject_reserve; CREATE TRIGGER reject_finish BEFORE UPDATE ON browser_web_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    let attempt = store.reserve(&metadata(), "run", None).unwrap();
    assert!(
        store
            .finish(attempt["id"].as_i64().unwrap(), "reported_success")
            .is_err()
    );
    assert!(store.reconcile("fixture", 1, 1, "not_applied").is_err());
    assert!(store.reserve(&metadata(), "verify", Some("job_2")).is_err());
    assert_eq!(store.page("fixture", None).unwrap()["total"], 1);
    db.close().unwrap();
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Controller, Database, Directory) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let db = db();
    let root = std::env::temp_dir().join(format!(
        "gennie-web-journal-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let controller = BrowserModule::new(root.clone())
        .with_database(db.clone())
        .controller();
    controller.inner.enabled.store(true, Ordering::Release);
    controller.set_automation_allowed(true);
    (controller, db, Directory(root))
}

#[test]
fn pages_reopen_and_keep_reviewed_history_without_reusing_ids() {
    let (_, memory, dir) = fixture();
    memory.close().unwrap();
    let file = dir.0.join("pages.sqlite3");
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    db.with_connection(|c|{
        let tx=c.transaction()?;
        for i in 1..=120{tx.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,operation,state,reason) VALUES('fixture','Prueba',?1,1,'run','uncertain','interrupted')",[format!("function_{i}")])?;}
        tx.commit()?;Ok(())
    }).unwrap();
    let store = attempts::Store(db.clone());
    let first = store.page("fixture", None).unwrap();
    assert_eq!(first["attempts"].as_array().unwrap().len(), 50);
    assert_eq!(first["next_cursor"], 50);
    store.reconcile("fixture", 1, 1, "applied").unwrap();
    store.reconcile("fixture", 51, 1, "not_applied").unwrap();
    db.close().unwrap();
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let store = attempts::Store(db.clone());
    let second = store.page("fixture", Some(50)).unwrap();
    assert_eq!(second["total"], 118);
    assert_eq!(second["attempts"][0]["id"], 52);
    assert_eq!(second["next_cursor"], 101);
    let last = store.page("fixture", Some(101)).unwrap();
    assert_eq!(last["attempts"].as_array().unwrap().len(), 19);
    assert_eq!(last["next_cursor"], Value::Null);
    assert_eq!(store.reviews("fixture").unwrap().len(), 2);
    db.with_connection(|c| {
        let count: i64 = c.query_row("SELECT count(*) FROM browser_web_attempts", [], |r| {
            r.get(0)
        })?;
        assert_eq!(count, 120);
        let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        assert_eq!(integrity, "ok");
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
}

#[test]
fn mismatched_preflight_never_reserves_or_sends_an_effect() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT.replace("\"plugin_id\":\"fixture\"", "\"plugin_id\":\"other\"");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    assert!(controller.automate(request()).is_err());
    assert!(!log.exists());
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    assert_eq!(controller.web_attempts("other", None).unwrap()["total"], 0);
    controller.stop().unwrap();
    db.close().unwrap();
}
const SCRIPT: &str = r#"while IFS= read -r line; do
case "$line" in
*'"action":"preflight"'*) printf '%s\n' '{"status":"ready","executed":false,"plugin_id":"fixture","plugin_name":"Prueba","function_name":"send","revision":1}';;
*) printf '%s\n' 'call' >> "$GENNIE_WEB_FIXTURE_LOG"; printf '%s\n' '{"status":"result_uncertain","message":"PRIVATE_FAILURE"}';;
esac
done"#;
fn request() -> Value {
    json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{"secret":"PRIVATE_ARGUMENT"}})
}
#[test]
fn cancelling_sent_web_calls_and_module_shutdown_keep_uncertainty_without_resending() {
    use std::time::{Duration, Instant};
    for shutdown in [false, true] {
        let (controller, db, dir) = fixture();
        let log = dir.0.join("calls");
        // Lee hasta EOF sin responder después del efecto; no crea nietos ni usa red.
        let script = SCRIPT.replace(
            "printf '%s\\n' '{\"status\":\"result_uncertain\",\"message\":\"PRIVATE_FAILURE\"}'",
            "while IFS= read -r ignored; do :; done; exit 0",
        );
        *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
        let context = gennie_mcp::CallContext::default();
        let task_controller = controller.clone();
        let task_context = context.clone();
        let task = std::thread::spawn(move || {
            task_controller.automate_with_context(request(), task_context)
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while !log.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let sent = log.exists();
        let started = Instant::now();
        if shutdown {
            controller.stop().unwrap();
        } else {
            context.cancel();
        }
        let result = task.join().unwrap().unwrap();
        assert!(sent, "No llegó el efecto del fixture");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(result["status"], "result_uncertain");
        assert_eq!(result["attempt"]["reason"], "transport_error");
        assert!(controller.inner.session.lock().unwrap().is_none());
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
        if shutdown {
            assert!(
                !context.is_cancelled(),
                "El cierre del módulo no cancela el contexto ajeno"
            );
            controller.inner.stopping.store(false, Ordering::Release);
            controller.inner.enabled.store(true, Ordering::Release);
        }
        assert_eq!(
            controller.automate(request()).unwrap()["status"],
            "review_required"
        );
        controller
            .reconcile_web("fixture", 1, 1, "applied")
            .unwrap();
        // La llamada de un consumidor nuevo sigue disponible sin repetir la función revisada.
        *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
            "while IFS= read -r line; do printf '%s\\n' '{\"ok\":true}'; done",
            &log,
        ));
        assert_eq!(
            controller.automate(json!({"method":"snapshot"})).unwrap()["ok"],
            true
        );
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
        controller.stop().unwrap();
        db.close().unwrap();
    }
}
#[test]
fn controller_persists_before_send_blocks_candidates_and_review_never_resends() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(SCRIPT, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["review_required"], true);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["attempts"][0]["active"],
        false
    );
    for method in ["plugin_run", "candidate", "plugins"] {
        let mut next = request();
        next["method"] = json!(method);
        if method == "plugins" {
            next["action"] = json!("delete");
        }
        assert_eq!(controller.call(next).unwrap()["status"], "review_required");
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    let guard = controller.inner.session.lock().unwrap();
    assert!(
        controller
            .reconcile_web("fixture", 1, 1, "not_applied")
            .is_err()
    );
    drop(guard);
    controller
        .reconcile_web("fixture", 1, 1, "not_applied")
        .unwrap();
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    let stored = attempts::Store(db.clone()).reviews("fixture").unwrap();
    assert!(!json!(stored).to_string().contains("PRIVATE"));
    assert_eq!(
        controller.automate(request()).unwrap()["review_required"],
        true
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\ncall\n");
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn reservation_failure_prevents_effect_and_transport_failure_removes_dead_session() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT.replace(
        "printf '%s\\n' '{\"status\":\"result_uncertain\",\"message\":\"PRIVATE_FAILURE\"}'",
        "exit 0",
    );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_reserve BEFORE INSERT ON browser_web_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;Ok(())}).unwrap();
    assert!(controller.automate(request()).is_err());
    assert!(!log.exists());
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_reserve")?;
        Ok(())
    })
    .unwrap();
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["status"], "result_uncertain");
    assert!(controller.inner.session.lock().unwrap().is_none());
    assert_eq!(
        controller.automate(request()).unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Requiere Node instalado; receta/contexto ficticios y SQLite propios, sin navegador ni red"]
fn node_recipe_effect_is_counted_once_across_unknown_result_and_sqlite_reopen() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {appendFile,access} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const store=createWebPlugins({directory:path.join(root,'recipes'),getContext:async()=>({request:{get:async()=>{
  await appendFile(path.join(root,'effects'),'call\n');
  return {status:()=>200,headers:()=>({'content-type':'application/json'}),dispose:async()=>{},body:async()=>{
    try{await access(path.join(root,'allow'));}catch{throw new Error('PRIVATE_BODY_FAILURE');}
    return Buffer.from('[{"ready":true}]');
  }};
}}})});
if(!(await store.dispatch({action:'get',plugin_id:'fixture'})).plugin){await store.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Prueba',origin:'https://fixture.invalid',functions:[{name:'send',description:'Consulta ficticia',parameters:{},transport:'http',path:'/synthetic',result_type:'array',result_pointer:''}]}});}
for await(const line of createInterface({input:process.stdin,crlfDelay:Infinity})){
  try{console.log(JSON.stringify(await store.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'Error fijo del fixture'}));}
}
"#;
    let (_, memory, dir) = fixture();
    memory.close().unwrap();
    let file = dir.0.join("journal.sqlite3");
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let module = BrowserModule::new(dir.0.clone()).with_database(db.clone());
    let controller = module.controller();
    controller.inner.enabled.store(true, Ordering::Release);
    controller.set_automation_allowed(true);
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let request = json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{}});
    let result = controller.automate(request.clone()).unwrap();
    assert_eq!(result["review_required"], true);
    assert_eq!(
        std::fs::read_to_string(dir.0.join("effects")).unwrap(),
        "call\n"
    );
    controller.stop().unwrap();
    db.close().unwrap();
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let module = BrowserModule::new(dir.0.clone()).with_database(db.clone());
    let controller = module.controller();
    attempts::Store(db.clone()).recover().unwrap();
    controller.inner.enabled.store(true, Ordering::Release);
    controller.set_automation_allowed(true);
    assert_eq!(
        controller.automate(request.clone()).unwrap()["status"],
        "review_required"
    );
    assert!(controller.inner.session.lock().unwrap().is_none());
    controller
        .reconcile_web("fixture", 1, 1, "not_applied")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.0.join("effects")).unwrap(),
        "call\n"
    );
    std::fs::write(dir.0.join("allow"), "").unwrap();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let output = controller.automate(request).unwrap();
    assert_eq!(output["status"], "ok");
    assert_eq!(output["review_required"], false);
    assert_eq!(output["data"], json!([{"ready":true}]));
    assert_eq!(
        std::fs::read_to_string(dir.0.join("effects")).unwrap(),
        "call\ncall\n"
    );
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn confirmed_delete_allows_pending_without_resolving_or_replaying() {
    let (controller, db, dir) = fixture();
    let store = attempts::Store(db.clone());
    let attempt = store.reserve(&metadata(), "run", None).unwrap();
    store
        .finish(attempt["id"].as_i64().unwrap(), "reported_failure")
        .unwrap();
    let other =
        json!({"plugin_id":"other","plugin_name":"Otra","function_name":"read","revision":1});
    let other_attempt = store.reserve(&other, "run", None).unwrap();
    store
        .finish(other_attempt["id"].as_i64().unwrap(), "reported_failure")
        .unwrap();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do case \"$line\" in *'\"action\":\"get\"'*) printf '%s\\n' '{\"plugin\":{\"revision\":1}}';; *) printf '%s\\n' 'call' >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"status\":\"deleted\"}';; esac; done",
        &log,
    ));
    let deleted = controller
        .delete_plugin_from_user("fixture", 1, || Ok(()))
        .unwrap();
    assert_eq!(deleted["status"], "deleted");
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        1
    );
    assert_eq!(controller.web_attempts("other", None).unwrap()["total"], 1);
    assert_eq!(
        controller.automate(request()).unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn deletion_never_cancels_any_job_when_browser_busy_or_revision_changed() {
    use std::sync::atomic::AtomicBool;
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do printf '%s\\n' '{\"plugin\":{\"revision\":2}}'; done",
        &log,
    ));
    let invoked = AtomicBool::new(false);
    let guard = controller.inner.session.lock().unwrap();
    let busy = controller.delete_plugin_from_user("fixture", 1, || {
        invoked.store(true, Ordering::Release);
        Ok(())
    });
    assert!(busy.unwrap_err().is::<BusyError>());
    assert!(!invoked.load(Ordering::Acquire));
    drop(guard);
    let stale = controller
        .delete_plugin_from_user("fixture", 1, || {
            invoked.store(true, Ordering::Release);
            Ok(())
        })
        .unwrap();
    assert_eq!(stale["status"], "revision_conflict");
    assert!(!invoked.load(Ordering::Acquire));
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn busy_cleanup_closes_only_owned_task_page_before_next_operation() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"status\":\"closed\",\"ok\":true}'; done",
        &log,
    ));
    let held = controller.inner.session.lock().unwrap();
    controller.close_owned_task_page("37", 0).unwrap();
    assert!(!log.exists());
    drop(held);
    controller.call(json!({"method":"snapshot"})).unwrap();
    let lines = std::fs::read_to_string(&log).unwrap();
    let parsed: Vec<Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["method"], "task_page");
    assert_eq!(parsed[0]["action"], "close");
    assert_eq!(parsed[0]["page_id"], "37");
    assert_eq!(parsed[1]["method"], "snapshot");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn deletion_flushes_deferred_owned_page_before_removing_plugin() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = r#"while IFS= read -r line; do
printf '%s\n' "$line" >> "$GENNIE_WEB_FIXTURE_LOG"
case "$line" in
*'"action":"get"'*) printf '%s\n' '{"plugin":{"revision":1}}';;
*'"action":"delete"'*) printf '%s\n' '{"status":"deleted"}';;
*) printf '%s\n' '{"status":"closed"}';;
esac
done"#;
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(script, &log));
    let busy = controller.inner.session.lock().unwrap();
    controller.close_owned_task_page("42", 0).unwrap();
    drop(busy);
    let result = controller
        .delete_plugin_from_user("fixture", 1, || Ok(()))
        .unwrap();
    assert_eq!(result["status"], "deleted");
    let lines = std::fs::read_to_string(&log).unwrap();
    let requests: Vec<Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["method"], "task_page");
    assert_eq!(requests[0]["page_id"], "42");
    assert_eq!(requests[1]["action"], "get");
    assert_eq!(requests[2]["action"], "delete");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn old_task_page_cleanup_never_closes_reused_id_in_new_session() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"ok\":true}'; done",
        &log,
    ));
    controller
        .inner
        .session_generation
        .store(1, Ordering::Release);
    let held = controller.inner.session.lock().unwrap();
    controller.close_owned_task_page("37", 1).unwrap();
    drop(held);
    controller
        .inner
        .session_generation
        .store(2, Ordering::Release);
    controller.call(json!({"method":"snapshot"})).unwrap();
    let lines = std::fs::read_to_string(&log).unwrap();
    let requests: Vec<Value> = lines
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "snapshot");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn task_creation_carries_session_identity_for_its_cleanup() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do printf '%s\\n' '{\"page_id\":\"37\"}'; done",
        &log,
    ));
    controller
        .inner
        .session_generation
        .store(5, Ordering::Release);
    let page = controller
        .call(json!({"method":"task_page","action":"create"}))
        .unwrap();
    assert_eq!(page["page_id"], "37");
    assert_eq!(page["session_generation"], 5);
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn scoped_tools_inject_trusted_origin_and_release_without_starting_another_process() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("scoped-calls");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(
        "while IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"open\":true}'; done",
        &log,
    ));
    let origin = std::sync::Arc::new(|| Ok(Some("chat:thread-a".to_owned())));
    let tools = scoped_contextual_tools(controller.clone(), origin);
    let snapshot = tools
        .iter()
        .find(|tool| tool.definition["name"] == "browser_snapshot")
        .unwrap();
    (snapshot.call)(json!({}), gennie_mcp::CallContext::default()).unwrap();
    assert!(
        (snapshot.call)(
            json!({"scope":"chat:other"}),
            gennie_mcp::CallContext::default()
        )
        .is_err()
    );
    controller.release_scope("chat:thread-a").unwrap();
    let requests: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["scope"], "chat:thread-a");
    assert_eq!(
        requests[1],
        json!({"method":"scope_end","scope":"chat:thread-a"})
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn scoped_requests_overlap_and_cancelling_one_preserves_the_other() {
    let (controller, db, _) = fixture();
    let script = r#"
import { createInterface } from 'node:readline';
for await (const line of createInterface({input:process.stdin})) {
  const {request_id,request} = JSON.parse(line);
  setTimeout(() => console.log(JSON.stringify({request_id,result:{scope:request.scope}})), request.scope === 'chat:slow' ? 800 : 0);
}

"#;
    *controller.inner.session.lock().unwrap() = Some(process::Session::multiplex_fixture(script));
    let cancel = gennie_mcp::CallContext::default();
    let slow_context = cancel.clone();
    let slow_controller = controller.clone();
    let (started, entered) = std::sync::mpsc::channel();
    let slow = std::thread::spawn(move || {
        started.send(()).unwrap();
        slow_controller.call_with_context(
            json!({"method":"snapshot","scope":"chat:slow"}),
            slow_context,
        )
    });
    entered.recv().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(controller.is_busy());
    let began = std::time::Instant::now();
    let fast = controller
        .call(json!({"method":"snapshot","scope":"chat:fast"}))
        .unwrap();
    assert_eq!(fast["scope"], "chat:fast");
    assert!(began.elapsed() < std::time::Duration::from_millis(600));
    cancel.cancel();
    assert!(
        slow.join()
            .unwrap()
            .unwrap_err()
            .is::<gennie_mcp::Cancelled>()
    );
    let next = controller
        .call(json!({"method":"snapshot","scope":"chat:fast"}))
        .unwrap();
    assert_eq!(next["scope"], "chat:fast");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Chromium real con perfil temporal y servidor HTTP local; no usa el perfil personal"]
fn real_chromium_scopes_overlap_and_cancel_independently() {
    use std::io::{Read, Write};
    use std::os::unix::fs::symlink;
    use std::sync::{Arc, atomic::AtomicBool};
    use std::time::{Duration, Instant};

    let (controller, db, dir) = fixture();
    let installed = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser");
    symlink(
        installed.join("runtime-patchright-1.64.2"),
        dir.0.join("runtime-patchright-1.64.2"),
    )
    .unwrap();
    symlink(installed.join("browsers"), dir.0.join("browsers")).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let stop_server = stopping.clone();
    let (slow_seen, slow_started) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut clients = Vec::new();
        while !stop_server.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let slow_seen = slow_seen.clone();
                    clients.push(std::thread::spawn(move || {
                        let mut bytes = [0; 8192];
                        let count = stream.read(&mut bytes).unwrap_or(0);
                        if String::from_utf8_lossy(&bytes[..count]).starts_with("GET /slow ") {
                            let _ = slow_seen.send(());
                            std::thread::sleep(Duration::from_millis(1200));
                        }
                        let body = b"<title>Prueba local</title>";
                        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        let _ = stream.write_all(body);
                    }));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
        for client in clients {
            let _ = client.join();
        }
    });
    let a = controller
        .call(json!({"method":"open","scope":"chat:a"}))
        .unwrap();
    let b = controller
        .call(json!({"method":"open","scope":"chat:b"}))
        .unwrap();
    assert_ne!(a["page_id"], b["page_id"]);
    let cancel = gennie_mcp::CallContext::default();
    let slow_context = cancel.clone();
    let slow_controller = controller.clone();
    let slow = std::thread::spawn(move || {
        slow_controller.call_with_context(
            json!({"method":"navigate","scope":"chat:a","url":format!("http://{address}/slow")}),
            slow_context,
        )
    });
    if slow_started.recv_timeout(Duration::from_secs(10)).is_err() {
        panic!(
            "La navegación lenta no llegó al servidor: {:?}",
            slow.join()
        );
    }
    let began = Instant::now();
    let fast = controller
        .call(json!({"method":"navigate","scope":"chat:b","url":format!("http://{address}/fast")}))
        .unwrap();
    assert_eq!(fast["status"], 200);
    assert!(began.elapsed() < Duration::from_millis(1000));
    cancel.cancel();
    assert!(
        slow.join()
            .unwrap()
            .unwrap_err()
            .is::<gennie_mcp::Cancelled>()
    );
    assert_eq!(
        controller
            .call(json!({"method":"snapshot","scope":"chat:b"}))
            .unwrap()["open"],
        true
    );
    controller.release_scope("chat:a").unwrap();
    controller.release_scope("chat:b").unwrap();
    controller.stop().unwrap();
    stopping.store(true, Ordering::Release);
    server.join().unwrap();
    db.close().unwrap();
}
