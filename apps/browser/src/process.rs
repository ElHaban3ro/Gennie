use gennie_core::Result;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::JoinHandle,
    time::Duration,
};
// El mismo código se importa desde Node en sondas y se embebe en el helper release.
fn worker_source() -> String {
    include_str!("worker.mjs").replace(
        "import { createWebPlugins } from './plugins.mjs';",
        include_str!("plugins.mjs"),
    )
}
const LIMIT: usize = 128 * 1024;
const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;
const PLUGIN_RESPONSE_TIMEOUT_SECS: u64 = 60;
pub struct Session {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
    closed: bool,
}
impl Session {
    pub fn start(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err("No se encuentra el directorio de datos de Gennie".into());
        }
        let runtime = root.join("runtime-patchright-1.64.2");
        if !runtime.join("node").is_file()
            || !runtime
                .join("Gennie Web.app/Contents/MacOS/Google Chrome for Testing")
                .is_file()
            || !runtime
                .join("node_modules/patchright/package.json")
                .is_file()
        {
            return Err("Falta preparar Patchright. Ejecuta scripts/setup_browser.sh y vuelve a abrir el navegador".into());
        }
        let mut child = Command::new(runtime.join("node"))
            .args(["--input-type=module", "--eval", &worker_source(), "--"])
            .arg(&runtime)
            .arg(root.join("profile"))
            .arg("--gennie-browser-worker")
            .env("PLAYWRIGHT_BROWSERS_PATH", root.join("browsers"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|_| "No se pudo iniciar el runtime de Patchright")?;
        let input = child.stdin.take();
        let output = child.stdout.take().ok_or("Falta el canal del navegador")?;
        let (tx, replies) = mpsc::sync_channel(2);
        let reader = match std::thread::Builder::new()
            .name("gennie-browser-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(output);
                loop {
                    let mut line = Vec::new();
                    match reader
                        .by_ref()
                        .take((RESPONSE_LIMIT + 1) as u64)
                        .read_until(b'\n', &mut line)
                    {
                        Ok(n) if n > 0 && n <= RESPONSE_LIMIT => {
                            let Ok(value) = serde_json::from_slice(&line) else {
                                break;
                            };
                            if tx.send(value).is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
            }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        Ok(Self {
            child,
            input,
            replies,
            reader: Some(reader),
            closed: false,
        })
    }
    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }
    pub fn request(&mut self, value: &Value) -> Result<Value> {
        let line = serde_json::to_vec(value)?;
        if line.len() > LIMIT - 1 {
            return Err("Petición de navegador demasiado grande".into());
        }
        let input = self.input.as_mut().ok_or("El navegador está cerrado")?;
        input.write_all(&line)?;
        input.write_all(b"\n")?;
        input.flush()?;
        let response = self.replies.recv_timeout(Duration::from_secs(if value["method"] == "plugin_run" || value["method"] == "candidate" { PLUGIN_RESPONSE_TIMEOUT_SECS } else { 12 }))
            .map_err(|_| "El navegador dejó de responder; el resultado de la acción es incierto. Comprueba el navegador antes de repetirla")?;
        if let Some(error) = response["error"].as_str() {
            return Ok(serde_json::json!({"error":error,"code":response["code"],"ok":false}));
        }
        Ok(response)
    }
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        // EOF pide context.close(), que guarda el perfil antes de terminar.
        self.input.take();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !self.exited() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let forced = !self.exited();
        if forced {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", self.child.id())])
                .status();
            let _ = self.child.kill();
        }
        let status = self.child.wait()?;
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if forced {
            return Err("El navegador requirió cierre forzado; la última escritura del perfil no está confirmada".into());
        }
        if !status.success() {
            return Err(
                "El navegador terminó con error; la última escritura del perfil no está confirmada"
                    .into(),
            );
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
