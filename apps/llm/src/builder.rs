//! Sesión técnica independiente: sin diario ni hilo conversacional de Gennie.
use crate::{Settings, codex};
use gennie_core::Result;
use std::path::PathBuf;
#[derive(Clone, Default)]
pub struct BuilderControl(codex::ProcessHandle);
impl BuilderControl {
    pub fn cancel(&self) {
        self.0.cancel();
    }
}
pub struct BuilderSession(codex::Session);
impl BuilderSession {
    pub fn connect(
        base: PathBuf,
        settings: &Settings,
        tools: gennie_mcp::Launch,
        instructions: String,
        saved_thread: Option<String>,
        control: BuilderControl,
    ) -> Result<Self> {
        Ok(Self(codex::Session::connect_builder(
            control.0,
            base,
            settings,
            tools,
            instructions,
            saved_thread,
        )?))
    }
    pub fn thread_id(&self) -> &str {
        self.0.thread_id()
    }
    pub fn set_goal(&mut self, objective: &str) -> Result<()> {
        self.0.builder_goal(objective)
    }
    pub fn set_goal_status(&mut self, status: &str) -> Result<()> {
        self.0.builder_goal_status(status)
    }
    pub fn turn(&mut self, text: &str) -> Result<()> {
        self.0.build_turn(text)
    }
}

/// Solo errores de comunicación identificados; no deducir fallos del proveedor a partir de texto libre.
pub fn is_retryable(error: &(dyn std::error::Error + 'static)) -> bool {
    if error.is::<codex::RetryableProviderError>() {
        return true;
    }
    error.downcast_ref::<std::io::Error>().is_some_and(|e| {
        matches!(
            e.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::UnexpectedEof
        )
    })
}
