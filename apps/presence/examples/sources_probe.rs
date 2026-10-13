//! Captura aislada del globo con fuentes, sin ventana del sistema ni DB.
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalSize, VecModel};
use soc_presence::{ChatBubble, ChatSource, PresenceWindow};
use std::{cell::RefCell, io::Write, rc::Rc};

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

fn main() -> soc_core::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Falta ruta PPM de salida")?;
    slint::platform::set_platform(Box::new(HeadlessPlatform))?;
    let window = WINDOW.with(|window| window.clone());
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    let ui = PresenceWindow::new()?;
    ui.set_active(true);
    ui.set_reduced_motion(true);
    ui.set_chat_bubbles(ModelRc::from(Rc::new(VecModel::from(vec![ChatBubble {
        title: "SOC".into(),
        text: "La serpiente más larga medida fue una pitón reticulada de 7,22 metros. La anaconda verde suele ser más pesada.".into(),
        from_user: false,
    }]))));
    ui.set_chat_sources(ModelRc::from(Rc::new(VecModel::from(vec![
        ChatSource {
            label: "Guinness World Records".into(),
            url: "https://www.guinnessworldrecords.com/".into(),
        },
        ChatSource {
            label: "Museo de Historia Natural de Londres".into(),
            url: "https://www.nhm.ac.uk/".into(),
        },
    ]))));
    let opened = Rc::new(RefCell::new(None::<String>));
    let opened_for_callback = opened.clone();
    ui.on_open_source(move |url| *opened_for_callback.borrow_mut() = Some(url.to_string()));
    ui.show()?;
    let mut pixels = vec![Pixel::default(); (WIDTH * HEIGHT) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in pixels {
        file.write_all(&[pixel.red, pixel.green, pixel.blue])?;
    }
    let bounds = ui.get_sources_panel_bounds();
    println!(
        "sources_panel x={} y={} width={} height={}",
        bounds.x, bounds.y, bounds.width, bounds.height
    );
    let position = LogicalPosition::new(bounds.x + 40.0, bounds.y + 31.0);
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
    assert_eq!(
        opened.borrow().as_deref(),
        Some("https://www.guinnessworldrecords.com/")
    );
    println!("first_source_click=ok");
    ui.hide()?;
    Ok(())
}
