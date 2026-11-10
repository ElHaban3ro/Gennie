//! Un solo paso con reserva durable para todos los consumidores de recetas web.
use super::*;
impl Controller {
    pub(super) fn review_gmail_attempt(
        &self,
        id: i64,
        session: &process::Session,
        context: &gennie_mcp::CallContext,
    ) -> Result<Value> {
        let store = self.attempt_store()?;
        let Some(evidence) = store.gmail_evidence(id)? else {
            return Ok(
                json!({"status":"review_required","message":"No hay evidencia previa suficiente para comprobar este intento automáticamente."}),
            );
        };
        let mut request = json!({"method":if evidence["candidate_id"].is_string() {"candidate"} else {"plugin_run"}, "action":"gmail_review", "plugin_id":evidence["plugin_id"], "function_name":evidence["function_name"], "expected_revision":evidence["revision"], "arguments":{"id":evidence["thread_id"]}, "gmail_evidence":evidence});
        if evidence["candidate_id"].is_string() {
            request["candidate_id"] = evidence["candidate_id"].clone();
        }
        let observation = decode_response(session.request_with_context(&request, context)?)?;
        if observation["status"] == "applied" {
            store.reconcile_gmail(id, "applied")?;
            self.execution_changed();
            Ok(
                json!({"status":"reviewed","resolution":"applied","method":"automatic","attempt_id":id,"executed":false}),
            )
        } else {
            Ok(
                json!({"status":"review_required","attempt_id":id,"executed":false,"message":"No se pudo confirmar el efecto completo en la misma cuenta; el intento sigue bloqueado."}),
            )
        }
    }
    pub(super) fn attempt_store(&self) -> Result<attempts::Store> {
        self.inner
            .attempts
            .lock()
            .map_err(|_| "Diario web inválido")?
            .clone()
            .ok_or_else(|| "El diario de ejecución web no está disponible".into())
    }
    pub fn execution_revision(&self) -> u64 {
        self.inner.execution_revision.load(Ordering::Acquire)
    }
    fn execution_changed(&self) {
        self.inner.execution_revision.fetch_add(1, Ordering::AcqRel);
    }
    pub fn web_attempts(&self, plugin: &str, after: Option<i64>) -> Result<Value> {
        let mut page = self.attempt_store()?.page(plugin, after)?;
        for attempt in page["attempts"].as_array_mut().unwrap() {
            attempt["active"] = json!(
                attempt["state"] == "running"
                    && attempt["id"].as_i64()
                        == Some(self.inner.active_web.load(Ordering::Acquire))
            );
            if attempt["state"] == "running" && attempt["active"] != true {
                attempt["state"] = json!("uncertain");
                attempt["reason"] = json!("completion_unrecorded");
            }
        }
        page["status"] = json!("ready");
        page["recent_reviews"] = json!(self.attempt_store()?.reviews(plugin)?);
        Ok(page)
    }
    /// Operación humana local; no la publica ningún inventario de herramientas.
    pub fn reconcile_web(
        &self,
        plugin: &str,
        id: i64,
        revision: i64,
        resolution: &str,
    ) -> Result<Value> {
        let _guard = self
            .inner
            .session
            .try_lock()
            .map_err(|_| "El navegador sigue trabajando; espera antes de revisar")?;
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("El módulo de navegador está desactivado".into());
        }
        let _execution = self
            .inner
            .web_execution_gate
            .try_lock()
            .map_err(|_| "El navegador sigue trabajando; espera antes de revisar")?;
        self.attempt_store()?
            .reconcile(plugin, id, revision, resolution)?;
        self.execution_changed();
        Ok(json!({"status":"reviewed","executed":false}))
    }
    /// Cierre local explícito sin atribuir el efecto; mantiene en cuarentena el programa antiguo.
    pub fn close_unconfirmed_web(&self, plugin: &str, id: i64, revision: i64) -> Result<Value> {
        let _guard = self
            .inner
            .session
            .try_lock()
            .map_err(|_| "El navegador sigue trabajando; espera antes de revisar")?;
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("El módulo de navegador está desactivado".into());
        }
        let _execution = self
            .inner
            .web_execution_gate
            .try_lock()
            .map_err(|_| "El navegador sigue trabajando; espera antes de revisar")?;
        self.attempt_store()?
            .close_unconfirmed(plugin, id, revision)?;
        self.execution_changed();
        Ok(json!({"status":"closed_unconfirmed","executed":false}))
    }
    pub(super) fn web_gate(&self, request: &Value) -> Result<Option<Value>> {
        let guarded = matches!(
            request["method"].as_str(),
            Some("plugin_run" | "candidate" | "plugins")
        ) && matches!(
            request["action"].as_str(),
            Some("run" | "verify" | "prepare" | "save" | "delete" | "promote")
        );
        if !guarded {
            return Ok(None);
        }
        let Some(plugin) = request["plugin_id"]
            .as_str()
            .or_else(|| request.pointer("/manifest/id").and_then(Value::as_str))
        else {
            return Ok(None);
        };
        let store = self.attempt_store()?;
        let executing = matches!(
            request["action"].as_str(),
            Some("run" | "verify" | "prepare")
        );
        let function = if executing {
            request["function_name"].as_str()
        } else {
            None
        };
        if executing && function.is_none() {
            return Ok(None);
        }
        if let Some(attempt) = store.pending(plugin, function)? {
            return Ok(Some(
                json!({"status":"review_required","executed":false,"review_required":true,"plugin_id":plugin,"plugin_name":attempt["server_name"],"function_name":request["function_name"],"attempt":attempt,"message":"Comprueba el efecto y revisa el intento en Plugins antes de repetir esa función o modificar el plugin."}),
            ));
        }
        Ok(None)
    }
    pub(super) fn web_metadata(&self, mut value: Value) -> Result<Value> {
        if let Some(records) = value["plugins"].as_array_mut() {
            for record in records {
                self.record_metadata(record)?;
            }
        }
        if let Some(record) = value.get_mut("plugin") {
            self.record_metadata(record)?;
        }
        Ok(value)
    }
    fn record_metadata(&self, record: &mut Value) -> Result<()> {
        let Some(id) = record
            .pointer("/manifest/id")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Ok(());
        };
        let page = self.web_attempts(&id, None)?;
        record["web_recent_reviews"] = page["recent_reviews"].clone();
        record["web_pending"] = page;
        Ok(())
    }
    pub(super) fn execute_web(
        &self,
        request: &Value,
        session: &process::Session,
        context: &gennie_mcp::CallContext,
    ) -> Result<(Value, bool)> {
        let mut preflight = request.clone();
        preflight["operation"] = request["action"].clone();
        preflight["action"] = json!("preflight");
        let mut prepared = decode_response(session.request_with_context(&preflight, context)?)?;
        if prepared["status"] != "ready" {
            return Ok((prepared, false));
        }
        if prepared["plugin_id"] != request["plugin_id"]
            || prepared["function_name"] != request["function_name"]
            || prepared["revision"] != request["expected_revision"]
            || prepared["executed"] != false
        {
            return Err("La comprobación previa no corresponde a esta función web".into());
        }
        let store = self.attempt_store()?;
        if store.quarantined(
            prepared["plugin_id"].as_str().unwrap_or_default(),
            prepared["function_name"].as_str().unwrap_or_default(),
        )? && !matches!(
            prepared["transport"].as_str(),
            Some("observe" | "gmail_inbox" | "gmail_print" | "gmail_importance")
        ) {
            return Ok((
                json!({"status":"quarantined_recipe","executed":false,"message":"El resultado anterior sigue sin confirmarse. No repitas ese programa; usa una lectura controlada o una revisión humana con evidencia."}),
                false,
            ));
        }
        if prepared["transport"] == "gmail_archive" {
            let mut observation = request.clone();
            observation["action"] = json!("gmail_evidence");
            let evidence = decode_response(session.request_with_context(&observation, context)?)?;
            if evidence["status"] == "ready" {
                prepared["gmail_evidence"] = evidence;
            }
        }
        context.check()?;
        let reserved = store.reserve(
            &prepared,
            request["action"].as_str().ok_or("Falta operación web")?,
            request["candidate_id"].as_str(),
        )?;
        self.inner
            .active_web
            .store(reserved["id"].as_i64().unwrap(), Ordering::Release);
        struct Active<'a>(&'a AtomicI64);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                self.0.store(0, Ordering::Release);
            }
        }
        let _active = Active(&self.inner.active_web);
        self.execution_changed();
        // Vincula la ejecución al tipo comprobado antes de crear la reserva.
        let mut execution_request = request.clone();
        execution_request["checked_transport"] = prepared["transport"].clone();
        if prepared["gmail_evidence"].is_object() {
            execution_request["gmail_evidence"] = prepared["gmail_evidence"].clone();
        }
        let result = session
            .request_with_context(&execution_request, context)
            .and_then(decode_response);
        let discard = result.is_err();
        let reason = match &result {
            _ if context.is_cancelled() => "transport_error",
            Ok(v) if v["status"] == "ok" => "reported_success",
            Ok(_) => "reported_failure",
            Err(_) => "transport_error",
        };
        let diagnostic = result
            .as_ref()
            .ok()
            .and_then(|value| value.get("diagnostic"));
        let not_started = !context.is_cancelled()
            && result.as_ref().is_ok_and(|v| {
                v["executed"] == false
                    && v["diagnostic"]["phase"] == "prepare"
                    && (v["status"] == "browser_unavailable"
                        || (prepared["transport"] == "gmail_archive"
                            && matches!(
                                v["status"].as_str(),
                                Some("archive_not_started" | "authentication_required")
                            )))
            });
        let saved = store.finish_with_outcome(
            reserved["id"].as_i64().unwrap(),
            reason,
            diagnostic,
            not_started,
        );
        self.execution_changed();
        let observation = matches!(
            prepared["transport"].as_str(),
            Some("observe" | "gmail_inbox" | "gmail_print" | "gmail_importance")
        );
        let mut uncertain =
            (reason != "reported_success" && !observation && !not_started) || saved.is_err();
        let mut value=result.unwrap_or_else(|_| if observation { json!({"status":"observation_failed","message":"No se pudo leer la página abierta; el resultado no está disponible."}) } else { json!({"status":"result_uncertain","message":"El navegador dejó de confirmar la operación; comprueba su efecto antes de repetirla."}) });
        let mut attempt = saved.unwrap_or(reserved);
        if uncertain
            && !context.is_cancelled()
            && attempt["state"] == "uncertain"
            && let Some(evidence) = store.gmail_evidence(attempt["id"].as_i64().unwrap())?
        {
            let mut review = request.clone();
            review["action"] = json!("gmail_review");
            review["gmail_evidence"] = evidence;
            if let Ok(response) = session
                .request_with_context(&review, context)
                .and_then(decode_response)
            {
                // Un efecto visible se confirma; la ausencia puede ser un efecto demorado.
                if response["status"] == "applied"
                    && store
                        .reconcile_gmail(attempt["id"].as_i64().unwrap(), "applied")
                        .is_ok()
                {
                    attempt["state"] = json!("reviewed");
                    attempt["resolution"] = json!("applied");
                    value["status"] = if request["action"] == "run" {
                        json!("ok")
                    } else {
                        json!("auto_reviewed")
                    };
                    value["automatic_review"] = json!("applied");
                    value["message"] = json!(
                        "Se comprobó que la conversación está archivada y conserva su estado de lectura."
                    );
                    uncertain = false;
                    self.execution_changed();
                }
            }
        }
        attempt["active"] = json!(false);
        if attempt["state"] == "running" {
            attempt["state"] = json!("uncertain");
            value["status"] = json!("result_uncertain");
        }
        value["executed"] = json!(!not_started);
        value["review_required"] = json!(uncertain);
        value["web_attempt_id"] = attempt["id"].clone();
        value["attempt"] = attempt;
        value["plugin_id"] = prepared["plugin_id"].clone();
        value["plugin_name"] = prepared["plugin_name"].clone();
        value["function_name"] = prepared["function_name"].clone();
        value["revision"] = prepared["revision"].clone();
        Ok((value, discard))
    }
}
