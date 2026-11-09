//! Mensajes durables del bot al constructor; nunca confirma una recepción incierta.
use super::*;
use gennie_db::params;

impl Controller {
    pub fn message_builder(&self, job_id: i64, request_key: &str, message: &str) -> Result<Value> {
        self.permitted()?;
        if job_id <= 0
            || request_key.trim().is_empty()
            || request_key.len() > 200
            || message.trim().is_empty()
            || message.chars().count() > 4000
            || request_key.chars().any(char::is_control)
            || message.contains('\0')
        {
            return Err("Mensaje al constructor inválido".into());
        }
        let conversation = self.0.journal.origin()?;
        let origin = self.0.journal.target(conversation)?;
        origin.validate()?;
        let origin_json = serde_json::to_string(&origin)?;
        let _guard = self
            .0
            .commit
            .lock()
            .map_err(|_| "Estado de promoción inválido")?;
        let state = self.0.store.0.with_connection(|c| {
            let tx = c.transaction()?;
            let existing = {
                let mut q = tx.prepare("SELECT job_id,origin,message,state FROM plugin_builder_messages WHERE request_key=?1")?;
                q.query_map([request_key], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.next().transpose()?
            };
            if let Some((saved_job,saved_origin,saved_message,state)) = existing {
                if saved_job != job_id || saved_origin != origin_json || saved_message != message {
                    return Err("La clave del mensaje ya pertenece a otra solicitud".into());
                }
                return Ok(state);
            }
            let job = {
                let mut q = tx.prepare("SELECT state,delegation_origin FROM plugin_build_jobs WHERE id=?1")?;
                q.query_map([job_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?)))?.next().transpose()?
            }.ok_or("Trabajo no encontrado")?;
            if job.1.as_deref() != Some(origin_json.as_str()) {
                return Err("El trabajo no pertenece a esta conversación y sesión".into());
            }
            if !matches!(job.0.as_str(), "queued" | "running" | "waiting_user" | "interrupted") {
                return Err("El trabajo ya no admite instrucciones".into());
            }
            let unresolved: i64 = tx.query_row("SELECT count(*) FROM plugin_builder_messages WHERE job_id=?1 AND state='uncertain'",[job_id],|r|r.get(0))?;
            if unresolved != 0 { return Err("Hay una instrucción anterior con recepción incierta; revisa el trabajo antes de continuar".into()); }
            let pending: i64 = tx.query_row("SELECT count(*) FROM plugin_builder_messages WHERE job_id=?1 AND state IN ('pending','sending')",[job_id],|r|r.get(0))?;
            if pending >= 16 { return Err("Hay demasiadas instrucciones pendientes para este trabajo".into()); }
            tx.execute("INSERT INTO plugin_builder_messages(job_id,request_key,origin,message,state) VALUES(?1,?2,?3,?4,'pending')",params![job_id,request_key,origin_json,message])?;
            if job.0 == "waiting_user" {
                tx.execute("UPDATE plugin_build_jobs SET state='queued',retry_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='waiting_user'",[job_id])?;
                tx.execute("INSERT INTO plugin_build_events(job_id,message,source_state,kind) VALUES(?1,'La respuesta está guardada y el trabajo se retomará.','queued','status')",[job_id])?;
            }
            tx.commit()?;
            Ok("pending".to_owned())
        })?;
        let job_state = self.0.store.get(job_id)?.state;
        self.notify();
        Ok(
            json!({"status":state,"job_id":job_id,"job_state":job_state,"message":match state.as_str() {
                "pending" if job_state == "interrupted" => "La instrucción está guardada; el trabajo interrumpido requiere reanudación explícita.",
                "pending" => "La instrucción está guardada para su entrega; aún no se ha confirmado su aplicación.",
                "sending" => "La instrucción está en envío; aún no se ha confirmado su recepción.",
                "delivered" => "El proveedor aceptó la instrucción o se incluyó en un turno completado; eso no confirma que se haya aplicado.",
                _ => "La recepción de la instrucción es incierta; no la repitas sin revisar el trabajo.",
            }}),
        )
    }
}

