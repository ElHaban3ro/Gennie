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
fn gmail_evidence_is_atomic_and_resolution_requires_uncertain_attempt() {
    let db = db();
    let store = attempts::Store(db.clone());
    let mut input = metadata();
    input["gmail_evidence"] =
        json!({"account_hash":"a".repeat(64),"thread_id":"abcdef","unread_before":true});
    let attempt = store.reserve(&input, "run", None).unwrap();
    let id = attempt["id"].as_i64().unwrap();
    assert!(store.reconcile_gmail(id, "applied").is_err());
    store.finish(id, "reported_failure").unwrap();
    assert_eq!(
        store.gmail_evidence(id).unwrap().unwrap()["unread_before"],
        true
    );
    store.reconcile_gmail(id, "applied").unwrap();
    assert!(store.gmail_evidence(id).unwrap().is_none());
    assert!(store.reconcile_gmail(id, "not_applied").is_err());
    assert_eq!(
        store.reviews("fixture").unwrap()[0]["resolution"],
        "applied"
    );
    assert_eq!(
        store.reviews("fixture").unwrap()[0]["automatic_resolution"],
        "applied"
    );
    db.close().unwrap();
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
fn read_timeout_explains_why_an_attempt_is_pending_without_saving_private_data() {
    let db = db();
    let store = attempts::Store(db.clone());
    let attempt = store.reserve(&metadata(), "verify", Some("job_1")).unwrap();
    store.finish_with_diagnostic(attempt["id"].as_i64().unwrap(), "reported_failure", Some(&json!({"code":"timeout","phase":"read","stage":"PRIVATE_CONTENT","read_attempts":3}))).unwrap();
    let pending = store.page("fixture", None).unwrap();
    assert_eq!(pending["attempts"][0]["failure_code"], "timeout");
    assert_eq!(pending["attempts"][0]["failure_phase"], "read");
    assert!(!pending.to_string().contains("PRIVATE_CONTENT"));
    db.close().unwrap();
}

#[test]
fn historical_attempts_upgrade_without_inventing_a_diagnostic() {
    let db = Database::memory().unwrap();
    db.migrate("browser", &attempts::MIGRATIONS[..1]).unwrap();
    db.with_connection(|c| { c.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,operation,state,reason) VALUES('fixture','Prueba','read',1,'verify','uncertain','reported_failure')", [])?; Ok(()) }).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let page = attempts::Store(db.clone()).page("fixture", None).unwrap();
    assert_eq!(page["attempts"][0]["failure_code"], Value::Null);
    assert_eq!(page["attempts"][0]["failure_phase"], Value::Null);
    assert_eq!(page["attempts"][0]["state"], "uncertain");
    db.close().unwrap();
}

#[test]
fn result_type_diagnostic_survives_upgrade_without_changing_previous_attempts() {
    let db = Database::memory().unwrap();
    db.migrate("browser", &attempts::MIGRATIONS[..3]).unwrap();
    db.with_connection(|c| { c.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,operation,state,reason,failure_code,failure_phase) VALUES('fixture','Prueba','old',1,'verify','uncertain','reported_failure','timeout','read')", [])?; Ok(()) }).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let store = attempts::Store(db.clone());
    let attempt = store.reserve(&metadata(), "verify", None).unwrap();
    store
        .finish_with_diagnostic(
            attempt["id"].as_i64().unwrap(),
            "reported_failure",
            Some(&json!({"code":"result_type_mismatch","phase":"execute"})),
        )
        .unwrap();
    let page = store.page("fixture", None).unwrap();
    let rows = page["attempts"].as_array().unwrap();
    let old = rows.iter().find(|v| v["tool"] == "old").unwrap();
    let new = rows.iter().find(|v| v["tool"] == "send").unwrap();
    assert_eq!(old["failure_code"], "timeout");
    assert_eq!(old["failure_phase"], "read");
    assert_eq!(new["failure_code"], "result_type_mismatch");
    assert_eq!(new["failure_phase"], "execute");
    assert_eq!(new["state"], "uncertain");
    db.close().unwrap();
}

#[test]
fn failed_controlled_observation_does_not_block_but_unknown_program_still_does() {
    let db = db();
    let store = attempts::Store(db.clone());
    let mut read = metadata();
    read["transport"] = json!("observe");
    let attempt = store.reserve(&read, "verify", None).unwrap();
    store
        .finish_with_diagnostic(attempt["id"].as_i64().unwrap(), "reported_failure", None)
        .unwrap();
    assert!(store.pending("fixture", Some("send")).unwrap().is_none());
    let another = store.reserve(&metadata(), "verify", None).unwrap();
    store
        .finish_with_diagnostic(another["id"].as_i64().unwrap(), "reported_failure", None)
        .unwrap();
    assert!(store.pending("fixture", Some("send")).unwrap().is_some());
    db.close().unwrap();
}

#[test]
fn unconfirmed_closure_preserves_uncertainty_and_quarantines_program() {
    let (controller, db, dir) = fixture();
    let store = attempts::Store(db.clone());
    let attempt = store.reserve(&metadata(), "verify", Some("job_1")).unwrap();
    let id = attempt["id"].as_i64().unwrap();
    store
        .finish_with_diagnostic(
            id,
            "reported_failure",
            Some(&json!({"code":"timeout","phase":"read"})),
        )
        .unwrap();
    assert!(store.close_unconfirmed("fixture", id, 2).is_err());
    store.close_unconfirmed("fixture", id, 1).unwrap();
    assert!(store.close_unconfirmed("fixture", id, 1).is_err());
    assert_eq!(store.page("fixture", None).unwrap()["total"], 0);
    let review = &store.reviews("fixture").unwrap()[0];
    assert_eq!(review["resolution"], "unknown");
    assert_eq!(review["reason"], "reported_failure");
    assert_eq!(review["failure_code"], "timeout");
    assert_eq!(review["reviewed_at"], Value::Null);
    let log = dir.0.join("calls");
    let program = SCRIPT.replace(
        "\"revision\":1}",
        "\"revision\":1,\"transport\":\"program\"}",
    );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&program, &log));
    let blocked = controller.automate(request()).unwrap();
    assert_eq!(blocked["status"], "quarantined_recipe");
    assert_eq!(blocked["executed"], false);
    assert!(!log.exists());
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn unconfirmed_closure_survives_upgrade_and_reopen() {
    let (_, memory, dir) = fixture();
    memory.close().unwrap();
    let path = dir.0.join("unconfirmed.sqlite3");
    let db = Database::open(&path, 500).unwrap();
    db.migrate("browser", &attempts::MIGRATIONS[..4]).unwrap();
    db.with_connection(|c| { c.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation,state,reason,failure_code,failure_phase) VALUES('fixture','Prueba','send',2,'job_1','verify','uncertain','reported_failure','timeout','read')",[])?; Ok(()) }).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let store = attempts::Store(db.clone());
    assert_eq!(store.page("fixture", None).unwrap()["total"], 1);
    store.close_unconfirmed("fixture", 1, 2).unwrap();
    db.close().unwrap();
    let db = Database::open(&path, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let store = attempts::Store(db.clone());
    assert_eq!(
        store.reviews("fixture").unwrap()[0]["resolution"],
        "unknown"
    );
    assert!(store.quarantined("fixture", "send").unwrap());
    assert!(!store.quarantined("fixture", "other").unwrap());
    db.with_connection(|c| {
        let status: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        assert_eq!(status, "ok");
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
}

#[test]
fn restarted_observation_is_closed_without_changing_other_uncertain_attempts() {
    let db = db();
    let store = attempts::Store(db.clone());
    let mut read = metadata();
    read["transport"] = json!("observe");
    let observation = store.reserve(&read, "verify", None).unwrap();
    let mut program = metadata();
    program["function_name"] = json!("send_other");
    store.reserve(&program, "verify", None).unwrap();
    store.recover().unwrap();
    assert!(store.pending("fixture", Some("send")).unwrap().is_none());
    assert!(
        store
            .pending("fixture", Some("send_other"))
            .unwrap()
            .is_some()
    );
    db.with_connection(|c| {
        let row: (String, String) = c.query_row(
            "SELECT state,reason FROM browser_web_attempts WHERE id=?1",
            [observation["id"].as_i64().unwrap()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        assert_eq!(row, ("completed".into(), "interrupted".into()));
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
}

#[test]
fn controller_reports_failed_observation_without_review_or_automatic_replay() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT
        .replace(
            "\"revision\":1}",
            "\"revision\":1,\"transport\":\"observe\"}",
        )
        .replace("\"result_uncertain\"", "\"site_changed\"");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["status"], "site_changed");
    assert_eq!(result["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn context_failure_before_execution_is_diagnostic_and_does_not_block() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT.replace(
        "\"status\":\"result_uncertain\",\"message\":\"PRIVATE_FAILURE\"",
        "\"status\":\"browser_unavailable\",\"executed\":false,\"diagnostic\":{\"code\":\"timeout\",\"phase\":\"prepare\"}",
    );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["status"], "browser_unavailable", "{result}");
    assert_eq!(result["executed"], false);
    assert_eq!(result["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    assert_eq!(
        controller.automate(request()).unwrap()["status"],
        "browser_unavailable"
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Usa Node real y SQLite temporal; sin navegador ni red"]
fn node_recovers_after_context_failure_without_review() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
let calls=0;
const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>{if(++calls===1){const e=new Error('private');e.name='TimeoutError';throw e;}return {newPage:async()=>({close:async()=>{}})};}});
await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Prueba',origin:'https://example.com',functions:[{name:'send',description:'Consulta',parameters:{},transport:'program',path:'/',result_type:'object',code:'return {ok:true};'}]}});
for await(const line of createInterface({input:process.stdin})) {try{console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'fixture'}));}}
"#;
    let (controller, db, dir) = fixture();
    controller.set_program_allowed(true);
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut call = request();
    call["arguments"] = json!({});
    let first = controller.automate(call.clone()).unwrap();
    assert_eq!(first["status"], "browser_unavailable", "{first}");
    assert_eq!(first["diagnostic"]["code"], "timeout");
    assert_eq!(first["executed"], false);
    assert_eq!(first["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    let second = controller.automate(call).unwrap();
    assert_eq!(second["status"], "ok", "{second}");
    assert_eq!(second["data"]["ok"], true);
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn sequence_claiming_read_phase_stays_blocked() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT
        .replace(
            "\"revision\":1}",
            "\"revision\":1,\"transport\":\"sequence\"}",
        )
        .replace(
            "\"message\":\"PRIVATE_FAILURE\"",
            "\"diagnostic\":{\"phase\":\"read\",\"code\":\"timeout\"}",
        );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["review_required"], true);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        1
    );
    assert_eq!(
        controller.automate(request()).unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "call\n");
    controller.stop().unwrap();
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
fn gmail_evidence_survives_restart_and_auto_review_is_attributed() {
    let (_, memory, dir) = fixture();
    memory.close().unwrap();
    let file = dir.0.join("gmail.sqlite3");
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let mut meta = metadata();
    meta["gmail_evidence"] =
        json!({"account_hash":"c".repeat(64),"thread_id":"abcdef","unread_before":false});
    let id = attempts::Store(db.clone())
        .reserve(&meta, "run", None)
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    db.close().unwrap();
    let db = Database::open(&file, 500).unwrap();
    db.migrate("browser", attempts::MIGRATIONS).unwrap();
    let store = attempts::Store(db.clone());
    store.recover().unwrap();
    assert_eq!(
        store.gmail_evidence(id).unwrap().unwrap()["thread_id"],
        "abcdef"
    );
    store.reconcile_gmail(id, "applied").unwrap();
    let receipt = &store.reviews("fixture").unwrap()[0];
    assert_eq!(receipt["automatic_resolution"], "applied");
    assert!(!receipt.to_string().contains("abcdef"));
    assert!(!receipt.to_string().contains(&"c".repeat(64)));
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
fn controlled_failure_before_first_interaction_does_not_require_review() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = SCRIPT.replace("\"revision\":1}", "\"revision\":1,\"transport\":\"page\"}").replace("{\"status\":\"result_uncertain\",\"message\":\"PRIVATE_FAILURE\"}", "{\"status\":\"execution_failed\",\"executed\":false,\"diagnostic\":{\"phase\":\"prepare\",\"code\":\"program_error\"}}");
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["executed"], false);
    assert_eq!(result["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn legacy_program_is_rejected_before_execution_and_without_attempt() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let program = SCRIPT.replace(
        "\"revision\":1}",
        "\"revision\":1,\"transport\":\"program\"}",
    );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&program, &log));
    let result = controller.automate(request()).unwrap();
    assert_eq!(result["status"], "migration_required");
    assert_eq!(result["executed"], false);
    assert!(!log.exists());
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn legacy_candidate_can_be_saved_for_incremental_migration() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("calls");
    let script = r#"while IFS= read -r line; do printf '%s\n' 'save' >> "$GENNIE_WEB_FIXTURE_LOG"; printf '%s\n' '{"status":"saved"}'; done"#;
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(script, &log));
    let result = controller.call(json!({"method":"candidate","action":"save","candidate_id":"job_1","manifest":{"functions":[{"transport":"program"}]}})).unwrap();
    assert_eq!(result["status"], "saved");
    assert_eq!(std::fs::read_to_string(log).unwrap(), "save\n");
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn gmail_review_uses_saved_evidence_and_never_resends_archive() {
    let (controller, db, dir) = fixture();
    let store = attempts::Store(db.clone());
    let mut meta = metadata();
    meta["transport"] = json!("gmail_archive");
    meta["gmail_evidence"] =
        json!({"account_hash":"a".repeat(64),"thread_id":"abcdef","unread_before":true});
    let attempt = store.reserve(&meta, "run", None).unwrap();
    let id = attempt["id"].as_i64().unwrap();
    store.finish(id, "reported_failure").unwrap();
    let log = dir.0.join("review_calls");
    let script = "while IFS= read -r line; do case \"$line\" in *'\"action\":\"gmail_review\"'*) printf '%s\\n' review >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"status\":\"applied\"}';; *) printf '%s\\n' unexpected >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"status\":\"error\"}';; esac; done";
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(script, &log));
    let result = controller
        .automate(json!({"method":"gmail_reconcile","attempt_id":id}))
        .unwrap();
    assert_eq!(result["status"], "reviewed");
    assert_eq!(result["resolution"], "applied");
    assert_eq!(std::fs::read_to_string(log).unwrap().trim(), "review");
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn gmail_review_keeps_block_for_inconclusive_or_missing_evidence() {
    let (controller, db, dir) = fixture();
    let store = attempts::Store(db.clone());
    let mut meta = metadata();
    meta["gmail_evidence"] =
        json!({"account_hash":"b".repeat(64),"thread_id":"abcdef","unread_before":true});
    let id = store.reserve(&meta, "run", None).unwrap()["id"]
        .as_i64()
        .unwrap();
    store.finish(id, "reported_failure").unwrap();
    let log = dir.0.join("inconclusive_calls");
    let script = "while IFS= read -r line; do printf '%s\\n' call >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{\"status\":\"inconclusive\"}'; done";
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(script, &log));
    assert_eq!(
        controller
            .automate(json!({"method":"gmail_reconcile","attempt_id":id}))
            .unwrap()["status"],
        "review_required"
    );
    assert_eq!(
        controller
            .automate(json!({"method":"gmail_reconcile","attempt_id":id+1}))
            .unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(log).unwrap().lines().count(), 1);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        1
    );
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn gmail_review_of_legacy_attempt_does_not_start_browser() {
    let (controller, db, _) = fixture();
    let store = attempts::Store(db.clone());
    let id = store.reserve(&metadata(), "run", None).unwrap()["id"]
        .as_i64()
        .unwrap();
    store.finish(id, "reported_failure").unwrap();
    let result = controller
        .automate(json!({"method":"gmail_reconcile","attempt_id":id}))
        .unwrap();
    assert_eq!(result["status"], "review_required");
    assert!(controller.inner.session.lock().unwrap().is_none());
    controller.stop().unwrap();
    db.close().unwrap();
}
#[test]
fn successful_automatic_review_reports_a_completed_run() {
    let (controller, db, dir) = fixture();
    let log = dir.0.join("actions");
    let account = "a".repeat(64);
    let script = format!(
        "while IFS= read -r line; do case \"$line\" in *'\"action\":\"preflight\"'*) printf '%s\\n' '{{\"status\":\"ready\",\"executed\":false,\"plugin_id\":\"fixture\",\"plugin_name\":\"Prueba\",\"function_name\":\"send\",\"revision\":1,\"transport\":\"gmail_archive\"}}';; *'\"action\":\"gmail_evidence\"'*) printf '%s\\n' '{{\"status\":\"ready\",\"account_hash\":\"{account}\",\"thread_id\":\"abcdef\",\"unread_before\":true}}';; *'\"action\":\"gmail_review\"'*) printf '%s\\n' '{{\"status\":\"applied\"}}';; *) printf '%s\\n' action >> \"$GENNIE_WEB_FIXTURE_LOG\"; printf '%s\\n' '{{\"status\":\"result_uncertain\"}}';; esac; done"
    );
    *controller.inner.session.lock().unwrap() = Some(process::Session::scripted(&script, &log));
    let mut call = request();
    call["action"] = json!("run");
    call["arguments"] = json!({"id":"abcdef"});
    let result = controller.automate(call).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["automatic_review"], "applied");
    assert_eq!(result["review_required"], false);
    assert_eq!(std::fs::read_to_string(log).unwrap().lines().count(), 1);
    controller.stop().unwrap();
    db.close().unwrap();
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
#[ignore = "Abre Chromium aislado sin red ni cuentas personales"]
fn real_chromium_observation_failure_recovers_through_journal() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {access} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const browser=await chromium.launch({headless:true,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const context=await browser.newContext();let requests=0;
 await context.route('**/*',route=>{requests++;return route.fulfill({status:200,contentType:'text/html',body:'<main>Datos de prueba</main>'});});
 const page=await context.newPage(); await page.goto('https://fixture.invalid/inbox');
 const plugins=createWebPlugins({directory:path.join(root,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Prueba',origin:'https://fixture.invalid',functions:[{name:'send',description:'Leer',parameters:{},transport:'observe',path:'/inbox',selector:'.row',result_type:'array'}]}});
 for await(const line of createInterface({input:process.stdin})) {
  try {
   const request=JSON.parse(line);
   if(request.action==='verify') { try { await access(path.join(root,'allow')); if(!await page.locator('.row').count()) await page.locator('main').evaluate(node=>{let e=document.createElement('span');e.className='row';e.textContent='OK';node.appendChild(e);}); } catch {} }
   const result=await plugins.dispatch(request);
   if(requests!==1) throw new Error('unexpected network');
   console.log(JSON.stringify(result));
  } catch { console.log(JSON.stringify({error:'Error del fixture'})); }
 }
} finally { await browser.close(); }
"#;
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let request = json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{}});
    let first = controller.automate(request.clone()).unwrap();
    assert_eq!(first["status"], "site_changed");
    assert_eq!(first["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    std::fs::write(dir.0.join("allow"), "").unwrap();
    let second = controller.automate(request).unwrap();
    assert_eq!(second["status"], "ok");
    assert_eq!(second["data"], json!(["OK"]));
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Reproduce Gmail con Chromium y SQLite aislados, sin red ni cuentas"]
fn gmail_detail_selector_timeout_preserves_uncertain_navigation() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {appendFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const browser=await chromium.launch({headless:true,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const context=await browser.newContext();
 await context.route('**/*',async route=>{
   if (new URL(route.request().url()).pathname === '/increment') {
    await appendFile(path.join(root,'effect'),'x');
    return route.fulfill({status:200,contentType:'text/plain',body:'ok'});
   }
   await appendFile(path.join(root,'navigation'),'x');
   await route.fulfill({status:200,contentType:'text/html; charset=utf-8',body:'<div role="main"><h2>Conversación ficticia</h2><p>Contenido ficticio</p></div>'});
 });
 const plugins=createWebPlugins({directory:path.join(root,'recipes'),getContext:()=>context});
 for(const [id,selector] of [['fixture','main h2'],['corrected','[role=main] h2']]) {
  const code=`return await runtime.read('read_detail',async()=>{await page.goto(origin+'/mail/u/0/#all/'+args.id);await page.locator('${selector}').first().waitFor({timeout:250});return {id:args.id,texto:await page.locator('[role=main]').innerText()};});`;
  await plugins.dispatch({action:'save',expected_revision:0,manifest:{id,name:'Gmail ficticio',origin:'https://mail.google.com',functions:[{name:'send',description:'Leer detalle',parameters:{id:'string'},transport:'program',path:'/',result_type:'object',code}]}});
 }
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'portal',name:'Portal ficticio',origin:'https://portal.invalid',functions:[{name:'send',description:'Consultar pedido',parameters:{},transport:'program',path:'/',result_type:'object',code:"return await runtime.read('load',async()=>{await page.goto(origin+'/orders');await page.evaluate(()=>fetch('/increment',{method:'POST'}));await page.locator('#missing').waitFor({timeout:250});return {};});"}]}});
 for await(const line of createInterface({input:process.stdin})) {try{console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'Error del fixture'}));}}
} finally { await browser.close(); }
"#;
    let (controller, db, dir) = fixture();
    controller.set_program_allowed(true);
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let request = json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{"id":"abcdef"},"program_allowed":true});
    let first = controller.automate(request.clone()).unwrap();
    assert_eq!(first["status"], "result_uncertain", "{first}");
    assert_eq!(first["diagnostic"]["code"], "timeout");
    assert_eq!(first["diagnostic"]["phase"], "read");
    assert_eq!(first["diagnostic"]["read_attempts"], 1);
    assert_eq!(first["review_required"], true);
    assert_eq!(
        std::fs::read_to_string(dir.0.join("navigation")).unwrap(),
        "x"
    );
    assert_eq!(
        controller.automate(request.clone()).unwrap()["status"],
        "review_required"
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("navigation")).unwrap(),
        "x"
    );
    let mut corrected = request;
    corrected["plugin_id"] = json!("corrected");
    let success = controller.automate(corrected).unwrap();
    assert_eq!(success["status"], "ok", "{success}");
    assert_eq!(
        success["data"]["texto"],
        "Conversación ficticia\n\nContenido ficticio"
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("navigation")).unwrap(),
        "xx"
    );
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        1
    );
    let mut portal = json!({"method":"plugin_run","action":"verify","plugin_id":"portal","function_name":"send","expected_revision":1,"arguments":{}});
    let portal_result = controller.automate(portal.clone()).unwrap();
    assert_eq!(portal_result["diagnostic"]["phase"], "read");
    assert_eq!(portal_result["review_required"], true);
    assert_eq!(std::fs::read_to_string(dir.0.join("effect")).unwrap(), "x");
    portal["action"] = json!("run");
    assert_eq!(
        controller.automate(portal).unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(dir.0.join("effect")).unwrap(), "x");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Chromium aislado y respuesta Gmail ficticia; sin red ni cuentas"]
