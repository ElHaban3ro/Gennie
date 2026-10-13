use soc_core::Result;
use soc_db::Database;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub read_files: bool,
    pub write_files: bool,
    pub run_commands: bool,
    pub local_actions: bool,
    pub model: String,
    pub effort: String,
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
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
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
    db.with_connection(|connection| {
        let settings = connection.query_row(
            "SELECT read_files,write_files,run_commands,local_actions,model,effort FROM llm_preferences WHERE id=1",
            [],
            |row| Ok(Settings {
                read_files: row.get(0)?,
                write_files: row.get(1)?,
                run_commands: row.get(2)?,
                local_actions: row.get(3)?,
                model: row.get(4)?,
                effort: row.get(5)?,
            }),
        )?;
        settings.validate()?;
        Ok(settings)
    })
}

pub(crate) fn save(db: &Database, settings: &Settings) -> Result<()> {
    settings.validate()?;
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        let changed = tx.execute(
            "UPDATE llm_preferences SET read_files=?1,write_files=?2,run_commands=?3,local_actions=?4,model=?5,effort=?6,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",
            soc_db::params![settings.read_files, settings.write_files, settings.run_commands, settings.local_actions, settings.model, settings.effort],
        )?;
        if changed != 1 {
            return Err("No existe la fila de ajustes de conversación".into());
        }
        tx.commit()?;
        Ok(())
    })
}
