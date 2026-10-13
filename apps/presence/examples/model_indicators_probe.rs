//! Estados de modelos simulados y hover sintético, sin modelos, audio, DB ni atajos.
//! --interactive conserva la ventana 30 s para comprobar el puntero físico.
#[allow(dead_code)]
#[path = "../src/desktop.rs"]
mod desktop;

use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use soc_presence::{IndicatorBounds, PresenceWindow, TranscriptionSettings};
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc, time::Duration};

fn set_models(ui: &PresenceWindow, voice: bool, decision: bool) {
    ui.set_voice_model_loaded(voice);
    ui.set_decision_model_loaded(decision);
    ui.set_voice_model_tooltip(
        format!(
            "Nemotron · reconocimiento de voz\n{}",
            if voice { "Cargado" } else { "Descargado" }
        )
        .into(),
    );
    ui.set_decision_model_tooltip(
        format!(
            "Laya · análisis de órdenes\n{}",
            if decision { "Cargado" } else { "Descargado" }
        )
        .into(),
    );
}

fn snapshot(ui: &PresenceWindow, output: &Path, label: &str) -> Vec<u8> {
    let pixels = ui.window().take_snapshot().expect("Captura de la sonda");
    std::fs::write(output.join(format!("{label}.rgba")), pixels.as_bytes())
        .expect("Guardar píxeles");
    std::fs::write(
        output.join(format!("{label}.size")),
        format!("{} {}", pixels.width(), pixels.height()),
    )
    .expect("Guardar dimensiones");
    println!(
        "snapshot={label} simulated_models=true active={} voice_loaded={} decision_loaded={} voice_bounds={:?} decision_bounds={:?}",
        ui.get_active(),
        ui.get_voice_model_loaded(),
        ui.get_decision_model_loaded(),
        ui.get_voice_indicator_bounds(),
        ui.get_decision_indicator_bounds(),
    );
    pixels.as_bytes().to_vec()
}

fn verify_frames(frames: &BTreeMap<&str, Vec<u8>>) -> soc_core::Result<()> {
    for (before, after, minimum, maximum) in [
        ("sleep_unloaded", "sleep_voice_loaded", 10, usize::MAX),
        (
            "sleep_voice_loaded",
            "sleep_voice_tooltip_synthetic",
            500,
            usize::MAX,
        ),
        (
            "active_both_loaded",
            "active_decision_tooltip_synthetic",
            500,
            usize::MAX,
        ),
        (
            "active_both_loaded",
            "active_tooltip_exited_synthetic",
            0,
            0,
        ),
    ] {
        let first = frames.get(before).ok_or("Falta captura previa")?;
        let second = frames.get(after).ok_or("Falta captura posterior")?;
        if first.len() != second.len() {
            return Err(format!("Dimensiones diferentes entre {before} y {after}").into());
        }
        let changed = first
            .chunks_exact(4)
            .zip(second.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "render_comparison={before}:{after} changed_pixels={changed} expected_min={minimum} expected_max={maximum}"
        );
        if !(minimum..=maximum).contains(&changed) {
            return Err(format!("Cambio visual inesperado entre {before} y {after}").into());
        }
    }
    Ok(())
}

fn synthetic_hover(ui: &PresenceWindow, voice: Option<bool>) {
    use slint::platform::WindowEvent;
    // Se inyecta únicamente en esta ventana; no mueve el cursor del ordenador.
    ui.window().dispatch_event(WindowEvent::PointerExited);
    if let Some(voice) = voice {
        let bounds = if voice {
            ui.get_voice_indicator_bounds()
        } else {
            ui.get_decision_indicator_bounds()
        };
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            ),
        });
    }
    println!("pointer_input=synthetic hover_voice={voice:?}");
}

