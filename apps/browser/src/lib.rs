//! Navegador propio: un contexto persistente compartido por el escritorio y MCP.
mod attempts;
mod process;
mod tools;
mod web_execution;
use gennie_core::{AppModule, AppState, Result};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    },
};
pub use tools::{contextual_tools, tool_output, tools, validate as validate_arguments};

#[derive(Debug)]
pub struct BusyError;
impl std::fmt::Display for BusyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("El navegador de Gennie está ocupado")
    }
}
impl std::error::Error for BusyError {}

#[derive(Debug)]
pub struct ClosedPageError;
impl std::fmt::Display for ClosedPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("El navegador o la pestaña están cerrados")
    }
}
impl std::error::Error for ClosedPageError {}

fn decode_response(value: Value) -> Result<Value> {
    if value["code"] == "closed" {
        return Err(Box::new(ClosedPageError));
    }
    if let Some(error) = value["error"].as_str() {
        return Err(error.to_owned().into());
    }
    Ok(value)
}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<Inner>,
}
struct Inner {
    root: PathBuf,
    enabled: AtomicBool,
    stopping: Arc<AtomicBool>,
    automation_allowed: AtomicBool,
    file_access_allowed: AtomicBool,
    program_allowed: AtomicBool,
    session: Mutex<Option<process::Session>>,
    attempts: Mutex<Option<attempts::Store>>,
    execution_revision: AtomicU64,
    active_web: AtomicI64,
}
impl Controller {
    pub fn new(root: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                root,
                enabled: AtomicBool::new(false),
                stopping: Arc::new(AtomicBool::new(false)),
                automation_allowed: AtomicBool::new(false),
                file_access_allowed: AtomicBool::new(false),
                program_allowed: AtomicBool::new(false),
                session: Mutex::new(None),
                attempts: Mutex::new(None),
                execution_revision: AtomicU64::new(0),
                active_web: AtomicI64::new(0),
            }),
        }
    }
    pub fn open(&self) -> Result<()> {
        self.call(json!({"method":"open"})).map(|_| ())
    }
    pub fn is_busy(&self) -> bool {
        matches!(
            self.inner.session.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        )
    }
    pub fn set_automation_allowed(&self, allowed: bool) {
        self.inner
            .automation_allowed
            .store(allowed, Ordering::Release);
    }
    pub fn set_file_access_allowed(&self, allowed: bool) {
        self.inner
            .file_access_allowed
            .store(allowed, Ordering::Release);
    }
    pub fn set_program_allowed(&self, allowed: bool) {
        self.inner.program_allowed.store(allowed, Ordering::Release);
    }
    pub fn automate(&self, request: Value) -> Result<Value> {
        self.automate_with_context(request, gennie_mcp::CallContext::default())
    }
    pub fn automate_with_context(
        &self,
        request: Value,
        context: gennie_mcp::CallContext,
    ) -> Result<Value> {
        context.check()?;
        if !self.inner.automation_allowed.load(Ordering::Acquire) {
            return Err(
                "El control de aplicaciones está desactivado en los ajustes del asistente".into(),
            );
        }
        if request["method"] == "upload"
            && request["files"]
                .as_array()
                .is_some_and(|files| !files.is_empty())
            && !self.inner.file_access_allowed.load(Ordering::Acquire)
        {
            return Err(
                "La subida de archivos requiere el permiso de lectura de archivos del asistente"
                    .into(),
            );
        }
        self.call_with_context(request, context)
    }
    /// Invoca la misma función verificada que MCP, con los permisos del navegador.
    /// Es bloqueante: consumir desde un worker, nunca desde el hilo visual.
    pub fn run_web_plugin(
        &self,
        plugin_id: &str,
        function_name: &str,
        revision: u64,
        arguments: Value,
    ) -> Result<Value> {
        self.automate(json!({"method":"plugin_run", "action":"run",
            "plugin_id":plugin_id, "function_name":function_name,
            "expected_revision":revision, "arguments":arguments}))
    }
    pub fn call(&self, request: Value) -> Result<Value> {
        self.call_with_context(request, gennie_mcp::CallContext::default())
    }
    pub fn call_with_context(
        &self,
        mut request: Value,
        context: gennie_mcp::CallContext,
    ) -> Result<Value> {
        let context = context.with_shutdown(self.inner.stopping.clone());
        context.check()?;
        if let Some(object) = request.as_object_mut() {
            object.insert(
                "program_allowed".into(),
                json!(self.inner.program_allowed.load(Ordering::Acquire)),
            );
        }
        let mut guard = match self.inner.session.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => return Err(Box::new(BusyError)),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("Estado del navegador inválido".into());
            }
        };
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("El módulo de navegador está desactivado".into());
        }
        if let Some(blocked) = self.web_gate(&request)? {
            return Ok(blocked);
        }
        if guard.as_mut().is_some_and(|s| s.exited()) {
            guard.take();
        }
        if guard.is_none() {
            *guard = Some(process::Session::start(&self.inner.root)?);
        }
        let session = guard.as_mut().ok_or("No se pudo iniciar el navegador")?;
        if matches!(
            request["method"].as_str(),
            Some("plugin_run" | "candidate" | "plugins")
        ) && matches!(request["action"].as_str(), Some("run" | "verify"))
        {
            return match self.execute_web(&request, session, &context) {
                Ok((value, discard)) => {
                    if discard {
                        guard.take();
                    }
                    Ok(value)
                }
                Err(error) => {
                    if !(error.is::<gennie_mcp::Cancelled>() && session.is_usable()) {
                        guard.take();
                    }
                    Err(error)
                }
            };
        }
        match session.request_with_context(&request, &context) {
            Ok(value) => self.web_metadata(decode_response(value)?),
            Err(error) => {
                // Un timeout no se reintenta: el efecto de una acción podría haberse aplicado.
                if !(error.is::<gennie_mcp::Cancelled>() && session.is_usable()) {
                    guard.take();
                }
                Err(error)
            }
        }
    }
    fn stop(&self) -> Result<()> {
        self.inner.stopping.store(true, Ordering::Release);
        self.inner.enabled.store(false, Ordering::Release);
        let mut guard = self
            .inner
            .session
            .lock()
            .map_err(|_| "Estado del navegador inválido")?;
        self.inner.enabled.store(false, Ordering::Release);
        if let Some(mut session) = guard.take() {
            session.close()?;
        }
        Ok(())
    }
}
pub struct BrowserModule {
    controller: Controller,
}
impl Default for BrowserModule {
    fn default() -> Self {
        let root = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Library/Application Support/Gennie/browser");
        Self {
            controller: Controller::new(root),
        }
    }
}
impl BrowserModule {
    /// Permite leer el diario con el módulo apagado; el arranque recupera reservas.
    pub fn with_database(self, db: gennie_db::Database) -> Self {
        *self
            .controller
            .inner
            .attempts
            .lock()
            .expect("Diario web inválido") = Some(attempts::Store(db));
        self
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            controller: Controller::new(root),
        }
    }
    pub fn controller(&self) -> Controller {
        self.controller.clone()
    }
}
impl AppModule for BrowserModule {
    fn name(&self) -> &'static str {
        "browser"
    }
    fn migrations(&self) -> &'static [gennie_core::Migration] {
        attempts::MIGRATIONS
    }
    fn start(&self, state: &AppState) -> Result<()> {
        // Registrar el módulo no abre ventanas ni descarga dependencias.
        let _guard = self
            .controller
            .inner
            .session
            .lock()
            .map_err(|_| "Estado del navegador inválido")?;
        if self.controller.inner.enabled.load(Ordering::Acquire) {
            return Ok(());
        }
        let store = attempts::Store(state.db.clone());
        store.recover()?;
        *self
            .controller
            .inner
            .attempts
            .lock()
            .map_err(|_| "Diario web inválido")? = Some(store);
        self.controller
            .inner
            .stopping
            .store(false, Ordering::Release);
        self.controller.inner.enabled.store(true, Ordering::Release);
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        self.controller.stop()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod web_execution_tests;
