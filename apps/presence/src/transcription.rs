use crate::{PresenceWindow, TranscriptionSettings};
use slint::ComponentHandle;
use soc_core::{AppState, Result};
use soc_transcription::{Controller, Event, Mode, Settings, TranscriptionModule};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};

fn queue_narration(
    messages: &mut crate::chat::Chat,
    queue: &mut VecDeque<(u64, String)>,
    tts: Option<&soc_tts::Controller>,
    bubble_id: u64,
    text: &str,
) {
    if !tts.is_some_and(|tts| tts.settings().enabled) {
        return;
    }
    let spoken = crate::answer::prepare(text).text;
    if !spoken.trim().is_empty() {
        messages.begin_audio(Some(bubble_id));
        queue.push_back((bubble_id, spoken));
    }
}

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
fn populate_llm(window: &TranscriptionSettings, settings: &soc_llm::Settings) {
    window.set_llm_read_files(settings.read_files);
    window.set_llm_write_files(settings.write_files);
    window.set_llm_run_commands(settings.run_commands);
    window.set_llm_local_actions(settings.local_actions);
    window.set_llm_model(settings.model.clone().into());
    window.set_llm_effort_index(match settings.effort.as_str() {
        "medium" => 1,
        "high" => 2,
        "xhigh" => 3,
        "max" => 4,
        "ultra" => 5,
        _ => 0,
    });
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
        llm: Option<&soc_llm::LlmModule>,
        tts: Option<&soc_tts::TtsModule>,
        history: crate::history::Writer,
    ) -> Result<Self> {
        let controller = Rc::new(RefCell::new(module.controller(state)?));
        let settings = TranscriptionSettings::new()?;
        let chat = Rc::new(RefCell::new(crate::chat::Chat::default()));
        ui.set_chat_bubbles(slint::ModelRc::from(chat.borrow().model.clone()));
        ui.set_chat_sources(slint::ModelRc::from(chat.borrow().sources.clone()));
        let decisions = decisions
            .map(|module| module.controller().map(Rc::new))
            .transpose()?;
        let llm = llm
            .map(|module| module.controller().map(Rc::new))
            .transpose()?;
        let tts = tts
            .map(|module| module.controller().map(Rc::new))
            .transpose()?;
        settings.set_tts_available(tts.is_some());
        if let Some(controller) = &tts {
            let saved = controller.settings();
            settings.set_tts_enabled(saved.enabled);
            settings.set_tts_voice_index(match saved.voice.as_str() {
                "em_alex" => 1,
                "em_santa" => 2,
                _ => 0,
            });
            settings.set_tts_speed_percent(saved.speed_percent as i32);
            settings.set_tts_status("La voz se prepara al recibir la primera respuesta".into());
        }
        let tts_save = tts.clone();
        let weak_tts_settings = settings.as_weak();
        settings.on_save_tts(move || {
            if let (Some(window), Some(controller)) =
                (weak_tts_settings.upgrade(), tts_save.as_ref())
            {
                let next = soc_tts::Settings {
                    enabled: window.get_tts_enabled(),
                    voice: match window.get_tts_voice_index() {
                        1 => "em_alex",
                        2 => "em_santa",
                        _ => "ef_dora",
                    }
                    .into(),
                    speed_percent: window.get_tts_speed_percent() as u32,
                };
                match controller.save(next) {
                    Ok(()) => {
                        window.set_tts_saving(true);
                        window.set_tts_status("Guardando…".into());
                    }
                    Err(error) => window.set_tts_status(error.to_string().into()),
                }
            }
        });
        let pending_tts = Rc::new(RefCell::new(HashMap::<u64, Option<u64>>::new()));
        let tts_preview = tts.clone();
        let weak_tts_settings = settings.as_weak();
        let weak_tts_ui = ui.as_weak();
        let pending_for_preview = pending_tts.clone();
        let chat_for_preview = chat.clone();
        settings.on_preview_tts(move || {
            if let (Some(window), Some(controller)) =
                (weak_tts_settings.upgrade(), tts_preview.as_ref())
            {
                let draft = soc_tts::Settings {
                    enabled: window.get_tts_enabled(),
                    voice: match window.get_tts_voice_index() {
                        1 => "em_alex",
                        2 => "em_santa",
                        _ => "ef_dora",
                    }
                    .into(),
                    speed_percent: window.get_tts_speed_percent() as u32,
                };
                match controller.preview(draft) {
                    Ok(id) => {
                        pending_for_preview.borrow_mut().insert(id, None);
                        chat_for_preview.borrow_mut().begin_audio(None);
                        if let Some(ui) = weak_tts_ui.upgrade() {
                            ui.set_active(true);
                            ui.set_speech_active(true);
                        }
                        window.set_tts_status("Preparando prueba de voz…".into());
                    }
                    Err(error) => window.set_tts_status(error.to_string().into()),
                }
            }
        });
        ui.set_llm_available(llm.is_some());
        settings.set_llm_available(llm.is_some());
        if let Some(controller) = &llm {
            populate_llm(&settings, &controller.settings());
        }
        settings.set_llm_status(
            if llm.is_some() {
                "Se conectará al hablar o al pulsar Conectar"
            } else {
                "Conversación no disponible"
            }
            .into(),
        );
        update_voice_indicator(ui, controller.borrow().model_status());
        if let Some(controller) = &decisions {
            update_decision_indicator(ui, &controller.model_status());
            populate_decisions(&settings, &controller.settings());
        }
        settings.set_decisions_available(decisions.is_some());
        let interaction_mode = Rc::new(std::cell::Cell::new(settings.get_interaction_mode_index()));
        let mode = interaction_mode.clone();
        let control = decisions.clone();
        let messages = chat.clone();
        settings.on_interaction_mode_changed(move |index| {
            mode.set(index);
            if index != 2
                && let Some(control) = &control
            {
                control.cancel();
                messages.borrow_mut().cancel_decision();
            }
        });
        let llm_control = llm.clone();
        let weak = settings.as_weak();
        settings.on_connect_llm(move || {
            if let Some(window) = weak.upgrade()
                && let Some(control) = &llm_control
            {
                match control.connect() {
                    Ok(()) => {
                        window.set_llm_busy(true);
                        window.set_llm_status("Conectando…".into());
                    }
                    Err(error) => window.set_llm_status(error.to_string().into()),
                }
            }
        });
        let llm_control = llm.clone();
        let weak = settings.as_weak();
        settings.on_save_llm(move || {
            if let (Some(window), Some(control)) = (weak.upgrade(), llm_control.as_ref()) {
                let next = soc_llm::Settings {
                    read_files: window.get_llm_read_files(),
                    write_files: window.get_llm_write_files(),
                    run_commands: window.get_llm_run_commands(),
                    local_actions: window.get_llm_local_actions(),
                    model: window.get_llm_model().to_string(),
                    effort: match window.get_llm_effort_index() {
                        1 => "medium",
                        2 => "high",
                        3 => "xhigh",
                        4 => "max",
                        5 => "ultra",
                        _ => "low",
                    }
                    .into(),
                };
                match control.save_settings(next) {
                    Ok(()) => {
                        window.set_llm_saving(true);
                        window.set_llm_settings_status("Guardando ajustes de Codex…".into());
                    }
                    Err(error) => window.set_llm_settings_status(error.to_string().into()),
                }
            }
        });
        let llm_control = llm.clone();
        let weak_ui = ui.as_weak();
        let weak_settings = settings.as_weak();
        let messages = chat.clone();
        ui.on_new_chat(move || {
            let Some(window) = weak_ui.upgrade() else {
                return;
            };
            let Some(control) = &llm_control else {
                return;
            };
            match control.new_chat() {
                Ok(()) => {
                    window.set_llm_busy(true);
                    if let Some(settings) = weak_settings.upgrade() {
                        settings.set_llm_busy(true);
                        settings.set_llm_status("Creando chat…".into());
                    }
                }
                Err(error) => {
                    messages.borrow_mut().decision(
                        &error.to_string(),
                        true,
                        Duration::from_secs(8),
                        Instant::now(),
                    );
                }
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
                if let Err(error) = crate::desktop::present_window(window.window()) {
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
        let llm_controller = llm.clone();
        let tts_controller = tts.clone();
        let pending_for_timer_audio = pending_tts.clone();
        let history_for_timer = history.clone();
        let pending_local_text = Rc::new(RefCell::new(None::<String>));
        let pending_for_timer = pending_local_text.clone();
        let mut last_decision_status = None;
        let mut partial_reply = String::new();
        let mut narration = crate::narration::Narration::default();
        let mut narration_queue = VecDeque::<(u64, String)>::new();
        let mut narration_suppressed = false;
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
                if let Some(controller) = &tts_controller {
                    for event in controller.poll() {
                        match event {
                            soc_tts::Event::Started(id) => {
                                if pending_for_timer_audio.borrow().contains_key(&id) {
                                    ui.set_active(true);
                                    if let Some(window) = weak_settings.upgrade() {
                                        window.set_tts_status("SOC está hablando…".into());
                                    }
                                }
                            }
                            soc_tts::Event::Finished(id) => {
                                if let Some(bubble_id) = pending_for_timer_audio.borrow_mut().remove(&id) {
                                    messages.finish_audio(bubble_id, ttl, now);
                                    if let Some(window) = weak_settings.upgrade() {
                                        window.set_tts_status("Reproducción terminada".into());
                                    }
                                }
                            }
                            soc_tts::Event::Failed(id, error) => {
                                if let Some(bubble_id) = pending_for_timer_audio.borrow_mut().remove(&id) {
                                    messages.finish_audio(bubble_id, ttl, now);
                                }
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_tts_status(format!("Voz no disponible: {error}").into());
                                }
                                messages.status("Voz", &format!("Voz no disponible: {error}"), true, ttl, now);
                            }
                            soc_tts::Event::Interrupted(id) => {
                                if let Some(bubble_id) = pending_for_timer_audio.borrow_mut().remove(&id) {
                                    messages.finish_audio(bubble_id, ttl, now);
                                }
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_tts_status("Voz interrumpida".into());
                                }
                            }
                            soc_tts::Event::Saved(saved) => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_tts_saving(false);
                                    window.set_tts_enabled(saved.enabled);
                                    window.set_tts_voice_index(match saved.voice.as_str() { "em_alex" => 1, "em_santa" => 2, _ => 0 });
                                    window.set_tts_speed_percent(saved.speed_percent as i32);
                                    window.set_tts_status("Voz de SOC guardada".into());
                                }
                            }
                            soc_tts::Event::SaveFailed(error) => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_tts_saving(false);
                                    window.set_tts_status(format!("Error al guardar: {error}").into());
                                }
                            }
                        }
                    }
                    ui.set_speech_active(!pending_for_timer_audio.borrow().is_empty());
                }
                if let Some(controller) = &llm_controller {
                    for event in controller.poll() {
                        match event {
                            soc_llm::Event::Connecting => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_busy(true);
                                    window.set_llm_status("Conectando…".into());
                                }
                            }
                            soc_llm::Event::Connected => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_connected(true);
                                    window.set_llm_status("Conversación disponible".into());
                                }
                            }
                            soc_llm::Event::Disconnected(error) => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_connected(false);
                                    window.set_llm_status(error.into());
                                }
                            }
                            soc_llm::Event::ReplyDelta(delta) => {
                                partial_reply.push_str(&delta);
                                if !partial_reply.trim().is_empty() {
                                    let bubble_id = messages.decision(&partial_reply, false, ttl, now);
                                    if !narration_suppressed {
                                        for sentence in narration.push(&delta) {
                                            queue_narration(&mut messages, &mut narration_queue, tts_controller.as_deref(), bubble_id, &sentence);
                                        }
                                    }
                                }
                            }
                            soc_llm::Event::Reply(reply) => {
                                let bubble_id = messages.decision(&reply, true, ttl, now);
                                if !narration_suppressed {
                                    let remaining = reply.strip_prefix(&partial_reply).unwrap_or(&reply);
                                    let mut sentences = narration.push(remaining);
                                    if let Some(last) = narration.finish() {
                                        sentences.push(last);
                                    }
                                    for sentence in sentences {
                                        queue_narration(&mut messages, &mut narration_queue, tts_controller.as_deref(), bubble_id, &sentence);
                                    }
                                }
                                narration.clear();
                                partial_reply.clear();
                                if let Err(error) = history_for_timer.save("assistant", &reply) {
                                    messages.status("Chat", error, true, ttl, now);
                                }
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_status("Conversación disponible".into());
                                }
                            }
                            soc_llm::Event::NewChatReady => {
                                messages.decision("Nuevo chat iniciado", true, ttl, now);
                                if let Err(error) = history_for_timer.save("system", "Nuevo chat iniciado") {
                                    messages.status("Chat", error, true, ttl, now);
                                }
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_status("Nuevo chat iniciado".into());
                                }
                            }
                            soc_llm::Event::Failed(error) => {
                                if !partial_reply.is_empty() {
                                    messages.decision(&partial_reply, true, ttl, now);
                                    partial_reply.clear();
                                }
                                narration.clear();
                                messages.decision(&error, true, ttl, now);
                                if let Err(save_error) = history_for_timer.save("assistant", &error) {
                                    messages.status("Chat", save_error, true, ttl, now);
                                }
                            }
                            soc_llm::Event::SettingsSaved(saved) => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_saving(false);
                                    populate_llm(&window, &saved);
                                    window.set_llm_settings_status("Ajustes de Codex guardados".into());
                                }
                            }
                            soc_llm::Event::SettingsSaveFailed(error) => {
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_llm_saving(false);
                                    window.set_llm_settings_status(format!("No se pudieron guardar los ajustes: {error}").into());
                                }
                            }
                        }
                    }
                    if let Some(window) = weak_settings.upgrade() {
                        window.set_llm_busy(controller.is_pending());
                    }
                    ui.set_llm_busy(controller.is_pending());
                }
                if let Some(tts) = &tts_controller {
                    while let Some((bubble_id, text)) = narration_queue.front() {
                        let bubble_id = *bubble_id;
                        match tts.speak(text) {
                            Ok(Some(id)) => {
                                narration_queue.pop_front();
                                pending_for_timer_audio.borrow_mut().insert(id, Some(bubble_id));
                                ui.set_active(true);
                            }
                            Ok(None) => {
                                narration_queue.pop_front();
                                messages.finish_audio(Some(bubble_id), ttl, now);
                            }
                            Err(error) if error.to_string() == "Cola de voz ocupada" => break,
                            Err(error) => {
                                narration_queue.pop_front();
                                messages.finish_audio(Some(bubble_id), ttl, now);
                                if let Some(window) = weak_settings.upgrade() {
                                    window.set_tts_status(format!("Voz no disponible: {error}").into());
                                }
                                messages.status("Voz", &format!("Voz no disponible: {error}"), true, ttl, now);
                            }
                        }
                    }
                    ui.set_speech_active(!pending_for_timer_audio.borrow().is_empty() || !narration_queue.is_empty());
                }
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
                        let text = pending_for_timer.borrow_mut().take();
                        if outcome.prediction.as_ref().is_some_and(|prediction| prediction.route == soc_decisions::Route::GodCall)
                            && let (Some(text), Some(llm)) = (text, llm_controller.as_ref())
                        {
                            match llm.speak(&text) {
                                Ok(()) => {
                                    narration_suppressed = false;
                                    messages.decision("Pensando…", false, ttl, now)
                                },
                                Err(error) => messages.decision(&error.to_string(), true, ttl, now),
                            };
                        } else {
                            let bubble_id = messages.decision(&outcome.message, true, ttl, now);
                            if let Some(tts) = &tts_controller {
                                match tts.speak(&crate::answer::prepare(&outcome.message).text) {
                                    Ok(Some(id)) => {
                                        pending_for_timer_audio.borrow_mut().insert(id, Some(bubble_id));
                                        messages.begin_audio(Some(bubble_id));
                                        ui.set_active(true);
                                        ui.set_speech_active(true);
                                    }
                                    Ok(None) => {}
                                    Err(error) => {
                                        if let Some(window) = weak_settings.upgrade() {
                                            window.set_tts_status(format!("Voz no disponible: {error}").into());
                                        }
                                        messages.status("Voz", &format!("Voz no disponible: {error}"), true, ttl, now);
                                    }
                                }
                            }
                            if let Err(error) = history_for_timer.save("assistant", &outcome.message) {
                                messages.status("Chat", error, true, ttl, now);
                            }
                        }
                    }
                }
                for event in events {
                    match event {
                        Event::DictationRequested => {
                            narration_suppressed = llm_controller.as_ref().is_some_and(|controller| controller.is_pending());
                            if narration_suppressed {
                                narration.clear();
                            }
                            for (bubble_id, _) in narration_queue.drain(..) {
                                messages.finish_audio(Some(bubble_id), ttl, now);
                            }
                            if let Some(tts) = &tts_controller
                                && !pending_for_timer_audio.borrow().is_empty()
                            {
                                match tts.interrupt() {
                                    Ok(()) => {
                                        let pending = pending_for_timer_audio.borrow_mut().drain().collect::<Vec<_>>();
                                        for (_, bubble_id) in pending {
                                            messages.finish_audio(bubble_id, ttl, now);
                                        }
                                        ui.set_speech_active(false);
                                        if let Some(window) = weak_settings.upgrade() {
                                            window.set_tts_status("Voz interrumpida".into());
                                        }
                                    }
                                    Err(error) => {
                                        if let Some(window) = weak_settings.upgrade() {
                                            window.set_tts_status(format!("No se pudo interrumpir la voz: {error}").into());
                                        }
                                    }
                                }
                            }
                            ui.set_active(true);
                        }
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
                                if finalized && let Err(error) = history_for_timer.save("user", &text) {
                                    messages.status("Chat", error, true, ttl, now);
                                }
                                if finalized && interaction_mode.get() == 0 {
                                    if let Some(controller) = &llm_controller {
                                        match controller.speak(&text) {
                                            Ok(()) => {
                                                narration_suppressed = false;
                                                messages.decision("Pensando…", false, ttl, now)
                                            },
                                            Err(error) => messages.decision(&error.to_string(), true, ttl, now),
                                        };
                                    } else {
                                        messages.decision("Conversación no disponible en esta instancia", true, ttl, now);
                                    }
                                }
                                if finalized && interaction_mode.get() == 2 && let Some(controller) = &decision_controller {
                                    if controller.is_pending() {
                                        messages.status("SOC", "Hay una orden en curso. Espera su resultado antes de dar otra.", true, ttl, now);
                                    } else {
                                        match controller.submit(&text, true) {
                                            Ok(()) => {
                                                *pending_local_text.borrow_mut() = Some(text.clone());
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
                if messages.expire(now) && !ui.get_history_open() {
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
