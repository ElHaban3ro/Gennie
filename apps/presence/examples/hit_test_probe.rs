//! Superficie aislada de verificación manual, nunca parte del binario soc.
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
slint::slint! {
    export component Probe inherits Window {
        title: "soc · click-through probe"; no-frame: true;
        width: 224px; height: 256px;
        in-out property <int> clicks: 0;
        background: #dedede;
        Text { text: "Clics recibidos: " + root.clicks; horizontal-alignment: center; vertical-alignment: center; color: #202020; }
        TouchArea { accessible-role: button; accessible-label: "Superficie de prueba"; clicked => {root.clicks+=1;} }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        .with_winit_event_loop_builder(builder)
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let probe = Probe::new()?;
    probe.show()?;
    let weak = probe.as_weak();
    slint::spawn_local(async move {
        if let Some(probe) = weak.upgrade()
            && let Ok(native) = probe.window().winit_window().await
            && let Some(monitor) = native.primary_monitor()
        {
            let scale = monitor.scale_factor();
            let origin = monitor.position();
            let size = monitor.size();
            // Alinea el contenido con el viewport de presence, sin contar marco.
            let inner = native.inner_size();
            let desired = winit::dpi::PhysicalPosition::new(
                origin.x + size.width as i32 - inner.width as i32 - (24.0 * scale) as i32,
                origin.y + size.height as i32 - inner.height as i32 - (80.0 * scale) as i32,
            );
            let outer = native.outer_position().unwrap();
            let actual = native.inner_position().unwrap();
            native.set_outer_position(winit::dpi::PhysicalPosition::new(
                desired.x - (actual.x - outer.x),
                desired.y - (actual.y - outer.y),
            ));
        }
    })?;
    probe.run()?;
    Ok(())
}
