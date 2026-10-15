//! Verifica geometría y relleno de los tres indicadores sin ventana nativa ni modelos.
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize};
use soc_presence::PresenceWindow;
use std::{io::Write, rc::Rc};

const WIDTH: u32 = 568;
const HEIGHT: u32 = 980;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}
struct HeadlessPlatform;
impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|window| window.clone()))
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Pixel {
    red: u8,
    green: u8,
    blue: u8,
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
fn render(window: &MinimalSoftwareWindow) -> Vec<Pixel> {
    let mut pixels = vec![Pixel::from_rgb(238, 238, 238); (WIDTH * HEIGHT) as usize];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH as usize);
    });
    pixels
}
fn main() -> soc_core::Result<()> {
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
    ui.show()?;

    let voice = ui.get_voice_indicator_bounds();
    let decision = ui.get_decision_indicator_bounds();
    let tts = ui.get_tts_indicator_bounds();
    let chat = ui.get_new_chat_button_bounds();
    assert!(voice.x + voice.width < decision.x);
    assert!(decision.x + decision.width < tts.x);
    assert!(tts.x + tts.width < chat.x);
    assert_eq!(voice.y, decision.y);
    assert_eq!(decision.y, tts.y);

    let before = render(&window);
    ui.set_tts_model_loaded(true);
    let after = render(&window);
    let changed_in_icon = (tts.y as usize..(tts.y + tts.height) as usize)
        .flat_map(|y| {
            (tts.x as usize..(tts.x + tts.width) as usize).map(move |x| y * WIDTH as usize + x)
        })
        .filter(|&index| before[index] != after[index])
        .count();
    assert!(
        changed_in_icon > 10,
        "El altavoz no refleja la carga de Kokoro"
    );

    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in after {
        file.write_all(&[pixel.red, pixel.green, pixel.blue])?;
    }
    println!(
        "three_indicators=ok tts_fill_changed_pixels={changed_in_icon} voice={voice:?} decision={decision:?} tts={tts:?} chat={chat:?}"
    );
    ui.hide()?;
    Ok(())
}
