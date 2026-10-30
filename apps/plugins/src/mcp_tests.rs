use super::*;
use gennie_db::Database;

fn specification() -> Specification {
    Specification {
        id: "fixture".into(),
        name: "Servidor de prueba".into(),
        connection: Connection::Http {
            url: "https://example.com/mcp".into(),
            bearer_token_env_var: None,
        },
        timeout_seconds: 10,
    }
}
fn database() -> Database {
    let db = Database::memory().unwrap();
    db.migrate("plugins", crate::MIGRATIONS).unwrap();
    db
}
fn catalogue() -> Value {
    json!({"echo":{"name":"echo","description":"Consulta ficticia","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}}})
}
fn test_service(db: Database, root: PathBuf) -> Service {
    let service = Service::new(db, root, Arc::new(|| Ok(gennie_llm::Settings::default())));
    service.start().unwrap();
    service
}
// La carpeta reservada es propia; el root del servicio empieza ausente.
struct Directory(PathBuf, PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT_DIRECTORY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "gennie-mcp-service-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_DIRECTORY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.join("service"), path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

#[test]
fn temporary_directories_do_not_share_cleanup_ownership() {
    let directories: Vec<_> = (0..4096).map(|_| Directory::new()).collect();
    let unique: std::collections::BTreeSet<_> = directories.iter().map(|d| &d.0).collect();
    assert_eq!(unique.len(), directories.len());
    std::fs::create_dir(&directories[0].0).unwrap();
    let marker = directories[0].0.join("owned.txt");
    std::fs::write(&marker, "fixture").unwrap();
    let first = directories.into_iter().next().unwrap();
    assert!(marker.is_file());
    drop(first);
    assert!(!marker.exists());
}

#[test]
fn connection_validation_rejects_secrets_in_url_unknown_fields_and_invalid_paths() {
    let valid = specification();
    valid.validate().unwrap();
    let mut too_long = valid.clone();
    too_long.timeout_seconds = super::super::mcp_source::MAX_TOOL_TIMEOUT_SECONDS + 1;
    assert!(too_long.validate().is_err());
    for url in [
        "https://user:secret@example.com/mcp",
        "https://example.com/mcp?token=secret",
        "https://example.com/mcp#secret",
        "http://example.com/mcp",
        "file:///private/data",
    ] {
        let mut invalid = valid.clone();
        invalid.connection = Connection::Http {
            url: url.into(),
            bearer_token_env_var: None,
        };
        assert!(invalid.validate().is_err());
    }
    let mut local = valid.clone();
    local.connection = Connection::Http {
        url: "http://127.0.0.1:1234/mcp".into(),
        bearer_token_env_var: Some("FIXTURE_TOKEN".into()),
    };
    local.validate().unwrap();
    assert!(serde_json::from_value::<Specification>(json!({"id":"fixture","name":"Prueba","timeout_seconds":10,"connection":{"type":"http","url":"https://example.com/mcp","token":"PRIVATE"}})).is_err());
    let mut invalid = valid;
    invalid.id = "../outside".into();
    assert!(invalid.validate().is_err());
}

#[test]
fn source_revisions_permissions_schema_changes_delete_and_recreate_are_conservative() {
    let db = database();
    let store = Store(db.clone());
    let spec = specification();
    let created = store.save(&spec, 0).unwrap();
    assert_eq!(created.revision, 1);
    assert!(!created.enabled);
    assert!(created.allowed_tools.is_empty());
    let discovered = store.catalogue(&spec.id, 1, &catalogue()).unwrap();
    let allowed = store
        .permissions(&spec.id, discovered.revision, &["echo".into()], true)
        .unwrap();
    let summaries = serde_json::to_value(store.list().unwrap()).unwrap();
    assert_eq!(summaries[0]["tool_count"], 1);
    assert_eq!(summaries[0]["allowed_count"], 1);
    assert!(summaries[0].get("tools").is_none());
    assert!(summaries[0].get("specification").is_none());
    let same = store
        .catalogue(&spec.id, allowed.revision, &catalogue())
        .unwrap();
    assert_eq!(same.revision, allowed.revision);
    assert_eq!(same.allowed_tools, ["echo"]);
    let mut next = catalogue();
    next["other"] = json!({"name":"other","inputSchema":{"type":"object"}});
    let added = store.catalogue(&spec.id, same.revision, &next).unwrap();
    assert_eq!(added.allowed_tools, ["echo"]);
    next["echo"]["inputSchema"]["required"] = json!(["text", "new"]);
    let changed = store.catalogue(&spec.id, added.revision, &next).unwrap();
    assert!(changed.allowed_tools.is_empty());
    assert!(
        store
            .permissions(&spec.id, allowed.revision, &["echo".into()], true)
            .is_err()
    );
    assert!(
        store
            .permissions(&spec.id, changed.revision, &["invented".into()], true)
            .is_err()
    );
    store.delete(&spec.id, changed.revision).unwrap();
    assert!(store.get(&spec.id).is_err());
    let recreated = store.save(&spec, 0).unwrap();
    assert!(recreated.revision > changed.revision);
    assert!(!recreated.enabled);
    assert!(recreated.allowed_tools.is_empty());
    assert!(store.save(&spec, created.revision).is_err());
    db.close().unwrap();
}

#[test]
fn source_updates_rollback_and_migration_reopen_preserve_jobs() {
    let dir = Directory::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let file = dir.0.join("fixture.sqlite3");
    let db = Database::open(&file, 50).unwrap();
    db.migrate("plugins", &crate::MIGRATIONS[..5]).unwrap();
    db.with_connection(|c| {c.execute("INSERT INTO plugin_build_jobs(request_key,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,max_attempts,max_seconds,summary) VALUES('old','fixture','https://example.com','Objetivo anterior',7,4,'waiting_user',4,600,'Esperando')",[])?;Ok(())}).unwrap();
    db.migrate("plugins", crate::MIGRATIONS).unwrap();
    let store = Store(db.clone());
    let saved = store.save(&specification(), 0).unwrap();
    db.with_connection(|c| {c.execute_batch("CREATE TRIGGER fail_mcp_change BEFORE UPDATE ON plugin_mcp_sources BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")?;Ok(())}).unwrap();
    assert!(
        store
            .catalogue("fixture", saved.revision, &catalogue())
            .is_err()
    );
    let after = store.get("fixture").unwrap();
    assert_eq!(after.revision, saved.revision);
    assert_eq!(after.tools, json!({}));
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER fail_mcp_change")?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    drop(store);
    let db = Database::open(&file, 50).unwrap();
    db.migrate("plugins", crate::MIGRATIONS).unwrap();
    assert_eq!(
        Store(db.clone()).get("fixture").unwrap().revision,
        saved.revision
    );
    db.with_connection(|c| {
        let text: String = c.query_row(
            "SELECT objective FROM plugin_build_jobs WHERE request_key='old'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(text, "Objetivo anterior");
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
}

#[test]
fn full_schema_validation_is_offline_and_errors_do_not_include_argument_values() {
    let schema = json!({"type":"object","properties":{"destination":{"$ref":"#/$defs/person"},"messages":{"type":"array","items":{"type":"string","minLength":1},"minItems":1}},"required":["destination","messages"],"additionalProperties":false,"$defs":{"person":{"oneOf":[{"type":"string","enum":["Cata"]},{"type":"object","properties":{"id":{"type":"integer","minimum":1}},"required":["id"],"additionalProperties":false}]}}});
    let check = validator(&schema).unwrap();
    assert!(check.is_valid(&json!({"destination":{"id":2},"messages":["Hola"]})));
    assert!(!check.is_valid(&json!({"destination":"Inventada","messages":[]})));
    for schema in [
        json!({"$ref":"https://127.0.0.1:1/private"}),
        json!({"$ref":"file:///private/secret.json"}),
    ] {
        assert!(validator(&schema).is_err());
    }
    assert!(!validator(&json!(false)).unwrap().is_valid(&json!({})));
    let db = database();
    let dir = Directory::new();
    let service = test_service(db.clone(), dir.0.clone());
    let saved = service.save(specification(), 0).unwrap();
    let revision = saved["revision"].as_i64().unwrap();
    let discovered = service
        .store
        .catalogue("fixture", revision, &catalogue())
        .unwrap();
    let allowed = service
        .permissions("fixture", discovered.revision, &["echo".into()], true)
        .unwrap();
    let public = service.tool_catalogue("fixture").unwrap();
    assert_eq!(public["id"], "fixture");
    assert!(public.get("specification").is_none());
    assert!(!public.to_string().contains("https://example.com/mcp"));
    assert!(
        service
            .get("fixture")
            .unwrap()
            .get("specification")
            .is_some()
    );
    let result = service
        .execute(
            "fixture",
            "echo",
            allowed["revision"].as_i64().unwrap(),
            json!({"text":123,"extra":"PRIVATE_VALUE"}),
        )
        .unwrap();
    assert_eq!(result["executed"], false);
    assert_eq!(result["status"], "invalid_arguments");
    assert!(!result.to_string().contains("PRIVATE_VALUE"));
    assert!(service.connections.lock().unwrap().is_empty());
    assert!(!dir.0.exists());
    service.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn busy_or_stale_requests_do_not_queue_or_recreate_connections() {
    let db = database();
    let dir = Directory::new();
    let service = test_service(db.clone(), dir.0.clone());
    service.save(specification(), 0).unwrap();
    let discovered = service.store.catalogue("fixture", 1, &catalogue()).unwrap();
    service
        .permissions("fixture", discovered.revision, &["echo".into()], true)
        .unwrap();
    let current = service.store.get("fixture").unwrap();
    let handle = service.handle(&current).unwrap();
    let guard = handle.client.lock().unwrap();
    let result = service
        .execute(
            "fixture",
            "echo",
            current.revision,
            json!({"text":"fixture"}),
        )
        .unwrap();
    assert_eq!(result["status"], "busy");
    assert_eq!(result["executed"], false);
    assert_eq!(service.state("fixture"), "working");
    assert!(!dir.0.exists());
    drop(guard);
    service.disconnect("fixture").unwrap();
    service
        .store
        .permissions("fixture", current.revision, &[], false)
        .unwrap();
    assert!(service.handle(&current).is_err());
    assert!(service.connections.lock().unwrap().is_empty());
    assert!(!dir.0.exists());
    service.stop().unwrap();
    db.close().unwrap();
}

#[test]
#[ignore = "Requiere Codex/helper instalados; servidor stdio ficticio, sin cuenta ni inferencia"]
fn installed_mcp_service_discovers_gates_calls_and_does_not_store_results() {
    let mut registry = gennie_mcp::Registry::default();
    let calls = Arc::new(AtomicU64::new(0));
    let observed = calls.clone();
    registry
        .register([gennie_mcp::Tool {
            definition: catalogue()["echo"].clone(),
            call: Box::new(move |args| {
                observed.fetch_add(1, Ordering::SeqCst);
                if args["text"] == "TOOL_ERROR" {
                    return Err("PRIVATE_TOOL_ERROR".into());
                }
                Ok(json!({"echo":args["text"]}))
            }),
        }])
        .unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let launch = server.launch();
    let mut spec = specification();
    spec.connection = Connection::Stdio {
        command: launch.helper.to_string_lossy().into(),
        args: vec!["--mcp".into(), launch.socket.to_string_lossy().into()],
        cwd: None,
        env_vars: vec![],
    };
    let dir = Directory::new();
    let db = database();
    let service = test_service(db.clone(), dir.0.clone());
    let result = (|| -> Result<()> {
        let saved = service.save(spec, 0)?;
        let revision = saved["revision"].as_i64().unwrap();
        let denied = service.execute(
            "fixture",
            "echo",
            revision,
            json!({"text":"PRIVATE_OUTPUT"}),
        )?;
        assert_eq!(denied["status"], "permission_required");
        assert!(!dir.0.exists());
        let connected = service.connect("fixture", revision)?;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let allowed = service.permissions(
            "fixture",
            connected["revision"].as_i64().unwrap(),
            &["echo".into()],
            true,
        )?;
        let output = service.execute(
            "fixture",
            "echo",
            allowed["revision"].as_i64().unwrap(),
            json!({"text":"PRIVATE_OUTPUT"}),
        )?;
        assert_eq!(output["status"], "ok");
        assert_eq!(
            output["result"]["structuredContent"]["echo"],
            "PRIVATE_OUTPUT"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        db.with_connection(|c| {
            let stored: String = c.query_row(
                "SELECT document||tools||allowed_tools FROM plugin_mcp_sources",
                [],
                |r| r.get(0),
            )?;
            assert!(!stored.contains("PRIVATE_OUTPUT"));
            Ok(())
        })?;
        // La reserva falla antes del envío: ningún efecto adicional.
        db.with_connection(|c| {c.execute_batch("CREATE TRIGGER refuse_reservation BEFORE INSERT ON plugin_mcp_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END")?;Ok(())})?;
        let revision = allowed["revision"].as_i64().unwrap();
        assert!(
            service
                .execute(
                    "fixture",
                    "echo",
                    revision,
                    json!({"text":"PRIVATE_NOT_SENT"})
                )
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        db.with_connection(|c| {
            c.execute_batch("DROP TRIGGER refuse_reservation")?;
            Ok(())
        })?;
        let failed = service.execute("fixture", "echo", revision, json!({"text":"TOOL_ERROR"}))?;
        assert_eq!(failed["status"], "tool_failed");
        assert_eq!(failed["review_required"], true);
        let blocked = service.execute(
            "fixture",
            "echo",
            revision,
            json!({"text":"DIFFERENT_PRIVATE_ARGUMENT"}),
        )?;
        assert_eq!(blocked["status"], "review_required");
        assert_eq!(blocked["executed"], false);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let checked = service.reconcile(
            "fixture",
            revision,
            failed["attempt_id"].as_i64().unwrap(),
            "applied",
        )?;
        let stale = service.execute(
            "fixture",
            "echo",
            revision,
            json!({"text":"PRIVATE_NOT_REPEATED"}),
        )?;
        assert_eq!(stale["status"], "revision_conflict");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // La respuesta llega, pero el commit final falla: el intento sigue bloqueado.
        db.with_connection(|c| {c.execute_batch("CREATE TRIGGER refuse_completion BEFORE UPDATE ON plugin_mcp_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END")?;Ok(())})?;
        let current = checked["revision"].as_i64().unwrap();
        let uncertain =
            service.execute("fixture", "echo", current, json!({"text":"PRIVATE_OUTPUT"}))?;
        assert_eq!(uncertain["status"], "result_uncertain");
        assert_eq!(uncertain["attempt"]["reason"], "completion_unrecorded");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let blocked = service.execute(
            "fixture",
            "echo",
            current,
            json!({"text":"PRIVATE_NOT_REPEATED"}),
        )?;
        assert_eq!(blocked["status"], "review_required");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        db.with_connection(|c| {
            c.execute_batch("DROP TRIGGER refuse_completion")?;
            let mut query=c.prepare("SELECT server_name||tool||state||reason||COALESCE(resolution,'') FROM plugin_mcp_attempts")?;
            let stored=query.query_map([],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?.join(" ");
            assert!(!stored.contains("PRIVATE"));Ok(())
        })?;
        Ok(())
    })();
    service.stop().unwrap();
    server.stop().unwrap();
    db.close().unwrap();
    result.unwrap();
}

fn allowed_source(service: &Service) -> i64 {
    service.save(specification(), 0).unwrap();
    let discovered = service.store.catalogue("fixture", 1, &catalogue()).unwrap();
    service
        .permissions("fixture", discovered.revision, &["echo".into()], true)
        .unwrap()["revision"]
        .as_i64()
        .unwrap()
}
fn reserve(service: &Service, revision: i64) -> Attempt {
    match service
        .attempts
        .reserve("fixture", "echo", revision)
        .unwrap()
    {
        Reservation::Reserved(attempt) => attempt,
        _ => panic!("La fixture debía reservar un intento"),
    }
}

#[test]
fn pending_effects_survive_reopen_permissions_and_config_changes_without_implicit_retry() {
    let dir = Directory::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let path = dir.0.join("attempts.sqlite3");
    let db = Database::open(&path, 50).unwrap();
    db.migrate("plugins", &crate::MIGRATIONS[..6]).unwrap();
    let source = Store(db.clone()).save(&specification(), 0).unwrap();
    assert_eq!(source.revision, 1);
    db.migrate("plugins", crate::MIGRATIONS).unwrap();
    let service = test_service(db.clone(), dir.0.join("client"));
    let discovered = service.store.catalogue("fixture", 1, &catalogue()).unwrap();
    let allowed = service
        .permissions("fixture", discovered.revision, &["echo".into()], true)
        .unwrap();
    let attempt = reserve(&service, allowed["revision"].as_i64().unwrap());
    assert!(!dir.0.join("client").exists());
    // Simula un cierre sin confirmación, sin volver a enviar la llamada.
    service.stop().unwrap();
    drop(service);
    db.close().unwrap();
    let db = Database::open(&path, 50).unwrap();
    db.migrate("plugins", crate::MIGRATIONS).unwrap();
    let service = test_service(db.clone(), dir.0.join("client"));
    let recovered = service.get("fixture").unwrap();
    assert_eq!(recovered["pending_attempts"][0]["id"], attempt.id);
    assert_eq!(recovered["pending_attempts"][0]["state"], "uncertain");
    assert_eq!(recovered["pending_attempts"][0]["reason"], "interrupted");
    let result = service
        .execute(
            "fixture",
            "echo",
            recovered["revision"].as_i64().unwrap(),
            json!({"text":"CHANGED_PRIVATE_ARGUMENT"}),
        )
        .unwrap();
    assert_eq!(result["status"], "review_required");
    assert_eq!(result["executed"], false);
    assert!(!dir.0.join("client").exists());
    let revision = recovered["revision"].as_i64().unwrap();
    let disabled = service
        .permissions("fixture", revision, &[], false)
        .unwrap();
    let saved = service
        .save(specification(), disabled["revision"].as_i64().unwrap())
        .unwrap();
    assert_eq!(saved["pending_attempts"][0]["id"], attempt.id);
    assert!(
        service
            .delete("fixture", saved["revision"].as_i64().unwrap())
            .is_err()
    );
    assert!(
        service
            .reconcile("fixture", revision, attempt.id, "not_applied")
            .is_err()
    );
    let checked = service
        .reconcile(
            "fixture",
            saved["revision"].as_i64().unwrap(),
            attempt.id,
            "applied",
        )
        .unwrap();
    assert_eq!(checked["pending_count"], 0);
    assert_eq!(checked["recent_reviews"][0]["resolution"], "applied");
    assert!(checked["revision"].as_i64().unwrap() > saved["revision"].as_i64().unwrap());
    assert!(!checked.to_string().contains("CHANGED_PRIVATE_ARGUMENT"));
    service.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn reservation_and_review_failures_rollback_and_live_attempts_cannot_be_acknowledged() {
    let db = database();
    let dir = Directory::new();
    let service = test_service(db.clone(), dir.0.clone());
    let revision = allowed_source(&service);
    db.with_connection(|c| {c.execute_batch("CREATE TRIGGER reject_mcp_reservation BEFORE INSERT ON plugin_mcp_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END")?;Ok(())}).unwrap();
    assert!(
        service
            .attempts
            .reserve("fixture", "echo", revision)
            .is_err()
    );
    assert_eq!(service.get("fixture").unwrap()["pending_count"], 0);
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_mcp_reservation")?;
        Ok(())
    })
    .unwrap();
    let attempt = reserve(&service, revision);
    service.live_attempts.lock().unwrap().insert(attempt.id);
    {
        let _live = LiveAttempt {
            service: &service,
            id: attempt.id,
        };
        assert!(
            service
                .reconcile("fixture", revision, attempt.id, "applied")
                .is_err()
        );
        assert_eq!(
            service.get("fixture").unwrap()["pending_attempts"][0]["active"],
            true
        );
    }
    db.with_connection(|c| {c.execute_batch("CREATE TRIGGER reject_mcp_finish BEFORE UPDATE ON plugin_mcp_attempts BEGIN SELECT RAISE(ABORT,'fixture'); END")?;Ok(())}).unwrap();
    assert!(
        service
            .attempts
            .finish(attempt.id, "reported_success")
            .is_err()
    );
    let pending = service.get("fixture").unwrap();
    assert_eq!(pending["pending_attempts"][0]["state"], "uncertain");
    assert_eq!(
        pending["pending_attempts"][0]["reason"],
        "completion_unrecorded"
    );
    assert!(
        service
            .reconcile("fixture", revision, attempt.id, "not_applied")
            .is_err()
    );
    assert_eq!(service.get("fixture").unwrap()["revision"], revision);
    db.with_connection(|c| {
        c.execute_batch("DROP TRIGGER reject_mcp_finish")?;
        Ok(())
    })
    .unwrap();
    assert!(
        service
            .reconcile("fixture", revision, attempt.id, "invented")
            .is_err()
    );
    assert!(
        service
            .reconcile("fixture", revision, attempt.id + 100, "not_applied")
            .is_err()
    );
    assert_eq!(service.get("fixture").unwrap()["revision"], revision);
    let mut other = specification();
    other.id = "other".into();
    service.save(other, 0).unwrap();
    assert!(
        service
            .reconcile("other", 1, attempt.id, "not_applied")
            .is_err()
    );
    assert_eq!(service.get("other").unwrap()["revision"], 1);
    let checked = service
        .reconcile("fixture", revision, attempt.id, "not_applied")
        .unwrap();
    assert!(matches!(
        service
            .attempts
            .reserve("fixture", "echo", revision)
            .unwrap(),
        Reservation::Rejected("revision_conflict")
    ));
    let next = reserve(&service, checked["revision"].as_i64().unwrap());
    assert!(next.id > attempt.id);
    service
        .attempts
        .finish(next.id, "reported_success")
        .unwrap();
    assert_eq!(service.get("fixture").unwrap()["pending_count"], 0);
    assert!(service.attempts.finish(next.id, "transport_error").is_err());
    service.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn pending_attempts_have_stable_pages_and_do_not_expose_arguments_or_results() {
    let db = database();
    let dir = Directory::new();
    let service = test_service(db.clone(), dir.0.clone());
    service.save(specification(), 0).unwrap();
    let tools: serde_json::Map<String, Value> = (0..101)
        .map(|n| {
            let name = format!("tool_{n:03}");
            (
                name.clone(),
                json!({"name":name,"inputSchema":{"type":"object"}}),
            )
        })
        .collect();
    let discovered = service
        .store
        .catalogue("fixture", 1, &Value::Object(tools.clone()))
        .unwrap();
    let allowed = service
        .permissions(
            "fixture",
            discovered.revision,
            &tools.keys().cloned().collect::<Vec<_>>(),
            true,
        )
        .unwrap();
    let revision = allowed["revision"].as_i64().unwrap();
    for tool in tools.keys() {
        assert!(matches!(
            service.attempts.reserve("fixture", tool, revision).unwrap(),
            Reservation::Reserved(_)
        ));
    }
    let mut ids = Vec::new();
    let mut cursor = None;
    loop {
        let page = service.pending_page("fixture", cursor).unwrap();
        assert_eq!(page["total"], 101);
        assert!(page["attempts"].as_array().unwrap().len() <= 50);
        ids.extend(
            page["attempts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["id"].as_i64().unwrap()),
        );
        cursor = page["next_cursor"].as_i64();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 101);
    assert!(ids.windows(2).all(|v| v[0] < v[1]));
    assert_eq!(
        service.get("fixture").unwrap()["pending_attempts"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    assert_eq!(service.list().unwrap()[0]["pending_count"], 101);
    assert!(service.pending_page("fixture", Some(-1)).is_err());
    db.with_connection(|c| {
        let mut s = c.prepare("SELECT name FROM pragma_table_info('plugin_mcp_attempts')")?;
        let columns = s
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        assert!(!columns.iter().any(|name| name.contains("argument")
            || name.contains("result")
            || name.contains("payload")));
        Ok(())
    })
    .unwrap();
    service.stop().unwrap();
    db.close().unwrap();
}

#[test]
fn stop_waits_for_final_database_work_and_rejects_new_calls() {
    let db = database();
    let dir = Directory::new();
    let service = Arc::new(test_service(db.clone(), dir.0.clone()));
    let revision = allowed_source(&service);
    let attempt = reserve(&service, revision);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let worker = service.clone();
    let thread = std::thread::spawn(move || {
        let _operation = worker.activity.enter().unwrap();
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        worker
            .attempts
            .finish(attempt.id, "transport_error")
            .unwrap();
    });
    entered_rx.recv().unwrap();
    let stopping = service.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let stop = std::thread::spawn(move || {
        stopping.stop().unwrap();
        done_tx.send(()).unwrap();
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while service.activity.enter().is_ok() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_millis(25))
            .is_err()
    );
    release_tx.send(()).unwrap();
    thread.join().unwrap();
    stop.join().unwrap();
    done_rx.recv().unwrap();
    assert!(service.get("fixture").is_err());
    assert!(
        service
            .execute("fixture", "echo", revision, json!({"text":"not sent"}))
            .is_err()
    );
    assert_eq!(
        service
            .attempts
            .pending_tool("fixture", "echo")
            .unwrap()
            .unwrap()
            .state,
        "uncertain"
    );
    db.close().unwrap();
}

#[test]
fn scheduled_preflight_validates_locally_without_transport_or_attempt_reservation() {
    let directory = Directory::new();
    let db = database();
    let service = test_service(db.clone(), directory.0.clone());
    let revision = allowed_source(&service);
    let ready = service
        .prepare_execution("fixture", "echo", revision, json!({"text":"fixture"}))
        .unwrap();
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["executed"], false);
    assert_eq!(
        service
            .prepare_execution("fixture", "echo", revision, json!({}))
            .unwrap()["status"],
        "invalid_arguments"
    );
    assert_eq!(
        service
            .prepare_execution("fixture", "echo", revision - 1, json!({"text":"fixture"}))
            .unwrap()["status"],
        "revision_conflict"
    );
    db.with_connection(|c| {
        let count: i64 =
            c.query_row("SELECT count(*) FROM plugin_mcp_attempts", [], |r| r.get(0))?;
        assert_eq!(count, 0);
        Ok(())
    })
    .unwrap();
    service.stop().unwrap();
}
