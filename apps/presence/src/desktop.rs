use slint::winit_030::{WinitWindowAccessor, winit};
use soc_core::Result;

pub fn copy_text(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let pasteboard = objc2_app_kit::NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        // SAFETY: constante NSString del framework AppKit, disponible en macOS.
        let text_type = unsafe { objc2_app_kit::NSPasteboardTypeString };
        if !pasteboard.setString_forType(&objc2_foundation::NSString::from_str(text), text_type) {
            return Err("No se pudieron copiar los valores".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("Selecciona los valores y cópialos desde el campo de texto".into())
    }
}

#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
pub struct DragGesture {
    anchor: Option<DragAnchor>,
    moved: bool,
}
#[cfg(target_os = "macos")]
pub fn with_drag<T>(
    gesture: &std::cell::RefCell<DragGesture>,
    action: impl FnOnce(&mut DragGesture) -> T,
) -> Option<T> {
    // set_outer_position puede entregar callbacks síncronos de macOS. El
    // movimiento exterior ya está procesando esa muestra: agrupar la reentrada.
    let mut gesture = gesture.try_borrow_mut().ok()?;
    Some(action(&mut gesture))
}
#[cfg(any(target_os = "macos", test))]
struct DragAnchor {
    pointer: (f64, f64),
    origin: (f64, f64),
}
#[cfg(any(target_os = "macos", test))]
impl DragGesture {
    pub(crate) fn begin_at(&mut self, pointer: (f64, f64), origin: (f64, f64)) {
        self.anchor = Some(DragAnchor { pointer, origin });
        self.moved = false;
    }
    pub(crate) fn advance(&mut self, pointer: (f64, f64), pressed: bool) -> Option<(f64, f64)> {
        let anchor = self.anchor.as_ref()?;
        let dx = pointer.0 - anchor.pointer.0;
        let dy = pointer.1 - anchor.pointer.1;
        self.moved |= dx.abs() > 6.0 || dy.abs() > 6.0;
        let next = self
            .moved
            .then_some((anchor.origin.0 + dx, anchor.origin.1 - dy));
        if !pressed {
            self.anchor = None;
        }
        next
    }
    pub fn was_moved(&self) -> bool {
        self.moved
    }

    pub fn captured(&self) -> bool {
        self.anchor.is_some()
    }

    #[cfg(target_os = "macos")]
    pub fn begin(&mut self, window: &slint::Window) -> Result<()> {
        // Una acción sintética sin botón físico pulsado no debe iniciar captura.
        self.anchor = None;
        self.moved = false;
        if objc2_app_kit::NSEvent::pressedMouseButtons() & 1 == 0 {
            return Ok(());
        }
        window
            .with_winit_window(|native| -> Result<()> {
                let pointer = objc2_app_kit::NSEvent::mouseLocation();
                let origin = native
                    .outer_position()?
                    .to_logical::<f64>(native.scale_factor());
                self.begin_at((pointer.x, pointer.y), (origin.x, origin.y));
                native.set_cursor_hittest(true)?;
                Ok(())
            })
            .ok_or("Ventana nativa no disponible para arrastrar")?
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn apply_sample(
        &mut self,
        window: &slint::Window,
        pointer: (f64, f64),
        pressed: bool,
    ) -> Result<()> {
        if self.anchor.is_none() {
            return Ok(());
        }
        let next = self.advance(pointer, pressed);
        window
            .with_winit_window(|native| -> Result<()> {
                if let Some((x, y)) = next {
                    native.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
                }
                if pressed {
                    native.set_cursor_hittest(true)?;
                }
                Ok(())
            })
            .ok_or("Ventana nativa no disponible para arrastrar")?
    }
    #[cfg(target_os = "macos")]
    pub fn update(&mut self, window: &slint::Window) -> Result<()> {
        if self.anchor.is_none() {
            return Ok(());
        }
        let pointer = objc2_app_kit::NSEvent::mouseLocation();
        let pressed = objc2_app_kit::NSEvent::pressedMouseButtons() & 1 != 0;
        self.apply_sample(window, (pointer.x, pointer.y), pressed)
    }
}

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
    let backend = i_slint_backend_winit::Backend::builder()
        .with_renderer_name("femtovg")
        .with_event_loop_builder(builder)
        .with_window_attributes_hook(|attributes| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowAttributesExtMacOS;
                attributes.with_accepts_first_mouse(true)
            }
            #[cfg(not(target_os = "macos"))]
            {
                attributes
            }
        });
    // Slint instala su menú genérico al activar ventanas sin MenuBar (ajustes
    // y About incluidos). La app posee un único menú nativo, sin esa sustitución.
    #[cfg(target_os = "macos")]
    let backend = backend.with_default_menu_bar(false);
    slint::platform::set_platform(Box::new(backend.build()?))?;
    Ok(())
}

// Solo para una apertura explícita del usuario; nunca desde animaciones/avisos.
pub fn present_window(window: &slint::Window) -> Result<()> {
    window.show()?;
    window
        .with_winit_window(|native| {
            native.set_minimized(false);
            native.focus_window();
        })
        .ok_or("Ventana nativa no disponible")?;
    Ok(())
}

