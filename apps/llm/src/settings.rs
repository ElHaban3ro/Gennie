use gennie_core::Result;
use gennie_db::Database;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub read_files: bool,
    pub write_files: bool,
    pub run_commands: bool,
    pub local_actions: bool,
    pub model: String,
    pub effort: String,
    pub personality: String,
    pub instructions: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            read_files: true,
            write_files: true,
            run_commands: true,
            local_actions: true,
            model: "gpt-6-luna".into(),
            effort: "low".into(),
            personality: String::new(),
            instructions: String::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        crate::architects::validate_text(&self.personality, 4000)?;
        crate::architects::validate_text(&self.instructions, 64000)?;
        if self.model.trim() != self.model
            || self.model.is_empty()
            || self.model.len() > 100
            || !self
                .model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err("Identificador de modelo inválido".into());
        }
        if !["low", "medium", "high", "xhigh", "max", "ultra"].contains(&self.effort.as_str()) {
            return Err("Esfuerzo de razonamiento inválido".into());
        }
        if self.write_files && !self.read_files {
            return Err("Para escribir archivos también debe estar activada la lectura".into());
        }
        if self.local_actions && !self.run_commands {
            return Err("El control de otras apps requiere activar los comandos locales".into());
        }
        Ok(())
    }
}

pub(crate) fn load(db: &Database) -> Result<Settings> {
    crate::architects::runtime_settings(db, crate::architects::Role::Bot)
}

pub(crate) fn save(db: &Database, settings: &Settings) -> Result<Settings> {
    settings.validate()?;
    let mut bot = crate::architects::load(db, crate::architects::Role::Bot)?;
    bot.read_files = settings.read_files;
    bot.write_files = settings.write_files;
    bot.run_commands = settings.run_commands;
    bot.local_actions = settings.local_actions;
    bot.model = settings.model.clone();
    bot.effort = Some(settings.effort.clone());
    // La API anterior solo edita esos seis campos; conserva la personalización avanzada.
    Ok(crate::architects::save(db, &bot)?.settings())
}

impl Settings {
    /// Texto propio y editable de la instancia, sin sumar un rol oculto.
    pub fn agent_instructions(&self) -> String {
        format!(
            "Personalidad:\n{}\n\nInstrucciones:\n{}",
            self.personality, self.instructions
        )
    }
    /// Las sesiones auxiliares añaden el contrato de la operación al mismo perfil.
    pub fn role_instructions(&self, base: &str) -> String {
        format!(
            "{}\n\nContrato de esta operación:\n{base}",
            self.agent_instructions()
        )
    }
}
