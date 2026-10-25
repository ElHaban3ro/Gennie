use crate::{Event, Mode, Settings, model, process::Process};
use gennie_core::Result;
use handy_keys::{HotkeyManager, HotkeyState};
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, SyncSender},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Downloading,
    Warming,
    Loading,
    Recording,
    Finishing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Disabled,
    Unloaded,
    Downloading,
    Loading,
    Ready,
    Failed,
}
pub struct Controller {
    pub settings: Settings,
    pub settings_status: String,
    process: Option<Process>,
    keys: Option<HotkeyManager>,
    awaiting_permission: Option<Instant>,
    capturing_shortcut: bool,
    phase: Phase,
    model_status: ModelStatus,
    last_progress: Instant,
    idle_since: Instant,
    model_path: PathBuf,
    sender: SyncSender<Settings>,
    replies: Receiver<std::result::Result<Settings, String>>,
    saving: bool,
    pending: Vec<Event>,
}
impl Controller {
    pub fn resident_bytes(&mut self) -> Option<u64> {
        self.process
            .as_mut()
            .map_or(Some(0), Process::resident_bytes)
    }

    pub(crate) fn new(
        settings: Settings,
        sender: SyncSender<Settings>,
        replies: Receiver<std::result::Result<Settings, String>>,
    ) -> Result<Self> {
        let mut c = Self {
            settings,
            settings_status: String::new(),
            process: None,
            keys: None,
            awaiting_permission: None,
            capturing_shortcut: false,
            phase: Phase::Idle,
            model_status: ModelStatus::Unloaded,
            last_progress: Instant::now(),
            idle_since: Instant::now(),
            model_path: model::path()?,
            sender,
            replies,
            saving: false,
            pending: Vec::new(),
        };
        c.configure_keys();
        if c.settings.enabled && !model::ready(&c.model_path) {
            c.prepare();
        } else if c.settings.enabled {
            c.warm()?;
        }
        Ok(c)
    }
    pub fn model_status(&self) -> ModelStatus {
        if self.settings.enabled {
            self.model_status
        } else {
            ModelStatus::Disabled
        }
    }
    fn configure_keys(&mut self) {
        self.keys.take();
        self.awaiting_permission = None;
        if self.capturing_shortcut {
            return;
        }
        if !self.settings.enabled {
            self.settings_status = "Transcripción desactivada".into();
            return;
        }
        #[cfg(target_os = "macos")]
        if !handy_keys::check_accessibility() {
            self.awaiting_permission = Some(Instant::now());
            self.settings_status =
                "El atajo necesita Accesibilidad. Pulsa Activar atajo para autorizar Gennie".into();
            return;
        }
        let result = (|| -> Result<HotkeyManager> {
            let manager = HotkeyManager::new_with_blocking()?;
            manager.register(self.settings.shortcut.parse()?)?;
            Ok(manager)
        })();
        match result {
            Ok(keys) => {
                self.keys = Some(keys);
                self.settings_status = "Atajo listo · español · audio local".into();
            }
            Err(_) => {
                self.settings_status =
                    "No se pudo activar el atajo. Revisa Accesibilidad y reintenta".into()
            }
        }
    }
    pub fn retry_keys(&mut self) {
        self.configure_keys();
        self.request_keys_permission();
    }
    pub fn capture_shortcut(&mut self, capturing: bool) {
        self.capturing_shortcut = capturing;
        if capturing
            && matches!(
                self.phase,
                Phase::Loading | Phase::Recording | Phase::Finishing
            )
        {
            self.cancel();
        }
        self.configure_keys();
        if !capturing
            && self.phase == Phase::Idle
            && self.process.is_none()
            && model::ready(&self.model_path)
            && let Err(error) = self.warm()
        {
            self.pending.push(Event::Error(error.to_string()));
        }
    }
    pub fn request_keys_permission(&mut self) {
        #[cfg(target_os = "macos")]
        if self.awaiting_permission.is_some() {
            self.settings_status = match crate::permissions::request_accessibility() {
                Ok(()) => {
                    "Autoriza Gennie en el panel abierto. El atajo se activará al conceder el permiso"
                        .into()
                }
                Err(error) => format!("No se pudo solicitar Accesibilidad: {error}"),
            };
        }
    }
    pub fn save(&mut self, settings: Settings) -> Result<()> {
        if self.saving {
            return Err("Ya se están guardando los ajustes".into());
        }
        settings.validate()?;
        self.sender
            .try_send(settings)
            .map_err(|_| "No se pudo enviar los ajustes")?;
        self.saving = true;
        self.settings_status = "Guardando…".into();
        Ok(())
    }
    pub fn prepare(&mut self) {
        if self.phase != Phase::Idle {
            return;
        }
        self.process.take();
        self.model_status = ModelStatus::Unloaded;
        match Process::spawn("--prepare-model", &self.model_path, None) {
            Ok(p) => {
                self.process = Some(p);
                self.phase = Phase::Downloading;
                self.model_status = ModelStatus::Downloading;
                self.last_progress = Instant::now();
                self.pending
                    .push(Event::Status("Verificando el archivo del modelo…".into()));
            }
            Err(_) => {
                self.model_status = ModelStatus::Failed;
                self.pending.push(Event::Error(
                    "No se pudo iniciar la descarga. Reintenta desde Transcripción".into(),
                ));
            }
        }
    }
    pub fn cancel(&mut self) {
        self.process.take();
        self.phase = Phase::Idle;
        self.model_status = ModelStatus::Unloaded;
        self.pending.push(Event::Cancelled);
    }
    fn start_worker(&mut self, command: &str) -> Result<()> {
        let result = (|| -> Result<()> {
            if self.process.is_none() {
                self.process = Some(Process::spawn(
                    "--transcription-worker",
                    &self.model_path,
                    self.settings.microphone_name.as_deref(),
                )?);
            }
            self.process
                .as_mut()
                .ok_or("Worker ausente")?
                .command(command)
        })();
        if result.is_err() {
            self.process.take();
            self.model_status = ModelStatus::Failed;
        } else if self.model_status != ModelStatus::Ready {
            self.model_status = ModelStatus::Loading;
        }
        result
    }
    fn warm(&mut self) -> Result<()> {
        if !self.settings.enabled || self.capturing_shortcut || self.phase != Phase::Idle {
            return Ok(());
        }
        self.start_worker("load")?;
        self.phase = Phase::Warming;
        self.last_progress = Instant::now();
        self.pending.push(Event::Status("Cargando modelo…".into()));
        Ok(())
    }
    fn begin(&mut self) -> Result<()> {
        if !self.settings.enabled || self.saving || self.capturing_shortcut {
            return Ok(());
        }
        self.pending.push(Event::DictationRequested);
        if !matches!(self.phase, Phase::Idle | Phase::Warming) {
            return Ok(());
        }
        if !model::ready(&self.model_path) {
            self.prepare();
            return Ok(());
        }
        self.start_worker("start")?;
        self.phase = Phase::Loading;
        self.last_progress = Instant::now();
        self.pending.push(Event::DictationStarted);
        Ok(())
    }
    fn end(&mut self) -> Result<()> {
        if matches!(self.phase, Phase::Loading | Phase::Recording) {
            if let Some(p) = self.process.as_mut() {
                p.command("stop")?;
            }
            self.phase = Phase::Finishing;
            self.last_progress = Instant::now();
        }
        Ok(())
    }
    fn key(&mut self, state: HotkeyState) -> Result<()> {
        match (self.settings.mode, state) {
            (Mode::Hold, HotkeyState::Pressed) => self.begin(),
            (Mode::Hold, HotkeyState::Released) => self.end(),
            (Mode::Toggle, HotkeyState::Pressed)
                if matches!(self.phase, Phase::Loading | Phase::Recording) =>
            {
                self.end()
            }
            (Mode::Toggle, HotkeyState::Pressed) => self.begin(),
            _ => Ok(()),
        }
    }
    fn observe(&mut self, event: &Event) -> (bool, bool) {
        // Cancelled del hijo solo termina la sesión: conserva el modelo cargado.
        match event {
            Event::ModelLoaded
            | Event::Listening
            | Event::Text { .. }
            | Event::NoSpeech(_)
            | Event::Cancelled => self.model_status = ModelStatus::Ready,
            Event::Ready if self.phase == Phase::Downloading => {
                self.model_status = ModelStatus::Unloaded;
            }
            Event::Error(_) => self.model_status = ModelStatus::Failed,
            _ => {}
        }
        match event {
            Event::Ready if self.phase == Phase::Downloading => {
                self.phase = Phase::Idle;
                (true, self.settings.enabled)
            }
            Event::ModelLoaded if self.phase == Phase::Warming => {
                self.phase = Phase::Idle;
                self.idle_since = Instant::now();
                (false, false)
            }
            Event::Listening if self.phase == Phase::Loading => {
                self.phase = Phase::Recording;
                (false, false)
            }
            Event::Text {
                finalized: true, ..
            }
            | Event::NoSpeech(_)
            | Event::Cancelled => {
                self.phase = Phase::Idle;
                self.idle_since = Instant::now();
                (false, false)
            }
            Event::Error(_) => {
                self.phase = Phase::Idle;
                (true, false)
            }
            _ => (false, false),
        }
    }
    pub fn tick(&mut self) -> Vec<Event> {
        #[cfg(target_os = "macos")]
        if self
            .awaiting_permission
            .is_some_and(|last| last.elapsed() >= Duration::from_secs(1))
        {
            self.awaiting_permission = Some(Instant::now());
            if handy_keys::check_accessibility() {
                self.configure_keys();
            }
        }
        if let Ok(reply) = self.replies.try_recv() {
            self.saving = false;
            match reply {
                Ok(settings) => {
                    // Cambiar tecla/tiempos no reinicia una descarga en curso.
                    if self.phase != Phase::Downloading || !settings.enabled {
                        self.cancel();
                    }
                    self.settings = settings;
                    self.configure_keys();
                    if self.settings.enabled
                        && self.phase == Phase::Idle
                        && !model::ready(&self.model_path)
                    {
                        self.prepare();
                    } else if self.settings.enabled
                        && let Err(error) = self.warm()
                    {
                        self.pending.push(Event::Error(error.to_string()));
                    }
                    self.pending.push(Event::Saved);
                }
                Err(error) => {
                    self.settings_status = error.clone();
                    self.pending.push(Event::Error(error));
                }
            }
        }
        let mut events = Vec::new();
        while let Some(key) = self.keys.as_ref().and_then(|keys| keys.try_recv()) {
            if self.key(key.state).is_err() {
                self.cancel();
                self.model_status = ModelStatus::Failed;
                self.pending.push(Event::Error(
                    "No se pudo activar el dictado. Reintenta".into(),
                ));
            }
        }
        let mut exited = false;
        if let Some(p) = self.process.as_mut() {
            exited = p.exited();
            events.extend(p.events());
        }
        let mut completed = false;
        let mut preload = false;
        for event in &events {
            self.last_progress = Instant::now();
            let (finished, should_load) = self.observe(event);
            completed |= finished;
            preload |= should_load;
        }
        if exited && !completed {
            self.phase = Phase::Idle;
            self.model_status = ModelStatus::Failed;
            events.push(Event::Error(
                "El proceso de transcripción terminó. Reintenta desde el menú".into(),
            ));
            completed = true;
        }
        if completed {
            self.process.take();
        }
        if preload && let Err(error) = self.warm() {
            self.pending.push(Event::Error(error.to_string()));
        }
        if self.process.is_some()
            && self.phase == Phase::Idle
            && self.idle_since.elapsed() >= Duration::from_secs(self.settings.unload_seconds)
        {
            self.process.take();
            self.model_status = ModelStatus::Unloaded;
            events.push(Event::Unloaded);
        }
        let timeout = if self.phase == Phase::Downloading {
            900
        } else {
            45
        };
        if self.process.is_some()
            && self.phase != Phase::Idle
            && self.last_progress.elapsed() >= Duration::from_secs(timeout)
        {
            self.process.take();
            self.phase = Phase::Idle;
            self.model_status = ModelStatus::Failed;
            events.push(Event::Error(
                "La transcripción no respondió a tiempo. Reintenta".into(),
            ));
        }
        events.splice(0..0, self.pending.drain(..));
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_dictation_finishes_without_reloading_the_worker() {
        let mut c = controller(Mode::Hold);
        c.observe(&Event::ModelLoaded);
        c.phase = Phase::Finishing;
        assert_eq!(
            c.observe(&Event::NoSpeech("Entrada silenciosa".into())),
            (false, false)
        );
        assert_eq!(c.phase, Phase::Idle);
        assert_eq!(c.model_status(), ModelStatus::Ready);
    }
    #[test]
    fn verified_file_is_not_a_loaded_model() {
        let mut c = controller(Mode::Hold);
        c.phase = Phase::Downloading;
        c.model_status = ModelStatus::Downloading;
        assert_eq!(c.observe(&Event::Ready), (true, true));
        assert_eq!(c.model_status(), ModelStatus::Unloaded);
        c.phase = Phase::Warming;
        c.model_status = ModelStatus::Loading;
        c.observe(&Event::ModelLoaded);
        assert_eq!(c.model_status(), ModelStatus::Ready);
        assert_eq!(c.phase, Phase::Idle);
    }
    #[test]
    fn session_cancellation_keeps_model_but_controller_cancellation_releases_it() {
        let mut c = controller(Mode::Hold);
        c.observe(&Event::ModelLoaded);
        c.phase = Phase::Finishing;
        c.observe(&Event::Cancelled);
        assert_eq!(c.model_status(), ModelStatus::Ready);
        c.cancel();
        assert_eq!(c.model_status(), ModelStatus::Unloaded);
        c.settings.enabled = false;
        assert_eq!(c.model_status(), ModelStatus::Disabled);
    }
    #[test]
    fn worker_error_clears_loaded_status_until_recovery() {
        let mut c = controller(Mode::Hold);
        c.observe(&Event::Listening);
        assert_eq!(c.model_status(), ModelStatus::Ready);
        assert_eq!(
            c.observe(&Event::Error("Fallo del worker".into())),
            (true, false)
        );
        assert_eq!(c.model_status(), ModelStatus::Failed);
        c.observe(&Event::ModelLoaded);
        assert_eq!(c.model_status(), ModelStatus::Ready);
    }
    #[test]
    fn preload_ready_keeps_pending_dictation_and_release_state() {
        let mut c = controller(Mode::Hold);
        c.phase = Phase::Warming;
        c.observe(&Event::ModelLoaded);
        assert_eq!(c.phase, Phase::Idle);
        c.observe(&Event::Listening);
        assert_eq!(c.phase, Phase::Idle);
        c.phase = Phase::Loading;
        c.observe(&Event::ModelLoaded);
        assert_eq!(c.phase, Phase::Loading);
        c.observe(&Event::Listening);
        assert_eq!(c.phase, Phase::Recording);
        c.key(HotkeyState::Released).unwrap();
        c.observe(&Event::ModelLoaded);
        c.observe(&Event::Listening);
        assert_eq!(c.phase, Phase::Finishing);
        c.observe(&Event::Cancelled);
        assert_eq!(c.phase, Phase::Idle);
    }
    fn controller(mode: Mode) -> Controller {
        let (sender, _) = std::sync::mpsc::sync_channel(1);
        let (_, replies) = std::sync::mpsc::channel();
        Controller {
            settings: Settings {
                enabled: true,
                shortcut: "Ctrl+Option+Space".into(),
                mode,
                bubble_seconds: 5,
                unload_seconds: 30,
                microphone_name: None,
            },
            settings_status: String::new(),
            process: None,
            keys: None,
            awaiting_permission: None,
            capturing_shortcut: false,
            phase: Phase::Loading,
            model_status: ModelStatus::Loading,
            last_progress: Instant::now(),
            idle_since: Instant::now(),
            model_path: "unused".into(),
            sender,
            replies,
            saving: false,
            pending: Vec::new(),
        }
    }
    #[test]
    fn pressing_dictation_key_requests_barge_in_even_while_model_loads() {
        let mut c = controller(Mode::Hold);
        c.key(HotkeyState::Pressed).unwrap();
        assert!(matches!(c.pending.as_slice(), [Event::DictationRequested]));
        c.settings.enabled = false;
        c.pending.clear();
        c.key(HotkeyState::Pressed).unwrap();
        assert!(c.pending.is_empty());
    }
    #[test]
    fn hold_release_during_model_load_finishes_and_repeat_does_not_restart() {
        let mut c = controller(Mode::Hold);
        c.key(HotkeyState::Pressed).unwrap();
        assert_eq!(c.phase, Phase::Loading);
        c.key(HotkeyState::Released).unwrap();
        assert_eq!(c.phase, Phase::Finishing);
        c.key(HotkeyState::Pressed).unwrap();
        assert_eq!(c.phase, Phase::Finishing);
    }
    #[test]
    fn toggle_ignores_release_and_second_press_finishes() {
        let mut c = controller(Mode::Toggle);
        c.key(HotkeyState::Released).unwrap();
        assert_eq!(c.phase, Phase::Loading);
        c.key(HotkeyState::Pressed).unwrap();
        assert_eq!(c.phase, Phase::Finishing);
        c.cancel();
        assert_eq!(c.phase, Phase::Idle);
    }
}
