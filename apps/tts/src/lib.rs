mod settings;

use serde::{Deserialize, Serialize};
pub use settings::Settings;
use soc_core::{AppModule, AppState, Migration, Result};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub struct TtsModule {
    worker: Mutex<Option<Worker>>,
}
struct Worker {
    sender: SyncSender<CommandMessage>,
    events: Arc<Mutex<Receiver<Event>>>,
    settings: Arc<Mutex<Settings>>,
    next_utterance: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    child: Arc<Mutex<Option<Child>>>,
    model_status: Arc<AtomicU8>,
    stopping: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}
#[derive(Clone)]
pub struct Controller {
    sender: SyncSender<CommandMessage>,
    events: Arc<Mutex<Receiver<Event>>>,
    settings: Arc<Mutex<Settings>>,
    next_utterance: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    child: Arc<Mutex<Option<Child>>>,
    model_status: Arc<AtomicU8>,
}
enum CommandMessage {
    Speak(u64, u64, String),
    Preview(u64, u64, Settings),
    Save(Settings),
    Shutdown,
}
#[derive(Debug)]
pub enum Event {
    Started(u64),
    Finished(u64),
    Failed(u64, String),
    Interrupted(u64),
    Saved(Settings),
    SaveFailed(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Disabled,
    Unloaded,
    Loading,
    Ready,
    Failed,
}
impl ModelStatus {
    fn from_atomic(value: u8) -> Self {
        match value {
            1 => Self::Loading,
            2 => Self::Ready,
            3 => Self::Failed,
            _ => Self::Unloaded,
        }
    }
    fn atomic(self) -> u8 {
        match self {
            Self::Loading => 1,
            Self::Ready => 2,
            Self::Failed => 3,
            Self::Disabled | Self::Unloaded => 0,
        }
    }
}
#[derive(Serialize)]
struct SpeakRequest<'a> {
    text: &'a str,
    voice: &'a str,
    speed: f32,
}
#[derive(Deserialize)]
struct SpeakResponse {
    event: Option<String>,
    ok: Option<bool>,
    error: Option<String>,
}
struct Process {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Default for TtsModule {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}
impl AppModule for TtsModule {
    fn name(&self) -> &'static str {
        "tts"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_preferences.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_unload_timeout.sql"),
            },
        ]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut guard = self.worker.lock().map_err(|_| "Worker TTS inválido")?;
        if guard.is_some() {
            return Ok(());
        }
        let initial = settings::load(&state.db)?;
        let db = state.db.clone();
        let (sender, receiver) = mpsc::sync_channel(8);
        let (event_sender, events) = mpsc::channel();
        let events = Arc::new(Mutex::new(events));
        let current = Arc::new(Mutex::new(initial));
        let next_utterance = Arc::new(AtomicU64::new(1));
        let generation = Arc::new(AtomicU64::new(0));
        let child = Arc::new(Mutex::new(None::<Child>));
        let model_status = Arc::new(AtomicU8::new(ModelStatus::Unloaded.atomic()));
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_settings = current.clone();
        let worker_child = child.clone();
        let worker_model_status = model_status.clone();
        let worker_stopping = stopping.clone();
        let worker_generation = generation.clone();
        let handle = std::thread::Builder::new()
            .name("tts-playback".into())
            .spawn(move || {
                let mut process = None;
                let mut idle_since: Option<Instant> = None;
                while let Some(command) = receive_command(
                    &receiver,
                    &mut process,
                    &worker_child,
                    &worker_model_status,
                    &worker_settings,
                    &mut idle_since,
                ) {
                    if worker_stopping.load(Ordering::SeqCst)
                        || matches!(command, CommandMessage::Shutdown)
                    {
                        break;
                    }
                    match command {
                        CommandMessage::Speak(id, request_generation, text) => {
                            if worker_generation.load(Ordering::SeqCst) != request_generation {
                                let _ = event_sender.send(Event::Interrupted(id));
                                continue;
                            }
                            let current = match worker_settings.lock() {
                                Ok(settings) => settings.clone(),
                                Err(_) => {
                                    let _ = event_sender.send(Event::Failed(
                                        id,
                                        "Ajustes TTS no disponibles".into(),
                                    ));
                                    continue;
                                }
                            };
                            if !current.enabled {
                                let _ = event_sender.send(Event::Finished(id));
                                continue;
                            }
                            let _ = event_sender.send(Event::Started(id));
                            let result = speak(
                                &mut process,
                                &worker_child,
                                &worker_model_status,
                                &worker_generation,
                                request_generation,
                                &text,
                                &current,
                            );
                            if worker_generation.load(Ordering::SeqCst) != request_generation {
                                close_process(&mut process, &worker_child, &worker_model_status);
                                let _ = event_sender.send(Event::Interrupted(id));
                                continue;
                            }
                            match result {
                                Ok(()) => {
                                    idle_since = Some(Instant::now());
                                    let _ = event_sender.send(Event::Finished(id));
                                }
                                Err(error) => {
                                    close_process(
                                        &mut process,
                                        &worker_child,
                                        &worker_model_status,
                                    );
                                    worker_model_status
                                        .store(ModelStatus::Failed.atomic(), Ordering::SeqCst);
                                    idle_since = None;
                                    let _ = event_sender.send(Event::Failed(id, error.to_string()));
                                }
                            }
                        }
                        CommandMessage::Preview(id, request_generation, settings) => {
                            if worker_generation.load(Ordering::SeqCst) != request_generation {
                                let _ = event_sender.send(Event::Interrupted(id));
                                continue;
                            }
                            let _ = event_sender.send(Event::Started(id));
                            let result = speak(
                                &mut process,
                                &worker_child,
                                &worker_model_status,
                                &worker_generation,
                                request_generation,
                                "Hola, soy SOC. Así suena mi voz.",
                                &settings,
                            );
                            if worker_generation.load(Ordering::SeqCst) != request_generation {
                                close_process(&mut process, &worker_child, &worker_model_status);
                                let _ = event_sender.send(Event::Interrupted(id));
                                continue;
                            }
                            match result {
                                Ok(()) => {
                                    idle_since = Some(Instant::now());
                                    let _ = event_sender.send(Event::Finished(id));
                                }
                                Err(error) => {
                                    close_process(
                                        &mut process,
                                        &worker_child,
                                        &worker_model_status,
                                    );
                                    worker_model_status
                                        .store(ModelStatus::Failed.atomic(), Ordering::SeqCst);
                                    idle_since = None;
                                    let _ = event_sender.send(Event::Failed(id, error.to_string()));
                                }
                            }
                        }
                        CommandMessage::Save(next) => match settings::save(&db, &next) {
                            Ok(()) => {
                                if let Ok(mut saved) = worker_settings.lock() {
                                    *saved = next.clone();
                                }
                                if !next.enabled {
                                    close_process(
                                        &mut process,
                                        &worker_child,
                                        &worker_model_status,
                                    );
                                    idle_since = None;
                                }
                                let _ = event_sender.send(Event::Saved(next));
                            }
                            Err(error) => {
                                let _ = event_sender.send(Event::SaveFailed(error.to_string()));
                            }
                        },
                        CommandMessage::Shutdown => break,
                    }
                }
                close_process(&mut process, &worker_child, &worker_model_status);
            })?;
        *guard = Some(Worker {
            sender,
            events,
            settings: current,
            next_utterance,
            generation,
            child,
            model_status,
            stopping,
            handle,
        });
        Ok(())
    }
    fn stop(&self, _state: &AppState) -> Result<()> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| "Worker TTS inválido")?
            .take();
        if let Some(worker) = worker {
            worker.stopping.store(true, Ordering::SeqCst);
            let _ = worker.sender.try_send(CommandMessage::Shutdown);
            if let Ok(mut child) = worker.child.lock()
                && let Some(child) = child.as_mut()
            {
                let _ = child.kill();
            }
            drop(worker.sender);
            worker
                .handle
                .join()
                .map_err(|_| "Worker TTS terminó inesperadamente")?;
        }
        Ok(())
    }
}

