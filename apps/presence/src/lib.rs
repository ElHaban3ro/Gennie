mod answer;
mod chat;
mod desktop;
mod history;
mod narration;
mod repository;
#[cfg(target_os = "macos")]
mod status_menu;
mod transcription;
use slint::ComponentHandle;
use soc_core::{AppModule, AppState, Migration, Result};
use std::sync::{
    Mutex,
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::JoinHandle;

slint::include_modules!();

fn history_time(value: &str) -> String {
    match (value.get(8..10), value.get(5..7), value.get(11..16)) {
        (Some(day), Some(month), Some(time)) => format!("{day}/{month} · {time} UTC"),
        _ => value.to_owned(),
    }
}

pub struct PresenceModule {
    worker: Mutex<Option<Worker>>,
}
struct Worker {
    sender: SyncSender<bool>,
    replies: Option<Receiver<std::result::Result<bool, String>>>,
    handle: JoinHandle<()>,
}
impl Default for PresenceModule {
    fn default() -> Self {
        Self {
            worker: Mutex::new(None),
        }
    }
}
impl AppModule for PresenceModule {
    fn name(&self) -> &'static str {
        "presence"
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        &["transcription", "decisions", "llm", "tts"]
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_preferences.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_history.sql"),
            },
            Migration {
                version: 3,
                sql: include_str!("../migrations/0003_remove_memory_activity.sql"),
            },
        ]
    }
    fn start(&self, state: &AppState) -> Result<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "Worker de presencia inválido")?;
        if worker.is_some() {
            return Ok(());
        }
        let db = state.db.clone();
        let (sender, receiver) = mpsc::sync_channel(8);
        let (reply_sender, replies) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("presence-preferences".into())
            .spawn(move || {
                for reduced in receiver {
                    let result =
                        repository::save(&db, reduced)
                            .map(|()| reduced)
                            .map_err(|error| {
                                eprintln!("event=preference_save_failed error={error}");
                                error.to_string()
                            });
                    let _ = reply_sender.send(result);
                }
            })?;
        *worker = Some(Worker {
            sender,
            replies: Some(replies),
            handle,
        });
        eprintln!("event=module_started module=presence");
        Ok(())
    }
    fn stop(&self, _state: &AppState) -> Result<()> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "Worker de presencia inválido")?;
        let worker = guard.take();
        if let Some(Worker { sender, handle, .. }) = worker {
            drop(sender);
            handle
                .join()
                .map_err(|_| "Worker de preferencias terminó inesperadamente")?;
            eprintln!("event=module_stopped module=presence");
        }
        Ok(())
    }
}

