//! Cancelación explícita de un consumidor, independiente de las otras conexiones.
use super::*;
#[derive(Clone, Default)]
pub struct CallContext {
    cancelled: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    parent: Option<Arc<CallContext>>,
}
#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Operación cancelada")
    }
}
impl std::error::Error for Cancelled {}
impl CallContext {
    pub(super) fn for_server(stopping: Arc<AtomicBool>) -> Self {
        Self {
            stopping,
            ..Self::default()
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
            || self.stopping.load(Ordering::Acquire)
            || self.parent.as_ref().is_some_and(|p| p.is_cancelled())
    }
    /// Suma el cierre del propietario sin cancelar el contexto del consumidor.
    pub fn with_shutdown(self, stopping: Arc<AtomicBool>) -> Self {
        Self {
            stopping,
            parent: Some(Arc::new(self)),
            ..Self::default()
        }
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Box::new(Cancelled))
        } else {
            Ok(())
        }
    }
}
pub struct ContextTool {
    pub definition: Value,
    pub call: Box<dyn Fn(Value, CallContext) -> Result<Value> + Send + Sync>,
}
impl Tool {
    pub fn with_context(
        self,
        call: impl Fn(Value, CallContext) -> Result<Value> + Send + Sync + 'static,
    ) -> ContextTool {
        ContextTool {
            definition: self.definition,
            call: Box::new(call),
        }
    }
}
impl From<Tool> for ContextTool {
    fn from(tool: Tool) -> Self {
        ContextTool {
            definition: tool.definition,
            call: Box::new(move |value, _| (tool.call)(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        sync::mpsc,
        time::Instant,
    };
    fn waiting() -> (Registry, mpsc::Receiver<()>, mpsc::Receiver<()>) {
        let (entered, ready) = mpsc::channel();
        let (cancelled, done) = mpsc::channel();
        let mut registry = Registry::default();
        registry
            .register([ContextTool {
                definition: json!({"name":"wait"}),
                call: Box::new(move |_, context| {
                    entered.send(()).unwrap();
                    while !context.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    cancelled.send(()).unwrap();
                    context.check()?;
                    Ok(json!({}))
                }),
            }])
            .unwrap();
        registry
            .register([Tool {
                definition: json!({"name":"fast"}),
                call: Box::new(|_| Ok(json!({"ok":true}))),
            }])
            .unwrap();
        (registry, ready, done)
    }
    #[test]
    fn peer_eof_cancels_only_that_connection_and_other_clients_still_work() {
        let (registry, entered, cancelled) = waiting();
        let mut server = Server::start(registry, &std::env::current_exe().unwrap()).unwrap();
        let mut stream = UnixStream::connect(&server.launch().socket).unwrap();
        writeln!(
            stream,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait"}})
        )
        .unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(stream);
        cancelled.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut other = UnixStream::connect(&server.launch().socket).unwrap();
        other
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        writeln!(
            other,
            "{}",
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"fast"}})
        )
        .unwrap();
        let mut line = String::new();
        BufReader::new(other).read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(result["result"]["structuredContent"]["ok"], true);
        server.stop().unwrap();
    }
    #[test]
    fn stopping_host_cancels_waiting_context_before_joining_and_does_not_call_cancelled_tools() {
        let (registry, entered, cancelled) = waiting();
        let context = CallContext::default();
        context.cancel();
        let result = registry
            .respond_with_context(
                json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wait"}}),
                context,
            )
            .unwrap();
        assert_eq!(result["result"]["isError"], true);
        assert!(entered.try_recv().is_err());
        let mut server = Server::start(registry, &std::env::current_exe().unwrap()).unwrap();
        let mut stream = UnixStream::connect(&server.launch().socket).unwrap();
        writeln!(
            stream,
            "{}",
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"wait"}})
        )
        .unwrap();
        entered.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        server.stop().unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        cancelled.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!server.launch().socket.exists());
    }
}