fn receive_command(
    receiver: &Receiver<CommandMessage>,
    process: &mut Option<Process>,
    child: &Arc<Mutex<Option<Child>>>,
    model_status: &AtomicU8,
    settings: &Arc<Mutex<Settings>>,
    idle_since: &mut Option<Instant>,
) -> Option<CommandMessage> {
    loop {
        if process.is_some() {
            let timeout = match settings.lock() {
                Ok(settings) => settings.unload_seconds.map(Duration::from_secs),
                Err(_) => return None,
            };
            if let Some(timeout) = timeout {
                let remaining =
                    timeout.saturating_sub(idle_since.unwrap_or_else(Instant::now).elapsed());
                if remaining.is_zero() {
                    close_process(process, child, model_status);
                    *idle_since = None;
                    continue;
                }
                match receiver.recv_timeout(remaining) {
                    Ok(command) => return Some(command),
                    Err(RecvTimeoutError::Timeout) => {
                        close_process(process, child, model_status);
                        *idle_since = None;
                    }
                    Err(RecvTimeoutError::Disconnected) => return None,
                }
            } else {
                return receiver.recv().ok();
            }
        } else {
            return receiver.recv().ok();
        }
    }
}
impl TtsModule {
    pub fn controller(&self) -> Result<Controller> {
        let guard = self.worker.lock().map_err(|_| "Worker TTS inválido")?;
        let worker = guard.as_ref().ok_or("TTS no iniciado")?;
        Ok(Controller {
            sender: worker.sender.clone(),
            events: worker.events.clone(),
            settings: worker.settings.clone(),
            next_utterance: worker.next_utterance.clone(),
            generation: worker.generation.clone(),
            child: worker.child.clone(),
            model_status: worker.model_status.clone(),
        })
    }
}
impl Controller {
    pub fn model_status(&self) -> ModelStatus {
        let mut status = ModelStatus::from_atomic(self.model_status.load(Ordering::SeqCst));
        if matches!(status, ModelStatus::Loading | ModelStatus::Ready)
            && self
                .child
                .lock()
                .ok()
                .and_then(|mut child| {
                    child
                        .as_mut()
                        .and_then(|child| child.try_wait().ok().flatten())
                })
                .is_some()
        {
            status = ModelStatus::Failed;
            self.model_status.store(status.atomic(), Ordering::SeqCst);
        }
        if status == ModelStatus::Unloaded && !self.settings().enabled {
            ModelStatus::Disabled
        } else {
            status
        }
    }
    pub fn settings(&self) -> Settings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn speak(&self, text: &str) -> Result<Option<u64>> {
        if text.trim().is_empty() || !self.settings().enabled {
            return Ok(None);
        }
        let id = self.next_utterance.fetch_add(1, Ordering::Relaxed);
        let generation = self.generation.load(Ordering::SeqCst);
        self.sender
            .try_send(CommandMessage::Speak(id, generation, text.to_owned()))
            .map_err(|_| "Cola de voz ocupada")?;
        Ok(Some(id))
    }
    pub fn preview(&self, settings: Settings) -> Result<u64> {
        settings.validate()?;
        let id = self.next_utterance.fetch_add(1, Ordering::Relaxed);
        let generation = self.generation.load(Ordering::SeqCst);
        self.sender
            .try_send(CommandMessage::Preview(id, generation, settings))
            .map_err(|_| "Cola de voz ocupada")?;
        Ok(id)
    }
    pub fn save(&self, next: Settings) -> Result<()> {
        next.validate()?;
        self.sender
            .try_send(CommandMessage::Save(next))
            .map_err(|_| "No se pudieron enviar los ajustes TTS".into())
    }
    pub fn interrupt(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(child) = self
            .child
            .lock()
            .map_err(|_| "Proceso TTS inválido")?
            .as_mut()
        {
            let _ = child.kill();
        }
        self.model_status
            .store(ModelStatus::Unloaded.atomic(), Ordering::SeqCst);
        Ok(())
    }
    pub fn poll(&self) -> Vec<Event> {
        self.events
            .lock()
            .map(|events| events.try_iter().collect())
            .unwrap_or_default()
    }
}

