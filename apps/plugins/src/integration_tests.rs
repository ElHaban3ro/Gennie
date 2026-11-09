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
        ..Default::default()
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
    fn target(&self, conversation_id: i64) -> Result<gennie_llm::delegation::Origin> {
        Ok(gennie_llm::delegation::Origin {
            conversation_id,
            bot_id: "gennie".into(),
            epoch: 1,
            provider_thread: None,
        })
    }
    fn deliver(&self, update: &gennie_llm::delegation::Update) -> Result<()> {
        let id = update.origin.conversation_id;
        let key = update.event_key.as_str();
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

#[test]
#[ignore = "Comparación real de construcción de plugin, con Codex y navegador headless temporal"]
fn real_plugin_agent_comparison() {
    struct ScopedBrowser {
        root: PathBuf,
        browser: gennie_browser::BrowserModule,
        state: AppState,
    }
    impl Drop for ScopedBrowser {
        fn drop(&mut self) {
            let _ = self.browser.stop(&self.state);
            let _ = self.state.db.close();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    let origin = std::env::var("GENNIE_PILOT_ORIGIN").unwrap();
    assert!(origin.starts_with("http://127.0.0.1:"));
    let root = std::env::temp_dir().join(format!(
        "gennie-real-agent-{}-{}",
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
    db.migrate(browser.name(), browser.migrations()).unwrap();
    browser.start(&state).unwrap();
    browser.controller().set_automation_allowed(true);
    let fixture = ScopedBrowser {
        root,
        browser,
        state,
    };
    let mut registry = gennie_mcp::Registry::default();
    let mut allowed = Vec::new();
    for tool in gennie_browser::tools(fixture.browser.controller()) {
        let name = tool.definition["name"].as_str().unwrap_or("").to_owned();
        if ![
            "browser_open",
            "browser_navigate",
            "browser_snapshot",
            "browser_read",
            "browser_plugins",
            "browser_plugin_run",
        ]
        .contains(&name.as_str())
        {
            continue;
        }
        let permitted_origin = origin.clone();
        allowed.push(gennie_mcp::Tool {
            definition: tool.definition,
            call: Box::new(move |mut args| {
                if name == "browser_open" {
                    args["background"] = json!(true);
                }
                if name == "browser_navigate"
                    && args["url"].as_str() != Some(format!("{permitted_origin}/main").as_str())
                {
                    return Err("Navegación fuera del sitio sintético".into());
                }
                if name == "browser_plugins" && args["action"] == "save" {
                    let m = &args["manifest"];
                    if m["id"] != "fixture"
                        || m["origin"] != permitted_origin
                        || !m["login_url"].is_null()
                        || !m["login_urls"].is_null()
                        || !m["functions"].as_array().is_some_and(|fs| {
                            !fs.is_empty()
                                && fs.iter().all(|f| {
                                    f["transport"] == "page"
                                        && f["path"] == "/main"
                                        && f["code"].is_null()
                                })
                        })
                    {
                        return Err(
                            "El piloto solo admite funciones de lectura page del sitio sintético"
                                .into(),
                        );
                    }
                }
                if name == "browser_plugins"
                    && !["get", "list", "save"].contains(&args["action"].as_str().unwrap_or(""))
                {
                    return Err("Acción no disponible en el piloto".into());
                }
                if name == "browser_plugin_run"
                    && (args["plugin_id"] != "fixture"
                        || !["verify", "run"].contains(&args["action"].as_str().unwrap_or("")))
                {
                    return Err("Solo se permite ejecutar el plugin del piloto".into());
                }
                (tool.call)(args)
            }),
        });
    }
    registry.register(allowed).unwrap();
    let helper =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let child_output = fixture.root.join("agent-result.json");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/probes/pilot_real_plugin_agent.py");
    let started = Instant::now();
    let status = std::process::Command::new("python3")
        .arg(script)
        .env("GENNIE_REAL_MCP_HELPER", server.launch().helper)
        .env("GENNIE_REAL_MCP_SOCKET", server.launch().socket)
        .env("GENNIE_REAL_AGENT_OUTPUT", &child_output)
        .status()
        .unwrap();
    let agent: Value = std::fs::read(&child_output)
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or(Value::Null);
    let catalog = fixture
        .browser
        .controller()
        .call(json!({"method":"plugins","action":"get","plugin_id":"fixture"}))
        .unwrap();
    let revision = catalog["plugin"]["revision"].as_u64();
    let result = revision.and_then(|revision| {
        fixture
            .browser
            .controller()
            .run_web_plugin("fixture", "extract", revision, json!({}))
            .ok()
    });
    server.stop().unwrap();
    let output = std::env::var_os("GENNIE_PILOT_OUTPUT").unwrap();
    std::fs::write(
        output,
        serde_json::to_vec(&json!({"agent":agent,"exit_code":status.code(),
        "result":result,"elapsed_ms":started.elapsed().as_millis()}))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "Verifica una receta sintética en Chromium headless con perfil temporal"]
fn real_browser_checks_comparison_manifest() {
    struct BrowserFixture {
        root: PathBuf,
        browser: gennie_browser::BrowserModule,
        state: AppState,
    }
    impl Drop for BrowserFixture {
        fn drop(&mut self) {
            let _ = self.browser.stop(&self.state);
            let _ = self.state.db.close();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    let manifest_path = std::env::var_os("GENNIE_PILOT_MANIFEST_FILE").expect("Falta manifiesto");
    let manifest: Value = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
    let origin = manifest["origin"].as_str().unwrap();
    assert!(origin.starts_with("http://127.0.0.1:"));
    let root = std::env::temp_dir().join(format!(
        "gennie-recipe-browser-{}-{}",
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
    db.migrate(browser.name(), browser.migrations()).unwrap();
    browser.start(&state).unwrap();
    browser.controller().set_automation_allowed(true);
    browser.controller().set_program_allowed(true);
    let fixture = BrowserFixture {
        root,
        browser,
        state,
    };
    let started = Instant::now();
    let save = fixture
        .browser
        .controller()
        .call(json!({"method":"plugins","action":"save","manifest":manifest,"expected_revision":0}))
        .unwrap();
    let verify=fixture.browser.controller().automate(json!({"method":"plugin_run","action":"verify", "plugin_id":"fixture", "function_name":"extract", "expected_revision":1,"arguments":{}})).unwrap();
    let run = fixture
        .browser
        .controller()
        .run_web_plugin("fixture", "extract", 1, json!({}))
        .ok();
    let output = std::env::var_os("GENNIE_PILOT_OUTPUT").expect("Falta resultado");
    std::fs::write(
        output,
        serde_json::to_vec(&json!({"save":save,"verify":verify,"run":run,
        "elapsed_ms":started.elapsed().as_millis()}))
        .unwrap(),
    )
    .unwrap();
}

// Ensayo optativo aislado: usa el constructor real y un servidor sintético externo al test.
#[test]
#[ignore = "Piloto real acotado con Codex, Chromium headless y servidor loopback propio"]
fn real_builder_runs_comparison_fixture() {
    fn collect_usage(dir: &std::path::Path, thread_id: &str, out: &mut Vec<Value>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_usage(&path, thread_id, out);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                let Ok(text) = std::fs::read_to_string(path) else {
                    continue;
                };
                let mut matches = false;
                let mut usage = None;
                for line in text.lines() {
                    let Ok(item) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    if item["type"] == "session_meta" && item["payload"]["id"] == thread_id {
                        matches = true;
                    }
                    if item["payload"]["type"] == "token_count" {
                        usage = item["payload"]["info"].get("total_token_usage").cloned();
                    }
                }
                if matches && let Some(value) = usage {
                    out.push(value);
                }
            }
        }
    }
    let origin = std::env::var("GENNIE_PILOT_ORIGIN").expect("Falta origen del sitio sintético");
    assert!(origin.starts_with("http://127.0.0.1:"));
    let case = std::env::var("GENNIE_PILOT_CASE").expect("Falta caso");
    assert!(["simple", "fallback", "compound"].contains(&case.as_str()));
    let settings_path =
        std::env::var_os("GENNIE_PILOT_SETTINGS_FILE").expect("Falta perfil de prueba");
    let settings_json =
        std::fs::read_to_string(settings_path).expect("No se pudo leer el perfil temporal");
    let settings_value: Value = serde_json::from_str(&settings_json).unwrap();
    let settings = Settings {
        model: settings_value["model"].as_str().unwrap().into(),
        effort: settings_value["effort"].as_str().unwrap().into(),
        personality: settings_value["personality"].as_str().unwrap_or("").into(),
        instructions: settings_value["instructions"].as_str().unwrap_or("").into(),
        read_files: settings_value["read_files"].as_bool().unwrap(),
        write_files: settings_value["write_files"].as_bool().unwrap(),
        run_commands: settings_value["run_commands"].as_bool().unwrap(),
        local_actions: settings_value["local_actions"].as_bool().unwrap(),
    };
    let root = std::env::temp_dir().join(format!(
        "gennie-comparison-{}-{}",
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
    db.migrate(browser.name(), browser.migrations()).unwrap();
    browser.start(&state).unwrap();
    browser.controller().set_automation_allowed(true);
    let names_only = std::env::var_os("GENNIE_PILOT_NAMES").is_some();
    browser.controller().set_program_allowed(
        !names_only
            && settings.read_files
            && settings.write_files
            && settings.run_commands
            && settings.local_actions,
    );
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let journal = Arc::new(Journal::default());
    let plugins = PluginsModule::new(
        db.clone(),
        browser.controller(),
        journal,
        crate::AgentSettings::shared(Arc::new(move || Ok(settings.clone()))),
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
    let fields = if case == "compound" {
        "Devuelve objeto con emails (enlaces mailto) y dates (atributos datetime)."
    } else {
        "Devuelve objeto con emails que contenga todos los enlaces mailto."
    };
    let objective = if names_only {
        format!(
            "Crea y verifica un plugin de solo lectura id fixture, función extract sin parámetros, que obtenga todos los nombres del directorio en un array de cadenas. El sitio autorizado es {origin}/main. La función debe usar transport=page y path=/main; guarda y verifica el plugin real. No uses otros destinos."
        )
    } else {
        format!(
            "Sitio sintético autorizado: {origin}/main, alternativa {origin}/backup. {fields} Crea un plugin reutilizable con id fixture y función extract sin parámetros, que consulte el DOM y devuelva el objeto solicitado. Prueba la función y publica únicamente si se verifica."
        )
    };
    let started = Instant::now();
    let job = controller
        .submit(
            BuildRequest {
                request_key: "comparison".into(),
                plugin_id: "fixture".into(),
                origin,
                objective,
                max_attempts: 4,
                max_seconds: 150,
                retry_of: None,
            },
            Some(7),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(150);
    let finished = loop {
        let current = controller.0.store.get(job.id).unwrap();
        if !matches!(current.state.as_str(), "queued" | "running") {
            break current;
        }
        if Instant::now() >= deadline {
            let _ = controller.cancel(job.id);
            break controller.0.store.get(job.id).unwrap();
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let result = if finished.state == "completed" {
        fixture
            .browser
            .controller()
            .run_web_plugin("fixture", "extract", 1, json!({}))
            .ok()
    } else {
        None
    };
    let mut usage = Vec::new();
    if let Some(thread) = &finished.thread_id {
        collect_usage(&fixture.root.join("builds"), thread, &mut usage);
    }
    let output = std::env::var("GENNIE_PILOT_OUTPUT").expect("Falta ruta del resultado");
    std::fs::write(
        output,
        serde_json::to_vec(&json!({"case":case,"state":finished.state,
        "elapsed_ms":started.elapsed().as_millis(),"attempts":finished.attempts,
        "thread_id":finished.thread_id,"output":result,"usage_candidates":usage,"summary":finished.summary}))
        .unwrap(),
    )
    .unwrap();
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
        crate::AgentSettings::shared(Arc::new(|| Ok(Settings::default()))),
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
        crate::AgentSettings::shared(Arc::new(move || Ok(settings.clone()))),
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
