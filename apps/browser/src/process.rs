use gennie_core::Result;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
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
const PLUGIN_RESPONSE_TIMEOUT_SECS: u64 = gennie_mcp::MAX_TOOL_EXECUTION_SECS;
type Pending = Arc<Mutex<HashMap<u64, SyncSender<Value>>>>;

pub struct Session {
    child: Mutex<Child>,
    input: Mutex<Option<ChildStdin>>,
    pending: Pending,
    legacy_replies: Mutex<Receiver<Value>>,
    legacy_gate: Mutex<()>,
    legacy: bool,
    next_id: AtomicU64,
    reader: Mutex<Option<JoinHandle<()>>>,
    closed: AtomicBool,
    healthy: Arc<AtomicBool>,
    restore_warning: Arc<AtomicBool>,
    cleanup_warning: Arc<AtomicBool>,
}
impl Session {
    pub fn start(root: &Path) -> Result<Arc<Self>> {
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
        let child = Command::new(runtime.join("node"))
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
        Self::from_child(child, false)
    }
    fn from_child(mut child: Child, legacy: bool) -> Result<Arc<Self>> {
        let input = child.stdin.take();
        let output = child.stdout.take().ok_or("Falta el canal del navegador")?;
        let (legacy_tx, legacy_replies) = mpsc::sync_channel(2);
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = pending.clone();
        let healthy = Arc::new(AtomicBool::new(true));
        let reader_healthy = healthy.clone();
        let restore_warning = Arc::new(AtomicBool::new(false));
        let reader_warning = restore_warning.clone();
        let cleanup_warning = Arc::new(AtomicBool::new(false));
        let reader_cleanup = cleanup_warning.clone();
        let reader = match std::thread::Builder::new()
            .name("gennie-browser-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(output);
                loop {
                    let mut line = Vec::new();
                    let value: Value = match reader
                        .by_ref()
                        .take((RESPONSE_LIMIT + 1) as u64)
                        .read_until(b'\n', &mut line)
                    {
                        Ok(n) if n > 0 && n <= RESPONSE_LIMIT => {
                            match serde_json::from_slice(&line) {
                                Ok(value) => value,
                                Err(_) => break,
                            }
                        }
                        _ => break,
                    };
                    if legacy {
                        if legacy_tx.send(value).is_err() {
                            break;
                        }
                    } else {
                        if value["event"] == "restore_failed" {
                            reader_warning.store(true, Ordering::Release);
                            continue;
                        }
                        if value["event"] == "cleanup_pending" {
                            reader_cleanup.store(true, Ordering::Release);
                            continue;
                        }
                        let Some(id) = value["request_id"].as_u64() else {
                            break;
                        };
                        let Some(result) = value.get("result") else {
                            break;
                        };
                        if let Ok(mut pending) = reader_pending.lock() {
                            if let Some(sender) = pending.remove(&id) {
                                let _ = sender.send(result.clone());
                            }
                        } else {
                            break;
                        }
                    }
                }
                reader_healthy.store(false, Ordering::Release);
                if let Ok(mut pending) = reader_pending.lock() {
                    pending.clear();
                }
            }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        Ok(Arc::new(Self {
            child: Mutex::new(child),
            input: Mutex::new(input),
            pending,
            legacy_replies: Mutex::new(legacy_replies),
            legacy_gate: Mutex::new(()),
            legacy,
            next_id: AtomicU64::new(1),
            reader: Mutex::new(Some(reader)),
            closed: AtomicBool::new(false),
            healthy,
            restore_warning,
            cleanup_warning,
        }))
    }
    #[cfg(test)]
    pub(super) fn node_fixture(script: &str, root: &Path) -> Arc<Self> {
        let child = Command::new("node")
            .args(["--input-type=module", "--eval", script])
            .env(
                "GENNIE_WEB_PLUGIN_MODULE",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("src/plugins.mjs"),
            )
            .env("GENNIE_WEB_FIXTURE_ROOT", root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        Self::from_child(child, true).unwrap()
    }
    #[cfg(test)]
    pub(super) fn scripted(script: &str, log: &Path) -> Arc<Self> {
        let child = Command::new("/bin/sh")
            .args(["-c", script])
            .env("GENNIE_WEB_FIXTURE_LOG", log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        Self::from_child(child, true).unwrap()
    }
    #[cfg(test)]
    pub(super) fn multiplex_fixture(script: &str) -> Arc<Self> {
        let child = Command::new("node")
            .args(["--input-type=module", "--eval", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        Self::from_child(child, false).unwrap()
    }
    pub fn exited(&self) -> bool {
        self.child
            .lock()
            .map_or(true, |mut child| !matches!(child.try_wait(), Ok(None)))
    }
    pub fn is_usable(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
            && self.input.lock().is_ok_and(|input| input.is_some())
            && !self.exited()
    }
    pub fn take_restore_warning(&self) -> bool {
        self.restore_warning.swap(false, Ordering::AcqRel)
    }
    pub fn take_cleanup_warning(&self) -> bool {
        self.cleanup_warning.swap(false, Ordering::AcqRel)
    }
    pub fn request_with_context(
        &self,
        value: &Value,
        context: &gennie_mcp::CallContext,
    ) -> Result<Value> {
        context.check()?;
        let _legacy_guard = if self.legacy {
            Some(
                self.legacy_gate
                    .lock()
                    .map_err(|_| "Canal de navegador inválido")?,
            )
        } else {
            None
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if id > 9_007_199_254_740_991 {
            return Err("Se agotaron los identificadores del navegador".into());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let request = if self.legacy {
            value.clone()
        } else {
            json!({"request_id":id,"request":value})
        };
        let mut line = serde_json::to_vec(&request)?;
        if line.len() > LIMIT - 1 {
            return Err("Petición de navegador demasiado grande".into());
        }
        line.push(b'\n');
        if !self.legacy {
            self.pending
                .lock()
                .map_err(|_| "Canal de navegador inválido")?
                .insert(id, sender);
        }
        struct Reservation<'a> {
            id: u64,
            pending: &'a Pending,
        }
        impl Drop for Reservation<'_> {
            fn drop(&mut self) {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&self.id);
                }
            }
        }
        let _reservation = Reservation {
            id,
            pending: &self.pending,
        };
        {
            let mut input = self
                .input
                .lock()
                .map_err(|_| "Canal de navegador inválido")?;
            let stream = input.as_ref().ok_or("El navegador está cerrado")?;
            if let Err(error) =
                gennie_mcp::write_cancellable(stream, &line, context, Duration::from_secs(12))
            {
                input.take();
                return Err(error);
            }
        }
        let deadline = Instant::now()
            + Duration::from_secs(
                if value["method"] == "plugin_run" || value["method"] == "candidate" {
                    PLUGIN_RESPONSE_TIMEOUT_SECS
                } else {
                    12
                },
            );
        let legacy = if self.legacy {
            Some(
                self.legacy_replies
                    .lock()
                    .map_err(|_| "Canal de navegador inválido")?,
            )
        } else {
            None
        };
        let response = loop {
            if context.is_cancelled() {
                if self.legacy {
                    self.input
                        .lock()
                        .map_err(|_| "Canal de navegador inválido")?
                        .take();
                }
                return Err(Box::new(gennie_mcp::Cancelled));
            }
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50));
            if wait.is_zero() {
                if self.legacy {
                    self.input
                        .lock()
                        .map_err(|_| "Canal de navegador inválido")?
                        .take();
                }
                return Err("El navegador dejó de responder; comprueba el efecto antes de repetir la operación".into());
            }
            let result = if let Some(legacy) = &legacy {
                legacy.recv_timeout(wait)
            } else {
                receiver.recv_timeout(wait)
            };
            match result {
                Ok(value) => {
                    context.check()?;
                    break value;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(
                        "El navegador dejó de responder; el resultado de la acción es incierto"
                            .into(),
                    );
                }
            }
        };
        if let Some(error) = response["error"].as_str() {
            return Ok(json!({"error":error,"code":response["code"],"ok":false}));
        }
        Ok(response)
    }
    pub fn close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.input
            .lock()
            .map_err(|_| "Canal de navegador inválido")?
            .take();
        // El worker concede hasta 30 s a Chromium para cerrar el perfil.
        let deadline = Instant::now() + Duration::from_secs(35);
        while !self.exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let forced = !self.exited();
        let mut child = self
            .child
            .lock()
            .map_err(|_| "Proceso de navegador inválido")?;
        if forced {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
        }
        let status = child.wait()?;
        drop(child);
        if let Some(reader) = self
            .reader
            .lock()
            .map_err(|_| "Lector de navegador inválido")?
            .take()
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiplex_matches_out_of_order_results_and_cancellation_keeps_session_usable() {
        let session = Session::multiplex_fixture(
            r#"
import {createInterface} from 'node:readline';
for await (const line of createInterface({input:process.stdin})) {
  const {request_id,request} = JSON.parse(line);
  if (request.value === 'notify') { console.log(JSON.stringify({event:'restore_failed'})); console.log(JSON.stringify({event:'cleanup_pending'})); }
  setTimeout(() => console.log(JSON.stringify({request_id,result:{value:request.value}})), request.delay);
}
"#,
        );
        let first = session.clone();
        let cancelled = gennie_mcp::CallContext::default();
        let cancel = cancelled.clone();
        let (sent, entered) = mpsc::channel();
        let slow = std::thread::spawn(move || {
            sent.send(()).unwrap();
            first.request_with_context(&json!({"value":"slow","delay":300}), &cancelled)
        });
        entered.recv().unwrap();
        std::thread::sleep(Duration::from_millis(80));
        let fast = session
            .request_with_context(
                &json!({"value":"fast","delay":0}),
                &gennie_mcp::CallContext::default(),
            )
            .unwrap();
        assert_eq!(fast["value"], "fast");
        cancel.cancel();
        assert!(
            slow.join()
                .unwrap()
                .unwrap_err()
                .is::<gennie_mcp::Cancelled>()
        );
        std::thread::sleep(Duration::from_millis(350));
        let next = session
            .request_with_context(
                &json!({"value":"next","delay":0}),
                &gennie_mcp::CallContext::default(),
            )
            .unwrap();
        assert_eq!(next["value"], "next");
        let notify = session
            .request_with_context(
                &json!({"value":"notify","delay":0}),
                &gennie_mcp::CallContext::default(),
            )
            .unwrap();
        assert_eq!(notify["value"], "notify");
        assert!(session.take_restore_warning());
        assert!(!session.take_restore_warning());
        assert!(session.take_cleanup_warning());
        assert!(!session.take_cleanup_warning());
        assert!(session.is_usable());
        session.close().unwrap();
    }
}
