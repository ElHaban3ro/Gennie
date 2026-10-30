//! Identidad/cancelación de una entrada explícita; no conserva texto ni espera red.
use crate::codex::ProcessHandle;
use gennie_core::Result;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub id: String,
    pub stopping: bool,
}
#[derive(Clone)]
pub(crate) struct Ticket {
    pub id: String,
    pub context: gennie_mcp::CallContext,
    gate: Arc<Mutex<()>>,
}
impl Ticket {
    /// Solo observación/commit/ack cortos; nunca ejecutar red dentro del gate.
    pub fn observe<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| "Estado de respuesta inválido")?;
        self.context.check()?;
        operation()
    }
}
struct State {
    prefix: String,
    sequence: u64,
    active: Option<Ticket>,
}
#[derive(Clone)]
pub(crate) struct Control {
    state: Arc<Mutex<State>>,
    process: ProcessHandle,
}
impl Control {
    pub fn new(process: ProcessHandle) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let prefix = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        Self {
            state: Arc::new(Mutex::new(State {
                prefix,
                sequence: 0,
                active: None,
            })),
            process,
        }
    }
    pub fn begin(&self) -> Result<Ticket> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Estado de respuesta inválido")?;
        if state.active.is_some() {
            return Err("Gennie aún está respondiendo".into());
        }
        state.sequence = state
            .sequence
            .checked_add(1)
            .ok_or("Se agotaron las identidades de respuesta")?;
        let ticket = Ticket {
            id: format!("{}-{}", state.prefix, state.sequence),
            context: Default::default(),
            gate: Default::default(),
        };
        state.active = Some(ticket.clone());
        Ok(ticket)
    }
    pub fn progress(&self) -> Option<Progress> {
        self.state.lock().ok()?.active.as_ref().map(|t| Progress {
            id: t.id.clone(),
            stopping: t.context.is_cancelled(),
        })
    }
    pub fn finish(&self, id: &str) {
        if let Ok(mut state) = self.state.lock()
            && state.active.as_ref().is_some_and(|t| t.id == id)
        {
            state.active = None;
        }
    }
    pub fn cancel(&self, id: &str) -> Result<bool> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Estado de respuesta inválido")?;
        let Some(ticket) = state.active.as_ref().filter(|t| t.id == id) else {
            return Ok(false);
        };
        let _guard = ticket
            .gate
            .lock()
            .map_err(|_| "Estado de respuesta inválido")?;
        ticket.context.cancel();
        // Mantener la identidad reservada hasta terminar kill: no alcanzar el siguiente hijo.
        self.process.kill();
        Ok(true)
    }
    pub fn cancel_current(&self) {
        if let Some(progress) = self.progress() {
            let _ = self.cancel(&progress.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_cancellation_and_completion_cannot_touch_a_new_response() {
        let control = Control::new(Default::default());
        let first = control.begin().unwrap();
        assert!(control.begin().is_err());
        assert!(!control.cancel("foreign").unwrap());
        assert!(control.cancel(&first.id).unwrap());
        assert!(control.cancel(&first.id).unwrap());
        assert!(
            first
                .observe(|| Ok(()))
                .unwrap_err()
                .is::<gennie_mcp::Cancelled>()
        );
        control.finish(&first.id);
        let second = control.begin().unwrap();
        assert_ne!(first.id, second.id);
        control.finish(&first.id);
        assert!(!control.cancel(&first.id).unwrap());
        assert!(!second.context.is_cancelled());
        assert_eq!(control.progress().unwrap().id, second.id);
    }
    #[test]
    fn cancellation_waits_for_an_observation_commit_but_never_for_network() {
        let control = Control::new(Default::default());
        let ticket = control.begin().unwrap();
        let (entered, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let worker_ticket = ticket.clone();
        let worker = std::thread::spawn(move || {
            worker_ticket.observe(|| {
                entered.send(()).unwrap();
                wait.recv().unwrap();
                Ok(())
            })
        });
        ready.recv().unwrap();
        let cancel_control = control.clone();
        let id = ticket.id.clone();
        let cancel = std::thread::spawn(move || cancel_control.cancel(&id));
        release.send(()).unwrap();
        worker.join().unwrap().unwrap();
        assert!(cancel.join().unwrap().unwrap());
        assert!(ticket.context.is_cancelled());
    }
}
