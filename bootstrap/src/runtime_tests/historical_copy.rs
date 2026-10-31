//! Solo migra y lee la copia privada preparada por historical_migration.py.
use super::*;
use gennie_presence::assistants::{self, Cursor};
use serde_json::json;
use std::{collections::BTreeSet, io::Write, os::unix::fs::OpenOptionsExt, time::Instant};

#[test]
#[ignore = "Copia histórica privada: requiere el lanzador, sin UI, modelos o servicios"]
fn historical_copy_migrates_reopens_and_pages_without_effectful_modules() {
    let path =
        PathBuf::from(std::env::var_os("GENNIE_TEST_MIGRATION_COPY").expect("Falta copia propia"));
    assert!(path.is_absolute() && path.is_file());
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some("audit.sqlite3")
    );
    let directory = path.parent().unwrap();
    assert!(
        directory
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("gennie-migration-copy-"))
    );
    assert!(!path.is_symlink());
    let config = Config {
        database_path: path.clone(),
        db_busy_timeout_ms: 500,
    };
    let check = Database::open(&path, 500).unwrap();
    check
        .with_connection(|c| {
            let active: i64 =
                c.query_row("SELECT count(*) FROM modules WHERE active<>0", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(active, 0);
            Ok(())
        })
        .unwrap();
    check.close().unwrap();
    let helper = std::env::current_exe().unwrap();
    let started = Instant::now();
    let mut backend = Backend::open_config(config.clone(), helper.to_str().unwrap())
        .expect("Falló migración de la copia; inspeccionar el log privado");
    let migration_open_us = started.elapsed().as_micros();
    assert!(backend.started.is_empty());
    assert!(backend.llm.is_none() && backend.executions.is_none());
    let mcp = backend.mcp.as_ref().unwrap();
    assert!(mcp.launch().tool_names.is_empty() && !mcp.has_clients());
    let expected = backend.state.db.with_connection(|c| {
        let mut query = c.prepare("SELECT h.id FROM presence_history h JOIN presence_conversations s ON s.id=h.conversation_id JOIN presence_assistant_conversations a ON a.conversation_id=s.id WHERE s.deleted=0 AND a.assistant_id='gennie'")?;
        Ok(query.query_map([], |r|r.get::<_,i64>(0))?.collect::<std::result::Result<BTreeSet<_>,_>>()?)
    }).unwrap();
    let (sessions, linked) = backend.state.db.with_connection(|c| {
        Ok((c.query_row("SELECT count(*) FROM presence_conversations", [], |r|r.get::<_,i64>(0))?,
            c.query_row("SELECT count(*) FROM presence_assistant_conversations WHERE assistant_id='gennie'", [], |r|r.get::<_,i64>(0))?))
    }).unwrap();
    assert_eq!(sessions, linked);
    let mut cursor: Option<Cursor> = None;
    let mut seen = BTreeSet::new();
    let mut pages = Vec::new();
    let mut first_measurement = None;
    loop {
        let started = Instant::now();
        let page =
            assistants::page(&backend.state.db, "gennie", cursor.as_ref(), 60, 24_000).unwrap();
        let elapsed_us = started.elapsed().as_micros();
        assert!(page.messages.len() <= 60);
        let characters: usize = page.messages.iter().map(|m| m.text.chars().count()).sum();
        let page_sessions = page
            .messages
            .iter()
            .map(|m| m.conversation_id)
            .collect::<BTreeSet<_>>()
            .len();
        assert!(page_sessions <= 3);
        assert!(characters <= 24_000 || page.messages.len() == 1);
        if first_measurement.is_none() {
            first_measurement = Some(
                json!({"messages":page.messages.len(),"text_characters":characters,"sessions":page_sessions,"elapsed_us":elapsed_us}),
            );
        }
        for entry in &page.messages {
            assert!(seen.insert(entry.id), "Mensaje duplicado entre páginas");
        }
        assert!(pages.len() <= expected.len() + 1);
        pages.push(json!({"messages":page.messages.len(),"text_characters":characters,"sessions":page_sessions,"elapsed_us":elapsed_us}));
        if let Some(next) = &page.before {
            assert!(!page.messages.is_empty());
            if let Some(previous) = &cursor {
                assert!(
                    (&next.created_at, next.message_id)
                        < (&previous.created_at, previous.message_id)
                );
            }
        }
        cursor = page.before;
        if cursor.is_none() {
            break;
        }
    }
    assert!(
        seen == expected,
        "La paginación no cubre exactamente los mensajes visibles"
    );
    let profile = assistants::load(&backend.state.db, "gennie").unwrap();
    assert_eq!(profile.name, "Gennie");
    let mut repeats = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        assistants::page(&backend.state.db, "gennie", None, 60, 24_000).unwrap();
        repeats.push(started.elapsed().as_micros());
    }
    backend.close().unwrap();
    let started = Instant::now();
    let mut reopened = Backend::open_config(config, helper.to_str().unwrap()).unwrap();
    let reopen_us = started.elapsed().as_micros();
    assert!(reopened.started.is_empty());
    let latest = assistants::page(&reopened.state.db, "gennie", None, 60, 24_000).unwrap();
    assert_eq!(
        latest.messages.len(),
        first_measurement.as_ref().unwrap()["messages"]
            .as_u64()
            .unwrap() as usize
    );
    reopened.close().unwrap();
    let report = json!({"migration_open_us":migration_open_us,"reopen_us":reopen_us,"active_modules_started":0,"empty_local_mcp_host_without_clients":true,"sessions":sessions,"linked_sessions":linked,"visible_messages":seen.len(),"pages":pages,"first_measured_page":first_measurement,"repeated_first_page_us":repeats,"reopen_page_length_preserved":true});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("report.json"))
        .unwrap();
    serde_json::to_writer_pretty(&mut file, &report).unwrap();
    file.write_all(b"\n").unwrap();
}
