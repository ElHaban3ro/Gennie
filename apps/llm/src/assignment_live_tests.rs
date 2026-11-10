//! Conversación acotada con herramientas y almacenamiento reales, sobre datos ficticios.
use super::*;
use gennie_core::AppModule;

#[test]
#[ignore = "Consume una conversación real de Codex con SQLite temporal"]
fn real_assignment_conversation_creates_on_first_call() {
    let db = Database::memory().unwrap();
    let llm = crate::LlmModule::default();
    db.migrate(llm.name(), llm.migrations()).unwrap();
    let assignments = gennie_assignments::AssignmentsModule;
    db.migrate(assignments.migration_namespace(), assignments.migrations())
        .unwrap();
    let store = gennie_assignments::Store::new(db.clone());
    let calls = Arc::new(Mutex::new(Vec::<(String, bool)>::new()));
    let mut registry = gennie_mcp::Registry::default();
    let wrapped = gennie_assignments::tools(store.clone())
        .into_iter()
        .map(|tool| {
            let name = tool.definition["name"].as_str().unwrap().to_owned();
            let calls = calls.clone();
            gennie_mcp::Tool {
                definition: tool.definition,
                call: Box::new(move |input| {
                    let fixture_input = input.clone();
                    let result = (tool.call)(input);
                    if let Err(error) = &result {
                        eprintln!("Argumentos ficticios: {fixture_input}");
                        eprintln!("Rechazo de fixture {name}: {error}");
                    }
                    calls.lock().unwrap().push((name.clone(), result.is_ok()));
                    result
                }),
            }
        });
    registry.register(wrapped).unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/gennie"));
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let base = std::env::temp_dir().join(format!(
        "gennie-assignment-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(base.clone());
    let settings = crate::settings::load(&db).unwrap();
    let process = ProcessHandle::default();
    let timeout = process.clone();
    let (finished, finish) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish.recv_timeout(Duration::from_secs(120)).is_err() {
            timeout.kill();
        }
    });
    let result = (|| -> Result<(String, String)> {
        let mut session = Session::connect_in_with_mode(
            &db,
            process,
            base,
            true,
            &settings,
            None,
            Some(server.launch()),
        )?;
        let first = session.speak("Quiero que revises mi correo de Gmail todos los días, me informes de lo importante y archives el spam y lo que no requiera mi atención. Conserva los demás mensajes. ¿Podemos hacerlo?")?;
        let second = session
            .speak("Sí, registra el encargo para las 09:00 de la mañana, hora de Bogotá.")?;
        Ok((first, second))
    })();
    let _ = finished.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    let (_, reply) = result.unwrap();
    let calls = calls.lock().unwrap();
    eprintln!("Llamadas MCP: {calls:?}; respuesta: {reply}");
    let creates: Vec<_> = calls
        .iter()
        .filter(|(name, _)| name == "assignments_create")
        .collect();
    assert_eq!(creates.len(), 1, "Se esperaba una sola creación");
    assert!(creates[0].1, "La creación falló");
    let records = store.list(0, 20).unwrap();
    assert_eq!(records.len(), 1);
    let d = &records[0].document;
    assert_eq!(d.timezone, "America/Bogota");
    assert_eq!(d.recurrence.as_ref().unwrap().local_time, "09:00");
    assert!(
        d.applications
            .iter()
            .any(|app| app.name.to_lowercase().contains("gmail"))
    );
}
