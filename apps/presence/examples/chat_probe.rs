//! Secuencia de chat con mensajes de prueba, sin audio/DB/atajos.
//! --decisions intercala los estados independientes de Laya y Nemotron.
#[path = "../src/answer.rs"]
mod answer;
#[path = "../src/chat.rs"]
#[allow(dead_code)]
mod chat;
use gennie_presence::{ChatBubble, ChatSource, PresenceWindow};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
fn main() -> gennie_core::Result<()> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Falta carpeta de capturas")?,
    );
    std::fs::create_dir_all(&output)?;
    let limit = std::env::args().any(|arg| arg == "--limit");
    let decisions = std::env::args().any(|arg| arg == "--decisions");
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let ui = PresenceWindow::new()?;
    ui.set_active(true);
    let messages = Rc::new(RefCell::new(chat::Chat::default()));
    ui.set_chat_bubbles(slint::ModelRc::from(messages.borrow().model.clone()));
    ui.set_chat_sources(slint::ModelRc::from(messages.borrow().sources.clone()));
    let ttl = if limit {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(3)
    };
    messages.borrow_mut().loaded(
        if limit { ttl } else { Duration::from_secs(2) },
        Instant::now(),
    );
    if decisions {
        for (time, text, finished) in [
            (0, "Cargando Laya…", false),
            (350, "Análisis de órdenes listo", true),
            (550, "Analizando la primera orden de prueba…", false),
            (1400, "Primer resultado simulado; sin ejecución", true),
            (1750, "Analizando la segunda orden de prueba…", false),
            (2450, "Segundo resultado simulado; sin ejecución", true),
        ] {
            let messages = messages.clone();
            slint::Timer::single_shot(Duration::from_millis(time), move || {
                messages
                    .borrow_mut()
                    .decision(text, finished, ttl, Instant::now());
            });
        }
        for (time, loaded) in [(800, false), (1100, true)] {
            let messages = messages.clone();
            slint::Timer::single_shot(Duration::from_millis(time), move || {
                let now = Instant::now();
                let mut chat = messages.borrow_mut();
                // Nemotron cambia de estado mientras la decisión conserva su globo activo.
                if loaded {
                    chat.loaded(ttl, now);
                } else {
                    chat.progress("Cargando Nemotron…", ttl, now);
                }
            });
        }
    }
    for (time, text) in [
        (500, "Primer dictado"),
        (1600, "Segundo dictado: aparece debajo del primero"),
        (2600, "Tercer dictado: los anteriores suben"),
    ] {
        let messages = messages.clone();
        slint::Timer::single_shot(Duration::from_millis(time), move || {
            let now = Instant::now();
            let mut chat = messages.borrow_mut();
            chat.begin(ttl, now);
            chat.listening(ttl, now);
            chat.progress("Preparando una sesión de prueba", ttl, now);
            chat.speech("Tú", text, false, ttl, now);
            chat.speech("Tú", text, true, ttl, now);
        });
    }
    if limit {
        let messages = messages.clone();
        slint::Timer::single_shot(Duration::from_millis(3300), move || {
            for text in [
                "Cuarto dictado",
                "Quinto dictado",
                "Sexto dictado",
                "Séptimo dictado",
            ] {
                let now = Instant::now();
                let mut chat = messages.borrow_mut();
                chat.begin(ttl, now);
                chat.speech("Tú", text, true, ttl, now);
            }
            let now = Instant::now();
            let mut chat = messages.borrow_mut();
            chat.begin(ttl, now);
            chat.error("Error de prueba cancelado", ttl, now);
            chat.cancel();
        });
    }
    let clock = slint::Timer::default();
    let expiration = messages.clone();
    let weak = ui.as_weak();
    clock.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(25),
        move || {
            if expiration.borrow_mut().expire(Instant::now())
                && let Some(ui) = weak.upgrade()
            {
                ui.set_active(false);
            }
        },
    );
    for stamp in [400, 1200, 2200, 2700, 2780, 2900, 3200, 4300, 5300, 6300] {
        let weak = ui.as_weak();
        let messages = messages.clone();
        let output = output.clone();
        slint::Timer::single_shot(Duration::from_millis(stamp), move || {
            if let Some(ui) = weak.upgrade()
                && let Ok(pixels) = ui.window().take_snapshot()
            {
                std::fs::write(output.join(format!("{stamp}.rgba")), pixels.as_bytes()).unwrap();
                std::fs::write(
                    output.join(format!("{stamp}.size")),
                    format!("{} {}", pixels.width(), pixels.height()),
                )
                .unwrap();
                println!(
                    "snapshot_ms={stamp} bubbles={} active={} diameter={:.1}",
                    messages.borrow().model.row_count(),
                    ui.get_active(),
                    ui.get_actual_diameter()
                );
            }
        });
    }
    slint::Timer::single_shot(Duration::from_millis(6600), || {
        slint::quit_event_loop().unwrap();
    });
    ui.run()?;
    clock.stop();
    ui.hide()?;
    Ok(())
}
