use crate::{PresenceWindow, TranscriptionSettings};
use slint::ComponentHandle;
use soc_core::{AppState, Result};
use soc_transcription::{Controller, Event, Mode, Settings, TranscriptionModule};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

pub struct TranscriptionUi {
    settings: TranscriptionSettings,
    timer: slint::Timer,
    controller: Rc<RefCell<Controller>>,
    decisions: Option<Rc<soc_decisions::Controller>>,
}
fn voice_model_indicator(status: soc_transcription::ModelStatus) -> (bool, &'static str) {
    use soc_transcription::ModelStatus;
    let text = match status {
        ModelStatus::Disabled => "Nemotron · reconocimiento de voz\nDesactivado",
        ModelStatus::Unloaded => "Nemotron · reconocimiento de voz\nDescargado de memoria",
        ModelStatus::Downloading => "Nemotron · reconocimiento de voz\nDescargando archivo…",
        ModelStatus::Loading => "Nemotron · reconocimiento de voz\nCargando en memoria…",
        ModelStatus::Ready => "Nemotron · reconocimiento de voz\nCargado en memoria",
        ModelStatus::Failed => "Nemotron · reconocimiento de voz\nError · modelo no disponible",
    };
    (status == ModelStatus::Ready, text)
}
fn decision_model_indicator(status: &soc_decisions::ModelStatus) -> (bool, &'static str) {
    use soc_decisions::ModelStatus;
    let text = match status {
        ModelStatus::Unloaded => "Laya · análisis de órdenes\nDescargado de memoria",
        ModelStatus::Loading => "Laya · análisis de órdenes\nCargando en memoria…",
        ModelStatus::Ready => "Laya · análisis de órdenes\nCargado en memoria",
        ModelStatus::Failed(_) => "Laya · análisis de órdenes\nError · modelo no disponible",
    };
    (*status == ModelStatus::Ready, text)
}
fn update_voice_indicator(ui: &PresenceWindow, status: soc_transcription::ModelStatus) {
    let (loaded, tooltip) = voice_model_indicator(status);
    ui.set_voice_model_loaded(loaded);
    ui.set_voice_model_tooltip(tooltip.into());
}
fn update_decision_indicator(ui: &PresenceWindow, status: &soc_decisions::ModelStatus) {
    let (loaded, tooltip) = decision_model_indicator(status);
    ui.set_decision_model_loaded(loaded);
    ui.set_decision_model_tooltip(tooltip.into());
}
fn populate(window: &TranscriptionSettings, controller: &Controller) {
    let s = &controller.settings;
    window.set_enabled(s.enabled);
    window.set_shortcut(s.shortcut.clone().into());
    window.set_mode_index(if s.mode == Mode::Hold { 0 } else { 1 });
    window.set_bubble_seconds(s.bubble_seconds as i32);
    window.set_unload_minutes(s.unload_seconds.div_ceil(60) as i32);
}
fn populate_decisions(window: &TranscriptionSettings, settings: &soc_decisions::Settings) {
    window.set_decision_preload(settings.preload);
    window.set_decision_keep_loaded(settings.unload_seconds.is_none());
    window.set_decision_unload_minutes(settings.unload_seconds.unwrap_or(600).div_ceil(60) as i32);
}
fn refresh_microphones(window: &TranscriptionSettings, selected: Option<&str>) -> Result<()> {
    let mut names = vec!["Predeterminado del sistema".to_string()];
    let devices = soc_transcription::input_devices();
    if let Ok(devices) = &devices {
        names.extend(devices.iter().cloned());
    }
    if let Some(name) = selected
        && !names[1..].iter().any(|value| value == name)
    {
        // Conservar la elección desconectada, sin sustituirla silenciosamente.
        names.push(name.into());
    }
    let index = selected
        .and_then(|name| {
            names[1..]
                .iter()
                .position(|value| value == name)
                .map(|i| i + 1)
        })
        .unwrap_or(0);
    window.set_microphones(slint::ModelRc::new(slint::VecModel::from(
        names
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    window.set_microphone_index(index as i32);
    devices.map(|_| ())
}
fn selected_microphone(window: &TranscriptionSettings) -> Option<String> {
    use slint::Model;
    (window.get_microphone_index() > 0)
        .then(|| {
            window
                .get_microphones()
                .row_data(window.get_microphone_index() as usize)
                .map(|name| name.to_string())
        })
        .flatten()
}
impl TranscriptionUi {
    pub fn new(
        ui: &PresenceWindow,
        module: &TranscriptionModule,
        state: &AppState,
        decisions: Option<&soc_decisions::DecisionsModule>,
    ) -> Result<Self> {
        let controller = Rc::new(RefCell::new(module.controller(state)?));
        let settings = TranscriptionSettings::new()?;
        let chat = Rc::new(RefCell::new(crate::chat::Chat::default()));
        ui.set_chat_bubbles(slint::ModelRc::from(chat.borrow().model.clone()));
        let decisions = decisions
            .map(|module| module.controller().map(Rc::new))
            .transpose()?;
        update_voice_indicator(ui, controller.borrow().model_status());
        if let Some(controller) = &decisions {
            update_decision_indicator(ui, &controller.model_status());
            populate_decisions(&settings, &controller.settings());
        }
        settings.set_decisions_available(decisions.is_some());
        let command_mode = Rc::new(std::cell::Cell::new(settings.get_command_mode_enabled()));
        let mode = command_mode.clone();
        let control = decisions.clone();
        let messages = chat.clone();
        settings.on_command_mode_changed(move |enabled| {
            mode.set(enabled);
            if !enabled && let Some(control) = &control {
                control.cancel();
                messages.borrow_mut().cancel_decision();
            }
        });
        populate(&settings, &controller.borrow());
        settings.set_status(controller.borrow().settings_status.clone().into());
        settings.set_model_status("El modelo se carga al abrir soc".into());
        let weak = settings.as_weak();
        let c = controller.clone();
        ui.on_transcription_settings(move || {
            if let Some(window) = weak.upgrade() {
                populate(&window, &c.borrow());
                if let Err(error) =
                    refresh_microphones(&window, c.borrow().settings.microphone_name.as_deref())
                {
                    window.set_status(
                        format!("No se pudieron listar los micrófonos: {error}").into(),
                    );
                }
                if let Err(error) = crate::desktop::present_settings(window.window()) {
                    window.set_status(format!("No se pudo abrir la configuración: {error}").into());
                    eprintln!("event=transcription_settings_failed module=presence error={error}");
                } else {
                    c.borrow_mut().request_keys_permission();
                }
            }
        });
        let weak = settings.as_weak();
        let c = controller.clone();
        settings.on_capture_shortcut(move |recording| {
            c.borrow_mut().capture_shortcut(recording);
            if let Some(window) = weak.upgrade() {
                window.set_recording_shortcut(recording);
            }
        });
        let c = controller.clone();
        crate::desktop::install_shortcut_capture(&settings, move || {
            c.borrow_mut().capture_shortcut(false)
        });
        let weak = settings.as_weak();
        settings.on_refresh_microphones(move || {
            if let Some(window) = weak.upgrade() {
                let selected = selected_microphone(&window);
                if let Err(error) = refresh_microphones(&window, selected.as_deref()) {
                    window.set_status(
                        format!("No se pudieron listar los micrófonos: {error}").into(),
                    );
                }
            }
        });
        let weak = settings.as_weak();
        let c = controller.clone();
        settings.on_save(move || {
            if let Some(window) = weak.upgrade() {
                let next = Settings {
                    enabled: window.get_enabled(),
                    shortcut: window.get_shortcut().to_string().trim().into(),
                    mode: if window.get_mode_index() == 0 {
                        Mode::Hold
                    } else {
                        Mode::Toggle
                    },
                    bubble_seconds: window.get_bubble_seconds() as u64,
                    unload_seconds: window.get_unload_minutes() as u64 * 60,
                    microphone_name: selected_microphone(&window),
                };
                if let Err(error) = c.borrow_mut().save(next) {
                    window.set_status(error.to_string().into());
                }
            }
        });
        let weak = settings.as_weak();
        let control = decisions.clone();
        settings.on_save_decisions(move || {
            if let Some(window) = weak.upgrade()
                && let Some(control) = &control
            {
                let next = soc_decisions::Settings {
                    preload: window.get_decision_preload(),
                    unload_seconds: (!window.get_decision_keep_loaded()).then_some(
                        u64::try_from(window.get_decision_unload_minutes()).unwrap_or(0) * 60,
                    ),
                };
                match control.save_settings(next) {
                    Ok(()) => {
                        window.set_decision_saving(true);
                        window.set_decision_settings_status("Guardando ajustes de Laya…".into());
                    }
                    Err(error) => window.set_decision_settings_status(error.to_string().into()),
                }
            }
        });
        let weak = settings.as_weak();
        let control = decisions.clone();
        settings.on_load_decisions(move || {
            if let Some(control) = &control
                && let Err(error) = control.load_model()
                && let Some(window) = weak.upgrade()
            {
                window.set_decision_settings_status(error.to_string().into());
            }
        });
        let control = decisions.clone();
        let messages = chat.clone();
        settings.on_unload_decisions(move || {
            if let Some(control) = &control {
                control.unload_model();
                messages.borrow_mut().cancel_decision();
            }
        });
        let c = controller.clone();
        settings.on_prepare(move || c.borrow_mut().prepare());
        let c = controller.clone();
        settings.on_retry_keys(move || c.borrow_mut().retry_keys());
        let c = controller.clone();
        let messages = chat.clone();
        settings.on_cancel(move || {
            c.borrow_mut().cancel();
            messages.borrow_mut().cancel_transcription();
        });
        let timer = slint::Timer::default();
        let c = controller.clone();
        let weak_ui = ui.as_weak();
        let weak_settings = settings.as_weak();

        let mut last_status = String::new();
        let decision_controller = decisions.clone();
        let mut last_decision_status = None;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(25),
            move || {
                let (events, status, ttl, voice_model_status) = {
                    let mut controller = c.borrow_mut();
                    (
                        controller.tick(),
                        controller.settings_status.clone(),
                        Duration::from_secs(controller.settings.bubble_seconds),
                        controller.model_status(),
                    )
                };
                if let Some(window) = weak_settings.upgrade()
                    && status != last_status
                {
                    last_status = status.clone();
                    window.set_status(last_status.clone().into());
                }
                let Some(ui) = weak_ui.upgrade() else {
                    return;
                };
                update_voice_indicator(&ui, voice_model_status);
                let now = Instant::now();
                let mut messages = chat.borrow_mut();
                if let Some(controller) = &decision_controller {
                    if let Some(saved) = controller.poll_settings() && let Some(window) = weak_settings.upgrade() {
                        window.set_decision_saving(false);
                        match saved {
                            Ok(settings) => {
                                populate_decisions(&window, &settings);
                                window.set_decision_settings_status("Ajustes de Laya guardados".into());
                            }
                            Err(error) => window.set_decision_settings_status(error.into()),
                        }
                    }
                    let outcomes = controller.poll();
                    let model_status = controller.model_status();
                    update_decision_indicator(&ui, &model_status);
                    if let Some(window) = weak_settings.upgrade() {
                        window.set_decision_model_loaded(model_status == soc_decisions::ModelStatus::Ready);
                        window.set_decision_model_busy(controller.is_pending() || model_status == soc_decisions::ModelStatus::Loading);
                    }
                    if last_decision_status.as_ref() != Some(&model_status) {
                        let (text, finished) = match &model_status {
                            soc_decisions::ModelStatus::Loading => ("Cargando Laya…".to_string(), false),
                            soc_decisions::ModelStatus::Ready if controller.is_pending() => ("Analizando la orden…".to_string(), false),
                            soc_decisions::ModelStatus::Ready => ("Análisis de órdenes listo".to_string(), true),
                            soc_decisions::ModelStatus::Unloaded => ("Laya no está cargado en memoria".to_string(), true),
                            soc_decisions::ModelStatus::Failed(error) => (format!("Control local no disponible: {error}"), true),
                        };
                        if let Some(window) = weak_settings.upgrade() {
                            window.set_decision_model_status(text.clone().into());
                        }
                        if model_status != soc_decisions::ModelStatus::Unloaded
                            && outcomes.is_empty()
                            && !(controller.is_pending() && matches!(&model_status, soc_decisions::ModelStatus::Failed(_)))
                        {
                            messages.decision(&text, finished, ttl, now);
                        }
                        last_decision_status = Some(model_status);
                    }
                    for outcome in outcomes {
                        messages.decision(&outcome.message, true, ttl, now);
                    }
                }
                for event in events {
                    match event {
                        Event::Status(text) => messages.progress(&text, ttl, now),
                        Event::Download(percent) => {
                            let text = format!("Descargando Nemotron · {percent}%");
                            if let Some(window) = weak_settings.upgrade() {
                                window.set_model_status(text.clone().into());
                            }
                            messages.status("Modelo local", &text, false, ttl, now);
                        }
                        Event::Ready => {
                            if let Some(window) = weak_settings.upgrade() {
                                window.set_model_status("Archivo del modelo verificado".into());
                            }
                            continue;
                        }
                        Event::ModelLoaded => {
                            if let Some(window) = weak_settings.upgrade() {
                                window
                                    .set_model_status("Reconocimiento de voz listo".into());
                            }
                            messages.loaded(ttl, now);
                        }
                        Event::DictationStarted => {
                            ui.set_active(true);
                            messages.begin(ttl, now);
                        }
                        Event::Listening => {
                            ui.set_active(true);
                            messages.listening(ttl, now);
                        }
                        Event::Text { text, finalized } => {
                            if !text.trim().is_empty() {
                                messages.speech(
                                    if finalized { "Tú" } else { "Tú · hablando" },
                                    &text,
                                    finalized,
                                    ttl,
                                    now,
                                );
                                if finalized && command_mode.get() && let Some(controller) = &decision_controller {
                                    if controller.is_pending() {
                                        messages.status("SOC", "Hay una orden en curso. Espera su resultado antes de dar otra.", true, ttl, now);
                                    } else {
                                        match controller.submit(&text, true) {
                                            Ok(()) => {
                                                let status = if controller.model_status() == soc_decisions::ModelStatus::Ready { "Analizando la orden…" } else { "Preparando Laya; la orden está en espera…" };
                                                messages.decision(status, false, ttl, now);
                                            }
                                            Err(error) => messages.status("SOC", &error.to_string(), true, ttl, now),
                                        }
                                    }
                                }
                            }
                        }
                        Event::NoSpeech(text) | Event::Error(text) => {
                            if let Some(window) = weak_settings.upgrade() {
                                window.set_model_status(text.clone().into());
                            }
                            messages.error(&text, ttl, now);
                        }
                        Event::Saved => {
                            if let Some(window) = weak_settings.upgrade() {
                                populate(&window, &c.borrow());
                                window.set_status(format!("Guardado · {status}").into());
                            }
                            continue;
                        }
                        Event::Cancelled => messages.cancel_transcription(),
                        Event::Unloaded => {
                            if let Some(window) = weak_settings.upgrade() {
                                window.set_model_status(
                                    "Modelo en reposo · se cargará al activar el dictado".into(),
                                );
                            }
                            continue;
                        }
                        Event::Heartbeat => continue,
                    }
                }
                if messages.expire(now) {
                    ui.set_active(false);
                }
            },
        );
        Ok(Self {
            settings,
            timer,
            controller,
            decisions,
        })
    }
    pub fn stop(&self) {
        self.timer.stop();
        self.controller.borrow_mut().cancel();
        if let Some(controller) = &self.decisions {
            controller.cancel();
        }
        let _ = self.settings.hide();
    }
}
impl Drop for TranscriptionUi {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loaded_models_fill_the_indicators() {
        use soc_transcription::ModelStatus as VoiceStatus;
        for status in [
            VoiceStatus::Disabled,
            VoiceStatus::Unloaded,
            VoiceStatus::Downloading,
            VoiceStatus::Loading,
            VoiceStatus::Failed,
        ] {
            assert!(!voice_model_indicator(status).0, "{status:?}");
        }
        assert!(voice_model_indicator(VoiceStatus::Ready).0);
        use soc_decisions::ModelStatus as DecisionStatus;
        for status in [
            DecisionStatus::Unloaded,
            DecisionStatus::Loading,
            DecisionStatus::Failed("detalle interno".into()),
        ] {
            let (loaded, tooltip) = decision_model_indicator(&status);
            assert!(!loaded, "{status:?}");
            assert!(!tooltip.contains("detalle interno"));
        }
        assert!(decision_model_indicator(&DecisionStatus::Ready).0);
    }
}
