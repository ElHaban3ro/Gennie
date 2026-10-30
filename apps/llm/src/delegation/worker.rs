use super::*;
use crate::{Event, Settings, codex};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::JoinHandle,
    time::Duration,
};
pub(crate) struct Worker {
    pub receiver: Receiver,
    stopping: Arc<AtomicBool>,
    process: codex::ProcessHandle,
    handle: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn start(
        db: Database,
        journal: Option<Arc<dyn ConversationJournal>>,
        events: Sender<Event>,
        guide: Option<crate::notices::guidance::GuideFactory>,
    ) -> Result<Self> {
        let (receiver, signal) = Receiver::new(db, journal);
        receiver.0.store.recover()?;
        crate::notices::Store(receiver.0.store.0.clone()).recover()?;
        let stopping = Arc::new(AtomicBool::new(false));
        let process = codex::ProcessHandle::default();
        let thread_receiver = receiver.clone();
        let thread_stopping = stopping.clone();
        let thread_process = process.clone();
        let handle = std::thread::Builder::new().name("bot-delegations".into()).spawn(move || {
            let mut warned = false;
            let mut recovering = false;
            while !thread_stopping.load(Ordering::Acquire) {
                let recovery = if recovering { thread_receiver.0.store.recover().and_then(|()|crate::notices::Store(thread_receiver.0.store.0.clone()).recover()) } else { Ok(()) };
                let result = recovery.and_then(|()| step(&thread_receiver, &|settings, prompt| {
                    codex::Session::render_delegation(thread_process.clone(), settings, prompt)
                })).and_then(|processed| {
                    if processed { return Ok(true); }
                    let Some(journal)=thread_receiver.0.journal.as_ref() else {return Ok(false);};
                    crate::notices::step_with_guide(&thread_receiver.0.store.0,journal.as_ref(),&|settings,instructions,prompt,output| {
                        codex::Session::render_scoped_message(thread_process.clone(),settings,instructions,prompt,output)
                    },&||thread_stopping.load(Ordering::Acquire),guide.as_ref())
                });
                match result {
                    Ok(true) => { warned = false; recovering = false; continue; }
                    Ok(false) => { warned = false; recovering = false; }
                    Err(_) => {
                        recovering = true;
                        if !warned {
                            let _ = events.send(Event::HistoryWarning("No pude confirmar una actualización del bot. Conservaré el resultado sin repetir acciones ni consultas.".into()));
                            warned = true;
                        }
                    }
                }
                if recovering { let _ = signal.recv_timeout(Duration::from_millis(500)); }
                else if signal.recv().is_err() { break; }
            }
        })?;
        Ok(Self {
            receiver,
            stopping,
            process,
            handle: Some(handle),
        })
    }
    pub fn close_ingress(&self) -> Result<()> {
        *self
            .receiver
            .0
            .accepting
            .lock()
            .map_err(|_| "Entrada del bot inválida")? = false;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        let gate = self.close_ingress();
        self.stopping.store(true, Ordering::Release);
        let _ = self.receiver.0.wake.try_send(());
        self.process.cancel();
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| "El worker del bot terminó inesperadamente")?;
        }
        gate
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn step(receiver: &Receiver, render: &impl Fn(&Settings, &str) -> Result<String>) -> Result<bool> {
    let Some((update, ready)) = receiver.0.store.next()? else {
        return Ok(false);
    };
    let journal = receiver
        .0
        .journal
        .as_ref()
        .ok_or("El bot no tiene diario de conversaciones")?;
    if ready {
        receiver.0.store.deliver(&update, journal.as_ref())?;
        return Ok(true);
    }
    let Some(context) = journal.delegation_context(&update.origin)? else {
        receiver.0.store.discard(&update)?;
        return Ok(true);
    };
    let bot = architects::load(&receiver.0.store.0, architects::Role::Bot)?;
    let mut duration_ms = None;
    let (text, failure) = if update.needs_model() {
        match prompt(&update, &context)
            .and_then(|prompt| {
                receiver.0.store.reserve_render(&update, &bot)?;
                let started = std::time::Instant::now();
                let result = render(&bot.settings(), &prompt);
                duration_ms =
                    Some(i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX));
                result
            })
            .and_then(|text| validate_rendered(&text))
        {
            Ok(text) => (text, None),
            Err(_) => (
                update.explanation_unavailable_message(),
                Some("render_failed"),
            ),
        }
    } else {
        (update.routine_message(), None)
    };
    let closing = !*receiver
        .0
        .accepting
        .lock()
        .map_err(|_| "Entrada del bot inválida")?;
    let failure = if closing && failure.is_some() {
        Some("interrupted")
    } else {
        failure
    };
    receiver
        .0
        .store
        .ready(&update, &text, bot.revision, failure, duration_ms)?;
    if closing {
        return Ok(true);
    }
    receiver.0.store.deliver(&update, journal.as_ref())?;
    Ok(true)
}
#[cfg(test)]
pub(super) fn test_step(
    receiver: &Receiver,
    render: &impl Fn(&Settings, &str) -> Result<String>,
) -> Result<bool> {
    step(receiver, render)
}
