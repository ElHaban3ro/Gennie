//! Captura del panel real de Slint, sin ventana del sistema ni datos personales.
use gennie_presence::{ChatSource, HistoryMessage, PresenceWindow};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalSize, VecModel};
use std::{cell::Cell, io::Write, rc::Rc};

const WIDTH: u32 = 568;
const HEIGHT: u32 = 980;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
}
struct HeadlessPlatform;
impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|window| window.clone()))
    }
}
#[derive(Clone, Copy)]
struct Pixel {
    red: u8,
    green: u8,
    blue: u8,
}
impl Default for Pixel {
    fn default() -> Self {
        Self {
            red: 238,
            green: 238,
            blue: 238,
        }
    }
}
impl TargetPixel for Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let keep = 255u32 - color.alpha as u32;
        self.red = (color.red as u32 + self.red as u32 * keep / 255).min(255) as u8;
        self.green = (color.green as u32 + self.green as u32 * keep / 255).min(255) as u8;
        self.blue = (color.blue as u32 + self.blue as u32 * keep / 255).min(255) as u8;
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

fn click(ui: &PresenceWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn main() -> gennie_core::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Falta ruta PPM de salida")?;
    slint::platform::set_platform(Box::new(HeadlessPlatform))?;
    let window = WINDOW.with(|window| window.clone());
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    let ui = PresenceWindow::new()?;
    ui.set_reduced_motion(true);
    ui.set_active(true);
    ui.set_llm_available(true);
    ui.set_history_open(true);
    ui.on_open_history(|| {});
    let new_chat_calls = Rc::new(Cell::new(0));
    let calls = new_chat_calls.clone();
    ui.on_new_chat(move || calls.set(calls.get() + 1));
    let send_calls = Rc::new(Cell::new(0));
    let sent = send_calls.clone();
    ui.on_send_chat(move |text| {
        sent.set(sent.get() + 1);
        text.as_str() == "Mensaje de prueba"
    });
    let source_calls = Rc::new(Cell::new(0));
    let sources = source_calls.clone();
    ui.on_open_source(move |url| {
        assert_eq!(url.as_str(), "https://example.com/aves");
        sources.set(sources.get() + 1);
    });
    ui.show()?;
    let mut pixels = vec![Pixel::default(); (WIDTH * HEIGHT) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    // La lectura de SQLite llega después de abrir el cajón; reproducir esa actualización.
    let sample_messages = vec![
        HistoryMessage { title: "Tú".into(), time: "13/10 · 10:24 UTC".into(), text: "¿Cuál es el ave más grande del mundo?".into(), from_user: true, system: false, sources: ModelRc::default() },
        HistoryMessage { title: "Gennie".into(), time: "13/10 · 10:24 UTC".into(), text: "Por masa y altura, es el avestruz. Puede alcanzar unos 2,8 metros y pesar más de 150 kg.".into(), from_user: false, system: false, sources: ModelRc::default() },
        HistoryMessage { title: "Actividad".into(), time: "".into(), text: "Nuevo chat iniciado".into(), from_user: false, system: true, sources: ModelRc::default() },
        HistoryMessage { title: "Tú".into(), time: "13/10 · 10:26 UTC".into(), text: "¿Y cuál tiene mayor envergadura?".into(), from_user: true, system: false, sources: ModelRc::default() },
        HistoryMessage { title: "Gennie".into(), time: "13/10 · 10:26 UTC".into(), text: "El albatros viajero tiene la mayor envergadura entre las aves vivas, con registros de más de 3,5 metros.".into(), from_user: false, system: false, sources: ModelRc::new(VecModel::from(vec![ChatSource { label: "Aves del mundo".into(), url: "https://example.com/aves".into() }])) },
    ];
    ui.set_history_messages(ModelRc::new(VecModel::from(sample_messages.clone())));
    // Slint calcula la altura final durante el siguiente render, como en la app.
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    ui.invoke_scroll_chat_to_bottom();
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    let offset = ui.get_history_scroll_offset();
    let maximum = ui.get_history_scroll_maximum();
    println!("initial_scroll_offset={offset} maximum={maximum}");
    assert!(maximum > 0.0, "La muestra debe exceder el alto visible");
    assert!(
        (offset + maximum).abs() < 1.0,
        "Chat no abre en el último mensaje"
    );
    click(&ui, 160.0, 684.0);
    assert_eq!(
        source_calls.get(),
        1,
        "La fuente guardada no recibió el clic"
    );
    let track = ui.get_history_scroll_track_bounds();
    let thumb = ui.get_history_scroll_thumb_bounds();
    assert!(
        thumb.y >= track.y && thumb.y + thumb.height <= track.y + track.height + 1.0,
        "La barra del chat sale del carril al abrir"
    );
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in &pixels {
        file.write_all(&[pixel.red, pixel.green, pixel.blue])?;
    }
    ui.window().dispatch_event(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(250.0, 500.0),
        delta_x: 0.0,
        delta_y: 220.0,
    });
    assert!(
        ui.get_history_scroll_offset().abs() < 1.0,
        "No se puede subir al inicio del chat"
    );
    let thumb = ui.get_history_scroll_thumb_bounds();
    assert!(
        thumb.y >= track.y && thumb.y + thumb.height <= track.y + track.height + 1.0,
        "La barra del chat sale del carril al subir"
    );
    ui.window().dispatch_event(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(250.0, 500.0),
        delta_x: 0.0,
        delta_y: -220.0,
    });
    assert!(
        (ui.get_history_scroll_offset() + maximum).abs() < 1.0,
        "No se puede bajar al final del chat"
    );
    let thumb = ui.get_history_scroll_thumb_bounds();
    assert!(
        thumb.y >= track.y && thumb.y + thumb.height <= track.y + track.height + 1.0,
        "La barra del chat sale del carril al bajar"
    );
    let mut updated_messages = sample_messages;
    updated_messages.push(HistoryMessage {
        title: "Tú".into(),
        time: "13/10 · 10:27 UTC".into(),
        text: "Gracias, sigue con la explicación.".into(),
        from_user: true,
        system: false,
        sources: ModelRc::default(),
    });
    ui.set_history_messages(ModelRc::new(VecModel::from(updated_messages)));
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    ui.invoke_scroll_chat_to_bottom();
    assert!(ui.get_history_open(), "Una respuesta nueva cerró el chat");
    let updated_maximum = ui.get_history_scroll_maximum();
    assert!(
        updated_maximum > maximum,
        "La conversación nueva no amplió el chat"
    );
    assert!(
        (ui.get_history_scroll_offset() + updated_maximum).abs() < 1.0,
        "El chat no siguió el mensaje nuevo"
    );
    let panel = ui.get_history_panel_bounds();
    assert_eq!(panel.width, 320.0);
    assert_eq!(panel.height, 488.0);
    assert!((panel.x + panel.width - 400.0).abs() < 1.0);
    assert!((panel.y + panel.height - (HEIGHT as f32 - 32.0 - 168.0 + 20.0)).abs() < 1.0);
    ui.set_history_draft("Mensaje de prueba".into());
    click(
        &ui,
        panel.x + panel.width - 40.0,
        panel.y + panel.height - 40.0,
    );
    assert_eq!(send_calls.get(), 1, "El botón no envió el texto");
    assert_eq!(
        ui.get_history_draft().as_str(),
        "",
        "No se limpió el texto enviado"
    );
    ui.set_history_draft("Conservar borrador".into());
    click(
        &ui,
        panel.x + panel.width - 40.0,
        panel.y + panel.height - 40.0,
    );
    assert_eq!(send_calls.get(), 2, "El segundo envío no llegó al callback");
    assert_eq!(
        ui.get_history_draft().as_str(),
        "Conservar borrador",
        "Se perdió el texto tras fallar"
    );
    click(&ui, panel.x + panel.width - 32.0, panel.y + 32.0);
    assert!(!ui.get_history_open(), "El botón cerrar no cerró el panel");
    let button = ui.get_history_button_bounds();
    click(
        &ui,
        button.x + button.width / 2.0,
        button.y + button.height / 2.0,
    );
    assert!(ui.get_history_open(), "El reloj no reabrió el panel");
    click(&ui, panel.x + panel.width - 121.0, panel.y + 32.0);
    assert!(ui.get_history_open(), "Nuevo chat cerró el panel");
    assert_eq!(
        new_chat_calls.get(),
        1,
        "Nuevo chat no usó el callback existente"
    );
    println!("history_panel=rendered close=ok reopen=ok new_chat=ok bounds={panel:?}");
    ui.hide()?;
    Ok(())
}
