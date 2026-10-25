use crate::{Event, model};
use gennie_core::Result;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

pub struct Process {
    child: Child,
    input: Option<ChildStdin>,
    messages: Arc<Mutex<VecDeque<Event>>>,
    reader: Option<JoinHandle<()>>,
    partial: std::path::PathBuf,
}
impl Process {
    pub fn resident_bytes(&mut self) -> Option<u64> {
        if self.child.try_wait().ok()?.is_some() {
            return Some(0);
        }
        gennie_core::process_resident_bytes(self.child.id())
    }

    pub fn spawn(flag: &str, path: &Path, microphone: Option<&str>) -> Result<Self> {
        let mut command = Command::new(
            crate::HELPER_EXECUTABLE
                .get()
                .cloned()
                .map(Ok)
                .unwrap_or_else(std::env::current_exe)?,
        );
        command.arg(flag).arg(path);
        if let Some(name) = microphone {
            command.arg("--microphone").arg(name);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take();
        let output = child.stdout.take().ok_or("Sin salida del worker")?;
        let messages = Arc::new(Mutex::new(VecDeque::with_capacity(16)));
        let queue = messages.clone();
        let reader = std::thread::Builder::new()
            .name("transcription-output".into())
            .spawn(move || {
                let mut output = BufReader::new(output);
                loop {
                    let mut bytes = Vec::new();
                    // El worker solo envía estados y un máximo de 360 caracteres.
                    let read = Read::by_ref(&mut output)
                        .take(8192)
                        .read_until(b'\n', &mut bytes);
                    if !matches!(read,Ok(n) if n>0) {
                        break;
                    }
                    if let Ok(event) = serde_json::from_slice::<Event>(&bytes)
                        && let Ok(mut q) = queue.lock()
                    {
                        if q.len() == 16 {
                            q.pop_front();
                        }
                        q.push_back(event);
                    }
                }
            });
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        };
        let partial = model::partial(path, child.id());
        Ok(Self {
            child,
            input,
            messages,
            reader: Some(reader),
            partial,
        })
    }
    pub fn command(&mut self, command: &str) -> Result<()> {
        let input = self.input.as_mut().ok_or("Worker cerrado")?;
        writeln!(input, "{command}")?;
        input.flush()?;
        Ok(())
    }
    pub fn events(&self) -> Vec<Event> {
        self.messages
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
    pub fn exited(&mut self) -> bool {
        if self.child.try_wait().is_ok_and(|s| s.is_some()) {
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
            true
        } else {
            false
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = std::fs::remove_file(&self.partial);
    }
}
