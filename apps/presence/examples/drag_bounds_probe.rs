//! Sonda nativa de límites del marco transparente; no abre DB ni captura audio.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../src/desktop.rs"]
mod desktop;

#[cfg(target_os = "macos")]
use soc_presence::{IndicatorBounds, PresenceWindow, TranscriptionSettings};

#[cfg(target_os = "macos")]
slint::slint! { export component Probe inherits Window { width: 400px; height: 460px; no-frame: true; background: transparent; } }

#[cfg(target_os = "macos")]
fn main() -> soc_core::Result<()> {
    if std::env::args().any(|arg| arg == "--interactive")
        || std::env::var("SOC_TEST_DRAG_INTERACTIVE").as_deref() == Ok("1")
    {
        return interactive();
    }
    use slint::winit_030::winit::{
        dpi::LogicalPosition,
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
    };
    use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

    desktop::initialize()?;
    let ui = Probe::new()?;
    let other = Probe::new()?;
    ui.show()?;
    other.show()?;
    let weak = ui.as_weak();
    let weak_other = other.as_weak();
    let verdict = std::rc::Rc::new(std::cell::Cell::new(false));
    let result = verdict.clone();
    slint::spawn_local(async move {
        let ui = weak.upgrade().unwrap();
        let native = ui.window().winit_window().await.unwrap();
        let baseline = std::env::args().any(|arg| arg == "--baseline");
        if !baseline {
            desktop::position(ui.window()).unwrap();
            // Comprobar también que la configuración sea idempotente.
            desktop::position(ui.window()).unwrap();
        }
        let handle = native.window_handle().unwrap();
        let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
            panic!("AppKit requerido")
        };
        // SAFETY: handle Winit vivo, acceso en main; el préstamo no escapa.
        let view = unsafe { raw.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
        let window = view.window().unwrap();
        let screen = window.screen().unwrap();
        let screen_frame = screen.frame();
        let diameter = 80.0;
        let mut wanted = window.frame();
        wanted.origin.y = screen_frame.origin.y + screen_frame.size.height - 32.0 - diameter;
        let constrained = window.constrainFrameRect_toScreen(wanted, Some(&screen));
        let face_top = screen_frame.origin.y + screen_frame.size.height
            - constrained.origin.y
            - 32.0
            - diameter;
        println!(
            "minimum_face_top={face_top} expected=0 window_height={}",
            wanted.size.height
        );
        if !baseline {
            let monitor = native.current_monitor().unwrap();
            let scale = native.scale_factor();
            let screen_top = monitor.position().to_logical::<f64>(scale).y;
            let transparent_top = wanted.size.height - 32.0 - diameter;
            native.set_outer_position(LogicalPosition::new(400.0, screen_top - transparent_top));
            let actual = native.outer_position().unwrap().to_logical::<f64>(scale);
            let positioned_top = actual.y - screen_top + transparent_top;
            // Reproducir la secuencia del nuevo controlador, no solo un set de posición.
            native.set_outer_position(LogicalPosition::new(400.0, screen_top + 400.0));
            let mut gesture = desktop::DragGesture::default();
            let origin = native.outer_position().unwrap().to_logical::<f64>(scale);
            let start_pointer = (736.0, -(origin.y + 388.0));
            gesture.begin_at(start_pointer, (origin.x, origin.y));
            let mut gesture_passed = true;
            for pointer_y in [screen_top + 500.0, screen_top + 300.0, screen_top + 40.0] {
                gesture
                    .apply_sample(ui.window(), (736.0, -pointer_y), true)
                    .unwrap();
                let moved = native.outer_position().unwrap().to_logical::<f64>(scale);
                let expected_face_top = pointer_y - screen_top - 40.0;
                let actual_face_top = moved.y - screen_top + transparent_top;
                println!("drag_sample_face_top={actual_face_top} expected={expected_face_top}");
                gesture_passed &=
                    (actual_face_top - expected_face_top).abs() < 1.0 && gesture.captured();
            }
            gesture
                .apply_sample(ui.window(), (736.0, -(screen_top + 40.0)), false)
                .unwrap();
            gesture_passed &= gesture.was_moved() && !gesture.captured();
            let origin = native.outer_position().unwrap().to_logical::<f64>(scale);
            gesture.begin_at((736.0, -(screen_top + 40.0)), (origin.x, origin.y));
            gesture
                .apply_sample(ui.window(), (736.0, -(screen_top + 40.0)), false)
                .unwrap();
            gesture_passed &= !gesture.was_moved() && !gesture.captured();
            println!(
                "positioned_face_top={positioned_top} expected=0 window_top={}",
                actual.y
            );
            // La posición de Slint puede ser correcta mientras AppKit limita
            // solo su hija de vidrio. Comprobar ambas superficies reales.
            let surface = desktop::NativeFaceSurface::new(ui.window(), 128.0)
                .unwrap()
                .unwrap();
            let children = window.childWindows().expect("Ventana hija de la cara");
            assert_eq!(children.len(), 1);
            let child = children.objectAtIndex(0);
            let mut surface_passed = true;
            wait_for_appkit().await;
            for diameter in [64.0_f32, 128.0] {
                let transparent_top = window.frame().size.height - 32.0 - f64::from(diameter);
                for face_top in [200.0, 120.0, 80.0, 40.0, 0.0, 40.0, 120.0, 200.0] {
                    native.set_outer_position(LogicalPosition::new(
                        400.0,
                        screen_top + face_top - transparent_top,
                    ));
                    surface.update(diameter);
                    // AppKit puede reajustar la hija al procesar el movimiento.
                    wait_for_appkit().await;
                    surface.update(diameter);
                    wait_for_appkit().await;
                    let parent = window.frame();
                    let actual = child.frame();
                    let offset_x = actual.origin.x + actual.size.width
                        - (parent.origin.x + parent.size.width - 24.0);
                    let offset_y = actual.origin.y - (parent.origin.y + 32.0);
                    println!(
                        "native_face_diameter={diameter} top={face_top} offset=({offset_x},{offset_y}) expected=(0,0)"
                    );
                    surface_passed &= offset_x.abs() < 1.0 && offset_y.abs() < 1.0;
                }
            }
            drop(surface);
            wait_for_appkit().await;
            let other = weak_other.upgrade().unwrap();
            let other_native = other.window().winit_window().await.unwrap();
            let other_handle = other_native.window_handle().unwrap();
            let RawWindowHandle::AppKit(other_raw) = other_handle.as_raw() else {
                panic!("AppKit requerido")
            };
            // SAFETY: segundo handle vivo en main, sin escapar el préstamo.
            let other_view = unsafe { other_raw.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
            let other_window = other_view.window().unwrap();
            let other_constrained = other_window.constrainFrameRect_toScreen(wanted, Some(&screen));
            let other_top = screen_frame.origin.y + screen_frame.size.height
                - other_constrained.origin.y
                - 32.0
                - diameter;
            desktop::clear_drag_bounds();
            let restored = window.constrainFrameRect_toScreen(wanted, Some(&screen));
            let restored_top = screen_frame.origin.y + screen_frame.size.height
                - restored.origin.y
                - 32.0
                - diameter;
            println!(
                "other_window_face_top={other_top} restored_face_top={restored_top} expected_positive"
            );
            result.set(
                face_top.abs() < 1.0
                    && positioned_top.abs() < 1.0
                    && gesture_passed
                    && surface_passed
                    && other_top > 1.0
                    && restored_top > 1.0,
            );
        } else {
            result.set(face_top.abs() < 1.0);
        }
        let _ = slint::quit_event_loop();
    })?;
    ui.run()?;
    desktop::clear_drag_bounds();
    ui.hide()?;
    other.hide()?;
    if !verdict.get() {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
async fn wait_for_appkit() {
    let ready = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut scheduled = false;
    std::future::poll_fn(|context| {
        if ready.get() {
            return std::task::Poll::Ready(());
        }
        if !scheduled {
            scheduled = true;
            let ready = ready.clone();
            let waker = context.waker().clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
                ready.set(true);
                waker.wake();
            });
        }
        std::task::Poll::Pending
    })
    .await
}

#[cfg(target_os = "macos")]
fn interactive() -> soc_core::Result<()> {
    use soc_core::{AppModule, AppState, Database, Shutdown};
    let state = AppState {
        config: std::sync::Arc::new(soc_config::Config {
            database_path: "unused".into(),
            db_busy_timeout_ms: 50,
        }),
        db: Database::memory()?,
        shutdown: Shutdown::default(),
    };
    let presence = soc_presence::PresenceModule::default();
    state.db.migrate(presence.name(), presence.migrations())?;
    presence.start(&state)?;
    // Observación de ventanas propias; no datos, permisos ni instrumentación en soc.
    let observer = slint::Timer::default();
    let mut last = None;
    let log_path = std::env::var_os("SOC_TEST_DRAG_LOG").map(std::path::PathBuf::from);
    observer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            let mtm = objc2::MainThreadMarker::new().unwrap();
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            for window in app.windows().iter() {
                if window.title().to_string() == "soc"
                    && let Some(screen) = window.screen()
                {
                    let frame = window.frame();
                    let screen = screen.frame();
                    let position = (
                        frame.origin.x.round() as i64,
                        (screen.origin.y + screen.size.height - frame.origin.y - frame.size.height)
                            .round() as i64,
                    );
                    if last != Some(position) {
                        println!("probe_window_origin={},{}", position.0, position.1);
                        if let Some(path) = &log_path {
                            use std::io::Write;
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                            {
                                let _ = writeln!(file, "{},{}", position.0, position.1);
                            }
                        }
                        last = Some(position);
                    }
                }
            }
        },
    );
    let result = presence.run(&state);
    observer.stop();
    presence.stop(&state)?;
    state.db.close()?;
    result
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Esta sonda requiere macOS");
}
