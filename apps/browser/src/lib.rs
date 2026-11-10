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
        atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
    },
};
pub use tools::{
    contextual_tools, scoped_contextual_tools, tool_output, tools, validate as validate_arguments,
};

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
    session: Mutex<Option<Arc<process::Session>>>,
    attempts: Mutex<Option<attempts::Store>>,
    execution_revision: AtomicU64,
    active_web: AtomicI64,
    web_execution_gate: Mutex<()>,
    pending_task_pages: Mutex<Vec<(u64, String)>>,
    session_generation: AtomicU64,
    active_scoped: AtomicUsize,
    restore_warning: AtomicBool,
    cleanup_warning: AtomicBool,
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
                web_execution_gate: Mutex::new(()),
                pending_task_pages: Mutex::new(Vec::new()),
                session_generation: AtomicU64::new(0),
                active_scoped: AtomicUsize::new(0),
                restore_warning: AtomicBool::new(false),
                cleanup_warning: AtomicBool::new(false),
            }),
        }
    }
    pub fn open(&self) -> Result<()> {
        self.call(json!({"method":"open"})).map(|_| ())
    }
    pub fn is_busy(&self) -> bool {
        self.inner.active_scoped.load(Ordering::Acquire) != 0
            || matches!(
                self.inner.session.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            )
    }
    /// Consume un aviso fijo del worker sin iniciar procesos ni bloquear al coordinador.
    pub fn take_restore_warning(&self) -> bool {
        let stored = self.inner.restore_warning.swap(false, Ordering::AcqRel);
        let current = self
            .inner
            .session
            .try_lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|session| session.take_restore_warning()))
            .unwrap_or(false);
        stored || current
    }
    pub fn take_cleanup_warning(&self) -> bool {
        let stored = self.inner.cleanup_warning.swap(false, Ordering::AcqRel);
        let current = self
            .inner
            .session
            .try_lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|session| session.take_cleanup_warning()))
            .unwrap_or(false);
        stored || current
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
        request: Value,
        context: gennie_mcp::CallContext,
    ) -> Result<Value> {
        self.call_inner(request, context)
    }
    /// Libera el ámbito de un turno sin arrancar el navegador si no se utilizó.
    pub fn release_scope(&self, scope: &str) -> Result<()> {
        let context = gennie_mcp::CallContext::default().with_shutdown(self.inner.stopping.clone());
        let mut guard = loop {
            context.check()?;
            match self.inner.session.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err("Estado del navegador inválido".into());
                }
            }
        };
        let Some(session) = guard.as_mut() else {
            return Ok(());
        };
        if session.exited() {
            guard.take();
            return Ok(());
        }
        let session = session.clone();
        self.inner.active_scoped.fetch_add(1, Ordering::AcqRel);
        struct Active<'a>(&'a AtomicUsize);
        impl Drop for Active<'_> {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::AcqRel);
            }
        }
        let _active = Active(&self.inner.active_scoped);
        drop(guard);
        match session.request_with_context(&json!({"method":"scope_end","scope":scope}), &context) {
            Ok(value) => {
                let value = decode_response(value)?;
                if value["modal_pending"] == true {
                    return Err(
                        "Hay un diálogo pendiente en la pestaña de trabajo de Gennie Web".into(),
                    );
                }
                Ok(())
            }
            Err(error) => {
                self.retire_unusable_session(&session);
                Err(error)
            }
        }
    }
    fn retire_unusable_session(&self, session: &Arc<process::Session>) {
        if session.take_restore_warning() {
            self.inner.restore_warning.store(true, Ordering::Release);
        }
        if session.take_cleanup_warning() {
            self.inner.cleanup_warning.store(true, Ordering::Release);
        }
        if !session.is_usable()
            && let Ok(mut current) = self.inner.session.lock()
            && current
                .as_ref()
                .is_some_and(|value| Arc::ptr_eq(value, session))
        {
            current.take();
        }
    }
    /// Cierra solo una pestaña del constructor creada por esa sesión.
    pub fn close_owned_task_page(&self, page_id: &str, generation: u64) -> Result<()> {
        if page_id.is_empty() || page_id.len() > 32 || !page_id.bytes().all(|c| c.is_ascii_digit())
        {
            return Err("Identificador de pestaña inválido".into());
        }
        let entry = (generation, page_id.to_owned());
        {
            let mut pending = self
                .inner
                .pending_task_pages
                .lock()
                .map_err(|_| "Cierre de pestañas inválido")?;
            if !pending.contains(&entry) {
                pending.push(entry);
            }
        }
        let mut guard = match self.inner.session.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("Estado del navegador inválido".into());
            }
        };
        if guard.as_mut().is_some_and(|session| session.exited()) {
            guard.take();
        }
        if let Some(session) = guard.as_mut() {
            let context =
                gennie_mcp::CallContext::default().with_shutdown(self.inner.stopping.clone());
            if let Err(error) = self.flush_owned_task_pages(session, &context) {
                guard.take();
                return Err(error);
            }
        } else {
            self.clear_owned_task_pages()?;
        }
        Ok(())
    }
    fn clear_owned_task_pages(&self) -> Result<()> {
        self.inner
            .pending_task_pages
            .lock()
            .map_err(|_| "Cierre de pestañas inválido")?
            .clear();
        Ok(())
    }
    fn flush_owned_task_pages(
        &self,
        session: &process::Session,
        context: &gennie_mcp::CallContext,
    ) -> Result<()> {
        loop {
            let entry = self
                .inner
                .pending_task_pages
                .lock()
                .map_err(|_| "Cierre de pestañas inválido")?
                .first()
                .cloned();
            let Some((generation, id)) = entry else {
                return Ok(());
            };
            if generation == self.inner.session_generation.load(Ordering::Acquire) {
                let request = json!({"method":"task_page","action":"close","page_id":id});
                decode_response(session.request_with_context(&request, context)?)?;
            }
            let mut pending = self
                .inner
                .pending_task_pages
                .lock()
                .map_err(|_| "Cierre de pestañas inválido")?;
            pending.retain(|value| value != &(generation, id.clone()));
        }
    }
    /// El caller confirma la eliminación y cancela solo su trabajo bajo la exclusión del navegador.
    pub fn delete_plugin_from_user(
        &self,
        plugin_id: &str,
        revision: i64,
        before_delete: impl FnOnce() -> Result<()>,
    ) -> Result<Value> {
        if revision < 1 {
            return Err("Revisión de plugin inválida".into());
        }
        let context = gennie_mcp::CallContext::default().with_shutdown(self.inner.stopping.clone());
        context.check()?;
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
        if self.inner.active_scoped.load(Ordering::Acquire) != 0 {
            return Err(Box::new(BusyError));
        }
        if guard.as_mut().is_some_and(|s| s.exited()) {
            guard.take();
        }
        if guard.is_none() {
            self.clear_owned_task_pages()?;
            *guard = Some(process::Session::start(&self.inner.root)?);
            self.inner.session_generation.fetch_add(1, Ordering::AcqRel);
        }
        if let Err(error) = self.flush_owned_task_pages(
            guard.as_mut().ok_or("No se pudo iniciar el navegador")?,
            &context,
        ) {
            guard.take();
            return Err(error);
        }
        let session = guard.as_mut().ok_or("No se pudo iniciar el navegador")?;
        let get = json!({"method":"plugins","action":"get","plugin_id":plugin_id});
        let current = match session
            .request_with_context(&get, &context)
            .and_then(decode_response)
        {
            Ok(value) => value,
            Err(error) => {
                guard.take();
                return Err(error);
            }
        };
        if current["plugin"]["revision"].as_i64() != Some(revision) {
            return Ok(json!({"status":"revision_conflict"}));
        }
        before_delete()?;
        let delete = json!({"method":"plugins","action":"delete","plugin_id":plugin_id,"expected_revision":revision});
        match session
            .request_with_context(&delete, &context)
            .and_then(decode_response)
        {
            Ok(value) => Ok(value),
            Err(error) => {
                guard.take();
                Err(error)
            }
        }
    }
    fn call_inner(&self, mut request: Value, context: gennie_mcp::CallContext) -> Result<Value> {
        let context = context.with_shutdown(self.inner.stopping.clone());
        context.check()?;
        if let Some(object) = request.as_object_mut() {
            object.insert(
                "program_allowed".into(),
                json!(self.inner.program_allowed.load(Ordering::Acquire)),
            );
        }
        let mut guard = loop {
            context.check()?;
            match self.inner.session.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err("Estado del navegador inválido".into());
                }
            }
        };
        if !self.inner.enabled.load(Ordering::Acquire) {
            return Err("El módulo de navegador está desactivado".into());
        }
        if let Some(blocked) = self.web_gate(&request)? {
            return Ok(blocked);
        }
        if request["method"] == "gmail_reconcile" {
            let id = request["attempt_id"]
                .as_i64()
                .filter(|id| *id > 0)
                .ok_or("Falta intento web")?;
            if self.attempt_store()?.gmail_evidence(id)?.is_none() {
                return Ok(
                    json!({"status":"review_required","attempt_id":id,"executed":false,"message":"No hay evidencia previa suficiente para comprobar este intento automáticamente."}),
                );
            }
        }
        let _catalogue_gate = if matches!(request["method"].as_str(), Some("plugins" | "candidate"))
            && matches!(
                request["action"].as_str(),
                Some("save" | "delete" | "promote")
            ) {
            Some(
                self.inner
                    .web_execution_gate
                    .try_lock()
                    .map_err(|_| Box::new(BusyError) as Box<dyn std::error::Error + Send + Sync>)?,
            )
        } else {
            None
        };
        if guard.as_mut().is_some_and(|s| s.exited()) {
            guard.take();
        }
        if guard.is_none() {
            self.clear_owned_task_pages()?;
            *guard = Some(process::Session::start(&self.inner.root)?);
            self.inner.session_generation.fetch_add(1, Ordering::AcqRel);
        }
        if let Err(error) = self.flush_owned_task_pages(
            guard.as_mut().ok_or("No se pudo iniciar el navegador")?,
            &context,
        ) {
            guard.take();
            return Err(error);
        }
        if request["method"] == "gmail_reconcile" {
            let id = request["attempt_id"]
                .as_i64()
                .filter(|id| *id > 0)
                .ok_or("Falta intento web")?;
            let _gate = self
                .inner
                .web_execution_gate
                .try_lock()
                .map_err(|_| "El navegador sigue trabajando")?;
            let result =
                self.review_gmail_attempt(id, guard.as_ref().ok_or("Falta navegador")?, &context);
            if guard.as_ref().is_some_and(|session| !session.is_usable()) {
                guard.take();
            }
            return result;
        }
        let independent = (request["scope"].is_string() || request["background_scope"] == true)
            && !matches!(
                request["method"].as_str(),
                Some("plugin_run" | "candidate" | "plugins" | "task_page")
            );
        if independent {
            let session = guard
                .as_ref()
                .ok_or("No se pudo iniciar el navegador")?
                .clone();
            self.inner.active_scoped.fetch_add(1, Ordering::AcqRel);
            struct Active<'a>(&'a AtomicUsize);
            impl Drop for Active<'_> {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let _active = Active(&self.inner.active_scoped);
            drop(guard);
            return match session.request_with_context(&request, &context) {
                Ok(value) => self.web_metadata(decode_response(value)?),
                Err(error) => {
                    self.retire_unusable_session(&session);
                    Err(error)
                }
            };
        }
        if matches!(
            request["method"].as_str(),
            Some("plugin_run" | "candidate" | "plugins")
        ) && matches!(request["action"].as_str(), Some("run" | "verify"))
        {
            let session = guard
                .as_ref()
                .ok_or("No se pudo iniciar el navegador")?
                .clone();
            self.inner.active_scoped.fetch_add(1, Ordering::AcqRel);
            struct Active<'a>(&'a AtomicUsize);
            impl Drop for Active<'_> {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::AcqRel);
                }
            }
            let _active = Active(&self.inner.active_scoped);
            drop(guard);
            let _web_gate = loop {
                context.check()?;
                match self.inner.web_execution_gate.try_lock() {
                    Ok(guard) => break guard,
                    Err(std::sync::TryLockError::WouldBlock) => {
                        std::thread::sleep(std::time::Duration::from_millis(20))
                    }
                    Err(std::sync::TryLockError::Poisoned(_)) => {
                        return Err("Ejecución web inválida".into());
                    }
                }
            };
            let result = self.execute_web(&request, &session, &context);
            self.retire_unusable_session(&session);
            return result.map(|(value, _discard)| value);
        }
        let session = guard.as_mut().ok_or("No se pudo iniciar el navegador")?;
        match session.request_with_context(&request, &context) {
            Ok(value) => {
                let mut value = decode_response(value)?;
                if request["method"] == "task_page"
                    && request["action"] == "create"
                    && value["page_id"].is_string()
                {
                    value["session_generation"] =
                        json!(self.inner.session_generation.load(Ordering::Acquire));
                }
                self.web_metadata(value)
            }
            Err(error) => {
                // Un timeout no se reintenta: el efecto de una acción podría haberse aplicado.
                if !session.is_usable() {
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
        if let Some(session) = guard.take() {
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