impl PresenceModule {
    pub fn run(&self, state: &AppState) -> Result<()> {
        self.run_with_transcription(state, None)
    }
    pub fn run_with_transcription(
        &self,
        state: &AppState,
        module: Option<&soc_transcription::TranscriptionModule>,
    ) -> Result<()> {
        self.run_with_decisions(state, module, None)
    }
    pub fn run_with_decisions(
        &self,
        state: &AppState,
        module: Option<&soc_transcription::TranscriptionModule>,
        decisions: Option<&soc_decisions::DecisionsModule>,
    ) -> Result<()> {
        self.run_with_llm(state, module, decisions, None)
    }
    pub fn run_with_llm(
        &self,
        state: &AppState,
        module: Option<&soc_transcription::TranscriptionModule>,
        decisions: Option<&soc_decisions::DecisionsModule>,
        llm: Option<&soc_llm::LlmModule>,
    ) -> Result<()> {
        self.run_with_tts(state, module, decisions, llm, None)
    }
    pub fn run_with_tts(
        &self,
        state: &AppState,
        module: Option<&soc_transcription::TranscriptionModule>,
        decisions: Option<&soc_decisions::DecisionsModule>,
        llm: Option<&soc_llm::LlmModule>,
        tts: Option<&soc_tts::TtsModule>,
    ) -> Result<()> {
        desktop::initialize()?;
        let reduced = repository::load(&state.db)?;
        let ui = PresenceWindow::new()?;
        ui.set_transcription_available(module.is_some());
        ui.set_reduced_motion(reduced);
        let source_launchers =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::<std::process::Child>::new()));
        let pending_sources = source_launchers.clone();
        ui.on_open_source(move |url| {
            let url = url.as_str();
            if !(url.starts_with("https://") || url.starts_with("http://"))
                || url.contains(char::is_whitespace)
            {
                return;
            }
            let program = if cfg!(target_os = "macos") {
                "/usr/bin/open"
            } else {
                "xdg-open"
            };
            match std::process::Command::new(program).arg(url).spawn() {
                Ok(child) => pending_sources.borrow_mut().push(child),
                Err(error) => eprintln!("event=source_open_failed error={error}"),
            }
        });
        let history_store = std::rc::Rc::new(history::Store::start(state.db.clone())?);
        let weak_history = ui.as_weak();
        let history_for_open = history_store.clone();
        ui.on_open_history(move || {
            if let Some(ui) = weak_history.upgrade() {
                ui.set_history_notice("Cargando chat…".into());
                ui.set_history_messages(slint::ModelRc::default());
                if let Err(error) = history_for_open.load() {
                    ui.set_history_notice(error.into());
                }
            }
        });
        let weak_history = ui.as_weak();
        let history_for_poll = history_store.clone();
        let history_scroll_generation = std::rc::Rc::new(std::cell::Cell::new(0_u64));
        let history_timer = slint::Timer::default();
        history_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                let mut refresh_chat = false;
                for event in history_for_poll.poll() {
                    match event {
                        history::Event::Loaded(result) => {
                            if let Some(ui) = weak_history.upgrade() {
                                match result {
                                    Ok(snapshot) => {
                                        let prior_offset = ui.get_history_scroll_offset();
                                        let follow_bottom =
                                            prior_offset + ui.get_history_scroll_maximum() <= 12.0;
                                        let items = snapshot
                                            .messages
                                            .iter()
                                            .map(|message| {
                                                let (title, from_user, system) =
                                                    match message.speaker.as_str() {
                                                        "user" => ("Tú", true, false),
                                                        "assistant" => ("SOC", false, false),
                                                        _ => ("Actividad", false, true),
                                                    };
                                                let prepared = if from_user || system {
                                                    None
                                                } else {
                                                    Some(answer::prepare(&message.text))
                                                };
                                                HistoryMessage {
                                                    title: title.into(),
                                                    text: prepared
                                                        .as_ref()
                                                        .map_or_else(
                                                            || message.text.clone(),
                                                            |answer| answer.text.clone(),
                                                        )
                                                        .into(),
                                                    time: history_time(&message.created_at).into(),
                                                    from_user,
                                                    system,
                                                    sources: slint::ModelRc::new(
                                                        slint::VecModel::from(
                                                            prepared.map_or_else(
                                                                Vec::new,
                                                                |answer| {
                                                                    answer
                                                                        .sources
                                                                        .into_iter()
                                                                        .map(|source| ChatSource {
                                                                            label: source
                                                                                .label
                                                                                .into(),
                                                                            url: source.url.into(),
                                                                        })
                                                                        .collect()
                                                                },
                                                            ),
                                                        ),
                                                    ),
                                                }
                                            })
                                            .collect::<Vec<_>>();
                                        ui.set_history_notice(
                                            "Todavía no hay mensajes guardados.".into(),
                                        );
                                        ui.set_history_messages(slint::ModelRc::new(
                                            slint::VecModel::from(items),
                                        ));
                                        let generation =
                                            history_scroll_generation.get().wrapping_add(1);
                                        history_scroll_generation.set(generation);
                                        let current_generation = history_scroll_generation.clone();
                                        let weak_scroll = ui.as_weak();
                                        slint::Timer::single_shot(
                                            std::time::Duration::from_millis(25),
                                            move || {
                                                if current_generation.get() != generation {
                                                    return;
                                                }
                                                if let Some(ui) = weak_scroll.upgrade()
                                                    && ui.get_history_open()
                                                {
                                                    if follow_bottom {
                                                        ui.invoke_scroll_chat_to_bottom();
                                                    } else {
                                                        ui.invoke_set_chat_scroll_offset(
                                                            prior_offset,
                                                        );
                                                    }
                                                }
                                            },
                                        );
                                    }
                                    Err(error) => {
                                        let notice = format!("No se pudo leer el chat: {error}");
                                        ui.set_history_notice(notice.into());
                                        ui.set_history_messages(slint::ModelRc::default());
                                    }
                                }
                            }
                        }
                        history::Event::Saved => refresh_chat = true,
                        history::Event::SaveFailed(error) => {
                            if let Some(ui) = weak_history.upgrade() {
                                ui.set_history_send_status(
                                    format!("No se pudo guardar: {error}").into(),
                                );
                                ui.set_history_open(true);
                            }
                        }
                    }
                }
                if refresh_chat
                    && let Some(ui) = weak_history.upgrade()
                    && ui.get_history_open()
                    && let Err(error) = history_for_poll.load()
                {
                    ui.set_history_notice(format!("No se pudo actualizar el chat: {error}").into());
                }
            },
        );
        let weak = ui.as_weak();
        ui.on_toggle(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_active() && ui.get_speech_active() {
                    return;
                }
                ui.set_active(!ui.get_active());
                if !ui.get_active() {
                    ui.set_history_open(false);
                }
                eprintln!("event=presence_state active={}", ui.get_active());
            }
        });
        let shutdown = state.shutdown.clone();
        #[cfg(target_os = "macos")]
        let drag = std::rc::Rc::new(std::cell::RefCell::new(desktop::DragGesture::default()));
        #[cfg(target_os = "macos")]
        {
            ui.set_manual_drag(true);
            let gesture = drag.clone();
            let weak = ui.as_weak();
            ui.on_begin_drag(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(Err(error)) =
                        desktop::with_drag(&gesture, |drag| drag.begin(ui.window()))
                {
                    eprintln!("event=drag_failed error={error}");
                }
            });
        }
        #[cfg(target_os = "macos")]
        {
            let gesture = drag.clone();
            let weak = ui.as_weak();
            ui.on_move_drag(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(Err(error)) =
                        desktop::with_drag(&gesture, |drag| drag.update(ui.window()))
                {
                    eprintln!("event=drag_failed error={error}");
                }
            });
            let gesture = drag.clone();
            let weak = ui.as_weak();
            ui.on_finish_drag(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(Err(error)) =
                        desktop::with_drag(&gesture, |drag| drag.update(ui.window()))
                {
                    eprintln!("event=drag_failed error={error}");
                }
            });
        }
        let weak = ui.as_weak();
        #[cfg(target_os = "macos")]
        let click_gesture = drag.clone();
        ui.on_face_clicked(move || {
            if let Some(ui) = weak.upgrade() {
                #[cfg(target_os = "macos")]
                {
                    match desktop::with_drag(&click_gesture, |gesture| {
                        gesture.update(ui.window())?;
                        Ok::<_, soc_core::Error>(gesture.was_moved())
                    }) {
                        Some(Ok(false)) => {}
                        Some(Err(error)) => {
                            eprintln!("event=drag_failed error={error}");
                            return;
                        }
                        _ => return,
                    }
                }
                ui.invoke_toggle();
            }
        });
        ui.on_exit(move || {
            shutdown.request();
            let _ = slint::quit_event_loop();
        });
        let (sender, replies) = {
            let mut worker = self.worker.lock().map_err(|_| "Worker inválido")?;
            let worker = worker.as_mut().ok_or("Presencia no iniciada")?;
            (
                worker.sender.clone(),
                worker.replies.take().ok_or("La interfaz ya se ejecutó")?,
            )
        };
        let reply_timer = slint::Timer::default();
        let weak = ui.as_weak();
        reply_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(25),
            move || {
                if let Ok(result) = replies.try_recv()
                    && let Some(ui) = weak.upgrade()
                {
                    ui.set_persist_pending(false);
                    match result {
                        Ok(reduced) => {
                            ui.set_reduced_motion(reduced);
                            ui.set_status_text("Guardado".into());
                        }
                        Err(_) => ui.set_status_text("Error al guardar · ver registro".into()),
                    }
                }
            },
        );
        let weak = ui.as_weak();
        // Reservado para la configuración futura; la UI no invoca este callback.
        ui.on_motion_changed(move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_persist_pending() {
                    return;
                }
                let next = !ui.get_reduced_motion();
                match sender.try_send(next) {
                    Ok(()) => {
                        ui.set_persist_pending(true);
                        ui.set_status_text("Guardando preferencia…".into());
                    }
                    Err(_) => ui.set_status_text("Error al guardar".into()),
                }
            }
        });
        let about = AboutWindow::new()?;
        let weak_about = about.as_weak();
        ui.on_about(move || {
            if let Some(about) = weak_about.upgrade() {
                let _ = about.show();
            }
        });
        let transcription_ui = module
            .map(|module| {
                transcription::TranscriptionUi::new(
                    &ui,
                    module,
                    state,
                    decisions,
                    llm,
                    tts,
                    history_store.writer(),
                )
            })
            .transpose()?;
        #[cfg(target_os = "macos")]
        let status_menu =
            std::rc::Rc::new(std::cell::RefCell::new(None::<status_menu::StatusMenu>));
        #[cfg(target_os = "macos")]
        let menu_events = status_menu.clone();
        let hit_timer = slint::Timer::default();
        let active_sources = source_launchers.clone();
        let weak = ui.as_weak();
        let mut hit_error_logged = false;
        #[cfg(target_os = "macos")]
        let drag_events = drag.clone();
        hit_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            move || {
                active_sources
                    .borrow_mut()
                    .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
                #[cfg(target_os = "macos")]
                if let Some(ui) = weak.upgrade()
                    && let Some(menu) = menu_events.borrow().as_ref()
                {
                    menu.process_events(&ui);
                }
                if let Some(ui) = weak.upgrade() {
                    #[cfg(target_os = "macos")]
                    let Some(captured) = desktop::with_drag(&drag_events, |gesture| {
                        if let Err(error) = gesture.update(ui.window())
                            && !hit_error_logged
                        {
                            eprintln!("event=drag_failed error={error}");
                            hit_error_logged = true;
                        }
                        gesture.captured()
                    }) else {
                        return;
                    };
                    #[cfg(not(target_os = "macos"))]
                    let captured = false;
                    match desktop::update_pointer_state(
                        ui.window(),
                        ui.get_actual_diameter(),
                        &[
                            ui.get_voice_indicator_bounds(),
                            ui.get_decision_indicator_bounds(),
                            ui.get_new_chat_button_bounds(),
                            ui.get_sources_panel_bounds(),
                            ui.get_history_button_bounds(),
                            ui.get_history_panel_bounds(),
                        ],
                        captured,
                    ) {
                        Ok(Some((x, y))) => {
                            ui.set_pointer_gaze_x(x);
                            ui.set_pointer_gaze_y(y);
                        }
                        Err(error) if !hit_error_logged => {
                            eprintln!("event=hit_test_failed error={error}");
                            hit_error_logged = true;
                        }
                        _ => {}
                    }
                }
            },
        );
        ui.show()?;
        let fatal_error = std::rc::Rc::new(std::cell::RefCell::new(None));
        let startup_error = fatal_error.clone();
        #[cfg(target_os = "macos")]
        let startup_menu = status_menu.clone();
        let weak = ui.as_weak();
        slint::spawn_local(async move {
            if let Some(ui) = weak.upgrade() {
                use slint::winit_030::WinitWindowAccessor;
                match ui.window().winit_window().await {
                    Ok(_) => {
                        let prepare = || -> Result<()> {
                            desktop::position(ui.window())?;
                            desktop::install_cursor_tracking(&ui);
                            #[cfg(target_os = "macos")]
                            {
                                *startup_menu.borrow_mut() = Some(status_menu::StatusMenu::new()?);
                                eprintln!("event=status_menu_ready");
                            }
                            Ok(())
                        };
                        if let Err(error) = prepare() {
                            eprintln!("event=window_setup_failed error={error}");
                            *startup_error.borrow_mut() = Some(error.to_string());
                            let _ = slint::quit_event_loop();
                        } else {
                            eprintln!("event=ui_ready");
                        }
                    }
                    Err(error) => {
                        eprintln!("event=window_creation_failed error={error}");
                        *startup_error.borrow_mut() = Some(error.to_string());
                        let _ = slint::quit_event_loop();
                    }
                }
            }
        })?;
        let run_result = ui.run();
        #[cfg(target_os = "macos")]
        desktop::clear_drag_bounds();
        if let Some(transcription) = &transcription_ui {
            transcription.stop();
        }
        hit_timer.stop();
        for mut child in source_launchers.borrow_mut().drain(..) {
            let _ = child.kill();
            let _ = child.wait();
        }
        reply_timer.stop();
        history_timer.stop();
        #[cfg(target_os = "macos")]
        status_menu.borrow_mut().take();
        ui.hide()?;
        about.hide()?;
        run_result?;
        if let Some(error) = fatal_error.borrow_mut().take() {
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;
    use soc_core::AppState;
    use soc_core::{Database, Shutdown};
    use std::sync::Arc;
    fn state() -> AppState {
        AppState {
            config: Arc::new(soc_core_config()),
            db: Database::memory().unwrap(),
            shutdown: Shutdown::default(),
        }
    }
    fn soc_core_config() -> soc_config::Config {
        soc_config::Config {
            database_path: "unused".into(),
            db_busy_timeout_ms: 50,
        }
    }
    #[test]
    fn worker_confirms_commit_and_reports_failure() {
        let state = state();
        let module = PresenceModule::default();
        state
            .db
            .migrate(module.name(), module.migrations())
            .unwrap();
        module.start(&state).unwrap();
        let (sender, replies) = {
            let mut worker = module.worker.lock().unwrap();
            let worker = worker.as_mut().unwrap();
            (worker.sender.clone(), worker.replies.take().unwrap())
        };
        sender.send(true).unwrap();
        assert!(
            replies
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .unwrap()
        );
        assert!(repository::load(&state.db).unwrap());
        state.db.close().unwrap();
        sender.send(false).unwrap();
        assert!(
            replies
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
        drop(sender);
        module.stop(&state).unwrap();
    }
}