impl store::Store {
    pub(crate) fn reserve_messages(&self, job_id: i64) -> Result<Vec<(i64, String)>> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let uncertain:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM plugin_builder_messages WHERE job_id=?1 AND state IN ('sending','uncertain'))",[job_id],|r|r.get(0))?;
            if uncertain {return Err("Hay una instrucción con recepción incierta".into());}
            let rows={let mut q=tx.prepare("SELECT id,message FROM plugin_builder_messages WHERE job_id=?1 AND state='pending' ORDER BY id LIMIT 16")?;
                q.query_map([job_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?};
            tx.execute("UPDATE plugin_builder_messages SET state='sending',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE job_id=?1 AND state='pending'",[job_id])?;
            tx.commit()?;Ok(rows)
        })
    }
    pub(crate) fn finish_messages(&self, job_id: i64, success: bool) -> Result<()> {
        self.0.with_connection(|c| {c.execute("UPDATE plugin_builder_messages SET state=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE job_id=?1 AND state='sending'",params![job_id,if success {"delivered"} else {"uncertain"}])?;Ok(())})
    }
    pub(crate) fn return_messages(&self, job_id: i64) -> Result<()> {
        self.0.with_connection(|c| { c.execute("UPDATE plugin_builder_messages SET state='pending',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE job_id=?1 AND state='sending'", [job_id])?; Ok(()) })
    }
    pub(crate) fn has_pending_messages(&self, job_id: i64) -> Result<bool> {
        self.0.with_connection(|c| Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM plugin_builder_messages WHERE job_id=?1 AND state='pending')", [job_id], |r| r.get(0))?))
    }
    pub(crate) fn has_unconsumed_messages(&self, job_id: i64) -> Result<bool> {
        self.0.with_connection(|c| Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM plugin_builder_messages WHERE job_id=?1 AND state IN ('pending','sending','uncertain'))",[job_id],|r|r.get(0))?))
    }
    pub(crate) fn has_uncertain_messages(&self, job_id: i64) -> Result<bool> {
        self.0.with_connection(|c| Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM plugin_builder_messages WHERE job_id=?1 AND state IN ('sending','uncertain'))",[job_id],|r|r.get(0))?))
    }
    pub(crate) fn uncertain_messages(&self, job_id: i64) -> Result<Value> {
        self.0.with_connection(|c| {
            let mut q = c.prepare("SELECT id,message FROM plugin_builder_messages WHERE job_id=?1 AND state='uncertain' ORDER BY id")?;
            let rows = q.query_map([job_id], |r| Ok(json!({"id":r.get::<_,i64>(0)?,"message":r.get::<_,String>(1)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
            Ok(json!(rows))
        })
    }
    pub(crate) fn review_message(
        &self,
        job_id: i64,
        message_id: i64,
        resolution: &str,
    ) -> Result<()> {
        let state = match resolution {
            "received" => "delivered",
            "resend" => "pending",
            _ => return Err("Resolución no válida".into()),
        };
        self.0.with_connection(|c| {
            let tx = c.transaction()?;
            let changed = tx.execute("UPDATE plugin_builder_messages SET state=?3,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2 AND job_id=?1 AND state='uncertain' AND EXISTS (SELECT 1 FROM plugin_build_jobs WHERE id=?1 AND state='interrupted')", params![job_id,message_id,state])?;
            if changed != 1 { return Err("La instrucción ya cambió o el trabajo no está interrumpido".into()); }
            tx.execute("INSERT INTO plugin_build_events(job_id,message,source_state,kind) VALUES(?1,?2,'interrupted','status')",params![job_id,if state == "pending" { "El usuario revisó la instrucción y decidió reenviarla al reanudar." } else { "El usuario confirmó que la instrucción fue recibida; revisa si se aplicó antes de continuar." }])?;
            tx.commit()?; Ok(())
        })
    }
    pub(crate) fn message_counts(&self, job_id: i64) -> Result<Value> {
        self.0.with_connection(|c| {
            let (pending,sending,delivered,uncertain):(i64,i64,i64,i64)=c.query_row(
                "SELECT count(*) FILTER (WHERE state='pending'),count(*) FILTER (WHERE state='sending'),count(*) FILTER (WHERE state='delivered'),count(*) FILTER (WHERE state='uncertain') FROM plugin_builder_messages WHERE job_id=?1",[job_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
            Ok(json!({"pending":pending,"sending":sending,"delivered":delivered,"uncertain":uncertain}))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    struct Journal(i64);
    impl BuildJournal for Journal {
        fn origin(&self) -> Result<i64> {
            Ok(self.0)
        }
        fn exists(&self, _: i64) -> Result<bool> {
            Ok(true)
        }
        fn target(&self, id: i64) -> Result<gennie_llm::delegation::Origin> {
            Ok(gennie_llm::delegation::Origin {
                conversation_id: id,
                bot_id: "gennie".into(),
                epoch: 1,
                provider_thread: None,
            })
        }
        fn deliver(&self, _: &gennie_llm::delegation::Update) -> Result<()> {
            Ok(())
        }
    }
    fn fixture(origin: i64) -> (PluginsModule, i64) {
        let db = Database::memory().unwrap();
        let browser = gennie_browser::BrowserModule::new(
            std::env::temp_dir().join("feedback-browser-unused"),
        );
        let module = PluginsModule::new(
            db.clone(),
            browser.controller(),
            Arc::new(Journal(origin)),
            AgentSettings::shared(Arc::new(|| Ok(Settings::default()))),
            PathBuf::from("unused"),
            std::env::temp_dir().join("feedback-unused"),
        );
        db.migrate(module.name(), module.migrations()).unwrap();
        module.controller.0.enabled.store(true, Ordering::Release);
        let request = BuildRequest {
            request_key: "build".into(),
            plugin_id: "fixture".into(),
            origin: "https://example.com".into(),
            objective: "Preparar".into(),
            max_attempts: 2,
            max_seconds: 30,
            retry_of: None,
        };
        let job = module.controller.0.store.create(&request, 7, 0).unwrap();
        (module, job.id)
    }

    #[test]
    fn waiting_reply_is_durable_idempotent_and_delivered_on_next_turn() {
        let (module, job) = fixture(7);
        let c = module.controller();
        c.0.store
            .transition(job, "waiting_user", "Esperando")
            .unwrap();
        let result = c
            .message_builder(job, "answer-1", "No iniciar sesión; usa la vía pública")
            .unwrap();
        assert_eq!(result["status"], "pending");
        assert_eq!(c.0.store.get(job).unwrap().state, "queued");
        assert_eq!(
            c.message_builder(job, "answer-1", "No iniciar sesión; usa la vía pública")
                .unwrap()["status"],
            "pending"
        );
        assert!(
            c.message_builder(job, "answer-1", "Otro contenido")
                .is_err()
        );
        let pending = c.0.store.reserve_messages(job).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "No iniciar sesión; usa la vía pública");
        assert!(c.0.store.has_unconsumed_messages(job).unwrap());
        c.0.store.finish_messages(job, true).unwrap();
        assert!(!c.0.store.has_unconsumed_messages(job).unwrap());
        assert_eq!(
            c.message_builder(job, "answer-1", "No iniciar sesión; usa la vía pública")
                .unwrap()["status"],
            "delivered"
        );
    }

    #[test]
    fn wrong_session_cannot_send_and_uncertain_delivery_blocks_resume() {
        let (foreign, job) = fixture(8);
        let c = foreign.controller();
        assert!(
            c.message_builder(job, "wrong", "No iniciar sesión")
                .is_err()
        );
        let (module, job) = fixture(7);
        let c = module.controller();
        c.0.store.transition(job, "running", "Activo").unwrap();
        c.message_builder(job, "change", "Cambia el resultado")
            .unwrap();
        assert_eq!(c.0.store.reserve_messages(job).unwrap().len(), 1);
        c.0.store.recover().unwrap();
        assert_eq!(c.0.store.get(job).unwrap().state, "interrupted");
        assert!(c.0.store.has_uncertain_messages(job).unwrap());
        assert!(c.resume(job).is_err());
        assert!(c.message_builder(job, "other", "Otra instrucción").is_err());
        let uncertain = c.0.store.uncertain_messages(job).unwrap();
        let id = uncertain[0]["id"].as_i64().unwrap();
        assert!(c.0.store.review_message(job + 1, id, "resend").is_err());
        assert!(c.0.store.review_message(job, id, "invalid").is_err());
        c.0.store.review_message(job, id, "resend").unwrap();
        assert!(!c.0.store.has_uncertain_messages(job).unwrap());
        assert_eq!(
            c.0.store.reserve_messages(job).unwrap()[0].1,
            "Cambia el resultado"
        );
        assert!(c.0.store.review_message(job, id, "resend").is_err());
    }

    #[test]
    fn bot_mcp_message_reaches_builder_inbox_and_is_not_promotion_ready() {
        let (module, job) = fixture(7);
        let c = module.controller();
        c.0.store.transition(job, "running", "Activo").unwrap();
        let mut registry = gennie_mcp::Registry::default();
        registry.register(crate::tools(c.clone())).unwrap();
        let response = registry
            .respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"plugin_build_jobs","arguments":{"action":"message","job_id":job,
                "request_key":"mcp-note","message":"No uses la vía privada; usa la pública"}}}))
            .unwrap();
        assert_eq!(response["result"]["isError"], false);
        assert!(c.0.store.has_unconsumed_messages(job).unwrap());
        let notes = c.0.store.reserve_messages(job).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].1, "No uses la vía privada; usa la pública");
        c.0.store.finish_messages(job, true).unwrap();
        assert!(!c.0.store.has_unconsumed_messages(job).unwrap());
    }
    #[test]
    #[ignore = "Prueba real aislada con Codex y MCP; consume inferencia"]
    fn real_builder_receives_bot_correction_in_active_turn() {
        use gennie_llm::builder::{BuilderControl, BuilderSession, FeedbackEvent};
        use std::time::{Duration, Instant};
        let (module, job) = fixture(7);
        let c = module.controller();
        c.0.store.transition(job, "running", "Activo").unwrap();
        let mut bot = gennie_mcp::Registry::default();
        bot.register(crate::tools(c.clone())).unwrap();
        let observed = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = observed.clone();
        let mut builder = gennie_mcp::Registry::default();
        builder.register([
            gennie_mcp::Tool { definition: json!({"name":"fixture_wait","description":"Espera breve de la prueba antes de registrar la decisión.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}), call: Box::new(|_| { std::thread::sleep(Duration::from_secs(4)); Ok(json!({"status":"ready"})) }) },
            gennie_mcp::Tool { definition: json!({"name":"fixture_result","description":"Registra la decisión final de la prueba.","inputSchema":{"type":"object","properties":{"choice":{"type":"string"}},"required":["choice"],"additionalProperties":false}}), call: Box::new(move |v| { seen.lock().unwrap().push(v["choice"].as_str().unwrap_or("").into()); Ok(json!({"status":"recorded"})) }) },
        ]).unwrap();
        let helper =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/flutter/debug/gennie");
        let mut server = gennie_mcp::Server::start(builder, &helper).unwrap();
        let root =
            std::env::temp_dir().join(format!("gennie-feedback-probe-{}", std::process::id()));
        let control = BuilderControl::default();
        let watchdog_control = control.clone();
        let (done, wait) = std::sync::mpsc::channel();
        let watchdog = std::thread::spawn(move || {
            if wait.recv_timeout(Duration::from_secs(90)).is_err() {
                watchdog_control.cancel();
            }
        });
        let started = Instant::now();
        let result = (|| -> Result<()> {
            let mut session=BuilderSession::connect(root.clone(), &Settings::default(), server.launch(), "Eres un constructor de una prueba aislada. Usa las herramientas de prueba y sigue la instrucción más reciente del usuario.".into(), None, control)?;
            let mut sent = false;
            let mut feedback = |event| -> Result<Option<String>> {
                match event {
                    FeedbackEvent::Poll => {
                        if !sent && started.elapsed() >= Duration::from_secs(3) {
                            let response=bot.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"plugin_build_jobs","arguments":{"action":"message","job_id":job,"request_key":"real-steer","message":"Corrección: registra corregido; no registres original."}}})).unwrap();
                            assert_eq!(response["result"]["isError"], false);
                            sent = true;
                        }
                        let notes = c.0.store.reserve_messages(job)?;
                        if notes.is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(notes[0].1.clone()))
                        }
                    }
                    FeedbackEvent::Accepted => {
                        c.0.store.finish_messages(job, true)?;
                        Ok(None)
                    }
                    FeedbackEvent::Rejected => {
                        c.0.store.return_messages(job)?;
                        Err("El proveedor rechazó steer".into())
                    }
                    FeedbackEvent::Uncertain => {
                        c.0.store.finish_messages(job, false)?;
                        Err("Recepción incierta".into())
                    }
                }
            };
            session.turn_with_feedback("Primero llama fixture_wait; después registra original con fixture_result. Sigue cualquier corrección posterior.", &mut feedback)?;
            assert!(sent, "El turno terminó antes de enviar la corrección");
            Ok(())
        })();
        let _ = done.send(());
        watchdog.join().unwrap();
        server.stop().unwrap();
        let _ = std::fs::remove_dir_all(root);
        result.unwrap();
        assert_eq!(*observed.lock().unwrap(), vec!["corregido"]);
        assert_eq!(c.0.store.message_counts(job).unwrap()["delivered"], 1);
        eprintln!("real_builder_steer: {:?}", started.elapsed());
    }
}
