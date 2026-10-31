//! Dos turnos semánticos optativos; sesión técnica propia con instrucciones del bot.
use super::*;

#[test]
#[ignore = "Consume dos respuestas ChatGPT (máximo 60 s), solo datos y herramientas locales ficticios"]
fn real_model_starts_local_review_only_after_explicit_request() {
    use gennie_llm::builder::{BuilderControl, BuilderSession};
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute("UPDATE modules SET active=1 WHERE name='assignments'", [])?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut backend = fixture.open();
    let helper = std::env::var_os("GENNIE_TEST_HELPER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/flutter/debug/gennie")
        });
    assert!(
        helper.is_file(),
        "Compila primero con scripts/dev.sh --build-only"
    );
    let document = |title: &str| {
        serde_json::from_value::<gennie_assignments::Document>(serde_json::json!({"title":title,"description":"Consultar novedades y preparar un resumen, sin enviar mensajes","kind":"recurring","priority":"normal","status":"active","timezone":"America/Bogota","recurrence":{"frequency":"daily","local_time":"08:00"}})).unwrap()
    };
    let a = backend
        .assignments
        .create("mail", document("Resumen de correo"))
        .unwrap();
    let b = backend
        .assignments
        .create("agenda", document("Revisión de agenda"))
        .unwrap();
    let observation = "Me cuesta acordarme de revisar el correo y la agenda. De momento solo te cuento esa dificultad; no quiero que prepares propuestas ni registres cambios todavía.";
    let request = "Genera una lista de propuestas de proactividad para los encargos Resumen de correo y Revisión de agenda. Tenlos en cuenta juntos para evitar duplicados y conservar las fuentes y permisos actuales. Quiero revisar los borradores antes de aplicar cualquier cambio. No uses la guía remota.";
    let thread = "semantic-review-fixture";
    append_conversation(&backend, thread, observation);
    let invocation = Arc::new(std::sync::Mutex::new(Some(thread.to_owned())));
    let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let mut tools = gennie_assignments::tools(backend.assignments.clone());
    tools.extend(proposal_tools(
        backend.assignments.clone(),
        backend.state.db.clone(),
        invocation,
        Arc::new(|_, _| Err("Guía remota no autorizada en la sonda".into())),
    ));
    for tool in &mut tools {
        let name = tool.definition["name"].as_str().unwrap().to_string();
        let calls = calls.clone();
        let old = std::mem::replace(&mut tool.call, Box::new(|_| Err("No preparado".into())));
        tool.call = Box::new(move |arguments| {
            calls
                .lock()
                .map_err(|_| "Diario ficticio inválido")?
                .push(name.clone());
            if matches!(name.as_str(), "assignments_create" | "assignments_update") {
                return Err("La sonda solo autoriza borradores, no cambios en planes".into());
            }
            old(arguments)
        });
    }
    let mut registry = gennie_mcp::Registry::default();
    registry.register(tools).unwrap();
    let mut server = gennie_mcp::Server::start(registry, &helper).unwrap();
    let control = BuilderControl::default();
    let deadline = control.clone();
    let (done, finish) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if finish
            .recv_timeout(std::time::Duration::from_secs(60))
            .is_err()
        {
            deadline.cancel();
        }
    });
    let result = (|| -> Result<()> {
        let settings = gennie_llm::Settings {
            read_files: false,
            write_files: false,
            run_commands: false,
            local_actions: false,
            ..Default::default()
        };
        let instructions = format!(
            "{}\nEsta es una sonda con encargos ficticios. Solo usa las herramientas MCP de Gennie, sin comandos, archivos, sitios, cuentas o efectos externos. Eres el bot que decide si preparar borradores; las observaciones y peticiones se interpretan según las instrucciones anteriores.",
            include_str!("../../../apps/llm/src/assistant-instructions.md")
        );
        let mut session = BuilderSession::connect(
            fixture.directory.join("isolated-provider"),
            &settings,
            server.launch(),
            instructions,
            None,
            control,
        )?;
        let started = std::time::Instant::now();
        session.turn(observation)?;
        if backend.assignments.reviews(a.id)?["total"] != 0
            || backend.assignments.reviews(b.id)?["total"] != 0
        {
            return Err("Preparó una revisión sin petición explícita".into());
        }
        let negative_ms = started.elapsed().as_millis();
        let journal = PresenceModule::default().conversation_journal(backend.state.db.clone());
        journal.append("codex", thread, "user", request)?;
        let started = std::time::Instant::now();
        session.turn(request)?;
        let positive_ms = started.elapsed().as_millis();
        let reviews = backend.assignments.reviews(a.id)?;
        let review = reviews["reviews"]
            .as_array()
            .and_then(|rows| rows.first())
            .ok_or("No preparó revisión tras la petición explícita")?;
        if reviews["total"] != 1
            || review["scope"]["assignments"]
                .as_array()
                .is_none_or(|rows| rows.len() != 2)
            || review["scope"]["use_guide"] != false
            || review["chat_request"]["text"] != request
            || review["proposals"]
                .as_array()
                .is_none_or(|rows| !rows.iter().any(|p| p["candidate"]["document"].is_object()))
        {
            return Err(
                "La revisión no conserva selección, fuente, límites o cambios revisables".into(),
            );
        }
        if backend.assignments.get(a.id)?.revision != 1
            || backend.assignments.get(b.id)?.revision != 1
            || calls
                .lock()
                .map_err(|_| "Diario ficticio inválido")?
                .iter()
                .any(|name| matches!(name.as_str(), "assignments_create" | "assignments_update"))
        {
            return Err("Intentó editar planes en lugar de proponer".into());
        }
        eprintln!(
            "chat_review_semantic negative_ms={negative_ms} positive_ms={positive_ms} reviews=1 selected=2 proposals={} plans_unchanged=true",
            review["proposals"].as_array().map_or(0, Vec::len)
        );
        Ok(())
    })();
    let _ = done.send(());
    watchdog.join().unwrap();
    server.stop().unwrap();
    backend.close().unwrap();
    result.unwrap();
}
