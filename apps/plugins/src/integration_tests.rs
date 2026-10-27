use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};

#[test]
#[ignore = "Consume una respuesta visual real de Codex; imagen sintética sin datos personales. Requiere Python con Pillow"]
fn real_builder_interprets_an_image_delivered_only_by_mcp() {
    let started = Instant::now();
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let alphabet = b"ACEFGHJKMNPQRTUVWXYZ23456789";
    let code: String = (0..6)
        .map(|i| alphabet[((seed >> (i * 8)) as usize) % alphabet.len()] as char)
        .collect();
    let python = std::env::var_os("GENNIE_VISUAL_TEST_PYTHON").unwrap_or_else(|| "python3".into());
    let font = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/assets/fonts/Inter-SemiBold.ttf");
    let output = std::process::Command::new(python)
        .args([
            "-c",
            r#"
from PIL import Image, ImageDraw, ImageFont
import io,base64,sys
image=Image.new('RGB',(800,240),'white')
draw=ImageDraw.Draw(image)
draw.text((36,26),'Panel de prueba',fill='black',font=ImageFont.truetype(sys.argv[2],24))
draw.text((36,76),sys.argv[1],fill='black',font=ImageFont.truetype(sys.argv[2],82))
buffer=io.BytesIO();image.save(buffer,format='PNG')
sys.stdout.write(base64.b64encode(buffer.getvalue()).decode())
"#,
            &code,
            font.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "La fixture necesita Python con Pillow"
    );
    let pixels = String::from_utf8(output.stdout).unwrap();
    let screenshot_called = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Mutex::new(None));
    let mut registry = gennie_mcp::Registry::default();
    let called = screenshot_called.clone();
    let received = observed.clone();
    registry.register([
        gennie_mcp::Tool {
            definition:json!({"name":"browser_screenshot","description":"Captura del panel de prueba. La información relevante solo está en la imagen; emite el bloque image para verla.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}),
            call:Box::new(move |_| {
                called.store(true,Ordering::Release);
                gennie_browser::tool_output("screenshot",json!({"page_id":"fixture","image":{"data":pixels,"mime_type":"image/png"}}))
            })
        },
        gennie_mcp::Tool {
            definition:json!({"name":"probe_result","description":"Registra el código que has leído visualmente en el panel.","inputSchema":{"type":"object","properties":{"observed_code":{"type":"string"}},"required":["observed_code"],"additionalProperties":false}}),
            call:Box::new(move |args| {
                let mut result=received.lock().unwrap();
                if result.is_some() { return Err("La sonda ya recibió una respuesta".into()); }
                *result=Some(args["observed_code"].as_str()==Some(code.as_str()));
                Ok(json!({"status":"recorded"}))
            })
        }
    ]).unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let root =
        std::env::temp_dir().join(format!("gennie-visual-probe-{}-{seed}", std::process::id()));
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(root.clone());
    let settings = Settings {
        read_files: false,
        write_files: false,
        run_commands: false,
        local_actions: false,
        model: std::env::var("GENNIE_PUBLIC_TEST_MODEL")
            .unwrap_or_else(|_| Settings::default().model),
        effort: std::env::var("GENNIE_PUBLIC_TEST_EFFORT")
            .unwrap_or_else(|_| Settings::default().effort),
    };
    let control = gennie_llm::builder::BuilderControl::default();
    let watchdog_control = control.clone();
    let (finished, finish) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish.recv_timeout(Duration::from_secs(30)).is_err() {
            watchdog_control.cancel();
        }
    });
    let result = (|| -> Result<()> {
        let instructions = "Eres un verificador visual de una fixture. Usa solo las herramientas MCP de la prueba. Consulta browser_screenshot y examina la imagen real; si orquestas con exec, emite los bloques content de type=image con image(bloque), no imprimas base64 como texto. Luego llama probe_result con el código de seis caracteres que ves. No hay información textual alternativa ni hace falta investigar APIs, archivos o sitios web. Termina con una frase breve.";
        let mut session = gennie_llm::builder::BuilderSession::connect(
            root,
            &settings,
            server.launch(),
            instructions.into(),
            None,
            control,
        )?;
        session.turn("Lee el código visible del panel y comunícalo a probe_result.")
    })();
    let _ = finished.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    result.unwrap();
    assert!(screenshot_called.load(Ordering::Acquire));
    assert_eq!(
        *observed.lock().unwrap(),
        Some(true),
        "El modelo no interpretó correctamente la imagen MCP"
    );
    eprintln!(
        "visual_probe image_requested=true visual_only_code_correct=true elapsed_seconds={:.2}",
        started.elapsed().as_secs_f64()
    );
}
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
    for name in ["runtime-patchright-1.64.2", "browsers"] {
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
                retry_of: None,
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

#[test]
#[ignore = "Consume una construcción real sobre una página pública sin autenticación y Chromium headless aislado"]
fn real_builder_generates_and_reuses_a_public_page_plugin() {
    let root = std::env::temp_dir().join(format!(
        "gennie-public-builder-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("browser")).unwrap();
    let installed = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Application Support/Gennie/browser");
    for name in ["runtime-patchright-1.64.2", "browsers"] {
        std::os::unix::fs::symlink(installed.join(name), root.join("browser").join(name)).unwrap();
    }
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
    browser.controller().set_file_access_allowed(true);
    browser.controller().set_program_allowed(true);
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let settings = Settings {
        model: std::env::var("GENNIE_PUBLIC_TEST_MODEL")
            .unwrap_or_else(|_| Settings::default().model),
        effort: std::env::var("GENNIE_PUBLIC_TEST_EFFORT")
            .unwrap_or_else(|_| Settings::default().effort),
        ..Settings::default()
    };
    eprintln!(
        "public_test model={} effort={}",
        settings.model, settings.effort
    );
    let journal = Arc::new(Journal::default());
    let plugins = PluginsModule::new(
        db.clone(),
        browser.controller(),
        journal.clone(),
        Arc::new(move || Ok(settings.clone())),
        helper,
        root.join("builds"),
    );
    db.migrate(plugins.name(), plugins.migrations()).unwrap();
    plugins.start(&state).unwrap();
    let fixture = Fixture {
        root,
        stop: Arc::new(AtomicBool::new(false)),
        server: None,
        plugins,
        browser,
        state,
    };
    let controller = fixture.plugins.controller();
    let started = Instant::now();
    let job = controller.submit(BuildRequest {
        request_key:"public-page-test".into(), plugin_id:"public_elements".into(), origin:"https://the-internet.herokuapp.com".into(),
        objective:"Prueba autorizada exclusivamente en https://the-internet.herokuapp.com/add_remove_elements/, página pública de demostración sin autenticación. Genera un plugin reutilizable con exactamente una función exercise_elements, parameters={count:integer}, transport=program, path=/add_remove_elements/, result_type=object. Investiga la página y usa los controles observados Add Element y Delete. En una pestaña nueva debe añadir count elementos, comprobar que hay count botones Delete, eliminarlos y confirmar que quedan cero. Usa runtime.read para las lecturas y runtime.action con verificación booleana para las acciones; devuelve {added:count,remaining:0}. Verifica con count=2, comprueba los resultados reales y comunica ready cuando la función esté comprobada. No accedas a otras webs ni cuentas, no solicites login y no programes tareas periódicas. Puedes modificar únicamente el DOM de esta página ficticia. No se te proporciona el código del plugin: debes generarlo y verificarlo mediante tus herramientas.".into(),
        max_attempts:8, max_seconds:600, retry_of:None,
    },Some(7)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut last = String::new();
    let finished = loop {
        let current = controller.0.store.get(job.id).unwrap();
        if current.summary != last {
            eprintln!(
                "public_test state={} summary={}",
                current.state, current.summary
            );
            last = current.summary.clone();
        }
        if !matches!(current.state.as_str(), "queued" | "running") {
            break current;
        }
        assert!(
            Instant::now() < deadline,
            "La prueba superó 120 s: {}",
            current.summary
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(finished.state, "completed", "{}", finished.summary);
    let catalogue = controller.catalogue().unwrap();
    let published = catalogue["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["manifest"]["id"] == "public_elements")
        .unwrap();
    assert_eq!(
        published["manifest"]["functions"].as_array().unwrap().len(),
        1
    );
    let revision = published["revision"].as_u64().unwrap();
    for count in [3, 1] {
        let result = fixture
            .browser
            .controller()
            .run_web_plugin(
                "public_elements",
                "exercise_elements",
                revision,
                json!({"count":count}),
            )
            .unwrap();
        assert_eq!(result["status"], "ok", "{result}");
        assert_eq!(result["data"]["added"], count);
        assert_eq!(result["data"]["remaining"], 0);
        eprintln!("public_test reuse count={count} result={}", result["data"]);
    }
    assert!(finished.thread_id.is_some());
    assert!(journal.0.lock().unwrap().iter().all(|(id, _)| *id == 7));
    assert_eq!(
        db.with_connection(|c| Ok(c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='codex_conversation'",
            [],
            |r| r.get::<_, i64>(0)
        )?))
        .unwrap(),
        0
    );
    eprintln!(
        "public_test completed elapsed_seconds={:.2} verified_and_reused=true isolated_profile=true",
        started.elapsed().as_secs_f64()
    );
}
