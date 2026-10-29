//! Multiplexa entrada/host para no dejar vivo el helper al cerrar su consumidor.
use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    net::Shutdown,
    os::fd::{AsFd, AsRawFd},
    time::Instant,
};
pub fn bridge(socket: &Path) -> Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut input = File::from(std::io::stdin().as_fd().try_clone_to_owned()?);
    let mut output = cancellable_io::NonblockingFile::new(&std::io::stdout())?;
    let mut queued = Vec::new();
    let mut written = 0;
    let mut requests = framing::Lines::default();
    let mut responses = framing::Lines::default();
    let mut pending = 0usize;
    let mut deadline = None::<Instant>;
    let mut buffer = [0; 8192];
    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(
                "El host MCP dejó de confirmar la operación; su resultado puede ser incierto"
                    .into(),
            );
        }
        let mut fds = [
            libc::pollfd {
                fd: input.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if queued.is_empty() {
                    stream.as_raw_fd()
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: output.file.as_raw_fd(),
                events: if queued.is_empty() { 0 } else { libc::POLLOUT },
                revents: 0,
            },
        ];
        // Los descriptores siguen vivos; el arreglo contiene exactamente tres pollfd
        // inicializados y poll solo escribe sus campos revents.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 50) };
        if ready < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e.into());
        }
        if fds[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                let _ = stream.shutdown(Shutdown::Both);
                return if pending == 0 && requests.is_empty() && queued.is_empty() {
                    Ok(())
                } else {
                    Err("El consumidor MCP cerró la entrada durante una operación".into())
                };
            }
            requests.push(&buffer[..n], LIMIT, |line| {
                // El host conserva sus errores de parse/forma, incluso con id=null.
                // Solo una notificación válida carece de respuesta.
                let expects = match serde_json::from_slice::<Value>(line) {
                    Ok(request) => {
                        request.get("id").is_some()
                            || request["method"].as_str().is_none()
                            || request["jsonrpc"] != "2.0"
                    }
                    Err(_) => true,
                };
                if expects && pending >= MAX_PENDING_REQUESTS {
                    return Err("Demasiadas peticiones MCP pendientes".into());
                }
                stream.write_all(line)?;
                if expects {
                    if pending == 0 {
                        deadline = Some(
                            Instant::now() + Duration::from_secs(BRIDGE_RESPONSE_TIMEOUT_SECS),
                        );
                    }
                    pending += 1;
                }
                Ok(())
            })?;
        }
        if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            let n = match stream.read(&mut buffer) {
                Ok(n) => n,
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
            };
            if n == 0 {
                return if pending == 0 && responses.is_empty() {
                    Ok(())
                } else {
                    Err("Respuesta MCP incompleta".into())
                };
            }
            responses.push(&buffer[..n], MAX_RESPONSE_BYTES, |line| {
                if pending == 0 {
                    return Err("Respuesta MCP sin petición pendiente".into());
                }
                queued.extend_from_slice(line);
                pending -= 1;
                deadline = (pending > 0)
                    .then(|| Instant::now() + Duration::from_secs(BRIDGE_RESPONSE_TIMEOUT_SECS));
                Ok(())
            })?;
        }
        if fds[2].revents & libc::POLLOUT != 0 {
            match output.file.write(&queued[written..]) {
                Ok(0) => return Err("El consumidor cerró la salida MCP".into()),
                Ok(n) => {
                    written += n;
                    if written == queued.len() {
                        queued.clear();
                        written = 0;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(e.into()),
            }
        }
        if fds[2].revents & libc::POLLHUP != 0 {
            return Err("El consumidor cerró la salida MCP".into());
        }
        if fds
            .iter()
            .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
        {
            return Err("El canal MCP se cerró durante la operación".into());
        }
    }
}