fn main() -> soc_core::Result<()> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Falta carpeta de capturas")?,
    );
    let interactive = std::env::args().any(|arg| arg == "--interactive");
    std::fs::create_dir_all(&output)?;
    desktop::initialize()?;
    let ui = PresenceWindow::new()?;
    // Eliminar animación decorativa hace comparables las capturas de relleno.
    ui.set_reduced_motion(true);
    set_models(&ui, interactive, interactive);
    ui.set_active(interactive);
    let weak = ui.as_weak();
    ui.on_face_clicked(move || {
        if interactive && let Some(ui) = weak.upgrade() {
            ui.set_active(!ui.get_active());
        }
    });

    let pointer_timer = slint::Timer::default();
    if interactive {
        let weak = ui.as_weak();
        pointer_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                if let Some(ui) = weak.upgrade() {
                    let gaze = desktop::update_pointer_state(
                        ui.window(),
                        ui.get_actual_diameter(),
                        &[
                            ui.get_voice_indicator_bounds(),
                            ui.get_decision_indicator_bounds(),
                        ],
                        false,
                    )
                    .expect("Hit test nativo de la sonda");
                    if let Some((x, y)) = gaze {
                        ui.set_pointer_gaze_x(x);
                        ui.set_pointer_gaze_y(y);
                    }
                }
            },
        );
    } else {
        // Los eventos OS no deben mezclar interacción física con la secuencia.
        // dispatch_event entra directamente en Slint y no pasa por este filtro.
        ui.window().on_winit_window_event(|_, event| {
            use slint::winit_030::{EventResult, winit::event::WindowEvent};
            if matches!(
                event,
                WindowEvent::CursorMoved { .. }
                    | WindowEvent::CursorEntered { .. }
                    | WindowEvent::CursorLeft { .. }
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::Touch(_)
            ) {
                EventResult::PreventDefault
            } else {
                EventResult::Propagate
            }
        });
    }

    ui.show()?;
    let weak = ui.as_weak();
    slint::spawn_local(async move {
        let ui = weak.upgrade().expect("Ventana de la sonda");
        let native = ui.window().winit_window().await.expect("Ventana Winit");
        desktop::position(ui.window()).expect("Posición nativa");
        if interactive {
            desktop::install_cursor_tracking(&ui);
        } else {
            native
                .set_cursor_hittest(false)
                .expect("Desactivar entrada OS en sonda automática");
        }
        native.set_title("soc · model indicators probe");
        println!(
            "probe_ready=true interactive={interactive} native_origin={:?} scale={} pointer_input={}",
            native.inner_position(),
            native.scale_factor(),
            if interactive { "physical" } else { "synthetic" }
        );
    })?;

    let frames: &[(u64, &str)] = if interactive {
        &[
            (600, "interactive_start"),
            (15000, "interactive_middle"),
            (29000, "interactive_end"),
        ]
    } else {
        &[
            (400, "sleep_unloaded"),
            (1100, "sleep_voice_loaded"),
            (2300, "sleep_voice_tooltip_synthetic"),
            (3100, "sleep_both_loaded"),
            (3800, "active_both_loaded"),
            (5000, "active_decision_tooltip_synthetic"),
            (5400, "active_tooltip_exited_synthetic"),
            (6000, "active_unloaded"),
            (6800, "sleep_unloaded_again"),
        ]
    };
    let captures = Rc::new(RefCell::new(BTreeMap::new()));
    for &(time, label) in frames {
        let output = output.clone();
        let weak = ui.as_weak();
        let captures = captures.clone();
        slint::Timer::single_shot(Duration::from_millis(time), move || {
            if let Some(ui) = weak.upgrade() {
                let pixels = snapshot(&ui, &output, label);
                captures.borrow_mut().insert(label, pixels);
            }
        });
    }
    if !interactive {
        for (time, voice, decision, active) in [
            (800, true, false, false),
            (2800, true, true, false),
            (3300, true, true, true),
            (5600, false, false, true),
            (6300, false, false, false),
        ] {
            let weak = ui.as_weak();
            slint::Timer::single_shot(Duration::from_millis(time), move || {
                if let Some(ui) = weak.upgrade() {
                    set_models(&ui, voice, decision);
                    ui.set_active(active);
                }
            });
        }
        for (time, voice) in [
            (1300, Some(true)),
            (2500, None),
            (4000, Some(false)),
            (5200, None),
        ] {
            let weak = ui.as_weak();
            slint::Timer::single_shot(Duration::from_millis(time), move || {
                if let Some(ui) = weak.upgrade() {
                    synthetic_hover(&ui, voice);
                }
            });
        }
    }
    slint::Timer::single_shot(
        Duration::from_millis(if interactive { 30000 } else { 7100 }),
        || slint::quit_event_loop().expect("Cerrar sonda"),
    );
    let result = ui.run();
    pointer_timer.stop();
    #[cfg(target_os = "macos")]
    desktop::clear_drag_bounds();
    ui.hide()?;
    result?;
    println!("probe_closed=true");
    if !interactive {
        verify_frames(&captures.borrow())?;
    }
    Ok(())
}
