use gennie_core::Result;
use gennie_db::{Database, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Hold,
    Toggle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub shortcut: String,
    pub mode: Mode,
    pub bubble_seconds: u64,
    pub unload_seconds: u64,
    pub microphone_name: Option<String>,
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.shortcut.len() > 100 || self.shortcut.contains(['\n', '\r']) {
            return Err("Atajo inválido".into());
        }
        self.shortcut
            .parse::<handy_keys::Hotkey>()
            .map_err(|_| "Usa un atajo como Ctrl+Option+Space")?;
        if !(2..=30).contains(&self.bubble_seconds) {
            return Err("Borrado: entre 2 y 30 segundos".into());
        }
        if self.unload_seconds > 600 {
            return Err("Descarga de memoria: entre 0 y 10 minutos".into());
        }
        if self
            .microphone_name
            .as_ref()
            .is_some_and(|name| name.is_empty() || name.len() > 512 || name.contains(['\n', '\r']))
        {
            return Err("Micrófono inválido".into());
        }
        Ok(())
    }
}
pub fn load(db: &Database) -> Result<Settings> {
    let settings = db.with_connection(|c| Ok(c.query_row(
        "SELECT enabled,shortcut,mode,bubble_seconds,unload_seconds,microphone_name FROM transcription_preferences WHERE id=1", [],
        |r| Ok(Settings { enabled: r.get(0)?, shortcut: r.get(1)?, mode: if r.get::<_,String>(2)? == "hold" { Mode::Hold } else { Mode::Toggle }, bubble_seconds: r.get::<_,u32>(3)? as u64, unload_seconds: r.get::<_,u32>(4)? as u64, microphone_name: r.get(5)? })
    )?))?;
    settings.validate()?;
    Ok(settings)
}
pub fn save(db: &Database, settings: &Settings) -> Result<()> {
    settings.validate()?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        tx.execute("UPDATE transcription_preferences SET enabled=?1,shortcut=?2,mode=?3,bubble_seconds=?4,unload_seconds=?5,microphone_name=?6,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1", params![settings.enabled,settings.shortcut,if settings.mode == Mode::Hold { "hold" } else { "toggle" },settings.bubble_seconds as i64,settings.unload_seconds as i64,settings.microphone_name])?;
        tx.commit()?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    #[test]
    fn migration_preserves_user_settings_and_updates_idle_timeout() {
        let db = Database::memory().unwrap();
        let module = crate::TranscriptionModule::default();
        db.migrate("transcription", &module.migrations()[..1])
            .unwrap();
        db.with_connection(|c| { c.execute("UPDATE transcription_preferences SET shortcut='Shift', mode='toggle', enabled=0, bubble_seconds=8, unload_seconds=31", [])?; Ok(()) }).unwrap();
        db.migrate("transcription", module.migrations()).unwrap();
        let settings = load(&db).unwrap();
        assert_eq!(settings.shortcut, "Shift");
        assert_eq!(settings.mode, Mode::Toggle);
        assert!(!settings.enabled);
        assert_eq!(settings.bubble_seconds, 8);
        assert_eq!(settings.unload_seconds, 600);
        assert_eq!(settings.microphone_name, None);
        let mut invalid = settings.clone();
        invalid.unload_seconds = 601;
        assert!(save(&db, &invalid).is_err());
        assert_eq!(load(&db).unwrap(), settings);
    }
    #[test]
    fn bubble_default_upgrade_preserves_other_preferences() {
        let module = crate::TranscriptionModule::default();
        for previous_seconds in [5, 8, 15] {
            let db = Database::memory().unwrap();
            db.migrate("transcription", &module.migrations()[..2])
                .unwrap();
            let mut previous = load(&db).unwrap();
            previous.enabled = false;
            previous.shortcut = "Shift".into();
            previous.mode = Mode::Toggle;
            previous.bubble_seconds = previous_seconds;
            previous.unload_seconds = 120;
            previous.microphone_name = Some("Micrófono USB".into());
            save(&db, &previous).unwrap();
            db.migrate("transcription", module.migrations()).unwrap();
            let mut expected = previous;
            if previous_seconds == 5 {
                expected.bubble_seconds = 15;
            }
            assert_eq!(load(&db).unwrap(), expected);
            db.migrate("transcription", module.migrations()).unwrap();
            assert_eq!(load(&db).unwrap(), expected);

            // Comprobar también los defaults SQL, no solo la fila migrada.
            db.with_connection(|c| {
                c.execute("DELETE FROM transcription_preferences WHERE id=1", [])?;
                c.execute("INSERT INTO transcription_preferences(id) VALUES (1)", [])?;
                Ok(())
            })
            .unwrap();
            let defaults = load(&db).unwrap();
            assert_eq!(defaults.bubble_seconds, 15);
            assert_eq!(defaults.unload_seconds, 600);
        }
    }
    #[test]
    fn settings_reopen_and_reject_invalid_values() {
        let path = std::env::temp_dir().join(format!(
            "gennie-transcription-{}.sqlite3",
            std::process::id()
        ));
        let db = Database::open(&path, 50).unwrap();
        db.migrate(
            "transcription",
            crate::TranscriptionModule::default().migrations(),
        )
        .unwrap();
        let mut s = load(&db).unwrap();
        assert_eq!(s.bubble_seconds, 15);
        assert_eq!(s.unload_seconds, 600);
        assert_eq!(s.microphone_name, None);
        s.microphone_name = Some("Micrófono USB".into());
        s.mode = Mode::Toggle;
        s.unload_seconds = 0;
        s.shortcut = "Ctrl+Shift+F9".into();
        save(&db, &s).unwrap();
        db.close().unwrap();
        let db = Database::open(&path, 50).unwrap();
        assert_eq!(load(&db).unwrap(), s);
        s.bubble_seconds = 0;
        assert!(save(&db, &s).is_err());
        assert_eq!(load(&db).unwrap().bubble_seconds, 15);
        db.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