#[derive(Default)]
struct ShortcutRecorder {
    modifiers: winit::keyboard::ModifiersState,
    candidate: Option<String>,
}
impl ShortcutRecorder {
    fn chord(&self, key: Option<winit::keyboard::KeyCode>) -> Option<String> {
        use winit::keyboard::KeyCode;
        let mut parts = Vec::new();
        if self.modifiers.control_key() {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt_key() {
            parts.push("Option".to_string());
        }
        if self.modifiers.shift_key() {
            parts.push("Shift".to_string());
        }
        if self.modifiers.super_key() {
            parts.push("Cmd".to_string());
        }
        if let Some(key) = key {
            let name = match key {
                KeyCode::ControlLeft
                | KeyCode::ControlRight
                | KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::SuperLeft
                | KeyCode::SuperRight => return None,
                KeyCode::Backspace => "Delete".into(),
                KeyCode::Delete => "ForwardDelete".into(),
                KeyCode::Enter | KeyCode::NumpadEnter => "Return".into(),
                KeyCode::Minus => "-".into(),
                KeyCode::Equal => "=".into(),
                KeyCode::BracketLeft => "[".into(),
                KeyCode::BracketRight => "]".into(),
                KeyCode::Backslash => "\\".into(),
                KeyCode::Semicolon => ";".into(),
                KeyCode::Quote => "'".into(),
                KeyCode::Comma => ",".into(),
                KeyCode::Period => ".".into(),
                KeyCode::Slash => "/".into(),
                KeyCode::Backquote => "`".into(),
                other => {
                    let name = format!("{other:?}");
                    name.strip_prefix("Key")
                        .or_else(|| name.strip_prefix("Digit"))
                        .or_else(|| name.strip_prefix("Arrow"))
                        .unwrap_or(&name)
                        .to_owned()
                }
            };
            parts.push(name);
        }
        soc_transcription::normalize_shortcut(&parts.join("+"))
    }
    fn modifiers_changed(&mut self, modifiers: winit::keyboard::ModifiersState) -> Option<String> {
        self.modifiers = modifiers;
        if modifiers.is_empty() {
            self.candidate.take()
        } else {
            // Conservar la combinación completa al soltar modificadores uno a uno.
            if self.candidate.as_ref().is_none_or(|value| {
                value.split('+').count() <= modifiers.bits().count_ones() as usize
            }) {
                self.candidate = self.chord(None);
            }
            None
        }
    }
}

pub fn install_shortcut_capture(
    settings: &crate::TranscriptionSettings,
    finished: impl Fn() + 'static,
) {
    use slint::ComponentHandle;
    use winit::{
        event::{ElementState, WindowEvent},
        keyboard::{KeyCode, PhysicalKey},
    };
    let weak = settings.as_weak();
    let mut recorder = ShortcutRecorder::default();
    settings.window().on_winit_window_event(move |_, event| {
        let Some(settings) = weak.upgrade() else {
            return slint::winit_030::EventResult::Propagate;
        };
        if !settings.get_recording_shortcut() {
            recorder = ShortcutRecorder::default();
            return slint::winit_030::EventResult::Propagate;
        }
        let mut complete = false;
        let shortcut = match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                recorder.modifiers_changed(modifiers.state())
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if event.physical_key == PhysicalKey::Code(KeyCode::Escape) {
                    complete = true;
                    None
                } else if let PhysicalKey::Code(key) = event.physical_key {
                    recorder.chord(Some(key))
                } else {
                    None
                }
            }
            WindowEvent::Focused(false) | WindowEvent::CloseRequested => {
                complete = true;
                None
            }
            _ => None,
        };
        if let Some(shortcut) = shortcut {
            settings.set_shortcut(shortcut.into());
            complete = true;
        }
        if complete {
            settings.set_recording_shortcut(false);
            recorder = ShortcutRecorder::default();
            finished();
        }
        match event {
            WindowEvent::KeyboardInput { .. } | WindowEvent::ModifiersChanged(_) => {
                slint::winit_030::EventResult::PreventDefault
            }
            _ => slint::winit_030::EventResult::Propagate,
        }
    });
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

// La superficie translúcida vive detrás de la ventana Slint: Winit exige que
// su propia NSView siga siendo contentView, y la ventana hija no recibe eventos.
#[cfg(target_os = "macos")]
pub struct NativeFaceSurface {
    window: objc2::rc::Retained<objc2_app_kit::NSWindow>,
    glass: Option<objc2::rc::Retained<objc2_app_kit::NSGlassEffectView>>,
    circle: objc2::rc::Retained<objc2_app_kit::NSBox>,
    wallpaper: *mut std::ffi::c_void,
    style: std::cell::Cell<Option<NativeFaceStyle>>,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, PartialEq)]
pub struct NativeFaceStyle {
    pub blur: f64,
    pub saturation: f64,
    pub brightness: f64,
    pub tint: f64,
    pub border_width: f64,
    pub border_opacity: f64,
    pub transition: f64,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn soc_face_wallpaper_create(
        container: *mut std::ffi::c_void,
        glass: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn soc_face_wallpaper_update(
        material: *mut std::ffi::c_void,
        window: *mut std::ffi::c_void,
        x: f64,
        y: f64,
        diameter: f64,
    );
    fn soc_face_wallpaper_destroy(material: *mut std::ffi::c_void);
    fn soc_face_wallpaper_native_blur_supported(material: *mut std::ffi::c_void) -> i32;
    fn soc_face_wallpaper_configure(
        material: *mut std::ffi::c_void,
        blur: f64,
        saturation: f64,
        brightness: f64,
        border_width: f64,
        border_opacity: f64,
        transition: f64,
    );
}

// El tamaño máximo del selector (224 px) más margen para la respiración.
#[cfg(target_os = "macos")]
const FACE_SURFACE_CANVAS: f64 = 226.0;

#[cfg(target_os = "macos")]
impl NativeFaceSurface {
    pub fn native_blur_supported(&self) -> bool {
        // SAFETY: el helper pertenece a esta superficie y solo se consulta en main.
        unsafe { soc_face_wallpaper_native_blur_supported(self.wallpaper) != 0 }
    }

