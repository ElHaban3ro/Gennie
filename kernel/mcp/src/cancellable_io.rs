//! Escrituras de pipes/sockets propias con cancelación aun si el consumidor no lee.
use super::*;
use std::{
    fs::File,
    io::Write,
    os::fd::{AsFd, AsRawFd},
    time::Instant,
};
/// El llamador debe poseer el único escritor durante toda la vida del guard.
/// dup comparte los flags de estado del descriptor; Drop restituye su modo.
pub(super) struct NonblockingFile {
    pub file: File,
    original: i32,
}
impl NonblockingFile {
    pub fn new(target: &impl AsFd) -> Result<Self> {
        let file = File::from(target.as_fd().try_clone_to_owned()?);
        let fd = file.as_raw_fd();
        // El File duplicado mantiene vivo el descriptor durante ambas llamadas fcntl.
        let original = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if original < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if unsafe { libc::fcntl(fd, libc::F_SETFL, original | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self { file, original })
    }
}
impl Drop for NonblockingFile {
    fn drop(&mut self) {
        // file sigue vivo durante Drop; el descriptor pertenece a este guard.
        unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_SETFL, self.original) };
    }
}
/// Requiere propiedad exclusiva de las escrituras mientras dura la llamada.
pub fn write_cancellable(
    target: &impl AsFd,
    bytes: &[u8],
    context: &CallContext,
    timeout: Duration,
) -> Result<()> {
    context.check()?;
    let mut writer = NonblockingFile::new(target)?;
    let fd = writer.file.as_raw_fd();
    let deadline = Instant::now() + timeout;
    let mut sent = 0;
    while sent < bytes.len() {
        context.check()?;
        if Instant::now() >= deadline {
            return Err("El canal dejó de aceptar datos; el resultado puede ser incierto".into());
        }
        match writer.file.write(&bytes[sent..]) {
            Ok(0) => return Err("El canal cerró durante el envío".into()),
            Ok(n) => sent += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                let mut pollfd = libc::pollfd {
                    fd,
                    events: libc::POLLOUT,
                    revents: 0,
                };
                // pollfd está inicializado y fd pertenece al File que sigue vivo.
                let ready = unsafe { libc::poll(&mut pollfd, 1, 50) };
                if ready < 0 {
                    let e = std::io::Error::last_os_error();
                    if e.kind() != std::io::ErrorKind::Interrupted {
                        return Err(e.into());
                    }
                }
                if pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                    return Err("El consumidor cerró el canal durante el envío".into());
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    context.check()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn a_blocked_writer_cancels_and_restores_its_original_fd_mode() {
        let (writer, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let original = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
        let context = CallContext::default();
        let other = context.clone();
        let task = std::thread::spawn(move || {
            let result = write_cancellable(
                &writer,
                &vec![b'x'; 8 * 1024 * 1024],
                &other,
                Duration::from_secs(10),
            );
            (writer, result)
        });
        let mut bytes = [0; 8];
        reader.read_exact(&mut bytes).unwrap();
        context.cancel();
        let (writer, result) = task.join().unwrap();
        assert!(result.unwrap_err().is::<Cancelled>());
        assert_eq!(
            // macOS añade un indicador interno tras escribir; comprobamos el modo modificado.
            unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) } & libc::O_NONBLOCK,
            original & libc::O_NONBLOCK
        );
    }
}
