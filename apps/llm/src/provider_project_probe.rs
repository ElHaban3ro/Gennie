//! Sonda aislada de lectura del proyecto con subagentes nativos.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn all_usage(dir: &Path, output: &mut Vec<Value>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            all_usage(&path, output);
            continue;
        }
        if path.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut id = None;
        let mut count = None;
        for line in text.lines() {
            let Ok(item) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if item["type"] == "session_meta" {
                id = item["payload"]["id"].as_str().map(str::to_owned);
            }
            if item["payload"]["type"] == "token_count" {
                count = item["payload"]["info"].get("total_token_usage").cloned();
            }
        }
        if let (Some(id), Some(count)) = (id, count) {
            let file = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let file_id = file.get(file.len().saturating_sub(36)..).unwrap_or("");
            output.push(json!({"thread":id,"file_thread":file_id,"usage":count}));
        }
    }
}

#[test]
#[ignore = "Sonda real de lectura de proyecto con subagentes; consume Codex"]
fn project_real_gennie_subagents() {
    let root = PathBuf::from(std::env::var_os("GENNIE_PROJECT_BASE").unwrap());
    let value: Value = serde_json::from_slice(
        &std::fs::read(std::env::var_os("GENNIE_PROJECT_SETTINGS").unwrap()).unwrap(),
    )
    .unwrap();
    let settings = Settings {
        read_files: true,
        write_files: false,
        run_commands: true,
        local_actions: false,
        model: value["model"].as_str().unwrap().into(),
        effort: value["effort"].as_str().unwrap().into(),
        personality: value["personality"].as_str().unwrap_or("").into(),
        instructions: value["instructions"].as_str().unwrap_or("").into(),
    };
    let process = ProcessHandle::default();
    let started = Instant::now();
    let result = (|| -> Result<Value> {
        let db = Database::memory()?;
        db.migrate(
            crate::LlmModule::default().name(),
            crate::LlmModule::default().migrations(),
        )?;
        let control = turn::Control::new(process.clone());
        let is_long = std::env::var("GENNIE_PROJECT_LONG").as_deref() == Ok("1");
        let scope = Scope::with_timeout(
            &control,
            Duration::from_secs(if is_long { 600 } else { 180 }),
        )?;
        let catalog = std::env::var("GENNIE_PROJECT_CATALOG")?;
        let config = vec![
            "web_search=\"disabled\"".into(),
            "features.multi_agent=true".into(),
            format!("agents.max_depth={}", if is_long { 2 } else { 1 }),
            format!("agents.max_threads={}", if is_long { 5 } else { 3 }),
            format!("model_catalog_json={}", serde_json::to_string(&catalog)?),
        ];
        let mut session = Session::spawn(
            process.clone(),
            root.clone(),
            &settings,
            None,
            &config,
            true,
            Some(scope.ticket.context.clone()),
        )?;
        session.bind_turn(Some(scope.ticket.clone()));
        session.authenticate_model(&settings)?;
        session.journal = Some(Arc::new(Journal::default()));
        let thread = session.open_thread(None, &instructions(&settings, &[]))?["thread"]["id"]
            .as_str()
            .ok_or("Falta hilo")?
            .to_owned();
        save_recorded_thread(&db, &thread, None, session.journal.as_deref())?;
        session.thread_id = thread.clone();
        let prompt = std::fs::read_to_string(
            std::env::var_os("GENNIE_PROJECT_PROMPT").ok_or("Falta tarea")?,
        )?;
        let mut messages = Vec::new();
        let (answer, protocol) = super::super::turn_observation::capture_with_root(&thread, || {
            session.speak_stream_reply(&prompt, None, |event| {
                if let StreamEvent::Completed(text) = event {
                    messages.push(text);
                }
            })
        });
        let mut diagnostic = json!({});
        super::diagnostics::finish(
            &mut diagnostic,
            &answer,
            started.elapsed().as_millis(),
            true,
        );
        let timed_out = scope.timed_out.load(Ordering::Acquire);
        drop(scope);
        session.bind_turn(None);
        drop(session);
        let mut counts = Vec::new();
        all_usage(&root.join("cli-home/sessions"), &mut counts);
        db.close()?;
        Ok(
            json!({"status":if answer.is_ok(){"turn_finished"}else{"turn_failed"},"failure":diagnostic["failure"],
            "scope_timed_out":timed_out,"thread_id":thread,"messages":messages,"protocol":protocol,"usage_by_thread":counts}),
        )
    })();
    let report = result.unwrap_or_else(|_| json!({"status":"setup_failed"}));
    let output = PathBuf::from(std::env::var_os("GENNIE_PROJECT_OUTPUT").unwrap());
    std::fs::write(&output, serde_json::to_vec(&report).unwrap()).unwrap();
    std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o600)).unwrap();
}