    pub fn new(window: &slint::Window, diameter: f32) -> Result<Option<Self>> {
        use objc2::runtime::AnyClass;
        use objc2_app_kit::{
            NSBackingStoreType, NSBox, NSBoxType, NSColor, NSGlassEffectView,
            NSGlassEffectViewStyle, NSTitlePosition, NSView, NSWindow, NSWindowOrderingMode,
            NSWindowStyleMask,
        };
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let native = window
            .with_winit_window(|native| -> Result<Self> {
                let handle = native.window_handle()?;
                let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
                    return Err("Ventana nativa no AppKit".into());
                };
                // SAFETY: el handle Winit mantiene viva la NSView durante este callback.
                let view = unsafe { raw.ns_view.cast::<objc2_app_kit::NSView>().as_ref() };
                let parent = view.window().ok_or("La cara no tiene NSWindow")?;
                let marker = objc2::MainThreadMarker::new().ok_or("AppKit requiere main")?;
                let frame = face_surface_frame(parent.frame());
                // SAFETY: se crea y configura en el hilo principal; AppKit conserva
                // la ventana hija y su vista hasta retirarlas explícitamente.
                let child = unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        objc2::msg_send![native_surface_window_class(), alloc],
                        frame,
                        NSWindowStyleMask::Borderless,
                        NSBackingStoreType::Buffered,
                        false,
                    )
                };
                child.setOpaque(false);
                let clear = NSColor::clearColor();
                child.setBackgroundColor(Some(&clear));
                child.setHasShadow(false);
                child.setIgnoresMouseEvents(true);
                child.setLevel(parent.level());
                let container = NSView::new(marker);
                container.setFrame(NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size: NSSize {
                        width: FACE_SURFACE_CANVAS,
                        height: FACE_SURFACE_CANVAS,
                    },
                });
                let circle = NSBox::new(marker);
                circle.setBoxType(NSBoxType::Custom);
                circle.setTitlePosition(NSTitlePosition::NoTitle);
                circle.setBorderWidth(0.0);
                circle.setCornerRadius(f64::from(diameter) / 2.0);
                let glass = if AnyClass::get(c"NSGlassEffectView").is_some() {
                    let glass = NSGlassEffectView::new(marker);
                    glass.setFrame(face_surface_view_frame(diameter));
                    glass.setStyle(NSGlassEffectViewStyle::Regular);
                    glass.setAlphaValue(1.0);
                    glass.setTintColor(None);
                    let _: () = unsafe {
                        objc2::msg_send![&glass, setCornerRadius: f64::from(diameter) / 2.0]
                    };
                    circle.setFrame(face_glass_content_frame(diameter));
                    circle.setFillColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
                        0.10, 0.11, 0.12, 0.24,
                    ));
                    glass.setContentView(Some(&circle));
                    container.addSubview(&glass);
                    Some(glass)
                } else {
                    circle.setFrame(face_surface_view_frame(diameter));
                    circle.setFillColor(&NSColor::colorWithSRGBRed_green_blue_alpha(
                        0.0, 0.0, 0.0, 0.18,
                    ));
                    container.addSubview(&circle);
                    None
                };
                child.setContentView(Some(&container));
                let wallpaper = glass.as_ref().map_or(std::ptr::null_mut(), |glass| {
                    // SAFETY: ambas NSView viven mientras exista la ventana hija.
                    unsafe {
                        soc_face_wallpaper_create(
                            (&*container as *const NSView).cast_mut().cast(),
                            (&**glass as *const NSGlassEffectView).cast_mut().cast(),
                        )
                    }
                });
                // SAFETY: parent y child son ventanas vivas del mismo hilo principal.
                unsafe { parent.addChildWindow_ordered(&child, NSWindowOrderingMode::Below) };
                child.orderFront(None);
                Ok(Self {
                    window: child,
                    glass,
                    circle,
                    wallpaper,
                    style: std::cell::Cell::new(None),
                })
            })
            .ok_or("Ventana Winit de presencia no disponible")??;
        Ok(Some(native))
    }

    pub fn configure(&self, style: NativeFaceStyle) {
        if self.style.get() == Some(style) {
            return;
        }
        self.circle
            .setFillColor(&objc2_app_kit::NSColor::colorWithSRGBRed_green_blue_alpha(
                0.10, 0.11, 0.12, style.tint,
            ));
        // SAFETY: el helper pertenece a esta superficie y solo se usa en main.
        unsafe {
            soc_face_wallpaper_configure(
                self.wallpaper,
                style.blur,
                style.saturation,
                style.brightness,
                style.border_width,
                style.border_opacity,
                style.transition,
            );
        }
        self.style.set(Some(style));
    }

    pub fn update(&self, diameter: f32) {
        if let Some(parent) = self.window.parentWindow() {
            let frame = face_surface_frame(parent.frame());
            if self.window.frame() != frame {
                self.window.setFrame_display(frame, true);
            }
            let surface_frame = face_surface_view_frame(diameter);
            if let Some(glass) = &self.glass {
                if glass.frame() != surface_frame {
                    glass.setFrame(surface_frame);
                    let _: () = unsafe {
                        objc2::msg_send![glass, setCornerRadius: f64::from(diameter) / 2.0]
                    };
                    self.circle.setFrame(face_glass_content_frame(diameter));
                    self.circle.setCornerRadius(f64::from(diameter) / 2.0);
                }
            } else if self.circle.frame() != surface_frame {
                self.circle.setFrame(surface_frame);
                self.circle.setCornerRadius(f64::from(diameter) / 2.0);
            }
            if !self.wallpaper.is_null() {
                // SAFETY: el material y la ventana hija siguen vivos en el hilo principal.
                unsafe {
                    soc_face_wallpaper_update(
                        self.wallpaper,
                        (&*self.window as *const objc2_app_kit::NSWindow)
                            .cast_mut()
                            .cast(),
                        surface_frame.origin.x,
                        surface_frame.origin.y,
                        f64::from(diameter),
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeFaceSurface {
    fn drop(&mut self) {
        if !self.wallpaper.is_null() {
            // SAFETY: es el puntero retenido por soc_face_wallpaper_create.
            unsafe { soc_face_wallpaper_destroy(self.wallpaper) };
        }
        if let Some(parent) = self.window.parentWindow() {
            parent.removeChildWindow(&self.window);
        }
        self.window.orderOut(None);
    }
}

#[cfg(target_os = "macos")]
fn face_surface_frame(parent: objc2_foundation::NSRect) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    NSRect {
        origin: NSPoint {
            x: parent.origin.x + parent.size.width - FACE_SURFACE_CANVAS - 24.0,
            y: parent.origin.y + 32.0,
        },
        size: NSSize {
            width: FACE_SURFACE_CANVAS,
            height: FACE_SURFACE_CANVAS,
        },
    }
}

#[cfg(target_os = "macos")]
fn face_surface_view_frame(diameter: f32) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let diameter = f64::from(diameter);
    NSRect {
        origin: NSPoint {
            x: FACE_SURFACE_CANVAS - diameter,
            y: 0.0,
        },
        size: NSSize {
            width: diameter,
            height: diameter,
        },
    }
}

#[cfg(target_os = "macos")]
fn face_glass_content_frame(diameter: f32) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let diameter = f64::from(diameter);
    NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: diameter,
            height: diameter,
        },
    }
}

