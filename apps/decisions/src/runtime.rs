use gennie_core::Result;
use serde::Deserialize;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::JoinHandle,
    time::Duration,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub python: PathBuf,
    pub model: PathBuf,
    #[serde(default)]
    pub adapter: Option<PathBuf>,
}
impl RuntimeConfig {
    pub fn load() -> Result<Self> {
        let home = std::env::var_os("HOME").ok_or("Falta directorio personal")?;
        let directory = PathBuf::from(home).join("Library/Application Support/soc/decisions");
        let next = directory.join("runtime-v2.json");
        let path = if next.is_file() {
            next
        } else {
            directory.join("runtime.json")
        };
        let bytes = std::fs::read(path)
            .map_err(|_| "Laya no configurado; consulta docs/decisions/control.md")?;
        let config: Self =
            serde_json::from_slice(&bytes).map_err(|_| "Configuración de Laya inválida")?;
        if !config.python.is_absolute()
            || !config.python.is_file()
            || !config.model.is_absolute()
            || !config.model.is_dir()
        {
            return Err("Intérprete y checkpoint local deben existir con rutas absolutas".into());
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config.model.join("rl_agent_config.json"))?)
                .map_err(|_| "Metadatos del checkpoint inválidos")?;
        if metadata.get("encoder").and_then(serde_json::Value::as_str)
            != Some("jhu-clsp/mmBERT-base")
        {
            return Err("El checkpoint debe ser Laya multilingüe (mmBERT-base)".into());
        }
        if config
            .adapter
            .as_ref()
            .is_some_and(|path| !path.is_absolute() || !path.join("manifest.json").is_file())
        {
            return Err("Adaptación local de Laya inválida".into());
        }
        Ok(config)
    }
}

