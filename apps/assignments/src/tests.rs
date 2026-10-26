use super::*;
use serde_json::json;
fn document() -> Document {
    serde_json::from_value(json!({
        "title":"Organizar el correo", "description":"Preparar un resumen para organizar mi mañana.",
        "reason":null,"context":"Petición del usuario en el chat", "kind":"recurring","priority":"high","status":"active",
        "timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"},
        "preparation":{"interval_minutes":10,"rationale":"Reunir cambios antes del resumen diario","steps":["Revisar nuevos mensajes","Agrupar asuntos pendientes"]},
        "applications":[{"name":"Gmail","kind":"web","url":"https://mail.google.com","plugin_available":true,"manual_required":false}]
    })).unwrap()
}
fn store() -> Store {
    let db = Database::memory().unwrap();
    db.migrate(
        AssignmentsModule.migration_namespace(),
        AssignmentsModule.migrations(),
    )
    .unwrap();
    Store::new(db)
}
#[test]
fn migrating_recorded_status_preserves_other_states_and_record_metadata() {
    let db = Database::memory().unwrap();
    let module = AssignmentsModule;
    db.migrate(module.migration_namespace(), &module.migrations()[..2])
        .unwrap();
    let mut legacy = document();
    legacy.applications[0].plugin_available = false;
    legacy.applications[0].manual_required = true;
    let encoded = serde_json::to_string(&legacy).unwrap();
    db.with_connection(|c| {
        for status in ["recorded", "paused", "completed", "cancelled"] {
            let saved = encoded.replace("\"status\":\"active\"", &format!("\"status\":\"{status}\""));
            c.execute("INSERT INTO assignments(request_key,revision,document,created_at,updated_at) VALUES(?1,7,?2,'2026-10-24T13:00:00.000Z','2026-10-25T13:00:00.000Z')", params![status, saved])?;
        }
        Ok(())
    }).unwrap();
    db.migrate(module.migration_namespace(), module.migrations())
        .unwrap();
    let store = Store::new(db.clone());
    for (id, key, status) in [
        (1, "recorded", Status::Active),
        (2, "paused", Status::Paused),
        (3, "completed", Status::Completed),
        (4, "cancelled", Status::Cancelled),
    ] {
        let row = store.get(id).unwrap();
        let mut expected = legacy.clone();
        expected.status = status;
        assert_eq!(row.document, expected);
        assert_eq!(row.revision, 7);
        assert_eq!(row.created_at, "2026-10-24T13:00:00.000Z");
        assert_eq!(row.updated_at, "2026-10-25T13:00:00.000Z");
        assert_eq!(store.create(key, expected).unwrap().id, id);
    }
    let mut invalid = serde_json::to_value(legacy).unwrap();
    invalid["status"] = json!("recorded");
    assert!(serde_json::from_value::<Document>(invalid).is_err());
    db.migrate(module.migration_namespace(), module.migrations())
        .unwrap();
    assert_eq!(store.get(1).unwrap().document.status, Status::Active);
}
#[test]
fn separates_preparation_from_notification_and_normalizes_unavailable_plugins() {
    let store = store();
    let r = store.create("petición-mañana", document()).unwrap();
    assert_eq!(r.document.preparation.interval_minutes, Some(10));
    assert_eq!(r.document.recurrence.as_ref().unwrap().local_time, "08:00");
    assert!(!r.document.applications[0].plugin_available);
    assert!(r.document.applications[0].manual_required);
    assert_eq!(r.document.reason, None);
    let revision = store.revision();
    assert_eq!(
        store.create("petición-mañana", document()).unwrap().id,
        r.id
    );
    assert!(store.revision() >= revision);
    assert_eq!(store.list(0, 100).unwrap().len(), 1);
    assert!(r.created_at.ends_with('Z'));
    assert!(
        store
            .create("petición-mañana", {
                let mut d = document();
                d.title = "Otro".into();
                d
            })
            .is_err()
    );
}
#[test]
fn rejects_invalid_schedules_and_invented_fields_without_writing() {
    let store = store();
    for time in ["24:00", "08:60", "8:00", "ab:cd", "é:00"] {
        let mut d = document();
        d.recurrence.as_mut().unwrap().local_time = time.into();
        assert!(store.create(time, d).is_err(), "{time}");
    }
    for zone in ["../UTC", "/etc/passwd", "Missing/Zone", "zone.tab", ""] {
        let mut d = document();
        d.timezone = zone.into();
        assert!(store.create("invalid-zone", d).is_err());
    }
    for at in [
        "2026-02-30T08:00:00Z",
        "2026-13-01T08:00:00Z",
        "2026-10-26T24:00:00Z",
        "2026-10-26T08:00:00-05:00",
        "mañana",
    ] {
        let mut d = document();
        d.kind = Kind::OneTime;
        d.recurrence = None;
        d.notify_at = Some(at.into());
        assert!(store.create("invalid-at", d).is_err(), "{at}");
    }
    let mut d = document();
    d.preparation.rationale = None;
    assert!(store.create("missing-reason", d).is_err());
    let mut d = document();
    d.recurrence.as_mut().unwrap().frequency = Frequency::Weekly;
    assert!(store.create("missing-days", d).is_err());
    let mut d = document();
    d.recurrence.as_mut().unwrap().weekdays = vec![1, 1];
    assert!(store.create("duplicated-days", d).is_err());
    let mut d = document();
    d.notify_at = Some("2026-10-26T08:00:00Z".into());
    assert!(store.create("mixed", d).is_err());
    let mut value = serde_json::to_value(document()).unwrap();
    value["cron"] = json!("* * * * *");
    assert!(serde_json::from_value::<Document>(value).is_err());
    assert!(store.list(0, 100).unwrap().is_empty());
    assert_eq!(store.revision(), 0);
}
#[test]
fn supports_goals_one_time_weekly_and_concurrent_edits() {
    let store = store();
    let mut goal = document();
    goal.kind = Kind::Goal;
    goal.recurrence = None;
    let r = store.create("research", goal.clone()).unwrap();
    goal.status = Status::Paused;
    let updated = store.update(r.id, r.revision, goal.clone()).unwrap();
    assert_eq!(updated.revision, 2);
    assert!(store.update(r.id, r.revision, goal).is_err());
    assert_eq!(store.get(r.id).unwrap().document.status, Status::Paused);
    let mut one = document();
    one.kind = Kind::OneTime;
    one.recurrence = None;
    one.notify_at = Some("2026-10-27T13:00:00Z".into());
    store.create("single", one).unwrap();
    let mut weekly = document();
    let r = weekly.recurrence.as_mut().unwrap();
    r.frequency = Frequency::Weekly;
    r.weekdays = vec![1, 3, 5];
    store.create("weekly", weekly).unwrap();
    assert_eq!(store.list(0, 2).unwrap().len(), 2);
    assert_eq!(store.list(2, 2).unwrap().len(), 1);
}
#[test]
fn reopening_retains_plans_migration_and_commit_failures_are_not_success() {
    let path = std::env::temp_dir().join(format!(
        "gennie-assignments-test-{}.sqlite3",
        std::process::id()
    ));
    let db = Database::open(&path, 500).unwrap();
    db.migrate(
        AssignmentsModule.migration_namespace(),
        AssignmentsModule.migrations(),
    )
    .unwrap();
    let store = Store::new(db.clone());
    let r = store.create("persist", follow_up_document()).unwrap();
    db.close().unwrap();
    assert!(store.update(r.id, r.revision, document()).is_err());
    let db = Database::open(&path, 500).unwrap();
    db.migrate(
        AssignmentsModule.migration_namespace(),
        AssignmentsModule.migrations(),
    )
    .unwrap();
    let reopened = Store::new(db.clone());
    assert_eq!(reopened.get(r.id).unwrap().document, r.document);
    db.with_connection(|c| { c.execute_batch("CREATE TRIGGER fail_event BEFORE INSERT ON assignments BEGIN SELECT RAISE(ABORT,'forced'); END;")?;Ok(()) }).unwrap();
    assert!(reopened.create("failed", document()).is_err());
    assert_eq!(reopened.revision(), 0);
    assert_eq!(reopened.list(0, 100).unwrap().len(), 1);
    db.close().unwrap();
    std::fs::remove_file(path).unwrap();
}

