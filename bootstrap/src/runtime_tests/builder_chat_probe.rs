//! Sonda aislada del chat real y el constructor real. No usa la base del usuario.
use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[test]
#[ignore = "Consume Codex real, Chromium aislado y un sitio loopback temporal"]
fn real_chat_builder_feedback_probe() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let db = Database::open(&fixture.config.database_path, 500).unwrap();
    db.with_connection(|c| {
        c.execute(
            "UPDATE modules SET active=1 WHERE name IN ('llm','browser','plugins')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let web = std::thread::spawn(move || {
        let mut connections = Vec::new();
        while !done.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => connections.push(std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut first_line = String::new();
                    if BufReader::new(&mut stream).read_line(&mut first_line).unwrap_or(0) == 0 { return; }
                    eprintln!("probe site_request={}", first_line.trim_end());
                    let body = if first_line.starts_with("GET /public ") {
                        "<html><body><h1>Catálogo público</h1><p>AZUL-42</p></body></html>"
                    } else {
                        "<html><body><h1>Acceso privado</h1><p>Esta sección requiere iniciar sesión.</p><input placeholder='Usuario'><input placeholder='Contraseña' type='password'></body></html>"
                    };
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                    let _ = stream.write_all(response.as_bytes());
                })),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        for connection in connections {
            let _ = connection.join();
        }
    });
    let helper = std::env::var("GENNIE_TEST_HELPER").expect("Falta helper de prueba");
    let mut backend = Backend::open_config(fixture.config.clone(), &helper).unwrap();
    let start = Instant::now();
    fn turn(b: &mut Backend, text: String, seconds: u64) -> Result<()> {
        b.submit_message(text, None)?;
        let end = Instant::now() + Duration::from_secs(seconds);
        loop {
            b.tick();
            if b.snapshot.turn.is_none() {
                if let Some(e) = &b.snapshot.error {
                    return Err(format!("Error del chat: {e}").into());
                }
                return Ok(());
            }
            if Instant::now() > end {
                return Err("Tiempo de prueba del chat agotado".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn jobs(b: &Backend) -> Result<Vec<(i64, String, String)>> {
        b.state.db.with_connection(|c| {
            let mut q = c.prepare("SELECT id,state,summary FROM plugin_build_jobs ORDER BY id")?;
            Ok(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?)
        })
    }
    fn messages(b: &Backend) -> Result<Vec<(String, String)>> {
        b.state.db.with_connection(|c| {
            let mut q =
                c.prepare("SELECT state,message FROM plugin_builder_messages ORDER BY id")?;
            Ok(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?)
        })
    }
    let result = (|| -> Result<()> {
        turn(
            &mut backend,
            format!(
                "Crea un plugin para consultar el contenido del sitio {origin}/. El sitio requiere acceder a la sección privada. Si el constructor necesita que inicie sesión, avísame en este chat. Autoriza únicamente esta web local de prueba y no uses otras cuentas ni webs."
            ),
            90,
        )?;
        eprintln!("probe bot_start_reply={:?}", backend.snapshot.reply);
        if std::env::var_os("GENNIE_PROBE_ACTIVE").is_some() {
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                backend.tick();
                let all = jobs(&backend)?;
                if all.iter().any(|j| j.1 == "running") {
                    break;
                }
                if all.iter().any(|j| {
                    ["waiting_user", "completed", "blocked", "interrupted"].contains(&j.1.as_str())
                }) || Instant::now() > deadline
                {
                    return Err(format!("No se encontró constructor activo: {all:?}").into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            eprintln!("probe active_before_user={:?}", jobs(&backend)?);
            turn(
                &mut backend,
                format!(
                    "Cambio de instrucciones para el constructor que está trabajando: no iniciaré sesión; usa solo {origin}/public y conserva literalmente AZUL-42 en el resultado."
                ),
                90,
            )?;
            let deadline = Instant::now() + Duration::from_secs(90);
            loop {
                backend.tick();
                let all = messages(&backend)?;
                let state = jobs(&backend)?;
                if all
                    .iter()
                    .any(|(state, text)| state == "delivered" && text.contains("AZUL-42"))
                {
                    eprintln!(
                        "probe active_delivered={all:?} jobs={state:?} elapsed={:?}",
                        start.elapsed()
                    );
                    break;
                }
                if state
                    .iter()
                    .any(|j| ["completed", "blocked", "interrupted"].contains(&j.1.as_str()))
                    || Instant::now() > deadline
                {
                    return Err(format!(
                        "No se confirmó la corrección activa: mensajes={all:?}; trabajos={state:?}"
                    )
                    .into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            fn observed_public(path: &std::path::Path) -> bool {
                let Ok(entries) = std::fs::read_dir(path) else {
                    return false;
                };
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && observed_public(&p) {
                        return true;
                    }
                    if p.extension().is_some_and(|v| v == "jsonl") {
                        let Ok(content) = std::fs::read_to_string(&p) else {
                            continue;
                        };
                        for line in content.lines() {
                            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                                continue;
                            };
                            let item = &v["payload"]["item"];
                            if item["type"] == "McpToolCall"
                                && item["tool"] == "browser_read"
                                && item["result"]["structuredContent"]["data"]
                                    .as_str()
                                    .is_some_and(|s| s.contains("AZUL-42"))
                                && item["result"]["structuredContent"]["url"]
                                    .as_str()
                                    .is_some_and(|s| s.ends_with("/public"))
                            {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            let logs = PathBuf::from(std::env::var_os("HOME").ok_or("Falta HOME temporal")?)
                .join("Library/Application Support/Gennie/plugins/builds/job_1/cli-home/sessions");
            let deadline = Instant::now() + Duration::from_secs(100);
            loop {
                backend.tick();
                if observed_public(&logs) {
                    eprintln!(
                        "probe active_applied=public_page_observed elapsed={:?}",
                        start.elapsed()
                    );
                    break;
                }
                let state = jobs(&backend)?;
                if state.iter().any(|j| j.1 == "completed") {
                    let catalog = backend.plugins.catalogue()?;
                    let plugin = catalog["plugins"]
                        .as_array()
                        .and_then(|items| items.iter().find(|p| p["manifest"]["origin"] == origin))
                        .ok_or("No se encontró el plugin publicado")?;
                    eprintln!("probe active_published={plugin}");
                    let revision = plugin["revision"]
                        .as_u64()
                        .ok_or("Falta revisión del plugin")?;
                    let functions = plugin["manifest"]["functions"]
                        .as_array()
                        .ok_or("Faltan funciones")?;
                    for function in functions {
                        if let Some(name) = function["name"].as_str() {
                            match backend.browser.run_web_plugin(
                                "fixture_publico",
                                name,
                                revision,
                                serde_json::json!({}),
                            ) {
                                Ok(result) => {
                                    eprintln!("probe active_independent_result={result}");
                                    if result["data"].to_string().contains("AZUL-42") {
                                        return Ok(());
                                    }
                                }
                                Err(error) => eprintln!("probe active_independent_error={error}"),
                            }
                        }
                    }
                    return Err(
                        "El plugin publicado no devolvió AZUL-42 en la comprobación independiente"
                            .into(),
                    );
                }
                if state
                    .iter()
                    .any(|j| ["blocked", "interrupted", "waiting_user"].contains(&j.1.as_str()))
                    || Instant::now() > deadline
                {
                    return Err(format!(
                        "No se observó aplicación en la página pública: {state:?}"
                    )
                    .into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        if jobs(&backend)?.is_empty() {
            return Err("El bot terminó sin iniciar una construcción".into());
        }
        let job = loop {
            backend.tick();
            let all = jobs(&backend)?;
            if let Some(j) = all.first() {
                if j.1 == "waiting_user" {
                    break j.0;
                }
                if ["completed", "blocked", "interrupted", "cancelled"].contains(&j.1.as_str()) {
                    return Err(format!("Trabajo terminó antes de esperar: {:?}", j).into());
                }
            }
            if Instant::now() > deadline {
                return Err(format!("No se observó waiting_user; trabajos={all:?}").into());
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        eprintln!("probe waiting_user job={job} elapsed={:?}", start.elapsed());
        turn(
            &mut backend,
            format!(
                "No voy a iniciar sesión. Dile al constructor que use la página pública {origin}/public y prepare el plugin con ese contenido."
            ),
            90,
        )?;
        eprintln!("probe bot_waiting_reply={:?}", backend.snapshot.reply);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            backend.tick();
            let all = messages(&backend)?;
            if all.iter().any(|(_, m)| m.contains("public")) {
                eprintln!("probe waiting_message={all:?} jobs={:?}", jobs(&backend)?);
                break;
            }
            if Instant::now() > deadline {
                return Err(format!(
                    "El bot no transmitió la respuesta; mensajes={all:?}; jobs={:?}",
                    jobs(&backend)?
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if std::env::var_os("GENNIE_PROBE_WAIT_ONLY").is_some() {
            let deadline = Instant::now() + Duration::from_secs(120);
            loop {
                backend.tick();
                let all = jobs(&backend)?;
                if all.iter().any(|j| j.0 == job && j.1 == "completed") {
                    break;
                }
                if all.iter().any(|j| {
                    j.0 == job && ["blocked", "interrupted", "cancelled"].contains(&j.1.as_str())
                }) || Instant::now() > deadline
                {
                    return Err(
                        format!("No terminó la construcción tras la respuesta: {all:?}").into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            let catalog = backend.plugins.catalogue()?;
            let plugin = catalog["plugins"]
                .as_array()
                .and_then(|p| p.iter().find(|p| p["manifest"]["origin"] == origin))
                .ok_or("No se publicó el plugin")?;
            eprintln!("probe waiting_published={plugin}");
            if !plugin.to_string().contains("/public") {
                return Err("El plugin no identifica la vía pública en el manifiesto".into());
            }
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            backend.tick();
            let all = jobs(&backend)?;
            if all.iter().any(|j| j.0 == job && j.1 == "running") {
                break;
            }
            if Instant::now() > deadline {
                return Err(format!("No volvió a running: {all:?}").into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        turn(&mut backend,"Otra corrección para el constructor: cuando la función consulte la página y devuelva su contenido, conserva literalmente el texto AZUL-42 en la salida de esa función, sin sustituirlo por otro valor.".into(),90)?;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            backend.tick();
            let all = messages(&backend)?;
            if all.iter().any(|(_, m)| m.contains("AZUL-42")) {
                eprintln!(
                    "probe active_message={all:?} jobs={:?} elapsed={:?}",
                    jobs(&backend)?,
                    start.elapsed()
                );
                break;
            }
            if jobs(&backend)?.iter().any(|j| {
                ["completed", "blocked", "interrupted", "cancelled"].contains(&j.1.as_str())
            }) || Instant::now() > deadline
            {
                return Err(format!(
                    "No se transmitió corrección activa: {all:?}; jobs={:?}",
                    jobs(&backend)?
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            backend.tick();
            let notes = messages(&backend)?;
            if notes.len() >= 2 && notes[0].0 == "delivered" && notes[1].0 == "delivered" {
                eprintln!(
                    "probe consecutive_delivered={notes:?} jobs={:?} elapsed={:?}",
                    jobs(&backend)?,
                    start.elapsed()
                );
                break;
            }
            let state = jobs(&backend)?;
            if state.iter().any(|j| {
                ["completed", "interrupted", "blocked", "cancelled"].contains(&j.1.as_str())
            }) || Instant::now() > deadline
            {
                return Err(format!(
                    "No se entregaron ambas correcciones: mensajes={notes:?}; trabajos={state:?}"
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            backend.tick();
            let state = jobs(&backend)?;
            if state.iter().any(|j| j.0 == job && j.1 == "completed") {
                break;
            }
            if state.iter().any(|j| {
                j.0 == job
                    && ["waiting_user", "blocked", "interrupted", "cancelled"]
                        .contains(&j.1.as_str())
            }) || Instant::now() > deadline
            {
                return Err(format!(
                    "Correcciones entregadas, pero resultado final no verificado: {state:?}"
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let catalog = backend.plugins.catalogue()?;
        let plugin = catalog["plugins"]
            .as_array()
            .and_then(|items| items.iter().find(|p| p["manifest"]["origin"] == origin))
            .ok_or("No se encontró el plugin publicado")?;
        eprintln!("probe final_manifest={}", plugin["manifest"]);
        let id = plugin["manifest"]["id"]
            .as_str()
            .ok_or("Falta identificador")?;
        let revision = plugin["revision"].as_u64().ok_or("Falta revisión")?;
        let functions = plugin["manifest"]["functions"]
            .as_array()
            .ok_or("Faltan funciones")?;
        for function in functions {
            if let Some(name) = function["name"].as_str()
                && let Ok(result) =
                    backend
                        .browser
                        .run_web_plugin(id, name, revision, serde_json::json!({}))
            {
                eprintln!("probe final_independent_result={result}");
                if result["data"].to_string().contains("AZUL-42") {
                    return Ok(());
                }
            }
        }
        Err("Las correcciones llegaron, pero el plugin publicado no devolvió AZUL-42".into())
    })();
    eprintln!(
        "probe functional_result={:?}",
        result.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );
    let closing = backend.close();
    stop.store(true, Ordering::Release);
    web.join().unwrap();
    if let Err(error) = &closing {
        eprintln!("probe shutdown_error={error}");
    }
    result.unwrap();
    closing.unwrap();
}
