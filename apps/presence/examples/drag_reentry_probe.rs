//! Movimiento nativo y callback reentrante forzado, sin DB, micrófono ni atajos.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../src/desktop.rs"]
mod desktop;
#[cfg(target_os = "macos")]
use gennie_presence::{IndicatorBounds, PresenceWindow, TranscriptionSettings};

#[cfg(target_os = "macos")]
fn main() -> gennie_core::Result<()> {
    use slint::{
        ComponentHandle,
        winit_030::{WinitWindowAccessor, winit::event::WindowEvent},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    desktop::initialize()?;
    let ui = PresenceWindow::new()?;
    let gesture = Rc::new(RefCell::new(desktop::DragGesture::default()));
    let coalesced = Rc::new(Cell::new(0));
    let g = gesture.clone();
    let count = coalesced.clone();
    ui.on_move_drag(move || {
        if desktop::with_drag(&g, |drag| drag.captured()).is_none() {
            count.set(count.get() + 1);
        }
    });
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, event| {
        if matches!(event, WindowEvent::Moved(_))
            && let Some(ui) = weak.upgrade()
        {
            ui.invoke_move_drag();
        }
        slint::winit_030::EventResult::Propagate
    });
    ui.show()?;
    let weak = ui.as_weak();
    let g = gesture.clone();
    let count = coalesced.clone();
    slint::spawn_local(async move {
        let ui = weak.upgrade().unwrap();
        let native = ui.window().winit_window().await.unwrap();
        desktop::position(ui.window()).unwrap();
        let origin = native
            .outer_position()
            .unwrap()
            .to_logical::<f64>(native.scale_factor());
        desktop::with_drag(&g, |drag| {
            drag.begin_at((0.0, 0.0), (origin.x, origin.y));
            for i in 1..=20 {
                drag.apply_sample(ui.window(), (-(i as f64) * 3.0, i as f64 * 3.0), true)
                    .unwrap();
                // Reproducir el callback del trace: la entrega espontánea de Moved
                // depende del gesto físico y aquí puede quedar en cola.
                ui.invoke_move_drag();
            }
            assert!(drag.captured() && drag.was_moved());
        });
        desktop::with_drag(&g, |drag| {
            drag.apply_sample(ui.window(), (-60.0, 60.0), false)
                .unwrap()
        });
        assert!(!g.borrow().captured());
        println!(
            "native_moves=20 reentrant_callbacks_coalesced={}",
            count.get()
        );
        assert!(
            count.get() >= 20,
            "Cada movimiento debe tolerar el callback reentrante"
        );
        slint::quit_event_loop().unwrap();
    })?;
    ui.run()?;
    desktop::clear_drag_bounds();
    ui.hide()?;
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Esta sonda requiere macOS");
}
