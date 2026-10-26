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
    pub fn turn(&mut self, text: &str, seconds: u64) -> Result<()> {
        self.0.build_turn(text, seconds)
    }
}
