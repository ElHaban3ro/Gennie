// Sonda de presentación con texto de una prueba ASR, sin micrófono ni DB.
use gennie_presence::{ChatBubble, PresenceWindow};
use slint::ComponentHandle;
use slint::Model;
use std::{path::PathBuf, time::Duration};
fn main() -> gennie_core::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let text = std::fs::read_to_string(args.next().ok_or("Falta texto de prueba")?)?;
    if text.chars().count() > 360 {
        return Err("Texto de sonda demasiado largo".into());
    }
    let output = PathBuf::from(args.next().ok_or("Falta carpeta de capturas")?);
    std::fs::create_dir_all(&output)?;
    let mut builder = slint::winit_030::winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::winit::platform::macos::{
            ActivationPolicy, EventLoopBuilderExtMacOS,
        };
        builder
            .with_activation_policy(ActivationPolicy::Accessory)
            .with_activate_ignoring_other_apps(false);
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_event_loop_builder(builder)
        .select()?;
    let ui = PresenceWindow::new()?;
    let model = std::rc::Rc::new(slint::VecModel::default());
    ui.set_chat_bubbles(slint::ModelRc::from(model.clone()));
    let first = model.clone();
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(1000), move || {
        if weak.upgrade().is_some() {
            first.push(ChatBubble {
                from_user: true,
                title: "Tú".into(),
                text: text.into(),
            });
        }
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(2500), move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_active(true);
        }
    });
    let clear = model.clone();
    slint::Timer::single_shot(Duration::from_millis(5000), move || {
        clear.set_vec(Vec::new());
    });
    for time in [900, 1030, 1100, 1250, 4500, 5040, 5110, 5250] {
        let weak = ui.as_weak();
        let output = output.clone();
        let model = model.clone();
        slint::Timer::single_shot(Duration::from_millis(time), move || {
            if let Some(ui) = weak.upgrade()
                && let Ok(pixels) = ui.window().take_snapshot()
            {
                let _ = std::fs::write(output.join(format!("{time}.rgba")), pixels.as_bytes());
                let _ = std::fs::write(
                    output.join(format!("{time}.size")),
                    format!("{} {}", pixels.width(), pixels.height()),
                );
                println!("snapshot_ms={time} visible={}", model.row_count() > 0);
            }
        });
    }
    slint::Timer::single_shot(Duration::from_millis(5500), move || {
        let _ = slint::quit_event_loop();
    });
    ui.run()?;
    Ok(())
}
