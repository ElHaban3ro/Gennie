//! Arquitectos base y catálogo ampliable; configuración y permisos efectivos.
use crate::Settings;
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use serde::{Deserialize, Serialize};

/// Estado esperado de habilitación, separado de fallos de lectura o validación.
#[derive(Debug)]
pub struct HelperDisabled;
impl std::fmt::Display for HelperDisabled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("El helper está desactivado")
    }
}
impl std::error::Error for HelperDisabled {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Bot,
    Helper,
    Guide,
}
impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bot => "bot",
            Self::Helper => "helper",
            Self::Guide => "guide",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Bot => "gennie",
            Self::Helper => "plugin_builder",
            Self::Guide => "proactivity_guide",
        }
    }
    fn provider(self) -> &'static str {
        match self {
            Self::Bot | Self::Helper => "codex_chatgpt",
            Self::Guide => "openrouter_decisions",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub role: String,
    pub revision: i64,
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
    pub personality: String,
    pub instructions: String,
    pub read_files: bool,
    pub write_files: bool,
    pub run_commands: bool,
    pub local_actions: bool,
}
impl Profile {
    pub fn is_base(&self) -> bool {
        [Role::Bot, Role::Helper, Role::Guide]
            .iter()
            .any(|role| role.id() == self.id)
    }
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.id.len() > 100 || self.revision < 1 {
            return Err("Identidad o proveedor de arquitecto inválido".into());
        }
        validate_label(&self.name, 200)?;
        validate_label(&self.role, 80)?;
        if let Some(role) = [Role::Bot, Role::Helper, Role::Guide]
            .into_iter()
            .find(|role| role.id() == self.id)
            && (self.role != role.as_str() || self.provider != role.provider())
        {
            return Err("No se puede cambiar el tipo o proveedor de un arquitecto base".into());
        }
        if !["codex_chatgpt", "openrouter_decisions"].contains(&self.provider.as_str())
            || (matches!(self.role.as_str(), "bot" | "helper") && self.provider != "codex_chatgpt")
            || (self.role == "guide" && self.provider != "openrouter_decisions")
        {
            return Err("Tipo y proveedor de arquitecto incompatibles".into());
        }
        validate_text(&self.personality, 1000)?;
        validate_text(&self.instructions, 6000)?;
        if self.provider == "openrouter_decisions" {
            if self.effort.is_some()
                || self.read_files
                || self.write_files
                || self.run_commands
                || self.local_actions
                || !["typesafe/jev-1.14", "openai/gpt-6-luna-decisions"]
                    .contains(&self.model.as_str())
            {
                return Err("La guía solo admite clasificación remota sin herramientas".into());
            }
        } else {
            if self.id == Role::Bot.id() && !self.enabled {
                return Err("El bot se desactiva desde el módulo de conversación".into());
            }
            self.settings().validate()?;
        }
        Ok(())
    }
    pub(crate) fn settings(&self) -> Settings {
        Settings {
            read_files: self.read_files,
            write_files: self.write_files,
            run_commands: self.run_commands,
            local_actions: self.local_actions,
            model: self.model.clone(),
            effort: self.effort.clone().unwrap_or_default(),
            personality: self.personality.clone(),
            instructions: self.instructions.clone(),
        }
    }
}
fn validate_label(text: &str, max: usize) -> Result<()> {
    if text.trim().is_empty()
        || text.trim() != text
        || text.chars().count() > max
        || text.chars().any(char::is_control)
    {
        return Err("Nombre o tipo de arquitecto vacío, demasiado largo o inválido".into());
    }
    Ok(())
}
pub(crate) fn validate_text(text: &str, max: usize) -> Result<()> {
    if text.chars().count() > max
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
    {
        return Err("Personalización demasiado larga o con caracteres no admitidos".into());
    }
    Ok(())
}
fn read(connection: &SqliteConnection, id: &str) -> Result<Profile> {
    let profile = connection.query_row(
        "SELECT id,revision,enabled,provider,model,effort,personality,instructions,read_files,write_files,run_commands,local_actions,name,role FROM llm_architect_profiles WHERE id=?1",
        params![id],
        |r| Ok(Profile {id:r.get(0)?, revision:r.get(1)?, enabled:r.get(2)?,provider:r.get(3)?,model:r.get(4)?,effort:r.get(5)?,personality:r.get(6)?,instructions:r.get(7)?,read_files:r.get(8)?,write_files:r.get(9)?,run_commands:r.get(10)?,local_actions:r.get(11)?,name:r.get(12)?,role:r.get(13)?}),
    )?;
    profile.validate()?;
    Ok(profile)
}
pub fn load(db: &Database, role: Role) -> Result<Profile> {
    load_by_id(db, role.id())
}
/// Las integraciones pueden referenciar cualquier instancia sin reemplazar las bases.
pub fn load_by_id(db: &Database, id: &str) -> Result<Profile> {
    db.with_connection(|c| read(c, id))
}
pub fn list(db: &Database) -> Result<Vec<Profile>> {
    db.with_connection(|c| {
        let mut statement = c.prepare("SELECT id FROM llm_architect_profiles ORDER BY CASE id WHEN 'gennie' THEN 0 WHEN 'plugin_builder' THEN 1 WHEN 'proactivity_guide' THEN 2 ELSE 3 END, updated_at, id")?;
        let ids = statement.query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| read(c, id))
            .collect()
    })
}
/// Lee perfil y límite del bot bajo el mismo lock; no añade permisos a una delegación.
pub fn runtime_settings(db: &Database, role: Role) -> Result<Settings> {
    runtime_settings_by_id(db, role.id())
}
/// Ajustes reutilizables, siempre limitados por los permisos del bot principal.
pub fn runtime_settings_by_id(db: &Database, id: &str) -> Result<Settings> {
    db.with_connection(|c| {
        let profile = read(c, id)?;
        if profile.provider != "codex_chatgpt" {
            return Err("Este arquitecto no ejecuta sesiones Codex".into());
        }
        if !profile.enabled {
            return Err(Box::new(HelperDisabled));
        }
        let mut settings = profile.settings();
        let ceiling = read(c, Role::Bot.id())?;
        settings.read_files &= ceiling.read_files;
        settings.write_files &= ceiling.write_files;
        settings.run_commands &= ceiling.run_commands;
        settings.local_actions &= ceiling.local_actions;
        settings.validate()?;
        Ok(settings)
    })
}
/// El revision recibido es la revisión esperada; devuelve el perfil confirmado.
pub fn save(db: &Database, profile: &Profile) -> Result<Profile> {
    profile.validate()?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let table = if profile.is_base() { "llm_architects" } else { "llm_architect_instances" };
        let changed = tx.execute(
            &format!("UPDATE {table} SET revision=revision+1,enabled=?1,model=?2,effort=?3,personality=?4,instructions=?5,read_files=?6,write_files=?7,run_commands=?8,local_actions=?9,name=?12,role=?13,provider=?14,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?10 AND revision=?11"),
            params![profile.enabled,profile.model,profile.effort,profile.personality,profile.instructions,profile.read_files,profile.write_files,profile.run_commands,profile.local_actions,profile.id,profile.revision,profile.name,profile.role,profile.provider],
        )?;
        if changed != 1 { return Err("El arquitecto cambió. Recarga su configuración antes de guardar".into()); }
        if profile.id == Role::Bot.id() {
            let changed = tx.execute("UPDATE llm_preferences SET read_files=?1,write_files=?2,run_commands=?3,local_actions=?4,model=?5,effort=?6,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",params![profile.read_files,profile.write_files,profile.run_commands,profile.local_actions,profile.model,profile.effort])?;
            if changed != 1 { return Err("No existe la fila de ajustes de conversación".into()); }
        }
        let saved = read(&tx, &profile.id)?;
        tx.commit()?;
        Ok(saved)
    })
}

