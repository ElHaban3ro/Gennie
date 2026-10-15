use soc_db::{Database, params};
use std::{
    sync::mpsc::{self, Receiver, SyncSender},
    thread::JoinHandle,
};

enum Command {
    Save(&'static str, String),
    Load,
    Shutdown,
}

pub enum Event {
    Loaded(std::result::Result<Snapshot, String>),
    Saved,
    SaveFailed(String),
}

pub struct Message {
    pub speaker: String,
    pub text: String,
    pub created_at: String,
}

pub struct Snapshot {
    pub messages: Vec<Message>,
}

#[derive(Clone)]
pub struct Writer(SyncSender<Command>);

impl Writer {
    pub fn save(&self, speaker: &'static str, text: &str) -> Result<(), &'static str> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.0
            .try_send(Command::Save(speaker, text.to_owned()))
            .map_err(|_| "No se pudo poner el mensaje en cola para el chat")
    }
}

pub struct Store {
    sender: SyncSender<Command>,
    events: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

impl Store {
    pub fn start(db: Database) -> soc_db::Result<Self> {
        let (sender, commands) = mpsc::sync_channel(64);
        let (events_sender, events) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("presence-history".into())
            .spawn(move || {
                while let Ok(command) = commands.recv() {
                    match command {
                        Command::Save(speaker, text) => match save(&db, speaker, &text) {
                            Ok(()) => {
                                let _ = events_sender.send(Event::Saved);
                            }
                            Err(error) => {
                                let _ = events_sender.send(Event::SaveFailed(error.to_string()));
                            }
                        },
                        Command::Load => {
                            let result = load(&db).map_err(|error| error.to_string());
                            let _ = events_sender.send(Event::Loaded(result));
                        }
                        Command::Shutdown => break,
                    }
                }
            })?;
        Ok(Self {
            sender,
            events,
            handle: Some(handle),
        })
    }

    pub fn writer(&self) -> Writer {
        Writer(self.sender.clone())
    }

    pub fn load(&self) -> Result<(), &'static str> {
        self.sender
            .try_send(Command::Load)
            .map_err(|_| "No se pudo abrir el chat")
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            eprintln!("event=history_worker_failed module=presence");
        }
    }
}

fn save(db: &Database, speaker: &str, text: &str) -> soc_db::Result<()> {
    db.with_connection(|connection| {
        connection.execute(
            "INSERT INTO presence_history(speaker,text) VALUES(?1,?2)",
            params![speaker, text],
        )?;
        Ok(())
    })
}

fn load(db: &Database) -> soc_db::Result<Snapshot> {
    db.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT speaker,text,created_at FROM presence_history ORDER BY id")?;
        let mut rows = statement.query([])?;
        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            let speaker: String = row.get(0)?;
            let text: String = row.get(1)?;
            let created_at: String = row.get(2)?;
            messages.push(Message {
                speaker,
                text,
                created_at,
            });
        }
        Ok(Snapshot { messages })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soc_core::AppModule;

    #[test]
    fn old_memory_notices_are_removed_without_erasing_chat_messages() {
        let db = Database::memory().unwrap();
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), &module.migrations()[..2])
            .unwrap();
        save(&db, "user", "Recuerdo guardado · #1\nOrigen: Chat").unwrap();
        save(&db, "system", "Nuevo chat iniciado").unwrap();
        save(&db, "system", "Recuperado (1) · #1: Dato\nOrigen: Chat").unwrap();
        save(&db, "system", "Recuerdo guardado · #2: Dato\nOrigen: Chat").unwrap();
        save(&db, "assistant", "Respuesta normal").unwrap();

        db.migrate(module.name(), module.migrations()).unwrap();
        let messages = load(&db).unwrap().messages;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].speaker, "user");
        assert_eq!(messages[1].text, "Nuevo chat iniciado");
        assert_eq!(messages[2].text, "Respuesta normal");
    }

    #[test]
    fn finalized_messages_survive_reopen_and_keep_order() {
        let path = std::env::temp_dir().join(format!(
            "soc-history-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Database::open(&path, 500).unwrap();
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        let store = Store::start(db.clone()).unwrap();
        let writer = store.writer();
        writer.save("user", "Una pregunta").unwrap();
        writer.save("assistant", "Una respuesta").unwrap();
        writer.save("system", "Nuevo chat iniciado").unwrap();
        store.load().unwrap();
        let mut saved = 0;
        let snapshot = loop {
            match store
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
            {
                Event::Saved => saved += 1,
                Event::Loaded(Ok(snapshot)) => break snapshot,
                _ => panic!("El chat no se cargó después de guardar"),
            }
        };
        assert_eq!(saved, 3);
        assert_eq!(snapshot.messages.len(), 3);
        assert_eq!(snapshot.messages[0].speaker, "user");
        assert_eq!(snapshot.messages[0].text, "Una pregunta");
        assert_eq!(snapshot.messages[1].speaker, "assistant");
        assert_eq!(snapshot.messages[1].text, "Una respuesta");
        assert_eq!(snapshot.messages[2].speaker, "system");
        assert_eq!(snapshot.messages[2].text, "Nuevo chat iniciado");
        drop(writer);
        drop(store);
        db.close().unwrap();
        let reopened = Database::open(&path, 500).unwrap();
        let snapshot = load(&reopened).unwrap();
        assert_eq!(snapshot.messages.len(), 3);
        assert_eq!(snapshot.messages[0].text, "Una pregunta");
        reopened.close().unwrap();
        let _ = std::fs::remove_file(path);
    }
}