#[cfg(target_os = "macos")]
fn set_backdrop_corner_radius(backdrop: &objc2_app_kit::NSVisualEffectView, radius: f64) {
    use objc2::runtime::AnyObject;
    let layer: Option<&AnyObject> = unsafe { objc2::msg_send![backdrop, layer] };
    if let Some(layer) = layer {
        let _: () = unsafe { objc2::msg_send![layer, setCornerRadius: radius] };
        let _: () = unsafe { objc2::msg_send![layer, setMasksToBounds: true] };
    }
}

// Una sola ventana hija recorta las vistas de vidrio al viewport de los globos.
// Sus marcos provienen del layout real de Slint, incluido el texto que cambia de altura.
#[cfg(target_os = "macos")]
pub struct NativeBubbleGlass {
    window: objc2::rc::Retained<objc2_app_kit::NSWindow>,
    container: objc2::rc::Retained<objc2_app_kit::NSView>,
    surfaces: Vec<NativeBubbleSurface>,
}

#[cfg(target_os = "macos")]
struct NativeBubbleSurface {
    backdrop: objc2::rc::Retained<objc2_app_kit::NSVisualEffectView>,
    glass: objc2::rc::Retained<objc2_app_kit::NSGlassEffectView>,
}

// La ventana principal puede quedar parcialmente fuera de pantalla. AppKit
// limita por defecto la ventana hija al borde superior y separa el vidrio
// de los rasgos y textos de Slint; cara y globos conservan el marco solicitado.
#[cfg(target_os = "macos")]
fn native_surface_window_class() -> &'static objc2::runtime::AnyClass {
    use objc2::{
        ClassType,
        runtime::{AnyClass, AnyObject, ClassBuilder, Imp, Sel},
    };
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::NSRect;
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();

    extern "C-unwind" fn constrain(
        _window: &AnyObject,
        _selector: Sel,
        frame: NSRect,
        _screen: Option<&NSScreen>,
    ) -> NSRect {
        frame
    }

    CLASS.get_or_init(|| {
        let builder = ClassBuilder::new(c"SOCNativeSurfaceWindow", NSWindow::class())
            .expect("Clase nativa de superficies disponible");
        let class = builder.register();
        let selector = objc2::sel!(constrainFrameRect:toScreen:);
        let inherited = NSWindow::class()
            .instance_method(selector)
            .expect("Método de límites NSWindow disponible");
        // SAFETY: misma ABI y codificación que el método heredado; no hay
        // instancias de esta clase todavía.
        let added = unsafe {
            let imp: Imp = std::mem::transmute(
                constrain
                    as extern "C-unwind" fn(&AnyObject, Sel, NSRect, Option<&NSScreen>) -> NSRect,
            );
            objc2::ffi::class_addMethod(
                class as *const _ as *mut _,
                selector,
                imp,
                objc2::ffi::method_getTypeEncoding(inherited),
            )
        };
        assert!(
            added.as_bool(),
            "No se pudo liberar el marco de la superficie"
        );
        class
    })
}