fn follow_up_document() -> Document {
    let mut d = document();
    d.recurrence.as_mut().unwrap().local_time = "07:30".into();
    d.follow_up = Some(FollowUpPolicy {
        scope: "Tareas de hoy y sin fecha; conservar cierres y reaperturas.".into(),
        signals: vec!["¿Este cambio afecta lo que el usuario quiere lograr hoy?".into(),
            "¿Hay una respuesta o aplazamiento que debamos respetar?".into()],
        decision_guidance: "Elegir si registrar sin interrumpir, preguntar o proponer ayuda según contexto e historial. Adaptar la anticipación a la tarea, sin umbral universal.".into(),
        action_scope: None,
    });
    d
}

#[test]
fn flexible_follow_up_survives_edits_and_can_be_removed_without_changing_delivery() {
    let store = store();
    let d = follow_up_document();
    let row = store.create("follow-up", d).unwrap();
    assert_eq!(
        store.create("follow-up", row.document.clone()).unwrap().id,
        row.id
    );
    assert_eq!(row.document.preparation.interval_minutes, Some(10));
    assert_eq!(
        row.document.recurrence.as_ref().unwrap().local_time,
        "07:30"
    );
    let mut edited = row.document.clone();
    edited.follow_up.as_mut().unwrap().signals =
        vec!["¿Habría algo útil que podamos adelantar?".into()];
    edited.follow_up.as_mut().unwrap().action_scope =
        Some("Preparar borradores; preguntar antes de enviar.".into());
    let changed = store.update(row.id, row.revision, edited.clone()).unwrap();
    assert_eq!(store.get(row.id).unwrap().document, edited);
    assert!(store.update(row.id, row.revision, row.document).is_err());
    let mut disabled = changed.document;
    disabled.follow_up = None;
    let removed = store.update(row.id, changed.revision, disabled).unwrap();
    assert!(removed.document.follow_up.is_none());
    assert_eq!(removed.document.recurrence.unwrap().local_time, "07:30");
}

