use super::*;

#[test]
fn snapshot_projects_saved_next_time_and_clears_it_after_pause_without_dispatch() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='assignments'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let document = serde_json::from_value(serde_json::json!({
        "title":"Revisión ficticia","description":"Consultar novedades",
        "kind":"recurring","priority":"normal","status":"active",
        "timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"}
    }))
    .unwrap();
    let assignment = backend
        .assignments
        .create("overview-fixture", document)
        .unwrap();
    let policy = serde_json::json!({
        "assignment":{"id":assignment.id,"instance_id":assignment.instance_id,"revision":assignment.revision,"document":assignment.document},
        "trigger":"delivery","calendar":{"kind":"weekly","zone":"America/Bogota","local_time":"08:00","weekdays":[1,2,3,4,5,6,7],"dst":{"repeated":"earlier","missing":"skip"}},
        "target":{"kind":"web","plugin_id":"fixture","function":"read","revision":1},
        "arguments":{},"route":"http","misfire":"skip_missed","max_lateness_minutes":5
    });
    backend.state.db.with_connection(|c| {
        let tx = c.transaction()?;
        tx.execute("INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state,next_at) VALUES('overview-policy',?1,?2,'delivery',1,1,'active','2099-12-31T23:45:00Z')", gennie_db::params![assignment.id,assignment.instance_id])?;
        tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES('overview-policy',1,?1)", [policy.to_string()])?;
        tx.commit()?;
        Ok(())
    }).unwrap();
    let revision = backend.assignments.revision();
    backend.refresh().unwrap();
    assert_eq!(
        backend.snapshot.assignments[0].next_execution_at.as_deref(),
        Some("2099-12-31T23:45:00Z")
    );
    assert_eq!(backend.assignments.revision(), revision);
    backend.close().unwrap();
    let mut reopened = fixture.open();
    assert_eq!(
        reopened.snapshot.assignments[0]
            .next_execution_at
            .as_deref(),
        Some("2099-12-31T23:45:00Z")
    );
    reopened.execution_control.as_ref().unwrap().request(serde_json::json!({"action":"pause","policy_id":"overview-policy","expected_revision":1})).unwrap();
    reopened.refresh().unwrap();
    assert!(reopened.snapshot.assignments[0].next_execution_at.is_none());
    assert_eq!(
        reopened.assignments.get(assignment.id).unwrap().revision,
        assignment.revision
    );
    reopened
        .state
        .db
        .with_connection(|c| {
            for table in [
                "assignment_execution_runs",
                "plugin_build_jobs",
                "codex_conversation",
                "presence_history",
            ] {
                assert_eq!(
                    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))?,
                    0
                );
            }
            Ok(())
        })
        .unwrap();
    reopened.close().unwrap();
}
