//! Sonda de entrada Slint para vista previa/guardar/deshacer, sin DB ni audio.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../src/appearance.rs"]
mod appearance;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../src/desktop.rs"]
mod desktop;
#[cfg(target_os = "macos")]
use gennie_presence::{AppearanceSettings, IndicatorBounds, PresenceWindow, TranscriptionSettings};
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum PreferenceUpdate {
    Appearance(appearance::Settings),
}

#[cfg(target_os = "macos")]
fn click(window: &slint::Window, x: f32, y: f32) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

#[cfg(target_os = "macos")]
async fn settle() {
    let ready = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut scheduled = false;
    std::future::poll_fn(|cx| {
        if ready.get() {
            return std::task::Poll::Ready(());
        }
        if !scheduled {
            scheduled = true;
            let ready = ready.clone();
            let waker = cx.waker().clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
                ready.set(true);
                waker.wake();
            });
        }
        std::task::Poll::Pending
    })
    .await
}

#[cfg(target_os = "macos")]
fn main() -> gennie_core::Result<()> {
    use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
    desktop::initialize()?;
    let host = PresenceWindow::new()?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(8);
    let ui = appearance::Ui::new(&host, Default::default(), sender)?;
    ui.window.show()?;
    let tested = ui.clone();
    let verdict = std::rc::Rc::new(std::cell::Cell::new(false));
    let passed = verdict.clone();
    slint::spawn_local(async move {
        let native = tested.window.window().winit_window().await.unwrap();
        native.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop);
        native.focus_window();
        settle().await;
        if std::env::args().any(|arg| arg == "--inspect") {
            let pixels = tested.window.window().take_snapshot().unwrap();
            std::fs::write("/private/tmp/gennie-controls-ui.rgba", pixels.as_bytes()).unwrap();
            std::fs::write(
                "/private/tmp/gennie-controls-ui.size",
                format!("{} {}", pixels.width(), pixels.height()),
            )
            .unwrap();
            passed.set(true);
            let _ = slint::quit_event_loop();
            return;
        }
        // Coordenadas del panel de 480×760; entrada directa al mismo árbol Slint.
        click(tested.window.window(), 282.0, 120.0);
        settle().await;
        let preview = tested.window.get_blur_radius().round() as i32;
        println!("slider_preview_blur={preview} expected=18");
        let mut ok = preview == 18 && tested.current().blur == 18.0;
        click(tested.window.window(), 390.0, 628.0);
        settle().await;
        if let Ok(PreferenceUpdate::Appearance(saved)) = receiver.try_recv() {
            ok &= saved.blur_radius == 18 && tested.window.get_saving();
            // La persistencia real y el commit se prueban en repository/worker.
            tested.saved(saved, Ok(()));
            click(tested.window.window(), 445.0, 120.0);
            settle().await;
            ok &= tested.window.get_blur_radius() > 18.0;
            click(tested.window.window(), 94.0, 628.0);
            settle().await;
            ok &= tested.window.get_blur_radius() == 18.0 && tested.current().blur == 18.0;
            click(tested.window.window(), 244.0, 628.0);
            settle().await;
            ok &= tested.window.get_blur_radius() == 0.0 && tested.current().blur == 0.0;
        } else {
            ok = false;
        }
        // La demostración modifica solo el renderer y se cancela al editar.
        let selected_summary = tested.window.get_values();
        let selected_blur = tested.current().blur;
        tested.window.invoke_preview_transition();
        ok &= tested.current().blur != selected_blur;
        ok &= tested.window.get_values() == selected_summary;
        tested.window.invoke_preview();
        ok &= tested.current().blur == selected_blur;
        tested.window.invoke_preview_transition();
        for _ in 0..12 {
            settle().await;
        }
        ok &= tested.current().blur == selected_blur;
        ok &= tested.window.get_values() == selected_summary;
        ok &= receiver.try_recv().is_err();
        println!("preview_save_undo_reset_transition={ok} expected=true");
        passed.set(ok);
        let _ = slint::quit_event_loop();
    })?;
    slint::run_event_loop()?;
    ui.window.hide()?;
    if !verdict.get() {
        std::process::exit(1);
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Esta sonda requiere macOS");
}