#[cfg(target_os = "macos")]
impl NativeBubbleGlass {
    pub fn new(window: &slint::Window, viewport: crate::IndicatorBounds) -> Result<Option<Self>> {
        use objc2::runtime::AnyClass;
        use objc2_app_kit::{
            NSBackingStoreType, NSColor, NSView, NSWindow, NSWindowOrderingMode, NSWindowStyleMask,
        };
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        if AnyClass::get(c"NSGlassEffectView").is_none() {
            return Ok(None);
        }
        let native = window
            .with_winit_window(|native| -> Result<Self> {
                let handle = native.window_handle()?;
                let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
                    return Err("Ventana nativa no AppKit".into());
                };
                // SAFETY: el handle Winit mantiene viva la vista durante este callback.
                let view = unsafe { raw.ns_view.cast::<NSView>().as_ref() };
                let parent = view.window().ok_or("La presencia no tiene NSWindow")?;
                let marker = objc2::MainThreadMarker::new().ok_or("AppKit requiere main")?;
                // SAFETY: creación y vínculo de ventanas vivas en el hilo principal.
                let child = unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        objc2::msg_send![native_surface_window_class(), alloc],
                        bubble_glass_window_frame(parent.frame(), &viewport),
                        NSWindowStyleMask::Borderless,
                        NSBackingStoreType::Buffered,
                        false,
                    )
                };
                child.setOpaque(false);
                child.setBackgroundColor(Some(&NSColor::clearColor()));
                child.setHasShadow(false);
                child.setIgnoresMouseEvents(true);
                child.setLevel(parent.level());
                let container = NSView::new(marker);
                container.setFrame(bubble_glass_content_frame(&viewport));
                child.setContentView(Some(&container));
                unsafe { parent.addChildWindow_ordered(&child, NSWindowOrderingMode::Below) };
                child.orderFront(None);
                Ok(Self {
                    window: child,
                    container,
                    surfaces: Vec::new(),
                })
            })
            .ok_or("Ventana Winit de presencia no disponible")??;
        Ok(Some(native))
    }

    pub fn update(
        &mut self,
        viewport: crate::IndicatorBounds,
        bounds: &[(usize, crate::IndicatorBounds)],
        count: usize,
        hidden: bool,
    ) {
        use objc2_app_kit::{
            NSGlassEffectView, NSGlassEffectViewStyle, NSVisualEffectBlendingMode,
            NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
        };

        let Some(parent) = self.window.parentWindow() else {
            return;
        };
        while self.surfaces.len() > count {
            if let Some(surface) = self.surfaces.pop() {
                surface.glass.removeFromSuperview();
                surface.backdrop.removeFromSuperview();
            }
        }
        let should_show = !hidden && count > 0 && viewport.height > 0.0;
        if !should_show {
            for surface in &self.surfaces {
                surface.glass.setHidden(true);
                surface.backdrop.setHidden(true);
            }
            return;
        }
        let frame = bubble_glass_window_frame(parent.frame(), &viewport);
        if self.window.frame() != frame {
            self.window.setFrame_display(frame, true);
        }
        let content_frame = bubble_glass_content_frame(&viewport);
        if self.container.frame() != content_frame {
            self.container.setFrame(content_frame);
        }
        while self.surfaces.len() < count {
            let Some(marker) = objc2::MainThreadMarker::new() else {
                return;
            };
            let index = self.surfaces.len();
            let backdrop = NSVisualEffectView::new(marker);
            backdrop.setMaterial(NSVisualEffectMaterial::Popover);
            backdrop.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            backdrop.setState(NSVisualEffectState::Active);
            backdrop.setWantsLayer(true);
            set_backdrop_corner_radius(&backdrop, 18.0);
            let glass = NSGlassEffectView::new(marker);
            glass.setStyle(NSGlassEffectViewStyle::Regular);
            glass.setTintColor(None);
            let _: () = unsafe { objc2::msg_send![&glass, setCornerRadius: 18.0_f64] };
            if let Some((_, bubble)) = bounds.iter().find(|(candidate, _)| *candidate == index) {
                let rect = bubble_glass_view_frame(&viewport, bubble);
                backdrop.setFrame(rect);
                glass.setFrame(rect);
            } else {
                glass.setHidden(true);
                backdrop.setHidden(true);
            }
            self.container.addSubview(&backdrop);
            self.container.addSubview(&glass);
            self.surfaces.push(NativeBubbleSurface { backdrop, glass });
        }
        for (index, surface) in self.surfaces.iter().enumerate() {
            if let Some((_, bubble)) = bounds.iter().find(|(candidate, _)| *candidate == index) {
                let rect = bubble_glass_view_frame(&viewport, bubble);
                if surface.glass.frame() != rect {
                    surface.glass.setFrame(rect);
                    surface.backdrop.setFrame(rect);
                }
                surface.glass.setHidden(false);
                surface.backdrop.setHidden(false);
            } else {
                surface.glass.setHidden(true);
                surface.backdrop.setHidden(true);
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeBubbleGlass {
    fn drop(&mut self) {
        if let Some(parent) = self.window.parentWindow() {
            parent.removeChildWindow(&self.window);
        }
        self.window.orderOut(None);
    }
}

#[cfg(target_os = "macos")]
fn bubble_glass_window_frame(
    parent: objc2_foundation::NSRect,
    viewport: &crate::IndicatorBounds,
) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    NSRect {
        origin: NSPoint {
            x: parent.origin.x + f64::from(viewport.x),
            y: parent.origin.y + parent.size.height - f64::from(viewport.y + viewport.height),
        },
        size: NSSize {
            width: f64::from(viewport.width),
            height: f64::from(viewport.height),
        },
    }
}

#[cfg(target_os = "macos")]
fn bubble_glass_content_frame(viewport: &crate::IndicatorBounds) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: f64::from(viewport.width),
            height: f64::from(viewport.height),
        },
    }
}

