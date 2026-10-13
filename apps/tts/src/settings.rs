use soc_core::Result;
use soc_db::{Database, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub voice: String,
    pub speed_percent: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            voice: "ef_dora".into(),
            speed_percent: 100,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !["ef_dora", "em_alex", "em_santa"].contains(&self.voice.as_str()) {
            return Err("Voz de Kokoro no admitida".into());
        }
        if !(70..=130).contains(&self.speed_percent) {
            return Err("Velocidad: entre 70 y 130 %".into());
        }
        Ok(())
    }
}
pub(crate) fn load(db: &Database) -> Result<Settings> {
    let settings = db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT enabled,voice,speed_percent FROM tts_preferences WHERE id=1",
            [],
            |r| {
                Ok(Settings {
                    enabled: r.get(0)?,
                    voice: r.get(1)?,
                    speed_percent: r.get(2)?,
                })
            },
        )?)
    })?;
    settings.validate()?;
    Ok(settings)
}
pub(crate) fn save(db: &Database, settings: &Settings) -> Result<()> {
    settings.validate()?;
    db.with_connection(|c| {
        let tx = c.transaction()?;
        if tx.execute("UPDATE tts_preferences SET enabled=?1,voice=?2,speed_percent=?3,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1", params![settings.enabled, settings.voice, settings.speed_percent])? != 1 {
            return Err("No se encontraron los ajustes de voz de SOC".into());
        }
        tx.commit()?;
        Ok(())
    })
}
