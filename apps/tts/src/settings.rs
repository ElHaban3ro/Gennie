use gennie_core::Result;
use gennie_db::{Database, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub provider: String,
    pub inworld_voice: String,
    pub inworld_model: String,
    pub inworld_language: String,
    pub inworld_delivery: String,
    pub inworld_normalization: String,
    pub enabled: bool,
    pub voice: String,
    pub speed_percent: u32,
    pub unload_seconds: Option<u64>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "kokoro".into(),
            inworld_voice: "Ashley".into(),
            inworld_model: "inworld-tts-2-flash".into(),
            inworld_language: "es-CO".into(),
            inworld_delivery: "BALANCED".into(),
            inworld_normalization: "AUTO".into(),
            enabled: true,
            voice: "ef_dora".into(),
            speed_percent: 100,
            unload_seconds: Some(600),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !["inworld-tts-2", "inworld-tts-2-flash"].contains(&self.inworld_model.as_str())
            || !["STABLE", "BALANCED", "CREATIVE"].contains(&self.inworld_delivery.as_str())
            || !["AUTO", "ON", "OFF"].contains(&self.inworld_normalization.as_str())
        {
            return Err("Opciones de Inworld inválidas".into());
        }
        if self.inworld_language.len() > 35
            || !self
                .inworld_language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("Idioma de Inworld inválido".into());
        }
        if !["kokoro", "inworld"].contains(&self.provider.as_str()) {
            return Err("Proveedor de voz no admitido".into());
        }
        if self.inworld_voice.trim().is_empty()
            || self.inworld_voice.len() > 128
            || self.inworld_voice.chars().any(char::is_control)
        {
            return Err("Identificador de voz de Inworld inválido".into());
        }
        if !["ef_dora", "em_alex", "em_santa"].contains(&self.voice.as_str()) {
            return Err("Voz de Kokoro no admitida".into());
        }
        let range = if self.provider == "inworld" {
            50..=150
        } else {
            70..=130
        };
        if !range.contains(&self.speed_percent) {
            return Err("Velocidad fuera del rango del proveedor".into());
        }
        if self
            .unload_seconds
            .is_some_and(|seconds| !(60..=7200).contains(&seconds) || seconds % 60 != 0)
        {
            return Err("Kokoro: elige entre 1 y 120 minutos o mantener cargado".into());
        }
        Ok(())
    }
}
pub(crate) fn load(db: &Database) -> Result<Settings> {
    let settings = db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT enabled,voice,speed_percent,unload_seconds,provider,inworld_voice,inworld_model,inworld_language,inworld_delivery,inworld_normalization FROM tts_preferences WHERE id=1",
            [],
            |r| {
                Ok(Settings {
                    inworld_model: r.get(6)?,
                    inworld_language: r.get(7)?,
                    inworld_delivery: r.get(8)?,
                    inworld_normalization: r.get(9)?,
                    provider: r.get(4)?,
                    inworld_voice: r.get(5)?,
                    enabled: r.get(0)?,
                    voice: r.get(1)?,
                    speed_percent: r.get(2)?,
                    unload_seconds: r.get::<_, Option<u32>>(3)?.map(u64::from),
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
        if tx.execute("UPDATE tts_preferences SET enabled=?1,voice=?2,speed_percent=?3,unload_seconds=?4,provider=?5,inworld_voice=?6,inworld_model=?7,inworld_language=?8,inworld_delivery=?9,inworld_normalization=?10,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1", params![settings.enabled, settings.voice, settings.speed_percent, settings.unload_seconds.map(|seconds| seconds as i64), settings.provider, settings.inworld_voice, settings.inworld_model, settings.inworld_language, settings.inworld_delivery, settings.inworld_normalization])? != 1 {
            return Err("No se encontraron los ajustes de voz de Gennie".into());
        }
        tx.commit()?;
        Ok(())
    })
}