#[cfg(target_os = "macos")]
fn bubble_glass_view_frame(
    viewport: &crate::IndicatorBounds,
    bubble: &crate::IndicatorBounds,
) -> objc2_foundation::NSRect {
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    NSRect {
        origin: NSPoint {
            x: f64::from(bubble.x - viewport.x),
            y: f64::from(viewport.y + viewport.height - bubble.y - bubble.height),
        },
        size: NSSize {
            width: f64::from(bubble.width),
            height: f64::from(bubble.height),
        },
    }
}

// Una lectura de cursor, sin event tap ni permisos de accesibilidad.
#[cfg(target_os = "macos")]
pub fn update_pointer_state(
    window: &slint::Window,
    diameter: f32,
    indicators: &[crate::IndicatorBounds],
    captured: bool,
) -> Result<Option<(f32, f32)>> {
    let pointer_state = window
        .with_winit_window(|native| -> Result<_> {
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
            let inside = contains_presence(
                local_x,
                local_y,
                center_x,
                center_y,
                diameter as f64 / 2.0,
                indicators,
            );
            let interactive = inside || captured;
            native.set_cursor_hittest(interactive)?;
            Ok((
                gaze_direction(local_x - center_x, local_y - center_y),
                interactive,
            ))
        })
        .transpose()?;
    let Some((gaze, interactive)) = pointer_state else {
        return Ok(None);
    };
    if !interactive {
        // AppKit puede omitir mouseExited al activar click-through. Limpiar
        // el hover fuera del préstamo Winit evita reentradas y tooltips retenidos.
        window.dispatch_event(slint::platform::WindowEvent::PointerExited);
    }
    Ok(Some(gaze))
}
#[cfg(not(target_os = "macos"))]
pub fn update_pointer_state(
    _window: &slint::Window,
    _diameter: f32,
    _indicators: &[crate::IndicatorBounds],
    _captured: bool,
) -> Result<Option<(f32, f32)>> {
    Ok(None)
}

// Saturación radial suave: conserva dirección y limita el desplazamiento al acercarse/alejarse.
fn gaze_direction(dx: f64, dy: f64) -> (f32, f32) {
    let distance = dx.hypot(dy);
    let scale = distance + 180.0;
    ((dx / scale) as f32, (dy / scale) as f32)
}

// Fallback portátil para eventos dentro de la ventana; macOS además lee el cursor global.
pub fn install_cursor_tracking(ui: &crate::PresenceWindow) {
    use slint::ComponentHandle;
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |window, event| {
        if let Some(ui) = weak.upgrade() {
            match event {
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    if let Some((x, y)) = window.with_winit_window(|native| {
                        let scale = native.scale_factor();
                        let point = position.to_logical::<f64>(scale);
                        let size = native.inner_size().to_logical::<f64>(scale);
                        let diameter = f64::from(ui.get_actual_diameter());
                        gaze_direction(
                            point.x - (size.width - 24.0 - diameter / 2.0),
                            point.y - (size.height - 32.0 - diameter / 2.0),
                        )
                    }) {
                        ui.set_pointer_gaze_x(x);
                        ui.set_pointer_gaze_y(y);
                    }
                }
                winit::event::WindowEvent::CursorLeft { .. } if !cfg!(target_os = "macos") => {
                    ui.set_pointer_gaze_x(0.0);
                    ui.set_pointer_gaze_y(0.0);
                }
                _ => {}
            }
        }
        slint::winit_030::EventResult::Propagate
    });
}

#[cfg(any(target_os = "macos", test))]
fn contains_circle(x: f64, y: f64, center_x: f64, center_y: f64, radius: f64) -> bool {
    (x - center_x).powi(2) + (y - center_y).powi(2) <= radius.powi(2)
}
#[cfg(any(target_os = "macos", test))]
fn contains_presence(
    x: f64,
    y: f64,
    center_x: f64,
    center_y: f64,
    radius: f64,
    indicators: &[crate::IndicatorBounds],
) -> bool {
    contains_circle(x, y, center_x, center_y, radius)
        || indicators.iter().any(|bounds| {
            // Las áreas llegan del layout animado de Slint, en píxeles lógicos.
            // Conservar libres tanto el espacio entre iconos como su contorno.
            bounds.width > 0.0
                && bounds.height > 0.0
                && x >= f64::from(bounds.x)
                && x < f64::from(bounds.x + bounds.width)
                && y >= f64::from(bounds.y)
                && y < f64::from(bounds.y + bounds.height)
        })
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
    configure_drag_bounds(&window)?;
    // Conservar políticas ajenas; estas alternativas son incompatibles con el overlay.
    let behavior = window.collectionBehavior()
        & !(Behavior::MoveToActiveSpace | Behavior::FullScreenPrimary | Behavior::FullScreenNone);
    window.setCollectionBehavior(
        behavior | Behavior::CanJoinAllSpaces | Behavior::FullScreenAuxiliary,
    );
    Ok(())
}

// Identidad, no préstamo ni puntero desreferenciable fuera del callback nativo.
#[cfg(target_os = "macos")]
static DRAG_WINDOW_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(target_os = "macos")]
static DRAG_WINDOW_CLASS: std::sync::OnceLock<&'static objc2::runtime::AnyClass> =
    std::sync::OnceLock::new();

