//! Sonda aislada de vidrio nativo con tres globos, sin DB, audio ni red.
#[cfg(target_os = "macos")]
use soc_presence::{ChatBubble, IndicatorBounds, PresenceWindow, TranscriptionSettings};

#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../src/desktop.rs"]
mod desktop;

#[cfg(target_os = "macos")]
fn main() -> soc_core::Result<()> {
    use slint::{
        ComponentHandle, Model, ModelRc, VecModel,
        winit_030::{WinitWindowAccessor, winit::dpi::LogicalPosition},
    };
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc, time::Duration};

    desktop::initialize()?;
    let ui = PresenceWindow::new()?;
    ui.set_reduced_motion(true);
    ui.set_active(true);
    let model = Rc::new(VecModel::from(vec![
        ChatBubble {
            title: "Tú".into(),
            text: "Hola, ¿cómo estás?".into(),
            from_user: true,
        },
        ChatBubble {
            title: "SOC".into(),
            text: "Pensando…".into(),
            from_user: false,
        },
        ChatBubble {
            title: "Transcripción".into(),
            text: "Se recibió audio durante 0.4 s, pero no se reconocieron palabras".into(),
            from_user: false,
        },
    ]));
    ui.set_chat_bubbles(ModelRc::from(model));
    let bounds = Rc::new(RefCell::new(BTreeMap::<usize, IndicatorBounds>::new()));
    let reported = bounds.clone();
    ui.on_bubble_geometry(move |index, x, y, width, height| {
        reported.borrow_mut().insert(
            index as usize,
            IndicatorBounds {
                x,
                y,
                width,
                height,
            },
        );
    });
    let bubble = Rc::new(RefCell::new(None::<desktop::NativeBubbleGlass>));
    let face = Rc::new(RefCell::new(None::<desktop::NativeFaceSurface>));
    let weak = ui.as_weak();
    let startup_bubble = bubble.clone();
    let startup_face = face.clone();
    let move_probe = std::env::args().any(|arg| arg == "--move");
    slint::spawn_local(async move {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.window().winit_window().await;
            desktop::position(ui.window()).expect("Posición de sonda");
            if let Some(glass) =
                desktop::NativeFaceSurface::new(ui.window(), ui.get_actual_diameter())
                    .expect("Superficie de cara")
            {
                *startup_face.borrow_mut() = Some(glass);
                ui.set_native_face_glass(true);
            }
            if let Some(glass) =
                desktop::NativeBubbleGlass::new(ui.window(), ui.get_bubble_viewport_bounds())
                    .expect("Vidrio de globos")
            {
                *startup_bubble.borrow_mut() = Some(glass);
                ui.set_native_bubble_glass(true);
            }
            println!("native_bubble_probe_ready");
            if move_probe {
                let weak = ui.as_weak();
                slint::Timer::single_shot(Duration::from_millis(500), move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.window().with_winit_window(|native| {
                            native.set_outer_position(LogicalPosition::new(1328.0, 450.0));
                        });
                        println!("native_bubble_probe_moved_down");
                    }
                });
                let weak = ui.as_weak();
                slint::Timer::single_shot(Duration::from_millis(1500), move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.window().with_winit_window(|native| {
                            native.set_outer_position(LogicalPosition::new(1328.0, -400.0));
                        });
                        println!("native_bubble_probe_moved_up");
                    }
                });
            }
        }
    })?;
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let closing_bubble = bubble.clone();
    let closing_face = face.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(glass) = face.borrow().as_ref() {
                glass.update(ui.get_actual_diameter());
            }
            if let Some(glass) = bubble.borrow_mut().as_mut() {
                let count = ui.get_chat_bubbles().row_count();
                let rects = bounds
                    .borrow()
                    .iter()
                    .map(|(index, rect)| (*index, rect.clone()))
                    .collect::<Vec<_>>();
                glass.update(ui.get_bubble_viewport_bounds(), &rects, count, false);
            }
        },
    );
    slint::Timer::single_shot(Duration::from_secs(30), move || {
        closing_bubble.borrow_mut().take();
        closing_face.borrow_mut().take();
        let _ = slint::quit_event_loop();
    });
    ui.run()?;
    timer.stop();
    ui.hide()?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Esta sonda requiere macOS");
}