#[test]
fn invalid_follow_up_never_writes_or_overwrites_a_valid_plan() {
    let store = store();
    let row = store
        .create("valid-follow-up", follow_up_document())
        .unwrap();
    let revision = store.revision();
    let mut cases = Vec::new();
    let mut d = row.document.clone();
    d.preparation.interval_minutes = None;
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().scope = " ".into();
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().decision_guidance = "".into();
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().signals = vec!["".into()];
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().signals = vec!["Pregunta".into(); 25];
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().signals = vec!["a".repeat(501)];
    cases.push(d);
    let mut d = row.document.clone();
    d.follow_up.as_mut().unwrap().action_scope = Some(" ".into());
    cases.push(d);
    for (i, invalid) in cases.into_iter().enumerate() {
        assert!(
            store
                .create(&format!("invalid-policy-{i}"), invalid.clone())
                .is_err()
        );
        assert!(store.update(row.id, row.revision, invalid).is_err());
    }
    assert_eq!(store.revision(), revision);
    assert_eq!(store.get(row.id).unwrap().document, row.document);
    assert_eq!(store.list(0, 100).unwrap().len(), 1);
}

#[test]
fn legacy_document_keeps_idempotency_and_does_not_enable_follow_up() {
    let store = store();
    let row = store.create("before-follow-up", document()).unwrap();
    let encoded_before = serde_json::to_string(&row.document).unwrap();
    assert!(!encoded_before.contains("follow_up"));
    // Simula la lectura canónica persistida por el binario anterior.
    let legacy: Document = serde_json::from_str(&encoded_before).unwrap();
    assert!(legacy.follow_up.is_none());
    assert_eq!(store.create("before-follow-up", legacy).unwrap().id, row.id);
}

#[test]
fn migrating_legacy_events_preserves_records_keys_revisions_and_checksums() {
    let db = Database::memory().unwrap();
    let module = AssignmentsModule;
    assert_eq!(module.name(), "assignments");
    db.migrate(module.migration_namespace(), &module.migrations()[..1])
        .unwrap();
    let mut legacy = follow_up_document();
    legacy.applications[0].plugin_available = false;
    legacy.applications[0].manual_required = true;
    let encoded = serde_json::to_string(&legacy).unwrap();
    db.with_connection(|c| {
        c.execute("INSERT INTO events(id,request_key,revision,document,created_at,updated_at) VALUES(41,'legacy-key',7,?1,'2026-10-24T13:00:00.000Z','2026-10-25T13:00:00.000Z')", [&encoded])?;
        Ok(())
    }).unwrap();
    let history_before: (String, String) = db
        .with_connection(|c| {
            Ok(c.query_row(
                "SELECT checksum,applied_at FROM _soc_migrations_events WHERE version=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .unwrap();
    db.migrate(module.migration_namespace(), module.migrations())
        .unwrap();
    let store = Store::new(db.clone());
    let row = store.get(41).unwrap();
    assert_eq!(row.document, legacy);
    assert_eq!(row.revision, 7);
    assert_eq!(row.created_at, "2026-10-24T13:00:00.000Z");
    assert_eq!(row.updated_at, "2026-10-25T13:00:00.000Z");
    assert_eq!(store.create("legacy-key", legacy.clone()).unwrap().id, 41);
    let history_after: (String, String) = db
        .with_connection(|c| {
            Ok(c.query_row(
                "SELECT checksum,applied_at FROM _soc_migrations_events WHERE version=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .unwrap();
    assert_eq!(history_before, history_after);
    db.with_connection(|c| {
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='events'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM _soc_migrations_events", [], |r| r
                .get::<_, i64>(0))?,
            3
        );
        Ok(())
    })
    .unwrap();
    legacy.title = "Encargo actualizado".into();
    assert_eq!(store.update(41, 7, legacy).unwrap().revision, 8);
    db.migrate(module.migration_namespace(), module.migrations())
        .unwrap();
    assert_eq!(store.get(41).unwrap().revision, 8);
}

#[test]
fn conflicting_destination_rolls_back_the_rename_without_losing_legacy_data() {
    let db = Database::memory().unwrap();
    let module = AssignmentsModule;
    db.migrate(module.migration_namespace(), &module.migrations()[..1])
        .unwrap();
    db.with_connection(|c| {
        c.execute(
            "INSERT INTO events(request_key,document) VALUES('legacy','{}')",
            [],
        )?;
        c.execute_batch("CREATE TABLE assignments(id INTEGER PRIMARY KEY)")?;
        Ok(())
    })
    .unwrap();
    assert!(
        db.migrate(module.migration_namespace(), module.migrations())
            .is_err()
    );
    db.with_connection(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM _soc_migrations_events", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        Ok(())
    })
    .unwrap();
}
