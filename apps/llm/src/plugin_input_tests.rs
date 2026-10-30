//! Escenario conversacional optativo; catálogo y efectos ficticios, sin sitios externos.
use super::*;
use gennie_core::AppModule;

#[test]
#[ignore = "Consume dos respuestas ChatGPT; MCP ficticio, sin navegador, cuentas ni efectos reales"]
fn real_model_asks_for_missing_plugin_input_and_resumes_without_an_assignment() {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let sent = Arc::new(Mutex::new(Vec::<Value>::new()));
    let plugin = json!({"manifest":{"id":"fixture_messages","name":"Mensajes de prueba","origin":"https://example.com",
        "functions":[{"name":"send_message","description":"Enviar un mensaje ficticio a la persona indicada. La única persona Cata es inequívoca en esta prueba.",
            "parameters":{"recipient":{"type":"string","label":"Destinatario","description":"Persona que nombró el usuario"},
                "message":{"type":"string","label":"Mensaje","description":"Texto exacto que el usuario quiere enviar"}},
            "transport":"program","path":"/","result_type":"object"}]},"revision":1,
        "verified":{"send_message":{"revision":1,"verified_at":"2026-10-28T00:00:00.000Z"}}});
    let mut registry = gennie_mcp::Registry::default();
    for (name, description, schema) in [
        (
            "browser_plugins",
            "Consulta el catálogo de plugins y sus funciones/parámetros con action list/get.",
            json!({"type":"object","properties":{"action":{"type":"string","enum":["list","get"]},"plugin_id":{"type":"string"}},"required":["action"],"additionalProperties":false}),
        ),
        (
            "browser_plugin_run",
            "Ejecuta una función publicada usando los parámetros declarados; una petición puntual no necesita Encargo.",
            json!({"type":"object","properties":{"action":{"type":"string","enum":["run","verify"]},"plugin_id":{"type":"string"},"function_name":{"type":"string"},"expected_revision":{"type":"integer"},"arguments":{"type":"object","additionalProperties":true}},"required":["action","plugin_id","function_name","expected_revision","arguments"],"additionalProperties":false}),
        ),
        (
            "plugin_build_start",
            "Construye una función que no exista; no sirve para completar datos de una petición.",
            json!({"type":"object","additionalProperties":true}),
        ),
        (
            "assignments_create",
            "Registra un encargo cuando lo solicita el usuario.",
            json!({"type":"object","additionalProperties":true}),
        ),
    ] {
        let calls = calls.clone();
        let sent = sent.clone();
        let plugin = plugin.clone();
        registry.register([gennie_mcp::Tool {
            definition:json!({"name":name,"description":description,"inputSchema":schema}),
            call:Box::new(move |args| {
                calls.lock().unwrap().push((name.into(),args.clone()));
                match name {
                    "browser_plugins" => Ok(if args["action"] == "get" { json!({"plugin":plugin}) } else { json!({"plugins":[plugin]}) }),
                    "browser_plugin_run" => {
                        let arguments = &args["arguments"];
                        if args["plugin_id"] != "fixture_messages" || args["function_name"] != "send_message" || args["expected_revision"] != 1 {
                            return Ok(json!({"status":"revision_conflict","executed":false}));
                        }
                        if arguments["recipient"].as_str().is_none() || arguments["message"].as_str().is_none_or(str::is_empty) {
                            return Ok(json!({"status":"input_required","executed":false,"fields":[{"name":"message","type":"string","label":"Mensaje","reason":"required"}]}));
                        }
                        sent.lock().unwrap().push(arguments.clone());
                        Ok(json!({"status":"ok","plugin_id":"fixture_messages","function_name":"send_message","plugin_name":"Mensajes de prueba","data":{"sent":true}}))
                    }
                    _ => Err("La prueba no autoriza crear encargos ni construir funciones".into()),
                }
            }),
        }]).unwrap();
    }
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie")
        });
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let base = std::env::temp_dir().join(format!(
        "gennie-plugin-input-{}-{}",
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
    let deadline = process.clone();
    let (done, finish) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish.recv_timeout(Duration::from_secs(60)).is_err() {
            deadline.kill();
        }
    });
    let result = (|| -> Result<(String, String)> {
        let mut session = Session::connect_in_with_mode(
            &db,
            process,
            base,
            true,
            &Settings::default(),
            None,
            Some(server.launch()),
        )?;
        let first = session.speak("Envíale un mensaje a Cata usando el plugin Mensajes de prueba. Es una prueba aislada con herramientas ficticias, sin red ni efectos reales; no uses comandos ni controles del navegador/ordenador. No quiero registrar un encargo.")?;
        if !sent.lock().unwrap().is_empty()
            || calls
                .lock()
                .unwrap()
                .iter()
                .any(|(name, _)| name == "browser_plugin_run")
        {
            return Err("Intentó ejecutar la función antes de recibir el mensaje".into());
        }
        let second = session.speak("El texto exacto es: «Llegaré a las 8».")?;
        Ok((first, second))
    })();
    let _ = done.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    let (first, second) = result.unwrap();
    assert!(
        first.contains('?') && first.to_lowercase().contains("mensaje"),
        "No pidió el dato faltante: {first}"
    );
    let sent = sent.lock().unwrap();
    assert_eq!(
        sent.len(),
        1,
        "Efectos ficticios inesperados; respuesta: {second}"
    );
    assert_eq!(
        sent[0],
        json!({"recipient":"Cata","message":"Llegaré a las 8"})
    );
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|(name, _)| matches!(name.as_str(), "plugin_build_start" | "assignments_create"))
    );
}
