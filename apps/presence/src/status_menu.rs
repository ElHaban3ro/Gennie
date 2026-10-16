use soc_core::Result;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{
        Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
        accelerator::{Accelerator, Code, Modifiers},
    },
};

use crate::PresenceWindow;
use slint::Model;

// Todos los recursos nativos se crean y destruyen en el event loop principal.
pub struct StatusMenu {
    _icon: TrayIcon,
    _app_menu: Menu,
    wake: MenuItem,
    rest: MenuItem,
    about: MenuItem,
    transcription: MenuItem,
    exit: MenuItem,
}

impl StatusMenu {
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let assistant = Submenu::new("Asistente", true);
        let wake = MenuItem::new("Despertar", true, None);
        let rest = MenuItem::new("Descansar", false, None);
        assistant.append_items(&[&wake, &rest])?;
        let about = MenuItem::new("Acerca de soc", true, None);
        let transcription = MenuItem::new("Voz y asistente…", true, None);
        let exit = MenuItem::new(
            "Salir de soc",
            true,
            Some(Accelerator::new(Modifiers::META, Code::KeyQ)),
        );
        menu.append_items(&[
            &assistant,
            &transcription,
            &about,
            &PredefinedMenuItem::separator(),
            &exit,
        ])?;

        let image = slint::Image::load_from_svg_data(include_bytes!("../ui/status-icon.svg"))?;
        let pixels = image
            .to_rgba8()
            .ok_or("No se pudo dibujar el icono de soc")?;
        let icon = Icon::from_rgba(pixels.as_bytes().to_vec(), pixels.width(), pixels.height())?;
        let icon = TrayIconBuilder::new()
            .with_id("soc-presence")
            .with_tooltip("soc")
            .with_icon_templated(icon)
            .with_menu(Box::new(menu))
            .build()?;
        // Slint ofrece un menú genérico en macOS. Instalar los mismos comandos
        // reales para que Transcripción y el cierre normal sean accesibles.
        let app_menu = Menu::new();
        let app = Submenu::new("soc", true);
        app.append_items(&[
            &about,
            &transcription,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(Some("Ocultar soc")),
            &PredefinedMenuItem::hide_others(Some("Ocultar otros")),
            &PredefinedMenuItem::show_all(Some("Mostrar todo")),
            &PredefinedMenuItem::separator(),
            &exit,
        ])?;
        app_menu.append(&app)?;
        app_menu.init_for_nsapp();
        Ok(Self {
            _icon: icon,
            _app_menu: app_menu,
            wake,
            rest,
            about,
            transcription,
            exit,
        })
    }

    pub fn process_events(&self, ui: &PresenceWindow) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if (event.id == *self.wake.id() && !ui.get_active())
                || (event.id == *self.rest.id() && ui.get_active())
            {
                ui.invoke_toggle();
            } else if event.id == *self.about.id() {
                ui.invoke_about();
            } else if event.id == *self.transcription.id() {
                ui.invoke_transcription_settings();
            } else if event.id == *self.exit.id() {
                ui.invoke_exit();
                return;
            }
        }
        self.wake.set_enabled(!ui.get_active());
        self.rest.set_enabled(
            ui.get_active()
                && !ui.get_speech_active()
                && ui.get_chat_bubbles().row_count() == 0
                && !ui.get_history_open(),
        );
        self.transcription
            .set_enabled(ui.get_transcription_available());
    }
}
