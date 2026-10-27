//! Navegador propio: un contexto persistente compartido por el escritorio y MCP.
mod process;
mod tools;
use gennie_core::{AppModule, AppState, Result};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
pub use tools::{tools, validate as validate_arguments};

#[derive(Debug)]
pub struct BusyError;
impl std::fmt::Display for BusyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("El navegador de Gennie está ocupado")
    }
}
impl std::error::Error for BusyError {}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<Inner>,
}
struct Inner {
    root: PathBuf,
    enabled: AtomicBool,
    automation_allowed: AtomicBool,
    file_access_allowed: AtomicBool,
    program_allowed: AtomicBool,
    session: Mutex<Option<process::Session>>,
}
impl Controller {
    pub fn new(root: PathBuf) -> Self {
        Self {
            inner: Arc::new(Inner {
                root,
                enabled: AtomicBool::new(false),
                automation_allowed: AtomicBool::new(false),
                file_access_allowed: AtomicBool::new(false),
                program_allowed: AtomicBool::new(false),
                session: Mutex::new(None),
            }),
        }
    }
    pub fn open(&self) -> Result<()> {
        self.call(json!({"method":"open"})).map(|_| ())
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
        self.call(request)
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
    pub fn call(&self, mut request: Value) -> Result<Value> {
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
        if guard.as_mut().is_some_and(|s| s.exited()) {
            guard.take();
        }
        if guard.is_none() {
            *guard = Some(process::Session::start(&self.inner.root)?);
        }
        let session = guard.as_mut().ok_or("No se pudo iniciar el navegador")?;
        match session.request(&request) {
            Ok(value) => {
                if let Some(error) = value["error"].as_str() {
                    return Err(error.to_owned().into());
                }
                Ok(value)
            }
            Err(error) => {
                // Un timeout no se reintenta: el efecto de una acción podría haberse aplicado.
                guard.take();
                Err(error)
            }
        }
    }
    fn stop(&self) -> Result<()> {
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
    fn start(&self, _: &AppState) -> Result<()> {
        // Registrar el módulo no abre ventanas ni descarga dependencias.
        self.controller.inner.enabled.store(true, Ordering::Release);
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        self.controller.stop()
    }
}

#[cfg(test)]
mod tests;