pub struct Process {
    child: Child,
    observed_pid: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    input: Option<ChildStdin>,
    replies: Receiver<String>,
    reader: Option<JoinHandle<()>>,
}
impl Process {
    pub fn start(
        config: &RuntimeConfig,
        pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
    ) -> Result<Self> {
        let mut process = Self::start_script(
            config,
            concat!(
                include_str!("../python/applications.py"),
                "\n",
                include_str!("../python/domain.py"),
                "\n",
                include_str!("../python/worker.py")
            ),
        )?;
        process.observe_pid(pid);
        Ok(process)
    }
    fn observe_pid(&mut self, pid: std::sync::Arc<std::sync::atomic::AtomicU32>) {
        pid.store(self.child.id(), std::sync::atomic::Ordering::Release);
        self.observed_pid = Some(pid);
    }
    #[cfg(test)]
    pub fn fake(script: &str) -> Result<Self> {
        Self::start_script(
            &RuntimeConfig {
                python: "/usr/bin/python3".into(),
                model: "/tmp".into(),
                adapter: None,
            },
            script,
        )
    }
    fn start_script(config: &RuntimeConfig, source: &str) -> Result<Self> {
        let mut command = Command::new(&config.python);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .args(["-u", "-c", source])
            .arg(&config.model)
            .arg(
                config
                    .adapter
                    .as_ref()
                    .map(|p| p.as_os_str())
                    .unwrap_or_default(),
            )
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("USE_TF", "0")
            .env("TOKENIZERS_PARALLELISM", "false")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "No se pudo iniciar el proceso local de Laya")?;
        let input = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or("Falta salida del proceso de Laya")?;
        let (tx, replies) = mpsc::sync_channel(4);
        let reader = std::thread::spawn(move || {
            let mut output = BufReader::new(stdout);
            loop {
                // IPC acotado por línea; un helper defectuoso no crece sin límite.
                let mut line = Vec::new();
                match (&mut output).take(16385).read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    _ if line.len() > 16384 => break,
                    _ => {
                        if let Ok(line) = String::from_utf8(line) {
                            if tx.try_send(line).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            observed_pid: None,
            input,
            replies,
            reader: Some(reader),
        })
    }
    pub fn request(
        &mut self,
        request: &serde_json::Value,
        cancel: &impl Fn() -> bool,
    ) -> Result<serde_json::Value> {
        if cancel() {
            return Err("Orden cancelada".into());
        }
        let input = self.input.as_mut().ok_or("Proceso de Laya cerrado")?;
        writeln!(input, "{request}")?;
        input.flush()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            if cancel() {
                return Err("Orden cancelada".into());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Laya excedió el tiempo de espera; operación sin reintento".into());
            }
            match self.replies.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => {
                    let value: serde_json::Value =
                        serde_json::from_str(&line).map_err(|_| "Respuesta de Laya inválida")?;
                    if value.get("error").is_some() {
                        let category =
                            match value.get("category").and_then(serde_json::Value::as_str) {
                                Some("os_timeout") => "os_timeout",
                                Some("os_command_failed") => "os_command_failed",
                                Some("os_unavailable") => "os_unavailable",
                                Some("invalid_response") => "invalid_response",
                                _ => "internal_error",
                            };
                        let (operation, message) = match request
                            .get("op")
                            .and_then(serde_json::Value::as_str)
                        {
                            Some("execute") => (
                                "execute",
                                "No pude confirmar el resultado de la orden; puede haberse realizado",
                            ),
                            Some("load") => ("load", "No pude preparar el modelo de órdenes"),
                            _ => ("decide", "No pude analizar la orden"),
                        };
                        eprintln!(
                            "event=local_worker_failed operation={operation} category={category}"
                        );
                        return Err(message.into());
                    }
                    return Ok(value);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("El proceso de Laya terminó; operación sin reintento".into()),
            }
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.input.take();
        #[cfg(unix)]
        {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if let Ok(pid) = i32::try_from(self.child.id()) {
                // Grupo propio creado por process_group: incluye osascript/open, nunca apps ajenas.
                unsafe {
                    kill(-pid, 9);
                }
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(pid) = &self.observed_pid {
            pid.store(0, std::sync::atomic::Ordering::Release);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn observed_pid_clears_when_the_owned_process_is_reaped() {
        let pid = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut process = Process::fake("import sys\nsys.stdin.readline()").unwrap();
        process.observe_pid(pid.clone());
        assert_eq!(
            pid.load(std::sync::atomic::Ordering::Acquire),
            process.child.id()
        );
        drop(process);
        assert_eq!(pid.load(std::sync::atomic::Ordering::Acquire), 0);
    }

    #[test]
    fn worker_failure_message_distinguishes_execution_from_inference() {
        let source = "import sys,json\nfor line in sys.stdin:\n print(json.dumps({'error':'local_worker_failed','category':'os_timeout'}),flush=True)";
        let mut process = Process::fake(source).unwrap();
        for (operation, expected) in [
            ("load", "No pude preparar el modelo de órdenes"),
            ("decide", "No pude analizar la orden"),
            (
                "execute",
                "No pude confirmar el resultado de la orden; puede haberse realizado",
            ),
        ] {
            let error = process
                .request(&serde_json::json!({"op":operation}), &|| false)
                .unwrap_err();
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn invalid_and_oversized_replies_and_eof_fail_without_a_stalled_reader() {
        for source in [
            "import sys\nsys.stdin.readline()\nprint('invalid json',flush=True)",
            "import sys\nsys.stdin.readline()\nprint('x'*16385,flush=True)",
            "import sys\nsys.stdin.readline()\nsys.stdout.buffer.write(b'\\xff\\n')\nsys.stdout.buffer.flush()",
            "import sys\nsys.stdin.readline()",
        ] {
            let start = std::time::Instant::now();
            let mut process = Process::fake(source).unwrap();
            assert!(
                process
                    .request(&serde_json::json!({"op":"load"}), &|| false)
                    .is_err()
            );
            drop(process);
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
}