/// Crear no activa trabajos, chats o consumidores; la identidad la genera el host.
pub fn create(db: &Database, draft: &Profile) -> Result<Profile> {
    db.with_connection(|c| {
        let tx = c.transaction()?;
        let mut profile = draft.clone();
        profile.id = tx.query_row("SELECT 'architect_' || lower(hex(randomblob(16)))", [], |r| r.get(0))?;
        profile.revision = 1;
        profile.validate()?;
        tx.execute("INSERT INTO llm_architect_instances (id,name,role,provider,revision,enabled,model,effort,personality,instructions,read_files,write_files,run_commands,local_actions) VALUES (?1,?2,?3,?4,1,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![profile.id,profile.name,profile.role,profile.provider,profile.enabled,profile.model,profile.effort,profile.personality,profile.instructions,profile.read_files,profile.write_files,profile.run_commands,profile.local_actions])?;
        let saved = read(&tx, &profile.id)?;
        tx.commit()?;
        Ok(saved)
    })
}

pub fn remove(db: &Database, id: &str, revision: i64) -> Result<()> {
    db.with_connection(|c| {
        let changed = c.execute(
            "DELETE FROM llm_architect_instances WHERE id=?1 AND revision=?2",
            params![id, revision],
        )?;
        if changed != 1 {
            return Err(
                "No se puede eliminar una base o una instancia modificada; recarga el catálogo"
                    .into(),
            );
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
