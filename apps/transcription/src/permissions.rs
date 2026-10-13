use soc_core::Result;
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(value: *const c_void);
}

// Solicitud explícita: no concede permisos ni se ejecuta en el arranque.
pub fn request_accessibility() -> Result<()> {
    // SAFETY: ABI del SDK de macOS. Clave/valor son constantes CF inmortales;
    // callbacks nulos son válidos para este diccionario de una sola entrada.
    // AX consume las opciones durante la llamada y después se libera el dueño.
    let trusted = unsafe {
        let key = kAXTrustedCheckOptionPrompt;
        let value = kCFBooleanTrue;
        let options = CFDictionaryCreate(
            std::ptr::null(),
            &key,
            &value,
            1,
            std::ptr::null(),
            std::ptr::null(),
        );
        if options.is_null() {
            return Err("No se pudo preparar la solicitud de Accesibilidad".into());
        }
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        CFRelease(options);
        trusted
    };
    if !trusted {
        handy_keys::open_accessibility_settings()?;
    }
    Ok(())
}
