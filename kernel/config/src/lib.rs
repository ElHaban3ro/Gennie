use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_path: PathBuf,
    pub db_busy_timeout_ms: u64,
}
// Secreto opcional, separado de Config y nunca incluido en AppState o Debug.
#[derive(Default)]
pub struct Secrets {
    pub openrouter_api_key: Option<String>,
}
impl Secrets {
    pub fn load(env_file: &Path) -> Result<Self> {
        let mut key = None;
        if env_file.exists() {
            for entry in dotenvy::from_path_iter(env_file).map_err(|_| "No se pudo leer .env")? {
                let (name, value) = entry.map_err(|_| "Sintaxis inválida en .env")?;
                if name == "GENNIE_OPENROUTER_API_KEY" {
                    key = Some(value);
                }
            }
        }
        if let Ok(value) = std::env::var("GENNIE_OPENROUTER_API_KEY") {
            key = Some(value);
        }
        Ok(Self {
            openrouter_api_key: key.filter(|s| !s.trim().is_empty()),
        })
    }
}

pub const VARIABLES: &[(&str, Option<&str>)] = &[
    ("GENNIE_DATABASE_PATH", None),
    ("GENNIE_DB_BUSY_TIMEOUT_MS", Some("500")),
    ("GENNIE_OPENROUTER_API_KEY", None),
];

impl Config {
    pub fn load(env_file: &Path) -> Result<(Self, Secrets)> {
        Self::load_with(env_file, |key| std::env::var(key).ok())
    }
    fn load_with(
        env_file: &Path,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<(Self, Secrets)> {
        let mut values = BTreeMap::new();
        if env_file.exists() {
            for entry in dotenvy::from_path_iter(env_file).map_err(|_| "No se pudo leer .env")? {
                let (key, value) = entry.map_err(|_| "Sintaxis inválida en .env")?;
                if let Some(exported) = lookup(&key) {
                    if exported != value {
                        eprintln!("event=config_shadowed key={key}");
                    }
                } else {
                    values.insert(key, value);
                }
            }
        }
        for (key, _) in VARIABLES {
            if let Some(value) = lookup(key) {
                values.insert((*key).into(), value);
            }
        }
        Ok((
            Self::from_values(&values)?,
            Secrets {
                openrouter_api_key: values
                    .get("GENNIE_OPENROUTER_API_KEY")
                    .filter(|s| !s.trim().is_empty())
                    .cloned(),
            },
        ))
    }

    pub fn from_values(values: &BTreeMap<String, String>) -> Result<Self> {
        let path = values
            .get("GENNIE_DATABASE_PATH")
            .filter(|s| !s.trim().is_empty())
            .ok_or("GENNIE_DATABASE_PATH es obligatoria y no puede estar vacía")?;
        if path == ":memory:" {
            return Err("GENNIE_DATABASE_PATH debe ser un archivo persistente".into());
        }
        let timeout = values
            .get("GENNIE_DB_BUSY_TIMEOUT_MS")
            .map(String::as_str)
            .unwrap_or("500")
            .parse::<u64>()
            .map_err(|_| "GENNIE_DB_BUSY_TIMEOUT_MS debe ser un entero")?;
        if !(1..=5000).contains(&timeout) {
            return Err("GENNIE_DB_BUSY_TIMEOUT_MS debe estar entre 1 y 5000".into());
        }
        Ok(Self {
            database_path: path.into(),
            db_busy_timeout_ms: timeout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_required_path_and_timeout() {
        let mut values = BTreeMap::new();
        assert!(Config::from_values(&values).is_err());
        values.insert("GENNIE_DATABASE_PATH".into(), "test.sqlite3".into());
        assert_eq!(
            Config::from_values(&values).unwrap().db_busy_timeout_ms,
            500
        );
        values.insert("GENNIE_DB_BUSY_TIMEOUT_MS".into(), "0".into());
        assert!(Config::from_values(&values).is_err());
    }
    #[test]
    fn example_matches_declared_variables() {
        let example = include_str!("../../../.env.example");
        let entries: BTreeMap<_, _> = example
            .lines()
            .filter(|s| !s.starts_with('#') && s.contains('='))
            .map(|s| s.split_once('=').unwrap())
            .collect();
        assert_eq!(entries.len(), VARIABLES.len());
        for (key, default) in VARIABLES {
            assert!(entries.contains_key(key));
            if let Some(default) = default {
                assert_eq!(entries[key], *default);
            }
        }
    }
    #[test]
    fn optional_openrouter_secret_obeys_environment_precedence() {
        let path = std::env::temp_dir().join(format!("gennie-secret-test-{}", std::process::id()));
        std::fs::write(
            &path,
            "GENNIE_DATABASE_PATH=file.sqlite3\nGENNIE_OPENROUTER_API_KEY=file-key\n",
        )
        .unwrap();
        let (_, secrets) = Config::load_with(&path, |key| {
            (key == "GENNIE_OPENROUTER_API_KEY").then(|| "env-key".into())
        })
        .unwrap();
        assert_eq!(secrets.openrouter_api_key.as_deref(), Some("env-key"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn environment_overrides_file_without_global_mutation() {
        let path = std::env::temp_dir().join(format!("gennie-env-{}", std::process::id()));
        std::fs::write(
            &path,
            "GENNIE_DATABASE_PATH=file.sqlite3\nGENNIE_DB_BUSY_TIMEOUT_MS=100\n",
        )
        .unwrap();
        let (config, _) = Config::load_with(&path, |key| {
            if key == "GENNIE_DATABASE_PATH" {
                Some("export.sqlite3".into())
            } else {
                None
            }
        })
        .unwrap();
        assert_eq!(config.database_path, PathBuf::from("export.sqlite3"));
        assert_eq!(config.db_busy_timeout_ms, 100);
        std::fs::remove_file(path).unwrap();
    }
}
