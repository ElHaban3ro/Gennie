//! Conexiones MCP de Plugins; sin worker periódico ni inferencia.
use super::{
    SettingsProvider,
    mcp_activity::Activity,
    mcp_attempts::{Attempt, Reservation, Store as AttemptStore},
    mcp_source::{Connection, Specification},
    mcp_store::{Source, Store},
};
use gennie_core::Result;
use gennie_llm::mcp_client::{Client, Control};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

struct ConnectionHandle {
    specification: Specification,
    control: Control,
    client: Mutex<Option<Client>>,
}
#[derive(Debug)]
struct Busy;
impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "El servidor MCP está trabajando; esta petición no se ejecutó"
        )
    }
}
impl std::error::Error for Busy {}
pub struct Service {
    store: Store,
    attempts: AttemptStore,
    activity: Activity,
    live_attempts: Mutex<HashSet<i64>>,
    root: PathBuf,
    settings: SettingsProvider,
    connections: Mutex<HashMap<String, Arc<ConnectionHandle>>>,
    stopped: AtomicBool,
    revision: AtomicU64,
}
fn validator(schema: &Value) -> Result<jsonschema::Validator> {
    jsonschema::options().offline()
        .with_pattern_options(jsonschema::PatternOptions::fancy_regex().backtrack_limit(10000).size_limit(1024*1024).dfa_size_limit(1024*1024))
        .build(schema).map_err(|_|"El esquema de la herramienta no se puede validar localmente, sin resolver referencias externas".into())
}
impl Service {
    pub fn new(db: gennie_db::Database, root: PathBuf, settings: SettingsProvider) -> Self {
        Self {
            attempts: AttemptStore(db.clone()),
            activity: Activity::default(),
            live_attempts: Mutex::new(HashSet::new()),
            store: Store(db),
            root,
            settings,
            connections: Mutex::new(HashMap::new()),
            stopped: AtomicBool::new(true),
            revision: AtomicU64::new(0),
        }
    }
    pub fn start(&self) -> Result<()> {
        self.activity.start(|| self.attempts.recover())?;
        self.stopped.store(false, Ordering::Release);
        Ok(())
    }
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
    fn changed(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }
    fn permitted(&self, spec: &Specification) -> Result<()> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("El cliente MCP se está cerrando".into());
        }
        let settings = (self.settings)()?;
        if !settings.local_actions || !settings.run_commands {
            return Err("MCP necesita comandos y control de aplicaciones habilitados".into());
        }
        if matches!(spec.connection, Connection::Stdio { .. })
            && (!settings.read_files || !settings.write_files)
        {
            return Err("El servidor stdio ejecuta un programa propio y requiere los cuatro permisos del asistente".into());
        }
        Ok(())
    }
    fn handle(&self, source: &Source) -> Result<Arc<ConnectionHandle>> {
        self.permitted(&source.specification)?;
        let mut map = self
            .connections
            .lock()
            .map_err(|_| "Conexiones MCP inválidas")?;
        self.permitted(&source.specification)?;
        // Comprobar después de tomar el mapa: una edición ya confirmada no debe
        // permitir que una petición antigua vuelva a crear el cliente anterior.
        if !self
            .store
            .is_current(&source.specification.id, source.revision)?
        {
            return Err("La conexión MCP cambió; consulta su revisión vigente".into());
        }
        if let Some(handle) = map.get(&source.specification.id) {
            if handle.specification == source.specification {
                return Ok(handle.clone());
            }
            return Err("La configuración MCP cambió; desconecta antes de continuar".into());
        }
        let handle = Arc::new(ConnectionHandle {
            specification: source.specification.clone(),
            control: Control::default(),
            client: Mutex::new(None),
        });
        map.insert(source.specification.id.clone(), handle.clone());
        Ok(handle)
    }
    fn with_client<T>(
        &self,
        source: &Source,
        call: impl FnOnce(&mut Client) -> Result<T>,
    ) -> Result<T> {
        self.with_client_context(source, &gennie_mcp::CallContext::default(), call)
    }
    fn with_client_context<T>(
        &self,
        source: &Source,
        context: &gennie_mcp::CallContext,
        call: impl FnOnce(&mut Client) -> Result<T>,
    ) -> Result<T> {
        context.check()?;
        let handle = self.handle(source)?;
        let mut guard = match handle.client.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => return Err(Box::new(Busy)),
            Err(std::sync::TryLockError::Poisoned(_)) => return Err("Cliente MCP inválido".into()),
        };
        self.permitted(&source.specification)?;
        if guard.is_none() {
            *guard = Some(Client::connect_with_context(
                self.root.join(&source.specification.id),
                &source.specification.transport(),
                source.specification.timeout_seconds,
                handle.control.clone(),
                context.clone(),
            )?);
        }
        guard
            .as_mut()
            .ok_or("Cliente MCP no disponible")?
            .bind_context(Some(context.clone()));
        let result = call(guard.as_mut().ok_or("Cliente MCP no disponible")?);
        guard
            .as_mut()
            .ok_or("Cliente MCP no disponible")?
            .bind_context(None);
        if result.is_err() || guard.as_ref().is_some_and(|client| !client.is_usable()) {
            guard.take();
        }
        result
    }
    pub fn disconnect(&self, id: &str) -> Result<()> {
        let _operation = self.activity.enter()?;
        self.disconnect_inner(id)
    }
    fn disconnect_inner(&self, id: &str) -> Result<()> {
        let handle = self
            .connections
            .lock()
            .map_err(|_| "Conexiones MCP inválidas")?
            .remove(id);
        if let Some(handle) = handle {
            handle.control.cancel();
            handle
                .client
                .lock()
                .map_err(|_| "Cliente MCP inválido")?
                .take();
        }
        self.changed();
        Ok(())
    }
    pub fn stop(&self) -> Result<()> {
        self.stopped.store(true, Ordering::Release);
        self.activity.close()?;
        let handles = self
            .connections
            .lock()
            .map_err(|_| "Conexiones MCP inválidas")?
            .drain()
            .map(|(_, h)| h)
            .collect::<Vec<_>>();
        for handle in &handles {
            handle.control.cancel();
        }
        self.activity.drain()?;
        for handle in handles {
            handle
                .client
                .lock()
                .map_err(|_| "Cliente MCP inválido")?
                .take();
        }
        Ok(())
    }
    pub fn cancel_if_disallowed(&self) {
        let handles = self
            .connections
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(id, h)| (id.clone(), h.specification.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (id, spec) in handles {
            if self.permitted(&spec).is_err() {
                let _ = self.disconnect(&id);
            }
        }
    }
    fn state(&self, id: &str) -> String {
        let handle = match self.connections.lock() {
            Ok(map) => map.get(id).cloned(),
            Err(_) => return "unavailable".into(),
        };
        match handle {
            None => "disconnected",
            Some(handle) => match handle.client.try_lock() {
                Ok(client) if client.is_some() => "connected",
                Ok(_) => "disconnected",
                Err(std::sync::TryLockError::WouldBlock) => "working",
                Err(std::sync::TryLockError::Poisoned(_)) => "unavailable",
            },
        }
        .into()
    }
    pub fn list(&self) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let mut result = Vec::new();
        for source in self.store.list()? {
            let mut value = serde_json::to_value(&source)?;
            value["connection_status"] = json!(self.state(&source.id));
            value["activity_revision"] = json!(self.revision().to_string());
            result.push(value);
        }
        Ok(json!(result))
    }
    fn presented(&self, source: Source) -> Result<Value> {
        let mut value = serde_json::to_value(&source)?;
        value["connection_status"] = json!(self.state(&source.specification.id));
        value["schema_status"] = Value::Object(
            source
                .tools
                .as_object()
                .ok_or("Catálogo MCP inválido")?
                .iter()
                .map(|(name, tool)| {
                    (
                        name.clone(),
                        json!(if validator(&tool["inputSchema"]).is_ok() {
                            "ready"
                        } else {
                            "unsupported"
                        }),
                    )
                })
                .collect(),
        );
        let page = self.pending_page_inner(&source.specification.id, None)?;
        value["pending_attempts"] = page["attempts"].clone();
        value["pending_next_cursor"] = page["next_cursor"].clone();
        value["pending_count"] = page["total"].clone();
        value["recent_reviews"] = json!(self.attempts.reviews(&source.specification.id)?);
        Ok(value)
    }
    pub fn get(&self, id: &str) -> Result<Value> {
        let _operation = self.activity.enter()?;
        self.presented(self.store.get(id)?)
    }
    pub fn tool_catalogue(&self, id: &str) -> Result<Value> {
        let mut value = self.get(id)?;
        let fields = value.as_object_mut().ok_or("Catálogo MCP inválido")?;
        let spec = fields
            .remove("specification")
            .ok_or("Falta identidad del catálogo MCP")?;
        fields.insert("id".into(), spec["id"].clone());
        fields.insert("name".into(), spec["name"].clone());
        fields.insert("transport".into(), spec["connection"]["type"].clone());
        Ok(value)
    }
    pub fn save(&self, spec: Specification, revision: i64) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let source = self.store.save(&spec, revision)?;
        self.disconnect_inner(&spec.id)?;
        self.changed();
        self.presented(source)
    }
    pub fn connect(&self, id: &str, revision: i64) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let source = self.store.get(id)?;
        if source.revision != revision {
            return Err("La conexión MCP cambió; recarga antes de conectar".into());
        }
        let inventory = self.with_client(&source, |client| client.inventory())?;
        let updated = self.store.catalogue(id, revision, &inventory["tools"])?;
        self.changed();
        self.presented(updated)
    }
    pub fn permissions(
        &self,
        id: &str,
        revision: i64,
        allowed: &[String],
        enabled: bool,
    ) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let current = self.store.get(id)?;
        for name in allowed {
            let tool = current
                .tools
                .get(name)
                .ok_or("Herramienta MCP no descubierta")?;
            validator(&tool["inputSchema"])?;
        }
        let source = self.store.permissions(id, revision, allowed, enabled)?;
        self.disconnect_inner(id)?;
        self.changed();
        self.presented(source)
    }
    pub fn delete(&self, id: &str, revision: i64) -> Result<Value> {
        let _operation = self.activity.enter()?;
        self.store.delete(id, revision)?;
        self.disconnect_inner(id)?;
        self.changed();
        Ok(json!({"status":"deleted","server_id":id}))
    }
    fn validate_execution(
        &self,
        source: &Source,
        tool: &str,
        revision: i64,
        arguments: &Value,
    ) -> Result<Option<Value>> {
        let id = source.specification.id.as_str();
        let metadata =
            || json!({"server_id":id,"server_name":source.specification.name,"tool":tool});
        let reject =
            |status: &str| Some(json!({"status":status,"executed":false,"source":metadata()}));
        if source.revision != revision {
            return Ok(reject("revision_conflict"));
        }
        if !source.enabled || !source.allowed_tools.iter().any(|name| name == tool) {
            return Ok(reject("permission_required"));
        }
        let Some(definition) = source.tools.get(tool) else {
            return Ok(reject("function_not_found"));
        };
        let Ok(validation) = validator(&definition["inputSchema"]) else {
            return Ok(reject("schema_unavailable"));
        };
        if !arguments.is_object() || serde_json::to_vec(arguments)?.len() > 60000 {
            return Ok(reject("invalid_arguments"));
        }
        let errors=validation.iter_errors(arguments).take(16).map(|e|json!({"path":e.instance_path().to_string(),"schema_path":e.schema_path().to_string()})).collect::<Vec<_>>();
        if !errors.is_empty() {
            return Ok(Some(
                json!({"status":"invalid_arguments","executed":false,"errors":errors,"source":metadata()}),
            ));
        }
        if let Some(attempt) = self.attempts.pending_tool(id, tool)? {
            return self.blocked(attempt, metadata()).map(Some);
        }
        self.permitted(&source.specification)?;
        Ok(None)
    }
    /// Valida contrato/permisos guardados; no descubre ni llama al servidor.
    pub fn prepare_execution(
        &self,
        id: &str,
        tool: &str,
        revision: i64,
        arguments: Value,
    ) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let source = self.store.get(id)?;
        if let Some(rejection) = self.validate_execution(&source, tool, revision, &arguments)? {
            return Ok(rejection);
        }
        Ok(
            json!({"status":"ready","executed":false,"source":{"server_id":id,"server_name":source.specification.name,"tool":tool},"revision":revision,"transport":"mcp"}),
        )
    }
    pub fn execute(&self, id: &str, tool: &str, revision: i64, arguments: Value) -> Result<Value> {
        self.execute_with_context(
            id,
            tool,
            revision,
            arguments,
            gennie_mcp::CallContext::default(),
        )
    }
    pub fn execute_with_context(
        &self,
        id: &str,
        tool: &str,
        revision: i64,
        arguments: Value,
        context: gennie_mcp::CallContext,
    ) -> Result<Value> {
        context.check()?;
        let _operation = self.activity.enter()?;
        let source = self.store.get(id)?;
        let metadata =
            || json!({"server_id":id,"server_name":source.specification.name,"tool":tool});
        if let Some(rejection) = self.validate_execution(&source, tool, revision, &arguments)? {
            return Ok(rejection);
        }
        // Refrescar contrato antes de actuar; cambios de esquema requieren leer la nueva revisión.
        let inventory =
            match self.with_client_context(&source, &context, |client| client.inventory()) {
                Err(error) if error.is::<Busy>() => {
                    return Ok(json!({"status":"busy","executed":false,"source":metadata()}));
                }
                value => value?,
            };
        let current = self.store.catalogue(id, revision, &inventory["tools"])?;
        if current.revision != revision {
            self.changed();
            return Ok(json!({"status":"revision_conflict","executed":false,"source":metadata()}));
        }
        // La revocación concurrente no comparte transacción con un servicio externo.
        let current = self.store.get(id)?;
        if !current.enabled
            || current.revision != revision
            || !current.allowed_tools.iter().any(|name| name == tool)
        {
            return Ok(
                json!({"status":"permission_required","executed":false,"source":metadata()}),
            );
        }
        let result = self.with_client_context(&current, &context, |client| {
            context.check()?;
            let mut live = self.live_attempts.lock().map_err(|_| "Estado de intentos MCP inválido")?;
            let attempt = match self.attempts.reserve(id, tool, revision)? {
                Reservation::Rejected(status) => return Ok(json!({"status":status,"executed":false,"source":metadata()})),
                Reservation::Blocked(attempt) => {
                    drop(live);
                    return self.blocked(attempt, metadata());
                }
                Reservation::Reserved(attempt) => attempt,
            };
            live.insert(attempt.id);
            drop(live);
            let _attempt = LiveAttempt { service: self, id: attempt.id };
            self.changed();
            let result = context.check().and_then(|_|client.call(tool, arguments));
            let reason = match &result {
                _ if context.is_cancelled()=>"transport_error",
                Ok(output) if output["isError"] != true => "reported_success",
                Ok(_) => "tool_error",
                Err(_) => "transport_error",
            };
            let saved = self.attempts.finish(attempt.id, reason);
            self.changed();
            let uncertain = saved.is_err() || reason != "reported_success";
            let output = result.ok();
            let mut detail = serde_json::to_value(saved.as_ref().unwrap_or(&attempt))?;
            detail["active"] = json!(false);
            if saved.is_err() {
                detail["state"] = json!("uncertain");
                detail["reason"] = json!("completion_unrecorded");
            }

            Ok(json!({"status":if saved.is_err() || reason=="transport_error" {"result_uncertain"} else if reason=="tool_error" {"tool_failed"}else{"ok"},
                "executed":true,"review_required":uncertain,"attempt_id":attempt.id,"attempt":detail,"result":output,"source":metadata(),
                "message":if uncertain {Some("Comprueba el efecto y revisa el intento en la ficha del plugin antes de repetir.")}else{None}}))
        });
        match result {
            Err(error) if error.is::<Busy>() => {
                Ok(json!({"status":"busy","executed":false,"source":metadata()}))
            }
            Ok(output) => Ok(output),
            Err(error) => Err(error),
        }
    }

    fn pending_page_inner(&self, id: &str, after: Option<i64>) -> Result<Value> {
        let (mut rows, total) = self.attempts.pending_page(id, after)?;
        let more = rows.len() > 50;
        if more {
            rows.pop();
        }
        let next = if more {
            rows.last().map(|a| a.id)
        } else {
            None
        };
        let values = rows
            .into_iter()
            .map(|a| self.present_attempt(a))
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"attempts":values,"total":total,"next_cursor":next}))
    }
    pub fn pending_page(&self, id: &str, after: Option<i64>) -> Result<Value> {
        let _operation = self.activity.enter()?;
        self.store.get(id)?;
        self.pending_page_inner(id, after)
    }
    fn present_attempt(&self, attempt: Attempt) -> Result<Value> {
        let active = self
            .live_attempts
            .lock()
            .map_err(|_| "Estado de intentos MCP inválido")?
            .contains(&attempt.id);
        let mut value = serde_json::to_value(&attempt)?;
        if !active && attempt.state == "running" {
            value["state"] = json!("uncertain");
            value["reason"] = json!("completion_unrecorded");
        }
        value["active"] = json!(active);
        Ok(value)
    }
    fn blocked(&self, attempt: Attempt, metadata: Value) -> Result<Value> {
        let attempt = self.present_attempt(attempt)?;
        Ok(
            json!({"status":if attempt["active"]==true {"busy"}else{"review_required"},"executed":false,"source":metadata,"attempt":attempt}),
        )
    }
    pub fn reconcile(
        &self,
        id: &str,
        revision: i64,
        attempt: i64,
        resolution: &str,
    ) -> Result<Value> {
        let _operation = self.activity.enter()?;
        let live = self
            .live_attempts
            .lock()
            .map_err(|_| "Estado de intentos MCP inválido")?;
        if live.contains(&attempt) {
            return Err("La llamada aún está en curso; desconecta y comprueba su resultado antes de revisarla".into());
        }
        self.attempts.reconcile(id, revision, attempt, resolution)?;
        drop(live);
        self.changed();
        self.presented(self.store.get(id)?)
    }
}

struct LiveAttempt<'a> {
    service: &'a Service,
    id: i64,
}
impl Drop for LiveAttempt<'_> {
    fn drop(&mut self) {
        if let Ok(mut live) = self.service.live_attempts.lock() {
            live.remove(&self.id);
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
