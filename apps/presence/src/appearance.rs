use crate::{AppearanceSettings, PreferenceUpdate, PresenceWindow};
use slint::ComponentHandle;
use soc_core::Result;
use std::{cell::Cell, rc::Rc, sync::mpsc::SyncSender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub blur_radius: i32,
    pub saturation_percent: i32,
    pub brightness_percent: i32,
    pub tint_percent: i32,
    pub border_percent: i32,
    pub border_width_hundredths: i32,
    pub transition_ms: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            blur_radius: 0,
            saturation_percent: 200,
            brightness_percent: 101,
            tint_percent: 5,
            border_percent: 0,
            border_width_hundredths: 67,
            transition_ms: 300,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<()> {
        for (name, value, min, max) in [
            ("Desenfoque", self.blur_radius, 0, 30),
            ("Saturación", self.saturation_percent, 0, 200),
            ("Luminosidad", self.brightness_percent, 50, 150),
            ("Tinte", self.tint_percent, 0, 60),
            ("Intensidad del borde", self.border_percent, 0, 100),
            ("Grosor del borde", self.border_width_hundredths, 0, 300),
            ("Transición", self.transition_ms, 0, 300),
        ] {
            if !(min..=max).contains(&value) {
                return Err(format!("{name}: valor fuera de rango").into());
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn native(self) -> crate::desktop::NativeFaceStyle {
        crate::desktop::NativeFaceStyle {
            blur: self.blur_radius.into(),
            saturation: f64::from(self.saturation_percent) / 100.0,
            brightness: f64::from(self.brightness_percent - 100) / 100.0,
            tint: f64::from(self.tint_percent) / 100.0,
            border_width: f64::from(self.border_width_hundredths) / 100.0,
            border_opacity: f64::from(self.border_percent) / 100.0,
            transition: f64::from(self.transition_ms) / 1000.0,
        }
    }

    pub fn summary(self) -> String {
        format!(
            "blur={}px; saturación={}%; luminosidad={}%; tinte={}%; borde={}%/{:.2}px; transición={}ms",
            self.blur_radius,
            self.saturation_percent,
            self.brightness_percent,
            self.tint_percent,
            self.border_percent,
            f64::from(self.border_width_hundredths) / 100.0,
            self.transition_ms
        )
    }
}

pub(crate) struct Ui {
    pub window: AppearanceSettings,
    current: Cell<Settings>,
    saved: Cell<Settings>,
    transition_preview: Cell<Option<Settings>>,
    preview_timer: slint::Timer,
}

impl Ui {
    pub fn new(
        host: &PresenceWindow,
        saved: Settings,
        sender: SyncSender<PreferenceUpdate>,
    ) -> Result<Rc<Self>> {
        saved.validate()?;
        let ui = Rc::new(Self {
            window: AppearanceSettings::new()?,
            current: Cell::new(saved),
            saved: Cell::new(saved),
            transition_preview: Cell::new(None),
            preview_timer: slint::Timer::default(),
        });
        ui.populate(saved);
        let weak = Rc::downgrade(&ui);
        host.on_appearance_settings(move || {
            if let Some(ui) = weak.upgrade()
                && let Err(error) = crate::desktop::present_window(ui.window.window())
            {
                ui.window
                    .set_notice(format!("No se pudo abrir Apariencia: {error}").into());
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_preview(move || {
            if let Some(ui) = weak.upgrade() {
                match ui.read() {
                    Ok(next) => {
                        ui.stop_transition_preview();
                        ui.current.set(next);
                        ui.window.set_values(next.summary().into());
                        ui.window
                            .set_notice("Vista previa · guarda para conservar al reiniciar".into());
                    }
                    Err(error) => ui.window.set_notice(error.to_string().into()),
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_save(move || {
            if let Some(ui) = weak.upgrade() {
                match ui.read().and_then(|next| {
                    sender.try_send(PreferenceUpdate::Appearance(next))?;
                    Ok(())
                }) {
                    Ok(()) => {
                        ui.window.set_saving(true);
                        ui.window.set_notice("Guardando apariencia…".into());
                    }
                    Err(error) => ui.window.set_notice(error.to_string().into()),
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_restore_saved(move || {
            if let Some(ui) = weak.upgrade() {
                ui.populate(ui.saved.get());
                ui.window
                    .set_notice("Se restauró el ajuste guardado".into());
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_reset(move || {
            if let Some(ui) = weak.upgrade() {
                ui.populate(Settings::default());
                ui.window
                    .set_notice("Valores iniciales en vista previa · sin guardar".into());
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_preview_transition(move || {
            if let Some(ui) = weak.upgrade() {
                let selected = ui.current.get();
                let mut sample = selected;
                sample.blur_radius = if selected.blur_radius < 15 { 25 } else { 0 };
                sample.brightness_percent = if selected.brightness_percent <= 100 {
                    125
                } else {
                    75
                };
                ui.transition_preview.set(Some(sample));
                ui.window
                    .set_notice("Probando ida y vuelta · tus valores se conservan".into());
                let weak = Rc::downgrade(&ui);
                ui.preview_timer.start(
                    slint::TimerMode::SingleShot,
                    std::time::Duration::from_millis(selected.transition_ms as u64 + 400),
                    move || {
                        if let Some(ui) = weak.upgrade() {
                            ui.transition_preview.set(None);
                            ui.window.set_notice(
                                "Prueba terminada · valores originales restaurados".into(),
                            );
                        }
                    },
                );
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.window.on_copy_values(move || {
            if let Some(ui) = weak.upgrade() {
                match crate::desktop::copy_text(&ui.current.get().summary()) {
                    Ok(()) => ui
                        .window
                        .set_notice("Valores copiados · puedes pegarlos en el chat".into()),
                    Err(error) => ui.window.set_notice(error.to_string().into()),
                }
            }
        });
        Ok(ui)
    }

    fn stop_transition_preview(&self) {
        self.preview_timer.stop();
        self.transition_preview.set(None);
    }

    fn populate(&self, value: Settings) {
        self.stop_transition_preview();
        self.window.set_blur_radius(value.blur_radius as f32);
        self.window
            .set_saturation_percent(value.saturation_percent as f32);
        self.window
            .set_brightness_percent(value.brightness_percent as f32);
        self.window.set_tint_percent(value.tint_percent as f32);
        self.window.set_border_percent(value.border_percent as f32);
        self.window
            .set_border_width_hundredths(value.border_width_hundredths as f32);
        self.window.set_transition_ms(value.transition_ms as f32);
        self.window.set_values(value.summary().into());
        self.current.set(value);
    }

    fn read(&self) -> Result<Settings> {
        let value = Settings {
            blur_radius: self.window.get_blur_radius().round() as i32,
            saturation_percent: self.window.get_saturation_percent().round() as i32,
            brightness_percent: self.window.get_brightness_percent().round() as i32,
            tint_percent: self.window.get_tint_percent().round() as i32,
            border_percent: self.window.get_border_percent().round() as i32,
            border_width_hundredths: self.window.get_border_width_hundredths().round() as i32,
            transition_ms: self.window.get_transition_ms().round() as i32,
        };
        value.validate()?;
        Ok(value)
    }

    #[cfg(target_os = "macos")]
    pub fn current(&self) -> crate::desktop::NativeFaceStyle {
        self.transition_preview
            .get()
            .unwrap_or_else(|| self.current.get())
            .native()
    }

    pub fn saved(&self, value: Settings, result: std::result::Result<(), String>) {
        self.window.set_saving(false);
        match result {
            Ok(()) => {
                self.saved.set(value);
                self.window.set_notice("Apariencia guardada".into());
            }
            Err(error) => self
                .window
                .set_notice(format!("No se pudo guardar: {error}").into()),
        }
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        let _ = self.window.hide();
    }
}
