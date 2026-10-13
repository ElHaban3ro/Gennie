use soc_core::Result;
use soc_db::{Database, params};
use std::{
    cell::{Cell, RefCell},
    sync::mpsc::{self, Receiver, Sender, SyncSender},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub preload: bool,
    /// None conserva el modelo en memoria hasta liberación explícita o cierre.
    pub unload_seconds: Option<u64>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            preload: true,
            unload_seconds: Some(600),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self
            .unload_seconds
            .is_some_and(|seconds| !(60..=7200).contains(&seconds) || seconds % 60 != 0)
        {
            return Err("Laya: elige entre 1 y 120 minutos o mantener cargado".into());
        }
        Ok(())
    }
    pub(crate) fn idle_expired(&self, elapsed: Duration) -> bool {
        self.unload_seconds
            .is_some_and(|seconds| elapsed >= Duration::from_secs(seconds))
    }
}
pub(crate) fn load(db: &Database) -> Result<Settings> {
    let settings = db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT preload,unload_seconds FROM decision_preferences WHERE id=1",
            [],
            |r| {
                Ok(Settings {
                    preload: r.get(0)?,
                    unload_seconds: r.get::<_, Option<u32>>(1)?.map(u64::from),
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
        if tx.execute("UPDATE decision_preferences SET preload=?1,unload_seconds=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1", params![settings.preload, settings.unload_seconds.map(|seconds| seconds as i64)])? != 1 {
            return Err("No se encontraron los ajustes de Laya".into());
        }
        tx.commit()?;
        Ok(())
    })
}

pub type SaveResult = std::result::Result<Settings, String>;
pub(crate) struct Client {
    saved: RefCell<Settings>,
    sender: SyncSender<Settings>,
    replies: Receiver<SaveResult>,
    saving: Cell<bool>,
}
impl Client {
    pub fn channel(initial: Settings) -> (Self, Receiver<Settings>, Sender<SaveResult>) {
        let (sender, requests) = mpsc::sync_channel(1);
        let (replies, receiver) = mpsc::channel();
        (
            Self {
                saved: RefCell::new(initial),
                sender,
                replies: receiver,
                saving: Cell::new(false),
            },
            requests,
            replies,
        )
    }
    pub fn settings(&self) -> Settings {
        self.saved.borrow().clone()
    }
    pub fn save(&self, next: Settings) -> Result<()> {
        next.validate()?;
        if self.saving.get() {
            return Err("Ya se están guardando los ajustes de Laya".into());
        }
        self.sender
            .try_send(next)
            .map_err(|_| "No se pudieron enviar los ajustes de Laya")?;
        self.saving.set(true);
        Ok(())
    }
    pub fn poll(&self) -> Option<SaveResult> {
        let result = match self.replies.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Disconnected) if self.saving.get() => {
                Err("El control de Laya se detuvo antes de confirmar el guardado".into())
            }
            Err(_) => return None,
        };
        self.saving.set(false);
        if let Ok(next) = &result {
            *self.saved.borrow_mut() = next.clone();
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soc_core::AppModule;

    #[test]
    fn settings_reopen_without_changing_attempts_and_reject_invalid_values() {
        let path =
            std::env::temp_dir().join(format!("soc-laya-settings-{}.sqlite3", std::process::id()));
        let db = Database::open(&path, 50).unwrap();
        let module = crate::DecisionsModule::default();
        db.migrate("decisions", &module.migrations()[..1]).unwrap();
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO decision_attempts(action,status) VALUES('open_app','verified')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db.migrate("decisions", module.migrations()).unwrap();
        assert_eq!(load(&db).unwrap(), Settings::default());
        let next = Settings {
            preload: false,
            unload_seconds: None,
        };
        save(&db, &next).unwrap();
        db.close().unwrap();
        let db = Database::open(&path, 50).unwrap();
        assert_eq!(load(&db).unwrap(), next);
        for seconds in [0, 59, 61, 7201, u64::MAX] {
            assert!(
                save(
                    &db,
                    &Settings {
                        unload_seconds: Some(seconds),
                        ..next.clone()
                    }
                )
                .is_err()
            );
        }
        assert_eq!(load(&db).unwrap(), next);
        assert_eq!(
            db.with_connection(|c| Ok(c.query_row(
                "SELECT status FROM decision_attempts",
                [],
                |r| r.get::<_, String>(0)
            )?))
            .unwrap(),
            "verified"
        );
        db.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn preferences_are_confirmed_only_after_commit_and_failure_keeps_previous_values() {
        let (client, requests, replies) = Client::channel(Settings::default());
        let next = Settings {
            preload: false,
            unload_seconds: None,
        };
        client.save(next.clone()).unwrap();
        assert!(client.save(next.clone()).is_err());
        assert_eq!(requests.recv().unwrap(), next);
        assert_eq!(client.settings(), Settings::default());
        replies.send(Err("fallo simulado".into())).unwrap();
        assert!(client.poll().unwrap().is_err());
        assert_eq!(client.settings(), Settings::default());
        client.save(next.clone()).unwrap();
        let _ = requests.recv().unwrap();
        replies.send(Ok(next.clone())).unwrap();
        assert_eq!(client.poll().unwrap().unwrap(), next);
        assert_eq!(client.settings(), next);
    }
}
