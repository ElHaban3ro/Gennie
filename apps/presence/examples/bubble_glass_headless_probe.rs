//! Comprueba los rectángulos de globos que Slint entrega al vidrio nativo.
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, ModelRc, PhysicalSize, VecModel};
use soc_presence::{ChatBubble, PresenceWindow};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
}

struct HeadlessPlatform;
impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|window| window.clone()))
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Pixel {
    red: u8,
    green: u8,
    blue: u8,
    blends: u16,
}
impl TargetPixel for Pixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        self.blends += u16::from(color.alpha > 0);
        let keep = 255u32 - color.alpha as u32;
        self.red = (color.red as u32 + self.red as u32 * keep / 255).min(255) as u8;
        self.green = (color.green as u32 + self.green as u32 * keep / 255).min(255) as u8;
        self.blue = (color.blue as u32 + self.blue as u32 * keep / 255).min(255) as u8;
    }
    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            blends: 0,
        }
    }
}

fn render() -> Vec<Pixel> {
    WINDOW.with(|window| {
        let mut pixels = vec![Pixel::from_rgb(200, 200, 200); 568 * 980];
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 568);
        });
        pixels
    })
}

fn main() -> soc_core::Result<()> {
    slint::platform::set_platform(Box::new(HeadlessPlatform))?;
    WINDOW.with(|window| window.set_size(PhysicalSize::new(568, 980)));
    let ui = PresenceWindow::new()?;
    ui.set_reduced_motion(true);
    ui.set_active(true);
    ui.set_native_face_glass(true);
    ui.set_native_bubble_glass(true);
    let reported = Rc::new(RefCell::new(BTreeMap::new()));
    let result = reported.clone();
    ui.on_bubble_geometry(move |index, x, y, width, height| {
        result.borrow_mut().insert(index, (x, y, width, height));
    });
    let model = Rc::new(VecModel::from(vec![
        ChatBubble {
            title: "SOC".into(),
            text: "Primer mensaje".into(),
            from_user: false,
        },
        ChatBubble {
            title: "Tú".into(),
            text: "Segundo mensaje que ocupa más de una línea para comprobar su altura".into(),
            from_user: true,
        },
    ]));
    ui.set_chat_bubbles(ModelRc::from(model.clone()));
    ui.show()?;
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        let _ = render();
    }
    let viewport = ui.get_bubble_viewport_bounds();
    let rects = reported.borrow();
    assert_eq!(rects.len(), 2, "faltan rectángulos de los globos");
    let first = rects[&0];
    let second = rects[&1];
    assert_eq!(first.2, 296.0);
    assert_eq!(second.2, 296.0);
    assert_eq!(first.0, viewport.x);
    assert_eq!(second.0, viewport.x + 24.0);
    assert!(first.3 > 40.0 && second.3 > first.3);
    assert!(first.1 >= viewport.y && second.1 >= first.1 + first.3);
    let pixels = render();
    let y = (first.1 + first.3 / 2.0) as usize;
    let inside = pixels[y * 568 + first.0 as usize + 6];
    let outside = pixels[y * 568 + first.0 as usize - 6];
    assert_eq!(
        inside.blends, outside.blends,
        "Slint aún dibuja un fondo sobre la burbuja nativa: inside={inside:?} outside={outside:?}"
    );
    let face_clear = pixels[884 * 568 + 426];
    let face_outside = pixels[884 * 568 + 406];
    let eye = pixels[873 * 568 + 457];
    assert_eq!(
        face_clear.blends, face_outside.blends,
        "Slint aún dibuja un fondo sobre la cara nativa"
    );
    assert!(
        eye.blends > face_clear.blends,
        "la cara nativa perdió los ojos dibujados por Slint"
    );
    let pixel_values = &pixels;
    let mouth = (896..914)
        .flat_map(|y| (467..495).map(move |x| pixel_values[y * 568 + x].blends))
        .max()
        .unwrap_or_default();
    assert!(
        mouth > face_clear.blends,
        "la cara nativa perdió la boca dibujada por Slint"
    );
    println!("bubble_geometry=ok viewport={viewport:?} first={first:?} second={second:?}");
    drop(rects);
    model.remove(0);
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        let _ = render();
    }
    let moved = reported.borrow()[&0];
    assert_eq!(moved.0, viewport.x + 24.0);
    assert_eq!(moved.3, second.3);
    ui.hide()?;
    Ok(())
}