fn python_path() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME").ok_or("HOME no disponible para Kokoro")?;
    let path =
        std::path::PathBuf::from(home).join("Library/Application Support/soc/tts/.venv/bin/python");
    if !path.is_file() {
        return Err("Kokoro no está instalado; consulta la configuración TTS".into());
    }
    Ok(path)
}
fn model_cache() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME").ok_or("HOME no disponible para Kokoro")?;
    Ok(std::path::PathBuf::from(home).join("Library/Application Support/AIModels/huggingface"))
}
fn speak(
    process: &mut Option<Process>,
    child: &Arc<Mutex<Option<Child>>>,
    model_status: &AtomicU8,
    generation: &AtomicU64,
    request_generation: u64,
    text: &str,
    settings: &Settings,
) -> Result<()> {
    let mut child_guard = child.lock().map_err(|_| "Proceso TTS inválido")?;
    if generation.load(Ordering::SeqCst) != request_generation {
        return Err("Reproducción interrumpida".into());
    }
    if process.is_none() {
        model_status.store(ModelStatus::Loading.atomic(), Ordering::SeqCst);
        let mut spawned = Command::new(python_path()?)
            .arg("-u")
            .arg("-c")
            .arg(include_str!("../python/worker.py"))
            .env("HF_HOME", model_cache()?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = spawned.stdin.take().ok_or("TTS sin entrada")?;
        let stdout = spawned.stdout.take().ok_or("TTS sin salida")?;
        *child_guard = Some(spawned);
        *process = Some(Process {
            stdin,
            stdout: BufReader::new(stdout),
        });
    }
    drop(child_guard);
    let process = process.as_mut().ok_or("Proceso TTS no disponible")?;
    serde_json::to_writer(
        &mut process.stdin,
        &SpeakRequest {
            text,
            voice: &settings.voice,
            speed: settings.speed_percent as f32 / 100.0,
        },
    )?;
    process.stdin.write_all(b"\n")?;
    process.stdin.flush()?;
    read_speak_response(
        &mut process.stdout,
        model_status,
        generation,
        request_generation,
    )
}

fn read_speak_response(
    reader: &mut impl BufRead,
    model_status: &AtomicU8,
    generation: &AtomicU64,
    request_generation: u64,
) -> Result<()> {
    loop {
        let mut response = String::new();
        if reader.read_line(&mut response)? == 0 {
            return Err("El proceso Kokoro terminó sin respuesta".into());
        }
        let response: SpeakResponse =
            serde_json::from_str(&response).map_err(|_| "Respuesta inválida de Kokoro")?;
        if generation.load(Ordering::SeqCst) != request_generation {
            return Err("Reproducción interrumpida".into());
        }
        if response.event.as_deref() == Some("loaded") {
            model_status.store(ModelStatus::Ready.atomic(), Ordering::SeqCst);
            continue;
        }
        return match response.ok {
            Some(true) => Ok(()),
            Some(false) => Err(format!(
                "Kokoro: {}",
                response
                    .error
                    .unwrap_or_else(|| "error de reproducción".into())
            )
            .into()),
            None => Err("Respuesta inválida de Kokoro".into()),
        };
    }
}
fn close_process(
    process: &mut Option<Process>,
    child: &Arc<Mutex<Option<Child>>>,
    model_status: &AtomicU8,
) {
    process.take();
    if let Ok(mut child) = child.lock()
        && let Some(mut child) = child.take()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    model_status.store(ModelStatus::Unloaded.atomic(), Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soc_config::Config;
    use soc_core::Shutdown;
    use soc_db::Database;
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    fn state() -> AppState {
        AppState {
            config: Arc::new(Config {
                database_path: PathBuf::from("unused"),
                db_busy_timeout_ms: 500,
            }),
            db: Database::memory().unwrap(),
            shutdown: Shutdown::default(),
        }
    }
    #[test]
    fn worker_reports_loaded_before_playback_finishes() {
        let status = AtomicU8::new(ModelStatus::Loading.atomic());
        let generation = AtomicU64::new(0);
        let mut output = std::io::Cursor::new(b"{\"event\":\"loaded\"}\n{\"ok\":true}\n");
        read_speak_response(&mut output, &status, &generation, 0).unwrap();
        assert_eq!(
            ModelStatus::from_atomic(status.load(Ordering::SeqCst)),
            ModelStatus::Ready
        );
    }
    #[test]
    fn settings_survive_module_restart_and_stop_with_controller_held() {
        let state = state();
        let module = TtsModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        assert_eq!(controller.settings(), Settings::default());
        let next = Settings {
            enabled: false,
            voice: "em_alex".into(),
            speed_percent: 115,
            unload_seconds: None,
        };
        controller.save(next.clone()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = controller.poll().into_iter().next() {
                assert!(matches!(event, Event::Saved(saved) if saved == next));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        module.stop(&state).unwrap();
        module.start(&state).unwrap();
        assert_eq!(module.controller().unwrap().settings(), next);
        assert!(
            module
                .controller()
                .unwrap()
                .save(Settings {
                    speed_percent: 200,
                    ..next
                })
                .is_err()
        );
        module.stop(&state).unwrap();
    }
    #[test]
    fn migration_keeps_voice_preferences_and_adds_default_timeout() {
        let state = state();
        let module = TtsModule::default();
        state
            .db
            .migrate(module.name(), &module.migrations()[..1])
            .unwrap();
        state.db.with_connection(|db| {
            db.execute(
                "UPDATE tts_preferences SET enabled=0, voice='em_santa', speed_percent=120 WHERE id=1",
                [],
            )?;
            Ok(())
        }).unwrap();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        assert_eq!(
            settings::load(&state.db).unwrap(),
            Settings {
                enabled: false,
                voice: "em_santa".into(),
                speed_percent: 120,
                unload_seconds: Some(600),
            }
        );
        for valid in [Some(60), Some(7200), None] {
            let next = Settings {
                unload_seconds: valid,
                ..Settings::default()
            };
            settings::save(&state.db, &next).unwrap();
            assert_eq!(settings::load(&state.db).unwrap(), next);
        }
        for invalid in [Some(0), Some(61), Some(7201)] {
            assert!(
                Settings {
                    unload_seconds: invalid,
                    ..Settings::default()
                }
                .validate()
                .is_err()
            );
        }
    }
    #[test]
    fn interrupt_discards_queued_audio_without_loading_kokoro() {
        let state = state();
        let module = TtsModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        let old_generation = controller.generation.load(Ordering::SeqCst);
        controller.interrupt().unwrap();
        let id = controller.next_utterance.fetch_add(1, Ordering::Relaxed);
        controller
            .sender
            .try_send(CommandMessage::Speak(
                id,
                old_generation,
                "Texto viejo".into(),
            ))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = controller.poll().into_iter().next() {
                assert!(matches!(event, Event::Interrupted(interrupted) if interrupted == id));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        module.stop(&state).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn idle_expiration_closes_owned_child_before_next_command() {
        let mut spawned = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut process = Some(Process {
            stdin: spawned.stdin.take().unwrap(),
            stdout: BufReader::new(spawned.stdout.take().unwrap()),
        });
        let child = Arc::new(Mutex::new(Some(spawned)));
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(CommandMessage::Shutdown).unwrap();
        let settings = Arc::new(Mutex::new(Settings {
            unload_seconds: Some(60),
            ..Settings::default()
        }));
        let model_status = AtomicU8::new(ModelStatus::Ready.atomic());
        let mut idle_since = Some(Instant::now() - Duration::from_secs(60));

        assert!(matches!(
            receive_command(
                &receiver,
                &mut process,
                &child,
                &model_status,
                &settings,
                &mut idle_since
            ),
            Some(CommandMessage::Shutdown)
        ));
        assert!(process.is_none());
        assert!(child.lock().unwrap().is_none());
        assert!(idle_since.is_none());
        assert_eq!(
            ModelStatus::from_atomic(model_status.load(Ordering::SeqCst)),
            ModelStatus::Unloaded
        );
    }
    #[cfg(unix)]
    #[test]
    fn keep_loaded_preserves_owned_child() {
        let mut spawned = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut process = Some(Process {
            stdin: spawned.stdin.take().unwrap(),
            stdout: BufReader::new(spawned.stdout.take().unwrap()),
        });
        let child = Arc::new(Mutex::new(Some(spawned)));
        let settings = Arc::new(Mutex::new(Settings {
            unload_seconds: None,
            ..Settings::default()
        }));
        let model_status = AtomicU8::new(ModelStatus::Ready.atomic());
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(CommandMessage::Shutdown).unwrap();
        let mut idle_since = Some(Instant::now() - Duration::from_secs(7200));
        assert!(matches!(
            receive_command(
                &receiver,
                &mut process,
                &child,
                &model_status,
                &settings,
                &mut idle_since
            ),
            Some(CommandMessage::Shutdown)
        ));
        assert!(process.is_some());
        close_process(&mut process, &child, &model_status);
    }
    #[cfg(unix)]
    #[test]
    fn interrupt_stops_only_its_owned_child() {
        let state = state();
        let module = TtsModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        *controller.child.lock().unwrap() =
            Some(Command::new("/bin/sleep").arg("30").spawn().unwrap());
        controller.interrupt().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if controller
                .child
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .try_wait()
                .unwrap()
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        module.stop(&state).unwrap();
    }
    #[test]
    #[ignore = "reproduce audio por el dispositivo de salida y necesita el modelo Kokoro local"]
    fn real_kokoro_worker_plays_preview() {
        let state = state();
        let module = TtsModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let controller = module.controller().unwrap();
        let id = controller.preview(Settings::default()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(event) = controller.poll().into_iter().next() {
                if matches!(event, Event::Started(_)) {
                    continue;
                }
                assert!(
                    matches!(event, Event::Finished(finished) if finished == id),
                    "{event:?}"
                );
                break;
            }
            assert!(Instant::now() < deadline, "Kokoro no respondió");
            std::thread::sleep(Duration::from_millis(25));
        }
        module.stop(&state).unwrap();
    }
}
