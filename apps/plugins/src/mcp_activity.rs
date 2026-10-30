//! El cierre cancela primero el transporte y espera también la escritura final del diario.
use gennie_core::Result;
use std::sync::{Condvar, Mutex};
#[derive(Default)]
struct State {
    open: bool,
    count: usize,
}
#[derive(Default)]
pub struct Activity {
    state: Mutex<State>,
    idle: Condvar,
}
pub struct Operation<'a>(&'a Activity);
impl Activity {
    pub fn start(&self, recover: impl FnOnce() -> Result<()>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| "Estado MCP inválido")?;
        if state.open {
            return Ok(());
        }
        if state.count != 0 {
            return Err("El cliente MCP aún se está cerrando".into());
        }
        recover()?;
        state.open = true;
        Ok(())
    }
    pub fn enter(&self) -> Result<Operation<'_>> {
        let mut state = self.state.lock().map_err(|_| "Estado MCP inválido")?;
        if !state.open {
            return Err("El cliente MCP no está activo o se está cerrando".into());
        }
        state.count += 1;
        Ok(Operation(self))
    }
    pub fn close(&self) -> Result<()> {
        self.state.lock().map_err(|_| "Estado MCP inválido")?.open = false;
        Ok(())
    }
    pub fn drain(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| "Estado MCP inválido")?;
        while state.count > 0 {
            state = self.idle.wait(state).map_err(|_| "Cierre MCP inválido")?;
        }
        Ok(())
    }
}
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.count -= 1;
            self.0.idle.notify_all();
        }
    }
}