fn gmail_print_failed_read_recovers_and_does_not_load_mail_resources() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {access,appendFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const browser=await chromium.launch({headless:true,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const browserContext=await browser.newContext();
 browserContext.on('request',()=>appendFile(path.join(root,'network'),'x'));
 const context={newPage:()=>browserContext.newPage(),request:{get:async(url,opts)=>{
  if(url!=='https://mail.google.com/mail/u/0/?view=pt&search=all&th=abcdef'||opts.maxRedirects!==0)throw Error('unexpected request');
  let allowed=true;try{await access(path.join(root,'allow'));}catch{allowed=false;}
  return {status:()=>allowed?200:403,headers:()=>({'content-type':'text/html'}),body:async()=>Buffer.from('<div class="maincontent"><h1>Asunto ficticio</h1><p>Contenido ficticio</p><script>SECRET_SCRIPT</script><img src="https://tracker.invalid/pixel"></div>'),dispose:async()=>{}};
 }}};
 const plugins=createWebPlugins({directory:path.join(root,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Leer',parameters:{id:'string'},transport:'gmail_print',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})){try{console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'fixture'}));}}
} finally {await browser.close();}
"#;
    let (controller, db, dir) = fixture();
    let store = attempts::Store(db.clone());
    let old = store.reserve(&metadata(), "verify", Some("job_1")).unwrap();
    let id = old["id"].as_i64().unwrap();
    store.finish(id, "reported_failure").unwrap();
    store.close_unconfirmed("fixture", id, 1).unwrap();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut call = request();
    call["arguments"] = json!({"id":"abcdef"});
    let first = controller.automate(call.clone()).unwrap();
    assert_eq!(first["status"], "access_denied");
    assert_eq!(first["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    std::fs::write(dir.0.join("allow"), "").unwrap();
    let second = controller.automate(call).unwrap();
    assert_eq!(second["status"], "ok", "{second}");
    assert!(
        second["data"]["texto"]
            .as_str()
            .unwrap()
            .contains("Contenido ficticio")
    );
    assert!(!second.to_string().contains("SECRET_SCRIPT"));
    assert!(!dir.0.join("network").exists());
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

const GMAIL_ARCHIVE_FIXTURE_NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {access,appendFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const browser=await chromium.launch({headless:true,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
let archived=false;
async function exists(name){try{await access(path.join(root,name));return true;}catch{return false;}}
try {
 const context=await browser.newContext();
 await context.exposeFunction('fixtureArchive',async()=>{await appendFile(path.join(root,'clicks'),'x');archived=true;});
 await context.route('**/*',async route=>{
  const failed=await exists('fail_before'),ambiguous=await exists('ambiguous'),delayed=await exists('delayed');
  const body=`<!doctype html><html><body><a href="https://accounts.google.com/SignOutOptions" aria-label="Google Account: Example (example@example.com)">Cuenta</a><main role="main"><div role="grid" style="min-height:40px"></div></main><script>
   const q=decodeURIComponent(location.hash.slice(8)).replace(/\\+/g,' ');
   const inbox=q.startsWith('in:inbox '),show=inbox?!${archived}:${archived}&&!${ambiguous};
   const grid=document.querySelector('[role=grid]');
   if(show){grid.innerHTML='<div role="row" class="zE"><span hidden data-legacy-thread-id="abcdef"></span><div role="checkbox" aria-checked="false">Seleccionar</div></div>';
     document.querySelector('[role=checkbox]').onclick=e=>{e.currentTarget.setAttribute('aria-checked','true');const b=document.querySelector('[aria-label="Archivar"]');if(b)b.style.display='block';};
     if(!${failed}){const b=document.createElement('div');b.setAttribute('role','button');b.setAttribute('aria-label','Archivar');b.style.display='none';b.textContent='Archivar';b.onclick=()=>${delayed} ? setTimeout(()=>window.fixtureArchive(),500) : window.fixtureArchive();document.body.appendChild(b);}
   }else{document.querySelector('main').append('No hay mensajes que coincidan con tus criterios.');}
  </script></body></html>`;
  await route.fulfill({status:200,contentType:'text/html; charset=utf-8',body});
 });
 const plugins=createWebPlugins({directory:path.join(root,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Archivar',parameters:{id:'string'},transport:'gmail_archive',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})){try{console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'fixture'}));}}
} finally {await browser.close();}
"#;

#[test]
#[ignore = "Chromium y Gmail ficticio; sin red ni cuentas"]
fn gmail_archive_verifies_identity_and_preserves_unread_without_repeating() {
    let (controller, db, dir) = fixture();
    std::fs::write(dir.0.join("fail_before"), "").unwrap();
    std::fs::write(dir.0.join("delayed"), "").unwrap();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(
        GMAIL_ARCHIVE_FIXTURE_NODE,
        &dir.0,
    ));
    let mut call = request();
    call["arguments"] = json!({"id":"abcdef"});
    let first = controller.automate(call.clone()).unwrap();
    assert_eq!(first["status"], "archive_not_started", "{first}");
    assert_eq!(first["executed"], false);
    assert_eq!(first["review_required"], false);
    assert!(!dir.0.join("clicks").exists());
    let evidence_count = db
        .with_connection(|c| {
            Ok(c.query_row(
                "SELECT count(*) FROM browser_gmail_archive_evidence",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap();
    assert_eq!(
        evidence_count, 1,
        "la cuenta y el estado deben reservarse antes de la acción"
    );
    std::fs::remove_file(dir.0.join("fail_before")).unwrap();
    let result = controller.automate(call.clone()).unwrap();
    assert_eq!(result["status"], "ok", "{result}");
    assert_eq!(result["data"]["archivada"], true);
    assert_eq!(result["data"]["no_leida"], true);
    assert_eq!(std::fs::read_to_string(dir.0.join("clicks")).unwrap(), "x");
    call["action"] = json!("run");
    let again = controller.automate(call).unwrap();
    assert_eq!(again["status"], "ok", "{again}");
    assert_eq!(again["data"]["ya_archivada"], true);
    assert_eq!(std::fs::read_to_string(dir.0.join("clicks")).unwrap(), "x");
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Chromium y Gmail ficticio; sin red ni cuentas"]
fn gmail_archive_uncertain_effect_never_clicks_twice() {
    let (controller, db, dir) = fixture();
    std::fs::write(dir.0.join("ambiguous"), "").unwrap();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(
        GMAIL_ARCHIVE_FIXTURE_NODE,
        &dir.0,
    ));
    let mut call = request();
    call["arguments"] = json!({"id":"abcdef"});
    let first = controller.automate(call.clone()).unwrap();
    assert_eq!(first["status"], "result_uncertain", "{first}");
    assert_eq!(first["review_required"], true);
    assert_eq!(std::fs::read_to_string(dir.0.join("clicks")).unwrap(), "x");
    assert_eq!(
        controller.automate(call).unwrap()["status"],
        "review_required"
    );
    assert_eq!(std::fs::read_to_string(dir.0.join("clicks")).unwrap(), "x");
    std::fs::remove_file(dir.0.join("ambiguous")).unwrap();
    let reviewed = controller
        .automate(json!({"method":"gmail_reconcile", "attempt_id": first["web_attempt_id"]}))
        .unwrap();
    assert_eq!(reviewed["status"], "reviewed", "{reviewed}");
    assert_eq!(reviewed["resolution"], "applied");
    assert_eq!(std::fs::read_to_string(dir.0.join("clicks")).unwrap(), "x");
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Archiva una promoción no importante autorizada por el usuario en Gmail real; requiere perfil exclusivo"]
fn real_gmail_authorized_unimportant_archive_recovers_confirmation() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {appendFile} from 'node:fs/promises';
import path from 'node:path';
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const context=await chromium.launchPersistentContext(process.env.GENNIE_GMAIL_LIVE_PROFILE,{headless:true,viewport:{width:1280,height:800},timeout:15000,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const page=await context.newPage(), q='in:inbox category:promotions -is:important -is:starred';
 await page.goto('https://mail.google.com/mail/u/0/#search/'+encodeURIComponent(q),{waitUntil:'domcontentloaded',timeout:30000});
 await page.locator('[role=grid] [role=row]').first().waitFor({state:'visible',timeout:20000});
 const candidates=await page.evaluate(()=>[...document.querySelectorAll('[role=grid] [role=row]')].filter(r=>r.getClientRects().length).map(r=>r.querySelector('[data-legacy-thread-id]')?.getAttribute('data-legacy-thread-id')).filter(id=>/^[a-f0-9]{1,16}$/i.test(id||'')));
 if(!candidates.length)throw Error('no_safe_candidate');await page.close();
 const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Archivar',parameters:{id:'string'},transport:'gmail_archive',path:'/mail/u/0/',result_type:'object'},{name:'importance',description:'Comprobar importancia',parameters:{id:'string'},transport:'gmail_importance',path:'/mail/u/0/',result_type:'object'}]}});
 let id;
 for(const candidate of candidates.slice(0,10)) {
   const check=await plugins.dispatch({action:'verify',plugin_id:'fixture',function_name:'importance',expected_revision:1,arguments:{id:candidate}});
   if(check.status==='ok' && check.data?.importante===false && typeof check.data?.no_leida==='boolean'){id=candidate;break;}
 }
 if(!id)throw Error('no_confirmed_unimportant_candidate');
 for await(const line of createInterface({input:process.stdin})){try{
   const req=JSON.parse(line);req.arguments={id};
   const result=await plugins.dispatch(req);
   if(['run','verify'].includes(req.action)) {
     await appendFile(path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'archive_calls'),'x');
     if(result.status==='ok' && result.data?.archivada===true && result.data?.ya_archivada===false) {console.log(JSON.stringify({status:'result_uncertain',diagnostic:{code:'postcondition_failed',phase:'verify'}}));continue;}
   }
   console.log(JSON.stringify(result));
 }catch{console.log(JSON.stringify({status:'fixture_failure'}));}}
} finally {await context.close();}
"#;
    let profile = std::env::var_os("GENNIE_GMAIL_LIVE_PROFILE")
        .expect("Se requiere perfil explícito y exclusivo");
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser/profile");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(profile)).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut call = request();
    call["arguments"] = json!({"id":"a"});
    let result = controller.automate(call).unwrap();
    assert_eq!(result["status"], "auto_reviewed", "{result}");
    assert_eq!(result["attempt"]["resolution"], "applied");
    assert_eq!(
        std::fs::read_to_string(dir.0.join("archive_calls")).unwrap(),
        "x"
    );
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Chromium real con Gmail simulado e interceptado; sin cuentas ni red externa"]
fn real_chromium_gmail_inbox_search_and_pagination_recovers_without_review() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {access,appendFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const root=process.env.GENNIE_WEB_FIXTURE_ROOT;
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const browser=await chromium.launch({headless:true,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const context=await browser.newContext();
 await context.exposeFunction('fixtureArchive',()=>appendFile(path.join(root,'archive'),'x'));
 await context.route('**/*',async route=>{
   let allow=true;try{await access(path.join(root,'allow'));}catch{allow=false;}
   const body=`<input aria-label="Buscar en el correo electrónico" onkeydown="if(event.key==='Enter'){document.getElementById('query').textContent=this.value;location.hash='search/in%3Ainbox'}"><span id="query"></span><button aria-label="Mostrar más mensajes">1–1</button><button aria-label="Resultados siguientes" aria-disabled="${!allow}" onclick="document.querySelector('[data-legacy-thread-id]').setAttribute('data-legacy-thread-id','second');document.querySelector('[role=row]').lastChild.textContent='Segundo';document.querySelector('[role=row]').className='yO';">Siguiente</button><button aria-label="Archivar" onclick="fixtureArchive()">Archivar</button><div role="grid"><div role="row" class="zE"><span data-legacy-thread-id="first"></span><span>Primero</span></div></div>`;
   await route.fulfill({status:200,contentType:'text/html; charset=utf-8',body});
 });
 const plugins=createWebPlugins({directory:path.join(root,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Leer',parameters:{pagina:{type:'integer',minimum:1,maximum:100}},transport:'gmail_inbox',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})) { try {console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'Error del fixture'}));} }
} finally { await browser.close(); }
"#;
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let request = json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{"pagina":2}});
    let mut page_one = request.clone();
    page_one["arguments"]["pagina"] = json!(1);
    let first_page = controller.automate(page_one).unwrap();
    assert_eq!(first_page["status"], "ok");
    assert_eq!(first_page["data"]["conversaciones"][0]["id"], "first");
    assert_eq!(first_page["data"]["conversaciones"][0]["no_leida"], true);
    let first = controller.automate(request.clone()).unwrap();
    assert_eq!(first["status"], "page_not_available", "{first}");
    assert_eq!(first["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    std::fs::write(dir.0.join("allow"), "").unwrap();
    let second = controller.automate(request).unwrap();
    assert_eq!(second["status"], "ok", "{second}");
    assert_eq!(second["data"]["conversaciones"][0]["id"], "second");
    assert_eq!(second["data"]["conversaciones"][0]["no_leida"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    assert!(!dir.0.join("archive").exists());
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Usa el perfil real de Gennie Web; solo con autorización explícita y Gennie cerrado"]
fn real_gmail_inbox_reads_two_pages_without_writes() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const profile=process.env.GENNIE_GMAIL_LIVE_PROFILE;
const context=await chromium.launchPersistentContext(profile,{headless:true,viewport:{width:1280,height:800},timeout:9000,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Leer Recibidos',parameters:{pagina:{type:'integer',minimum:1,maximum:100}},transport:'gmail_inbox',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})) {try{console.log(JSON.stringify(await plugins.dispatch(JSON.parse(line))));}catch{console.log(JSON.stringify({error:'Error de consulta'}));}}
} finally {await context.close();}
"#;
    let profile = std::env::var_os("GENNIE_GMAIL_LIVE_PROFILE")
        .expect("Se requiere perfil explícito y exclusivo");
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser/profile");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(profile)).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut request = json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{"pagina":1}});
    let result = (|| -> Result<(Value, Value, i64)> {
        let first = controller.automate(request.clone())?;
        request["arguments"]["pagina"] = json!(2);
        let second = controller.automate(request)?;
        let pending = controller.web_attempts("fixture", None)?["total"]
            .as_i64()
            .unwrap_or(-1);
        Ok((first, second, pending))
    })();
    controller.stop().unwrap();
    db.close().unwrap();
    let (first, second, pending) = result.unwrap();
    eprintln!(
        "Página 1: status={}, fase={}; página 2: status={}, fase={}; pendientes={pending}",
        first["status"],
        first["diagnostic"]["stage"],
        second["status"],
        second["diagnostic"]["stage"]
    );
    eprintln!(
        "Control siguiente: coincidencias={}, deshabilitado={}; primera página declara siguiente={}",
        second["diagnostic"]["matches"],
        second["diagnostic"]["disabled"],
        first["data"]["siguiente"]
    );
    assert_eq!(first["status"], "ok");
    assert_eq!(second["status"], "ok");
    let one = first["data"]["conversaciones"].as_array().unwrap();
    let two = second["data"]["conversaciones"].as_array().unwrap();
    assert!(!one.is_empty() && !two.is_empty());
    assert!(
        one.iter()
            .chain(two)
            .all(|row| row["no_leida"].is_boolean() || row["no_leida"].is_null())
    );
    assert!(
        one.iter().chain(two).any(|row| row["no_leida"] == true),
        "La muestra no contenía ninguna conversación sin leer"
    );
    assert_ne!(one[0]["id"], two[0]["id"]);
    assert_eq!(pending, 0);
    assert_eq!(first["review_required"], false);
    assert_eq!(second["review_required"], false);
    eprintln!(
        "Lectura y paginación reales verificadas; {} y {} conversaciones; sin datos impresos.",
        one.len(),
        two.len()
    );
}

#[test]
#[ignore = "Lee una conversación sin leer con la sesión web real y comprueba su estado; requiere Gennie cerrado"]
fn real_gmail_print_reads_without_marking_unread() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const context=await chromium.launchPersistentContext(process.env.GENNIE_GMAIL_LIVE_PROFILE,{headless:true,viewport:{width:1280,height:800},timeout:15000,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const inbox=await context.newPage();
 await inbox.goto('https://mail.google.com/mail/u/0/#inbox',{waitUntil:'domcontentloaded',timeout:30000});
 await inbox.getByRole('textbox',{name:'Buscar en el correo electrónico',exact:true}).waitFor({timeout:20000});
 const item=await inbox.evaluate(()=>{const row=[...document.querySelectorAll('[role=grid] [role=row].zE')].find(r=>r.getClientRects().length&&r.querySelector('[data-legacy-thread-id]'));return row?{id:row.querySelector('[data-legacy-thread-id]').getAttribute('data-legacy-thread-id'),subject:row.querySelector('.bog')?.textContent||''}:null});
 if (!item || !/^[a-f0-9]+$/i.test(item.id)) throw new Error('No hay conversación sin leer disponible');
 const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Leer detalle',parameters:{id:'string'},transport:'gmail_print',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})) {
  try {
   const request=JSON.parse(line);request.arguments={id:item.id};
   const result=await plugins.dispatch(request);
   if (request.action==='verify' && result.status==='ok') {
    const content=result.data.texto;
    await inbox.reload({waitUntil:'domcontentloaded',timeout:30000});
    await inbox.getByRole('textbox',{name:'Buscar en el correo electrónico',exact:true}).waitFor({timeout:20000});
    const after=await inbox.evaluate(id=>{const e=[...document.querySelectorAll('[data-legacy-thread-id]')].find(x=>x.getAttribute('data-legacy-thread-id')===id);const row=e?.closest('[role=row]');return !row?'not_visible':row.classList.contains('zE')?'unread':row.classList.contains('yO')?'read':'ambiguous';},item.id);
    result.data={content_present:content.length>0,subject_present:!!item.subject&&content.includes(item.subject),after};
   }
   console.log(JSON.stringify(result));
  }catch{console.log(JSON.stringify({error:'Fallo de la prueba real'}));}
 }
} finally {await context.close();}
"#;
    let profile = std::env::var_os("GENNIE_GMAIL_LIVE_PROFILE")
        .expect("Se requiere perfil explícito y exclusivo");
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser/profile");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(profile)).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let result = controller.automate(json!({"method":"plugin_run","action":"verify","plugin_id":"fixture","function_name":"send","expected_revision":1,"arguments":{"id":"a"}})).unwrap();
    assert_eq!(result["status"], "ok", "{result}");
    assert_eq!(result["data"]["content_present"], true);
    assert_eq!(result["data"]["subject_present"], true);
    assert_eq!(result["data"]["after"], "unread");
    assert_eq!(result["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Consulta una conversación ya archivada con la sesión real, sin modificarla; requiere Gennie cerrado"]
fn real_gmail_archive_already_archived_is_idempotent() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const context=await chromium.launchPersistentContext(process.env.GENNIE_GMAIL_LIVE_PROFILE,{headless:true,viewport:{width:1280,height:800},timeout:15000,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const page=await context.newPage(),q='in:anywhere -in:inbox -in:trash -in:spam is:unread';
 await page.goto('https://mail.google.com/mail/u/0/#search/'+encodeURIComponent(q),{waitUntil:'domcontentloaded',timeout:30000});
 await page.locator('[role=grid] [role=row].zE').first().waitFor({state:'attached',timeout:20000});
 const id=await page.locator('[role=grid] [role=row].zE [data-legacy-thread-id]').first().getAttribute('data-legacy-thread-id');
 if (!/^[a-f0-9]{1,16}$/i.test(id||'')) throw Error('missing_fixture');
 await page.close();
 const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Archivar',parameters:{id:'string'},transport:'gmail_archive',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})){try{const req=JSON.parse(line);req.arguments={id};const result=await plugins.dispatch(req);if(result.data)result.data={archivada:result.data.archivada,ya_archivada:result.data.ya_archivada,no_leida:result.data.no_leida};console.log(JSON.stringify(result));}catch{console.log(JSON.stringify({error:'fixture'}));}}
} finally {await context.close();}
"#;
    let profile = std::env::var_os("GENNIE_GMAIL_LIVE_PROFILE")
        .expect("Se requiere perfil explícito y exclusivo");
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser/profile");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(profile)).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut call = request();
    call["arguments"] = json!({"id":"a"});
    let result = controller.automate(call).unwrap();
    assert_eq!(result["status"], "ok", "{result}");
    assert_eq!(result["data"]["archivada"], true);
    assert_eq!(result["data"]["ya_archivada"], true);
    assert_eq!(result["data"]["no_leida"], true);
    assert_eq!(result["review_required"], false);
    assert_eq!(
        controller.web_attempts("fixture", None).unwrap()["total"],
        0
    );
    controller.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Archiva exactamente la promoción autorizada en Gmail real; requiere Gennie cerrado"]
fn real_gmail_archive_one_authorized_unread_promotion() {
    const NODE: &str = r#"
import {createInterface} from 'node:readline';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const runtime=path.join(process.env.HOME,'Library/Application Support/Gennie/browser/runtime-patchright-1.64.2');
const {chromium}=createRequire(path.join(runtime,'package.json'))('patchright');
const {createWebPlugins}=await import(pathToFileURL(process.env.GENNIE_WEB_PLUGIN_MODULE));
const context=await chromium.launchPersistentContext(process.env.GENNIE_GMAIL_LIVE_PROFILE,{headless:true,viewport:{width:1280,height:800},timeout:15000,executablePath:path.join(runtime,'Gennie Web.app/Contents/MacOS/Google Chrome for Testing')});
try {
 const page=await context.newPage(),q='in:inbox category:promotions is:unread -is:starred -is:important';
 await page.goto('https://mail.google.com/mail/u/0/#search/'+encodeURIComponent(q),{waitUntil:'domcontentloaded',timeout:30000});
 await page.locator('[role=grid] [role=row].zE').first().waitFor({state:'attached',timeout:20000});
 const matches=await page.evaluate(()=>[...document.querySelectorAll('[role=grid] [role=row].zE')].filter(r=>r.getClientRects().length&&r.querySelector('.bog')?.textContent?.trim()==='4K all day'&&r.querySelector('[email]')?.getAttribute('email')?.endsWith('@tella.tv')).map(r=>r.querySelector('[data-legacy-thread-id]')?.getAttribute('data-legacy-thread-id')));
 if(matches.length!==1||!/^[a-f0-9]{1,16}$/i.test(matches[0]||''))throw Error('missing_authorized_fixture');
 const id=matches[0],response=await context.request.get('https://mail.google.com/mail/u/0/?view=pt&search=all&th='+id,{timeout:20000,maxRedirects:0});
 if(response.status()!==200)throw Error('print_unavailable');
 const html=await response.text();await response.dispose();
 const blank=await context.newPage();await blank.route('**/*',route=>route.abort());
 const safe=await blank.evaluate(html=>{const d=new DOMParser().parseFromString(html,'text/html'),m=d.querySelector('.maincontent');return !!m&&m.querySelectorAll('table.message').length===1&&/unsubscribe|darse de baja|cancelar suscripci[oó]n|anular suscripci[oó]n/i.test(m.textContent||'');},html);
 await blank.close();await page.close();if(!safe)throw Error('fixture_not_single_promotion');
 const plugins=createWebPlugins({directory:path.join(process.env.GENNIE_WEB_FIXTURE_ROOT,'recipes'),getContext:()=>context});
 await plugins.dispatch({action:'save',expected_revision:0,manifest:{id:'fixture',name:'Gmail',origin:'https://mail.google.com',functions:[{name:'send',description:'Archivar',parameters:{id:'string'},transport:'gmail_archive',path:'/mail/u/0/',result_type:'object'}]}});
 for await(const line of createInterface({input:process.stdin})){try{const req=JSON.parse(line);req.arguments={id};const result=await plugins.dispatch(req);if(result.data)result.data={archivada:result.data.archivada,ya_archivada:result.data.ya_archivada,no_leida:result.data.no_leida};console.log(JSON.stringify(result));}catch{console.log(JSON.stringify({error:'fixture'}));}}
} finally {await context.close();}
"#;
    let profile = std::env::var_os("GENNIE_GMAIL_LIVE_PROFILE")
        .expect("Se requiere perfil explícito y exclusivo");
    let expected = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser/profile");
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(profile)).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
    let (controller, db, dir) = fixture();
    *controller.inner.session.lock().unwrap() = Some(process::Session::node_fixture(NODE, &dir.0));
    let mut call = request();
    call["arguments"] = json!({"id":"a"});
    let first = controller.automate(call.clone()).unwrap();
    let second = if first["status"] == "ok" {
        call["action"] = json!("run");
        Some(controller.automate(call).unwrap())
    } else {
        None
    };
    let pending = controller.web_attempts("fixture", None).unwrap()["total"].clone();
    controller.stop().unwrap();
    db.close().unwrap();
    assert_eq!(first["status"], "ok", "{first}");
    assert_eq!(first["data"]["archivada"], true);
    assert_eq!(first["data"]["ya_archivada"], false);
    assert_eq!(first["data"]["no_leida"], true);
    let second = second.unwrap();
    assert_eq!(second["status"], "ok", "{second}");
    assert_eq!(second["data"]["ya_archivada"], true);
    assert_eq!(second["data"]["no_leida"], true);
    assert_eq!(pending, 0);
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
  const {request_id,request,cancel_request_id} = JSON.parse(line);
  if (cancel_request_id !== undefined) continue;
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