// La clase Winit conserva su layout/ivars. Solo la presencia omite el límite del marco.
#[cfg(target_os = "macos")]
fn configure_drag_bounds(window: &objc2_app_kit::NSWindow) -> Result<()> {
    use objc2::{
        ClassType,
        runtime::{AnyObject, Imp, Sel},
    };
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::NSRect;
    use std::sync::atomic::Ordering;

    extern "C-unwind" fn constrain(
        window: &AnyObject,
        _selector: Sel,
        frame: NSRect,
        screen: Option<&NSScreen>,
    ) -> NSRect {
        if window as *const AnyObject as usize == DRAG_WINDOW_ID.load(Ordering::Relaxed) {
            frame
        } else {
            // SAFETY: método heredado de NSWindow, misma firma y objeto vivo en main.
            unsafe {
                objc2::msg_send![super(window, NSWindow::class()), constrainFrameRect: frame, toScreen: screen]
            }
        }
    }
    let mut class = window.class();
    while let Some(parent) = class.superclass() {
        if std::ptr::eq(parent, NSWindow::class()) {
            break;
        }
        class = parent;
    }
    if !class
        .superclass()
        .is_some_and(|parent| std::ptr::eq(parent, NSWindow::class()))
    {
        return Err("Clase Winit de presencia incompatible con NSWindow".into());
    }
    if let Some(installed) = DRAG_WINDOW_CLASS.get() {
        if !std::ptr::eq(*installed, class) {
            return Err("Clase nativa de presencia cambió".into());
        }
    } else {
        let selector = objc2::sel!(constrainFrameRect:toScreen:);
        let inherited = NSWindow::class()
            .instance_method(selector)
            .ok_or("Método de límites NSWindow ausente")?;
        // SAFETY: misma ABI que el método heredado. Se añade un override a la clase
        // Winit de este proceso, sin reemplazar NSWindow ni cambiar clase/layout.
        // Toda instancia distinta de la presencia delega al método original de NSWindow.
        let added = unsafe {
            let imp: Imp = std::mem::transmute(
                constrain
                    as extern "C-unwind" fn(&AnyObject, Sel, NSRect, Option<&NSScreen>) -> NSRect,
            );
            objc2::ffi::class_addMethod(
                class as *const _ as *mut _,
                selector,
                imp,
                objc2::ffi::method_getTypeEncoding(inherited),
            )
        };
        if !added.as_bool() {
            return Err("Winit ya define una política de límites incompatible".into());
        }
        let _ = DRAG_WINDOW_CLASS.set(class);
    }
    // La UI conserva la ventana viva. Solo se compara esta identidad; nunca se dereferencia.
    DRAG_WINDOW_ID.store(window as *const NSWindow as usize, Ordering::Relaxed);
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn clear_drag_bounds() {
    DRAG_WINDOW_ID.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn bubble_glass_uses_slint_layout_and_viewport_clip() {
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let parent = NSRect {
            origin: NSPoint { x: 100.0, y: 200.0 },
            size: NSSize {
                width: 568.0,
                height: 980.0,
            },
        };
        let viewport = crate::IndicatorBounds {
            x: 224.0,
            y: 0.0,
            width: 320.0,
            height: 804.0,
        };
        let bubble = crate::IndicatorBounds {
            x: 248.0,
            y: 706.0,
            width: 296.0,
            height: 98.0,
        };
        let window = bubble_glass_window_frame(parent, &viewport);
        let content = bubble_glass_content_frame(&viewport);
        let view = bubble_glass_view_frame(&viewport, &bubble);
        assert_eq!(window.origin.x, 324.0);
        assert_eq!(window.origin.y, 376.0);
        assert_eq!(window.size.height, 804.0);
        assert_eq!(content.size, window.size);
        assert_eq!(view.origin.x, 24.0);
        assert_eq!(view.origin.y, 0.0);
        assert_eq!(view.size.width, 296.0);
        assert_eq!(view.size.height, 98.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn breathing_keeps_native_window_frame_stable() {
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let parent = NSRect {
            origin: NSPoint { x: 100.0, y: 200.0 },
            size: NSSize {
                width: 568.0,
                height: 980.0,
            },
        };
        let resting = face_surface_frame(parent);
        for diameter in [63.2, 63.6, 64.2, 64.8] {
            assert_eq!(face_surface_frame(parent), resting);
            let view = face_surface_view_frame(diameter);
            assert!((view.origin.x + view.size.width - FACE_SURFACE_CANVAS).abs() < 0.001);
            assert_eq!(view.size.height, f64::from(diameter));
            assert!(
                (resting.origin.x + view.origin.x
                    - (parent.origin.x + parent.size.width - f64::from(diameter) - 24.0))
                    .abs()
                    < 0.001
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn drag_native_reentry_is_coalesced_without_losing_outer_gesture() {
        let gesture = std::cell::RefCell::new(DragGesture::default());
        with_drag(&gesture, |drag| {
            drag.begin_at((0.0, 0.0), (50.0, 50.0));
            assert_eq!(drag.advance((30.0, 30.0), true), Some((80.0, 20.0)));
            // set_outer_position entrega WindowDidMove síncrono en macOS.
            assert!(
                with_drag(&gesture, |_| panic!(
                    "No debe ejecutar la actualización reentrante"
                ))
                .is_none()
            );
            assert!(drag.captured());
            assert!(drag.was_moved());
        });
        assert_eq!(
            with_drag(&gesture, |drag| drag.advance((30.0, 30.0), false)),
            Some(Some((80.0, 20.0)))
        );
        assert!(!gesture.borrow().captured());
    }
    #[test]
    fn shortcut_capture_keeps_chord_until_modifiers_are_released() {
        use winit::keyboard::{KeyCode, ModifiersState as M};
        let mut recorder = ShortcutRecorder::default();
        assert_eq!(recorder.modifiers_changed(M::CONTROL | M::SHIFT), None);
        assert_eq!(
            recorder.chord(Some(KeyCode::KeyK)),
            Some("Ctrl+Shift+K".into())
        );
        assert_eq!(recorder.modifiers_changed(M::SHIFT), None);
        assert_eq!(
            recorder.modifiers_changed(M::empty()),
            Some("Ctrl+Shift".into())
        );
        assert_eq!(recorder.modifiers_changed(M::SHIFT), None);
        assert_eq!(recorder.modifiers_changed(M::empty()), Some("Shift".into()));
        assert_eq!(recorder.chord(Some(KeyCode::Space)), Some("Space".into()));
        assert_eq!(
            recorder.chord(Some(KeyCode::Backspace)),
            Some("Delete".into())
        );
    }
    #[test]
    fn drag_sequence_crosses_native_ceiling_and_suppresses_release_click() {
        let mut drag = DragGesture::default();
        drag.begin_at((500.0, 500.0), (400.0, 400.0));
        assert_eq!(drag.advance((500.0, 900.0), true), Some((400.0, 0.0)));
        assert_eq!(drag.advance((500.0, 1248.0), true), Some((400.0, -348.0)));
        assert_eq!(drag.advance((500.0, 1250.0), false), Some((400.0, -350.0)));
        assert!(drag.was_moved());
        assert!(drag.anchor.is_none());
        assert_eq!(drag.advance((900.0, 900.0), false), None);
        assert_eq!(drag.advance((900.0, 900.0), true), None);
        drag.begin_at((500.0, 1250.0), (400.0, -350.0));
        assert_eq!(drag.advance((502.0, 1252.0), false), None);
        assert!(!drag.was_moved());
    }
    #[test]
    fn dragging_out_and_back_still_suppresses_click() {
        let mut drag = DragGesture::default();
        drag.begin_at((0.0, 0.0), (50.0, 50.0));
        assert_eq!(drag.advance((50.0, 50.0), true), Some((100.0, 0.0)));
        assert_eq!(drag.advance((0.0, 0.0), false), Some((50.0, 50.0)));
        assert!(drag.was_moved());
    }
    #[test]
    fn gaze_is_centered_directional_and_radially_bounded() {
        assert_eq!(gaze_direction(0.0, 0.0), (0.0, 0.0));
        let near = gaze_direction(20.0, -40.0);
        let far = gaze_direction(2000.0, -4000.0);
        assert!(near.0 > 0.0 && near.1 < 0.0);
        assert!((near.1 / near.0 + 2.0).abs() < 0.0001);
        assert!(far.0 > near.0 && far.1.abs() > near.1.abs());
        assert!(far.0.hypot(far.1) < 1.0);
        let opposite = gaze_direction(-2000.0, 4000.0);
        assert_eq!(opposite, (-far.0, -far.1));
    }

    #[test]
    fn circle_hit_test_excludes_transparent_square_corners() {
        assert!(contains_circle(160.0, 184.0, 160.0, 184.0, 40.0));
        assert!(contains_circle(200.0, 184.0, 160.0, 184.0, 40.0));
        assert!(!contains_circle(199.0, 223.0, 160.0, 184.0, 40.0));
        assert!(!contains_circle(12.0, 12.0, 160.0, 184.0, 40.0));
        assert!(contains_circle(116.0, 140.0, 116.0, 140.0, 84.0));
    }

    #[test]
    fn indicator_hit_test_preserves_circle_gaps_and_transparent_surroundings() {
        let indicators = [
            crate::IndicatorBounds {
                x: 132.0,
                y: 232.0,
                width: 24.0,
                height: 24.0,
            },
            crate::IndicatorBounds {
                x: 164.0,
                y: 232.0,
                width: 24.0,
                height: 24.0,
            },
            crate::IndicatorBounds {
                x: 196.0,
                y: 232.0,
                width: 24.0,
                height: 24.0,
            },
        ];
        let hit = |x, y| contains_presence(x, y, 160.0, 184.0, 40.0, &indicators);
        assert!(hit(160.0, 184.0));
        assert!(hit(200.0, 184.0));
        assert!(hit(144.0, 244.0));
        assert!(hit(176.0, 244.0));
        assert!(hit(208.0, 244.0));
        assert!(hit(132.0, 232.0));
        assert!(!hit(156.0, 244.0));
        assert!(!hit(160.0, 244.0));
        assert!(!hit(192.0, 244.0));
        assert!(!hit(144.0, 256.0));
        assert!(!hit(160.0, 228.0));
        assert!(!hit(199.0, 223.0));
        assert!(!hit(12.0, 12.0));
    }

    #[test]
    fn indicator_hit_test_follows_layout_and_ignores_empty_bounds() {
        let indicators = [
            crate::IndicatorBounds {
                x: 42.5,
                y: 68.5,
                width: 24.0,
                height: 24.0,
            },
            crate::IndicatorBounds {
                x: 130.0,
                y: 230.0,
                width: 0.0,
                height: 24.0,
            },
        ];
        assert!(contains_presence(
            54.5,
            80.5,
            160.0,
            184.0,
            84.0,
            &indicators
        ));
        assert!(!contains_presence(
            144.0,
            244.0,
            160.0,
            184.0,
            40.0,
            &indicators
        ));
        assert!(!contains_presence(
            130.0,
            240.0,
            160.0,
            184.0,
            40.0,
            &indicators
        ));
    }
}
