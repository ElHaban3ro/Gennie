use slint::winit_030::{WinitWindowAccessor, winit};
use soc_core::Result;

pub fn initialize() -> Result<()> {
    let builder = winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    let builder = {
        let mut builder = builder;
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
        builder.with_activate_ignoring_other_apps(false);
        builder
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_event_loop_builder(builder)
        .with_winit_window_attributes_hook(|attributes| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowAttributesExtMacOS;
                attributes.with_accepts_first_mouse(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                attributes
            }
        })
        .select()?;
    Ok(())
}

pub fn prepare(window: &slint::Window) -> Result<()> {
    window.on_winit_window_event(|_, event| {
        if let winit::event::WindowEvent::KeyboardInput { event, .. } = event
            && event.state == winit::event::ElementState::Pressed
            && event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
        {
            let _ = slint::quit_event_loop();
        }
        slint::winit_030::EventResult::Propagate
    });
    Ok(())
}
pub fn position(window: &slint::Window) -> Result<()> {
    window
        .with_winit_window(|native| -> Result<()> {
            native.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowExtMacOS;
                native.set_has_shadow(false);
                configure_spaces(native)?;
            }
            if let Some(monitor) = native
                .current_monitor()
                .or_else(|| native.primary_monitor())
            {
                let scale = monitor.scale_factor();
                let origin = monitor.position();
                let size = monitor.size();
                let window_size = native.outer_size();
                // Reserva 80 px para Dock; ajustar tras evaluación del usuario.
                let margin = (24.0 * scale) as i32;
                let bottom = (80.0 * scale) as i32;
                native.set_outer_position(winit::dpi::PhysicalPosition::new(
                    origin.x + size.width as i32 - window_size.width as i32 - margin,
                    origin.y + size.height as i32 - window_size.height as i32 - bottom,
                ));
                eprintln!(
                    "event=window_positioned scale={scale} monitor_width={} monitor_height={}",
                    size.width, size.height
                );
            }
            Ok(())
        })
        .ok_or("Ventana nativa todavía no disponible")?
}

// Una lectura de cursor, sin event tap ni permisos de accesibilidad.
#[cfg(target_os = "macos")]
pub fn update_hit_test(window: &slint::Window, diameter: f32) -> Result<()> {
    window
        .with_winit_window(|native| -> Result<()> {
            let pointer = objc2_app_kit::NSEvent::mouseLocation();
            let primary = native.primary_monitor().ok_or("No hay monitor principal")?;
            let main_height = primary.size().height as f64 / primary.scale_factor();
            let scale = native.scale_factor();
            let origin = native.inner_position()?.to_logical::<f64>(scale);
            let size = native.inner_size().to_logical::<f64>(scale);
            let local_x = pointer.x - origin.x;
            let local_y = main_height - pointer.y - origin.y;
            let center_x = size.width - 24.0 - diameter as f64 / 2.0;
            let center_y = size.height - 32.0 - diameter as f64 / 2.0;
            let inside =
                contains_circle(local_x, local_y, center_x, center_y, diameter as f64 / 2.0);
            native.set_cursor_hittest(inside)?;
            Ok(())
        })
        .unwrap_or(Ok(()))
}
#[cfg(not(target_os = "macos"))]
pub fn update_hit_test(_window: &slint::Window, _diameter: f32) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn contains_circle(x: f64, y: f64, center_x: f64, center_y: f64, radius: f64) -> bool {
    (x - center_x).powi(2) + (y - center_y).powi(2) <= radius.powi(2)
}
#[cfg(target_os = "macos")]
fn configure_spaces(native: &winit::window::Window) -> Result<()> {
    use objc2_app_kit::{NSView, NSWindowCollectionBehavior as Behavior};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = native.window_handle()?;
    let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
        return Err("Ventana nativa no AppKit".into());
    };
    // SAFETY: Winit garantiza que ns_view es el NSView de este handle vivo.
    // Se usa solo en el hilo principal; ni el préstamo ni el puntero escapan.
    let view = unsafe { raw.ns_view.cast::<NSView>().as_ref() };
    let window = view
        .window()
        .ok_or("La vista de presencia no tiene NSWindow")?;
    // Conservar políticas ajenas; estas alternativas son incompatibles con el overlay.
    let behavior = window.collectionBehavior()
        & !(Behavior::MoveToActiveSpace | Behavior::FullScreenPrimary | Behavior::FullScreenNone);
    window.setCollectionBehavior(
        behavior | Behavior::CanJoinAllSpaces | Behavior::FullScreenAuxiliary,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_hit_test_excludes_transparent_square_corners() {
        assert!(contains_circle(160.0, 184.0, 160.0, 184.0, 40.0));
        assert!(contains_circle(200.0, 184.0, 160.0, 184.0, 40.0));
        assert!(!contains_circle(199.0, 223.0, 160.0, 184.0, 40.0));
        assert!(!contains_circle(12.0, 12.0, 160.0, 184.0, 40.0));
        assert!(contains_circle(116.0, 140.0, 116.0, 140.0, 84.0));
    }
}
