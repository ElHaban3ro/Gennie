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
