//! Sonda optativa con modelo real y datos exclusivamente ficticios.
use super::*;

#[test]
#[ignore = "Consume un turno real con cuenta Codex; solo herramientas ficticias"]
fn real_mail_context_discovery() {
    let base = std::env::temp_dir().join(format!("gennie-mail-probe-{}", std::process::id()));
    assert!(!base.exists());
    std::fs::create_dir(&base).unwrap();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _cleanup = PrivateDirectory(base.clone());
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut registry = gennie_mcp::Registry::default();
    for (name, desc, schema) in [
        (
            "assignments_list",
            "Lista resúmenes de encargos registrados, incluidos pausados y cancelados. Usa assignments_get para consultar sus detalles. Paginación por offset.",
            json!({"type":"object","properties":{"offset":{"type":"integer"},"limit":{"type":"integer"}},"additionalProperties":false}),
        ),
        (
            "assignments_get",
            "Consulta los detalles de un encargo registrado por su id.",
            json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"],"additionalProperties":false}),
        ),
        (
            "browser_plugins",
            "Consulta el catálogo de plugins web con action=list o get y sus funciones.",
            json!({"type":"object","properties":{"action":{"type":"string","enum":["list","get"]},"plugin_id":{"type":"string"}},"required":["action"],"additionalProperties":false}),
        ),
        (
            "browser_plugin_run",
            "Ejecuta una función web disponible y autorizada.",
            json!({"type":"object","properties":{"plugin_id":{"type":"string"},"function_name":{"type":"string"},"action":{"type":"string"},"arguments":{"type":"object"}},"required":["plugin_id","function_name","action","arguments"]}),
        ),
    ] {
        let calls = calls.clone();
        let name_owned = name.to_string();
        registry.register([gennie_mcp::Tool {
            definition: json!({"name":name,"description":desc,"inputSchema":schema,"annotations":{"readOnlyHint":true}}),
            call: Box::new(move |_args| {
                calls.lock().unwrap().push(name_owned.clone());
                Ok(match name_owned.as_str() {
                    "assignments_list" => json!({"assignments":[{"id":1,"title":"Revisar correos pendientes","status":"active","applications":[{"name":"Gmail","kind":"web"}]}],"has_more":false,"timezone":"America/Bogota"}),
                    "assignments_get" => json!({"assignment":{"id":1,"title":"Revisar correos pendientes","status":"active","applications":[{"name":"Gmail","kind":"web"}]}}),
                    "browser_plugins" => json!({"plugins":[{"id":"gmail_fixture","name":"Gmail ficticio","description":"Consulta correos pendientes","functions":[]}]}),
                    _ => json!({"status":"unavailable","executed":false,"message":"Función ficticia no disponible"}),
                })
            }),
        }]).unwrap();
    }
    let helper = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let socket = server.launch().socket.clone();
    let result = (|| -> Result<()> {
        let db = Database::memory()?;
        let module = crate::LlmModule::default();
        db.migrate(module.name(), module.migrations())?;
        let mut settings = crate::architects::load(&db, crate::architects::Role::Bot)?.settings();
        settings.model = "gpt-6.1-terra".into();
        settings.effort = "low".into();
        let plan = Plan {
            version: 2,
            scenario: Scenario::PluginSemantics,
            mode: "live".into(),
            output_dir: base.clone(),
            nonce: "0".repeat(32),
            model: settings.model.clone(),
            effort: settings.effort.clone(),
            model_turns_max: 3,
            cell_timeout_seconds: 60,
        };
        let process = ProcessHandle::default();
        let control = turn::Control::new(process.clone());
        let scope = Scope::new(&control)?;
        let mut session = Session::spawn(
            process,
            base.clone(),
            &settings,
            Some(&server.launch()),
            &[
                "web_search=\"disabled\"".into(),
                "features.multi_agent=false".into(),
                "features.apply_patch_freeform=false".into(),
            ],
            true,
            Some(scope.ticket.context.clone()),
        )?;
        session.bind_turn(Some(scope.ticket.clone()));
        session.authenticate_model(&settings)?;
        session.journal = Some(Arc::new(Journal::default()));
        let thread = session.open_thread(None, &instructions(&settings, &session.tool_names))?;
        let id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or("Hilo ausente")?;
        save_recorded_thread(&db, id, None, session.journal.as_deref())?;
        session.thread_id = id.into();
        let mut report = json!({"model_turns":0});
        let reply = answer(
            &plan,
            &mut session,
            "Ayúdame a revisar los mails pendientes",
            None,
            &mut report,
        )?;
        println!("Respuesta ficticia: {reply}");
        println!("Consultas ficticias: {:?}", calls.lock().unwrap());
        assert!(
            reply.to_lowercase().contains("gmail"),
            "No identificó la fuente ficticia"
        );
        session.bind_turn(None);
        drop(session);
        drop(scope);
        db.close()?;
        Ok(())
    })();
    server.stop().unwrap();
    assert!(!socket.exists());
    result.unwrap();
    let calls = calls.lock().unwrap();
    assert!(
        calls.iter().any(|x| x == "assignments_list"),
        "No consultó los encargos"
    );
    assert!(
        calls.iter().any(|x| x == "browser_plugins"),
        "No consultó los plugins"
    );
}
