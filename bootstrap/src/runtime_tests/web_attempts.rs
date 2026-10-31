//! Diario accesible sin navegador y recuperación por su propietario, sin runtime web.
use super::*;
use serde_json::json;
#[test]
fn web_attempts_survive_inactive_browser_recover_and_review_without_execution() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.state.db.with_connection(|c|{
        c.execute("UPDATE modules SET active=1 WHERE name='plugins'",[])?;
        c.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation) VALUES('fixture','Prueba','send',7,'job_9','verify')",[])?;Ok(())
    }).unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    let pending = backend
        .plugins
        .request(json!({"action":"web_attempts","plugin_id":"fixture"}))
        .unwrap();
    assert_eq!(pending["total"], 1);
    assert_eq!(pending["attempts"][0]["active"], false);
    assert_eq!(pending["attempts"][0]["reason"], "completion_unrecorded");
    assert!(backend.plugins.request(json!({"action":"web_reconcile","plugin_id":"fixture","attempt_id":1,"expected_revision":7,"resolution":"not_applied"})).is_err());
    backend
        .state
        .db
        .with_connection(|c| {
            c.execute("UPDATE modules SET active=1 WHERE name='browser'", [])?;
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
    let mut backend = fixture.open();
    let pending = backend
        .plugins
        .request(json!({"action":"web_attempts","plugin_id":"fixture"}))
        .unwrap();
    assert_eq!(pending["attempts"][0]["state"], "uncertain");
    assert_eq!(pending["attempts"][0]["reason"], "interrupted");
    let before = backend.plugins.revision();
    assert!(backend.plugins.request(json!({"action":"web_reconcile","plugin_id":"fixture","attempt_id":1,"expected_revision":8,"resolution":"not_applied"})).is_err());
    backend.plugins.request(json!({"action":"web_reconcile","plugin_id":"fixture","attempt_id":1,"expected_revision":7,"resolution":"not_applied"})).unwrap();
    assert!(backend.plugins.revision() > before);
    assert_eq!(
        backend
            .plugins
            .request(json!({"action":"web_attempts","plugin_id":"fixture"}))
            .unwrap()["total"],
        0
    );
    backend
        .state
        .db
        .with_connection(|c| {
            let resolution: String = c.query_row(
                "SELECT resolution FROM browser_web_attempts WHERE id=1",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(resolution, "not_applied");
            for table in [
                "presence_history",
                "codex_conversation",
                "plugin_mcp_attempts",
                "plugin_build_jobs",
                "assignment_execution_runs",
            ] {
                let count: i64 =
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
                assert_eq!(count, 0);
            }
            Ok(())
        })
        .unwrap();
    backend.close().unwrap();
}
