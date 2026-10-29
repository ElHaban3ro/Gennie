//! La lectura de EOF sigue activa mientras una herramienta espera IO.
use super::*;
use std::{
    io::{Read, Write},
    net::Shutdown,
    sync::mpsc::{self, RecvTimeoutError},
};
pub(super) fn serve(
    mut stream: UnixStream,
    registry: &Registry,
    stopping: Arc<AtomicBool>,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut input = stream.try_clone()?;
    let context = CallContext::for_server(stopping);
    let reader_context = context.clone();
    let (sender, requests) = mpsc::sync_channel(MAX_PENDING_REQUESTS);
    let reader = std::thread::Builder::new()
        .name("gennie-mcp-input".into())
        .spawn(move || {
            let result = (|| -> Result<()> {
                let mut frames = framing::Lines::default();
                let mut buffer = [0; 8192];
                while !reader_context.is_cancelled() {
                    match input.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => frames.push(&buffer[..n], LIMIT, |line| {
                            sender
                                .try_send(serde_json::from_slice::<Value>(line).ok())
                                .map_err(|_| {
                                    "La cola de peticiones MCP está llena o cerrada".into()
                                })
                        })?,
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock
                                    | std::io::ErrorKind::TimedOut
                                    | std::io::ErrorKind::Interrupted
                            ) =>
                        {
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Ok(())
            })();
            reader_context.cancel();
            result
        })?;
    let result = (|| -> Result<()> {
        while !context.is_cancelled() {
            let request = match requests.recv_timeout(Duration::from_millis(50)) {
                Ok(value) => value,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            context.check()?;
            let response = match request {
                Some(request) => registry.respond_with_context(request, context.clone()),
                None => Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"JSON inválido"}}),
                ),
            };
            if context.is_cancelled() {
                break;
            }
            if let Some(response) = response {
                stream.write_all(&encode_response(response)?)?;
                stream.write_all(b"\n")?;
            }
        }
        Ok(())
    })();
    context.cancel();
    let _ = stream.shutdown(Shutdown::Both);
    let read = reader
        .join()
        .map_err(|_| "El lector MCP terminó inesperadamente")?;
    result.and(read)
}
