//! Prueba de elección de herramientas con Codex real y navegador simulado, sin ventanas.
use super::*;
use gennie_core::AppModule;

#[test]
fn capabilities_distinguish_missing_tools_from_disabled_permissions() {
    let mut settings = Settings::default();
    let tools = vec![
        "browser_open".into(),
        "browser_navigate".into(),
        "browser_snapshot".into(),
    ];
    let ready = session_capabilities(&settings, &tools);
    assert_eq!(ready["gennie_web"]["available"], true);
    assert_eq!(ready["gennie_web"]["tools"], json!(tools));
    assert_eq!(ready["commands"]["network_enabled"], false);
    assert_eq!(
        ready["computer_control"]["system_permission_verified"],
        false
    );

    let absent = session_capabilities(&settings, &[]);
    assert_eq!(absent["gennie_web"]["available"], false);
    assert_eq!(absent["gennie_web"]["tools_registered"], false);
    assert_eq!(absent["gennie_web"]["permission_enabled"], true);
    let incomplete = session_capabilities(&settings, &tools[..2]);
    assert_eq!(incomplete["gennie_web"]["available"], false);

    settings.local_actions = false;
    let disabled = session_capabilities(&settings, &tools);
    assert_eq!(disabled["gennie_web"]["available"], false);
    assert_eq!(disabled["gennie_web"]["tools_registered"], true);
    assert_eq!(disabled["gennie_web"]["permission_enabled"], false);
    assert_eq!(disabled["computer_control"]["permission_enabled"], false);
    settings.run_commands = false;
    assert_eq!(
        session_capabilities(&settings, &tools)["commands"]["permission_enabled"],
        false
    );
}

#[test]
#[ignore = "Consume una respuesta real de ChatGPT; MCP simulado sin navegador ni acceso a cuentas"]
fn real_model_uses_browser_api_after_direct_access_failure() {
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let current = Arc::new(Mutex::new(None::<String>));
    let mut registry = gennie_mcp::Registry::default();
    let tools = [
        ("browser_open", "Abre el navegador de Gennie y devuelve la pestaña activa.", json!({"type":"object","properties":{}})),
        ("browser_navigate", "Navega la pestaña del navegador de Gennie a una URL.", json!({"type":"object","properties":{"url":{"type":"string"},"page_id":{"type":"string"}},"required":["url"]})),
        ("browser_snapshot", "Lee pestañas y contenido accesible del navegador de Gennie.", json!({"type":"object","properties":{"page_id":{"type":"string"}}})),
    ].into_iter().map(|(name, description, schema)| {
        let calls = calls.clone();
        let current = current.clone();
        gennie_mcp::Tool {
            definition: json!({"name":name,"description":description,"inputSchema":schema}),
            call: Box::new(move |args| {
                calls.lock().unwrap().push(name.into());
                let mut current = current.lock().unwrap();
                Ok(match name {
                    "browser_open" => { *current = Some("about:blank".into()); json!({"open":true,"page_id":"1"}) }
                    "browser_navigate" => {
                        assert_eq!(args["url"], "https://example.com/post/42");
                        *current = Some(args["url"].as_str().unwrap().into());
                        json!({"ok":true,"page_id":"1","url":args["url"]})
                    }
                    _ if current.as_deref() == Some("https://example.com/post/42") => json!({"open":true,"page_id":"1","tabs":[{"page_id":"1","url":"https://example.com/post/42","title":"Resultado de la prueba"}],"snapshot":"- article: La palabra publicada es ORBITA-42. Autor: Laboratorio de pruebas."}),
                    _ => json!({"open":current.is_some(),"tabs":[]}),
                })
            }),
        }
    });
    registry.register(tools).unwrap();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let base = std::env::temp_dir().join(format!(
        "gennie-browser-policy-{}-{}",
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
    let db = Database::memory().unwrap();
    let module = crate::LlmModule::default();
    db.migrate(module.name(), module.migrations()).unwrap();
    let process = ProcessHandle::default();
    let timeout_process = process.clone();
    let (finished, finish) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish.recv_timeout(Duration::from_secs(30)).is_err() {
            timeout_process.kill();
        }
    });
    let result = (|| -> Result<String> {
        let mut session = Session::connect_in_with_mode(
            &db,
            process,
            base,
            true,
            &Settings::default(),
            None,
            Some(server.launch()),
        )?;
        session.speak("Prueba de lectura: la descarga directa de https://example.com/post/42 ya falló con HTTP 403. Comprueba qué palabra publicó el autor y respóndeme con ella. No ejecutes comandos ni uses la interfaz del ordenador; las herramientas de navegador de esta prueba son simuladas y no acceden a sitios ni cuentas reales.")
    })();
    let _ = finished.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    let reply = result.unwrap();
    let calls = calls.lock().unwrap();
    eprintln!("Secuencia MCP elegida: {calls:?}");
    assert!(
        calls.iter().any(|name| name == "browser_open"),
        "No abrió el navegador por API: {reply}"
    );
    assert!(
        calls.iter().any(|name| name == "browser_navigate"),
        "No navegó por API: {reply}"
    );
    assert!(
        calls.iter().any(|name| name == "browser_snapshot"),
        "No observó la página por API: {reply}"
    );
    assert!(
        reply.contains("ORBITA-42"),
        "No respondió con el dato observado: {reply}"
    );
}
