use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
#[derive(Default)]
struct Journal(Mutex<Vec<(i64, String)>>);
impl BuildJournal for Journal {
    fn origin(&self) -> Result<i64> {
        Ok(7)
    }
    fn exists(&self, id: i64) -> Result<bool> {
        Ok(id == 7)
    }
    fn deliver(&self, id: i64, key: &str, _: &str) -> Result<()> {
        let mut events = self.0.lock().unwrap();
        if !events.iter().any(|(_, k)| k == key) {
            events.push((id, key.into()));
        }
        Ok(())
    }
}
struct Fixture {
    root: PathBuf,
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
    plugins: PluginsModule,
    browser: gennie_browser::BrowserModule,
    state: AppState,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.plugins.stop(&self.state);
        let _ = self.browser.stop(&self.state);
        self.stop.store(true, Ordering::Release);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
        let _ = self.state.db.close();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
#[ignore = "Consume una construcción breve con Codex real, Chromium headless y un sitio local; sin cuentas web personales"]
fn real_builder_creates_fixture_plugin_without_touching_the_chat_thread() {
    let root = std::env::temp_dir().join(format!(
        "gennie-builder-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("browser")).unwrap();
    let installed = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser");
    for name in ["runtime-1.64.1", "browsers"] {
        std::os::unix::fs::symlink(installed.join(name), root.join("browser").join(name)).unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let server = std::thread::spawn(move || {
        while !server_stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let mut buffer = [0; 4096];
                    let _ = stream.read(&mut buffer);
                    let body = r#"{"items":[{"id":"fixture-1","subject":"Dato de prueba"}]}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    });
    let db = Database::open(&root.join("fixture.sqlite3"), 500).unwrap();
    let state = AppState {
        db: db.clone(),
        config: Arc::new(gennie_config::Config {
            database_path: root.join("fixture.sqlite3"),
            db_busy_timeout_ms: 500,
        }),
        shutdown: Default::default(),
    };
    let browser = gennie_browser::BrowserModule::new(root.join("browser"));
    browser.start(&state).unwrap();
    browser.controller().set_automation_allowed(true);
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let journal = Arc::new(Journal::default());
    let plugins = PluginsModule::new(
        db.clone(),
        browser.controller(),
        journal.clone(),
        Arc::new(|| Ok(Settings::default())),
        helper,
        root.join("builds"),
    );
    db.migrate(plugins.name(), plugins.migrations()).unwrap();
    plugins.start(&state).unwrap();
    let fixture = Fixture {
        root,
        stop,
        server: Some(server),
        plugins,
        browser,
        state,
    };
    let controller = fixture.plugins.controller();
    let objective = format!(
        "Esta es una sonda de integración autorizada sobre un sitio local propio. Crea exactamente una función list_items: GET {origin}/items, parameters={{}}, result_type=array, result_pointer=/items, fields={{id:/id,subject:/subject}}. La respuesta conocida contiene fixture-1 y Dato de prueba. Guarda un candidato plugin_id=fixture, origin={origin}, name=Prueba local, description=Consulta local de prueba, capabilities=[Listar elementos], limitations=[Solo lectura local]. Verifica esa función con arguments={{}}, comprueba los datos y usa plugin_build_report state=ready. No necesitas explorar ni iniciar sesión: el endpoint es público y está documentado aquí. Evita herramientas innecesarias."
    );
    let job = controller
        .submit(
            BuildRequest {
                request_key: "smoke".into(),
                plugin_id: "fixture".into(),
                origin,
                objective,
                max_attempts: 2,
                max_seconds: 35,
            },
            Some(7),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(50);
    let finished = loop {
        let current = controller.0.store.get(job.id).unwrap();
        if !matches!(current.state.as_str(), "queued" | "running") {
            break current;
        }
        assert!(
            Instant::now() < deadline,
            "El trabajo no terminó dentro del límite de la sonda"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(finished.state, "completed", "{}", finished.summary);
    let published = controller.catalogue().unwrap();
    assert_eq!(published["plugins"][0]["manifest"]["id"], "fixture");
    let data = fixture
        .browser
        .controller()
        .run_web_plugin("fixture", "list_items", 1, json!({}))
        .unwrap();
    assert_eq!(data["data"][0]["id"], "fixture-1");
    assert!(journal.0.lock().unwrap().iter().all(|(id, _)| *id == 7));
    assert!(finished.thread_id.is_some());
    // El constructor no crea ni sustituye la tabla/hilo del asistente conversacional.
    assert!(
        db.with_connection(|c| Ok(c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='codex_conversation'",
            [],
            |r| r.get::<_, i64>(0)
        )?))
        .unwrap()
            == 0
    );
}
