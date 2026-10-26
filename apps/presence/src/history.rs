use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
};

pub(crate) struct Journal {
    pub db: Database,
    pub revision: Arc<AtomicU64>,
}

impl gennie_llm::ConversationJournal for Journal {
    fn is_empty(&self, provider: &str, thread_id: &str) -> Result<bool> {
        self.db.with_connection(|connection| {
            Ok(connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM presence_conversations c JOIN presence_current_conversation active ON active.conversation_id=c.id WHERE c.provider=?1 AND c.provider_thread_id=?2 AND c.deleted=0 AND NOT EXISTS(SELECT 1 FROM presence_history h WHERE h.conversation_id=c.id))",
                params![provider, thread_id],
                |row| row.get(0),
            )?)
        })
    }
    fn activate(
        &self,
        connection: &SqliteConnection,
        provider: &str,
        thread_id: &str,
        title: Option<&str>,
    ) -> Result<()> {
        connection.execute("INSERT INTO presence_conversations(provider,provider_thread_id,title) VALUES(?1,?2,'Nuevo chat') ON CONFLICT(provider,provider_thread_id) DO NOTHING", params![provider,thread_id])?;
        connection.execute("INSERT INTO presence_current_conversation(id,conversation_id) SELECT 1,id FROM presence_conversations WHERE provider=?1 AND provider_thread_id=?2 ON CONFLICT(id) DO UPDATE SET conversation_id=excluded.conversation_id", params![provider,thread_id])?;
        if let Some(title) = title {
            let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
            let title = if title.is_empty() { None } else { Some(title) };
            connection.execute("UPDATE presence_conversations SET provider_title=?3 WHERE provider=?1 AND provider_thread_id=?2 AND deleted=0",params![provider,thread_id,title])?;
        }
        Ok(())
    }
    fn append(&self, provider: &str, thread_id: &str, speaker: &str, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let inserted = self.db.with_connection(|connection| {
            let tx = connection.transaction()?;
            let (id, deleted): (i64, bool) = tx.query_row("SELECT id,deleted FROM presence_conversations WHERE provider=?1 AND provider_thread_id=?2", params![provider,thread_id], |row| Ok((row.get(0)?,row.get(1)?)))?;
            // Una respuesta ya en curso no debe recrear un chat eliminado.
            if deleted && speaker != "user" { return Ok(false); }
            if deleted { tx.execute("UPDATE presence_conversations SET deleted=0 WHERE id=?1", [id])?; }
            tx.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,?2,?3)",params![id,speaker,text])?;
            tx.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[id])?;
            tx.commit()?;
            Ok(true)
        })?;
        if inserted {
            self.committed();
        }
        Ok(())
    }
    fn claim_title_input(
        &self,
        provider: &str,
        thread_id: &str,
    ) -> Result<Option<(String, String)>> {
        self.db.with_connection(|connection| {
            let tx = connection.transaction()?;
            let input = {
                let mut statement = tx.prepare("SELECT c.id,u.text,a.text FROM presence_conversations c JOIN presence_history u ON u.id=(SELECT min(id) FROM presence_history WHERE conversation_id=c.id AND speaker='user') JOIN presence_history a ON a.id=(SELECT min(id) FROM presence_history WHERE conversation_id=c.id AND speaker='assistant' AND id>u.id) WHERE c.provider=?1 AND c.provider_thread_id=?2 AND c.deleted=0 AND c.local_title IS NULL AND c.provider_title IS NULL AND c.title_generation_attempted=0")?;
                let mut rows = statement.query(params![provider, thread_id])?;
                match rows.next()? {
                    Some(row) => Some((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
                    None => None,
                }
            };
            if let Some((id, user, assistant)) = input {
                tx.execute("UPDATE presence_conversations SET title_generation_attempted=1 WHERE id=?1", [id])?;
                tx.commit()?;
                Ok(Some((user, assistant)))
            } else {
                Ok(None)
            }
        })
    }
    fn update_title(&self, provider: &str, thread_id: &str, title: &str) -> Result<()> {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = if title.is_empty() { None } else { Some(title) };
        let changed = self.db.with_connection(|connection| {
            Ok(connection.execute("UPDATE presence_conversations SET provider_title=?3 WHERE provider=?1 AND provider_thread_id=?2 AND deleted=0 AND provider_title IS NOT ?3",params![provider,thread_id,title])?)
        })?;
        if changed > 0 {
            self.committed();
        }
        Ok(())
    }
    fn committed(&self) {
        self.revision.fetch_add(1, Ordering::Release);
    }
}

enum Command {
    Load(Option<i64>),
    Rename(i64, String),
    Delete(i64),
    Shutdown,
}
#[derive(Clone, Copy)]
pub enum Mutation {
    Rename,
    Delete,
}
pub enum Event {
    Loaded(Option<i64>, std::result::Result<Snapshot, String>),
    Changed,
    Modified(i64, Mutation, std::result::Result<(), String>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub speaker: String,
    pub text: String,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub updated_at: String,
    pub last_message: Option<String>,
}
pub struct Snapshot {
    pub messages: Vec<Message>,
    pub conversations: Vec<Conversation>,
    pub selected_id: Option<i64>,
    pub active_id: Option<i64>,
    pub selected_messages: Vec<Message>,
}
pub struct Store {
    sender: SyncSender<Command>,
    events: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
    revision: Arc<AtomicU64>,
    observed: std::cell::Cell<u64>,
}
impl Store {
    pub fn start(db: Database, revision: Arc<AtomicU64>) -> Result<Self> {
        let (sender, commands) = mpsc::sync_channel(8);
        let (events_sender, events) = mpsc::channel();
        let worker_revision = revision.clone();
        let handle = std::thread::Builder::new()
            .name("presence-history".into())
            .spawn(move || {
                while let Ok(command) = commands.recv() {
                    match command {
                        Command::Load(selected) => {
                            let _ = events_sender.send(Event::Loaded(
                                selected,
                                load(&db, selected).map_err(|error| error.to_string()),
                            ));
                        }
                        Command::Rename(id, title) => {
                            let result = rename(&db, id, &title).map_err(|error| error.to_string());
                            if result.is_ok() {
                                worker_revision.fetch_add(1, Ordering::Release);
                            }
                            let _ =
                                events_sender.send(Event::Modified(id, Mutation::Rename, result));
                        }
                        Command::Delete(id) => {
                            let result = delete(&db, id).map_err(|error| error.to_string());
                            if result.is_ok() {
                                worker_revision.fetch_add(1, Ordering::Release);
                            }
                            let _ =
                                events_sender.send(Event::Modified(id, Mutation::Delete, result));
                        }
                        Command::Shutdown => break,
                    }
                }
            })?;
        Ok(Self {
            sender,
            events,
            handle: Some(handle),
            revision,
            observed: std::cell::Cell::new(0),
        })
    }
    pub fn load(&self, selected: Option<i64>) -> std::result::Result<(), &'static str> {
        self.sender
            .try_send(Command::Load(selected))
            .map_err(|_| "No se pudo cargar el historial; vuelve a intentarlo")
    }
    pub fn rename(&self, id: i64, title: &str) -> std::result::Result<(), &'static str> {
        self.sender
            .try_send(Command::Rename(id, title.to_owned()))
            .map_err(|_| "No se pudo guardar el nombre; vuelve a intentarlo")
    }
    pub fn delete(&self, id: i64) -> std::result::Result<(), &'static str> {
        self.sender
            .try_send(Command::Delete(id))
            .map_err(|_| "No se pudo eliminar el chat; vuelve a intentarlo")
    }
    pub fn poll(&self) -> Vec<Event> {
        let mut events: Vec<_> = self.events.try_iter().collect();
        let revision = self.revision.load(Ordering::Acquire);
        if revision != self.observed.replace(revision) {
            events.push(Event::Changed);
        }
        events
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
pub fn rename(db: &Database, id: i64, title: &str) -> Result<()> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || title.chars().count() > 200 {
        return Err("Escribe un nombre de entre 1 y 200 caracteres".into());
    }
    db.with_connection(|connection| {
        if connection.execute(
            "UPDATE presence_conversations SET local_title=?2 WHERE id=?1 AND deleted=0",
            params![id, title],
        )? != 1
        {
            return Err("El chat ya no está disponible".into());
        }
        Ok(())
    })
}
pub fn delete(db: &Database, id: i64) -> Result<()> {
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        if tx.execute("UPDATE presence_conversations SET deleted=1,title='Nuevo chat',automatic_title=1,local_title=NULL,provider_title=NULL WHERE id=?1 AND deleted=0",[id])? != 1 {return Err("El chat ya no está disponible".into());}
        tx.execute("DELETE FROM presence_history WHERE conversation_id=?1",[id])?;
        tx.commit()?;
        Ok(())
    })
}
fn messages(connection: &SqliteConnection, id: Option<i64>) -> Result<Vec<Message>> {
    let mut statement = connection.prepare(
        "SELECT speaker,text,created_at FROM presence_history WHERE conversation_id=?1 ORDER BY id",
    )?;
    let rows = statement.query_map([id], |row| {
        Ok(Message {
            speaker: row.get(0)?,
            text: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
/// Registra un aviso local sin reactivar chats eliminados ni contactar al proveedor.
/// Los errores del proveedor pueden haber sido guardados por su propio worker.
pub fn record_system_message(
    db: &Database,
    conversation_id: Option<i64>,
    text: &str,
    may_already_exist: bool,
) -> Result<bool> {
    db.with_connection(|connection| {
        let tx = connection.transaction()?;
        let id = match conversation_id {
            Some(id) => {
                let visible: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM presence_conversations WHERE id=?1 AND deleted=0)",
                    [id], |row| row.get(0),
                )?;
                if !visible { return Ok(false); }
                id
            }
            None => {
                tx.execute("INSERT INTO presence_conversations(title,automatic_title) VALUES('Avisos del sistema',0)", [])?;
                tx.last_insert_rowid()
            }
        };
        if may_already_exist {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM presence_history WHERE id=(SELECT max(id) FROM presence_history WHERE conversation_id=?1) AND speaker='system' AND text=?2)",
                params![id,text], |row| row.get(0),
            )?;
            if exists { return Ok(true); }
        }
        tx.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'system',?2)", params![id,text])?;
        tx.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(true)
    })
}

/// Destino explícito e idempotente para avances de trabajos; nunca cambia el chat activo.
pub fn deliver_message(
    db: &Database,
    conversation: i64,
    event_key: &str,
    text: &str,
) -> Result<()> {
    if event_key.is_empty() || event_key.len() > 200 || text.is_empty() || text.len() > 4000 {
        return Err("Mensaje de trabajo inválido".into());
    }
    db.with_connection(|c| {
        let tx=c.transaction()?;
        let visible:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM presence_conversations WHERE id=?1 AND deleted=0)",[conversation],|r|r.get(0))?;
        if !visible {return Ok(());}
        let inserted=tx.execute("INSERT OR IGNORE INTO presence_message_deliveries(event_key,conversation_id) VALUES(?1,?2)",params![event_key,conversation])?;
        if inserted>0 {
            tx.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(?1,'system',?2)",params![conversation,text])?;
            tx.execute("UPDATE presence_conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[conversation])?;
        }
        tx.commit()?;Ok(())
    })
}
pub fn conversation_for_thread(db: &Database, thread: &str) -> Result<i64> {
    db.with_connection(|c|Ok(c.query_row("SELECT id FROM presence_conversations WHERE provider='codex' AND provider_thread_id=?1 AND deleted=0",[thread],|r|r.get(0))?))
}
pub fn conversation_exists(db: &Database, id: i64) -> Result<bool> {
    db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT EXISTS(SELECT 1 FROM presence_conversations WHERE id=?1 AND deleted=0)",
            [id],
            |r| r.get(0),
        )?)
    })
}

pub fn load(db: &Database, selected: Option<i64>) -> Result<Snapshot> {
    db.with_connection(|connection| {
        // El índice (conversation_id,id) resuelve el último mensaje sin cargar
        // todos los historiales. El límite afecta solo a la vista previa.
        let mut statement = connection.prepare("SELECT c.id,coalesce(c.local_title,c.provider_title,CASE WHEN c.provider IS NULL THEN c.title ELSE 'Nuevo chat' END),c.updated_at,(SELECT substr(h.text,1,512) FROM presence_history h WHERE h.conversation_id=c.id ORDER BY h.id DESC LIMIT 1) FROM presence_conversations c WHERE c.deleted=0 ORDER BY c.updated_at DESC,c.id DESC")?;
        let rows = statement.query_map([], |row| Ok(Conversation {id:row.get(0)?,title:row.get(1)?,updated_at:row.get(2)?,last_message:row.get(3)?}))?;
        let conversations: Vec<_> = rows.collect::<std::result::Result<_,_>>()?;
        let active = connection.prepare("SELECT c.id FROM presence_current_conversation a JOIN presence_conversations c ON c.id=a.conversation_id WHERE a.id=1 AND c.deleted=0")?.query_map([], |row|row.get::<_,i64>(0))?.next().transpose()?;
        let selected_id = selected.filter(|id| conversations.iter().any(|chat|chat.id==*id)).or(active).or_else(|| conversations.first().map(|chat| chat.id));
        Ok(Snapshot {messages:messages(connection,active)?,selected_messages:messages(connection,selected_id)?,conversations,selected_id,active_id:active})
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_core::AppModule;
    use gennie_llm::ConversationJournal;

    fn journal(db: &Database) -> Journal {
        Journal {
            db: db.clone(),
            revision: Arc::default(),
        }
    }
    fn activate(journal: &Journal, id: &str) {
        journal
            .db
            .with_connection(|connection| {
                let tx = connection.transaction()?;
                journal.activate(&tx, "codex", id, None)?;
                tx.commit()?;
                Ok(())
            })
            .unwrap();
        journal.committed();
    }
    fn migrate(db: &Database) {
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
    }
    #[test]
    fn plugin_messages_keep_the_origin_and_are_idempotent_after_chat_changes() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "first");
        let first = conversation_for_thread(&db, "first").unwrap();
        activate(&journal, "second");
        let second = conversation_for_thread(&db, "second").unwrap();
        deliver_message(&db, first, "build:1", "Primer avance").unwrap();
        deliver_message(&db, first, "build:1", "Primer avance").unwrap();
        deliver_message(&db, second, "build:2", "Segundo avance").unwrap();
        let one = load(&db, Some(first)).unwrap();
        assert_eq!(one.active_id, Some(second));
        assert_eq!(one.selected_messages.len(), 1);
        assert_eq!(one.selected_messages[0].text, "Primer avance");
        delete(&db, first).unwrap();
        deliver_message(&db, first, "build:3", "Respuesta tardía").unwrap();
        assert!(!conversation_exists(&db, first).unwrap());
        assert_eq!(load(&db, None).unwrap().conversations.len(), 1);
    }
    #[test]
    fn failed_plugin_message_rolls_back_delivery_receipt() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "first");
        let id = conversation_for_thread(&db, "first").unwrap();
        db.with_connection(|c|{c.execute_batch("CREATE TRIGGER reject_plugin_message BEFORE INSERT ON presence_history BEGIN SELECT RAISE(ABORT,'blocked'); END;")?;Ok(())}).unwrap();
        assert!(deliver_message(&db, id, "build:1", "Avance").is_err());
        db.with_connection(|c| {
            c.execute_batch("DROP TRIGGER reject_plugin_message")?;
            Ok(())
        })
        .unwrap();
        deliver_message(&db, id, "build:1", "Avance").unwrap();
        assert_eq!(load(&db, Some(id)).unwrap().selected_messages.len(), 1);
    }
    #[test]
    fn system_notices_rollback_on_failure_and_do_not_revive_deleted_chats() {
        let db = Database::memory().unwrap();
        migrate(&db);
        db.with_connection(|connection| {
            connection.execute_batch("CREATE TRIGGER reject_notice BEFORE INSERT ON presence_history BEGIN SELECT RAISE(ABORT,'blocked'); END;")?;
            Ok(())
        }).unwrap();
        assert!(record_system_message(&db, None, "Error: fallo", false).is_err());
        assert!(load(&db, None).unwrap().conversations.is_empty());
        db.with_connection(|connection| {
            connection.execute_batch("DROP TRIGGER reject_notice")?;
            Ok(())
        })
        .unwrap();
        assert!(record_system_message(&db, None, "Error: fallo", false).unwrap());
        let snapshot = load(&db, None).unwrap();
        let id = snapshot.selected_id.unwrap();
        assert!(snapshot.active_id.is_none());
        assert_eq!(snapshot.selected_messages[0].speaker, "system");
        delete(&db, id).unwrap();
        assert!(!record_system_message(&db, Some(id), "Error: tardío", false).unwrap());
        assert!(load(&db, None).unwrap().conversations.is_empty());
    }

    #[test]
    fn only_visible_active_chats_without_any_messages_can_be_reused() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        assert!(!journal.is_empty("codex", "missing").unwrap());
        activate(&journal, "first");
        assert!(journal.is_empty("codex", "first").unwrap());
        let first = load(&db, None).unwrap().active_id.unwrap();
        rename(&db, first, "Nombre propio").unwrap();
        assert!(journal.is_empty("codex", "first").unwrap());
        activate(&journal, "second");
        assert!(!journal.is_empty("codex", "first").unwrap());
        // Consultar otra conversación no debe alterar cuál se reutiliza.
        load(&db, Some(first)).unwrap();
        assert!(journal.is_empty("codex", "second").unwrap());
        assert!(!journal.is_empty("other", "second").unwrap());
        for speaker in ["user", "assistant", "system"] {
            activate(&journal, speaker);
            journal
                .append("codex", speaker, speaker, "Contenido")
                .unwrap();
            assert!(!journal.is_empty("codex", speaker).unwrap());
        }
        activate(&journal, "deleted");
        let deleted = load(&db, None).unwrap().active_id.unwrap();
        delete(&db, deleted).unwrap();
        assert!(!journal.is_empty("codex", "deleted").unwrap());
        db.close().unwrap();
        assert!(journal.is_empty("codex", "deleted").is_err());
    }

    #[test]
    fn catalog_preview_tracks_last_message_of_every_speaker_without_switching_chats() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "first");
        let first = load(&db, None).unwrap().active_id.unwrap();
        for speaker in ["user", "assistant", "system"] {
            journal.append("codex", "first", speaker, speaker).unwrap();
            let snapshot = load(&db, None).unwrap();
            assert_eq!(
                snapshot.conversations[0].last_message.as_deref(),
                Some(speaker)
            );
        }
        let text = "🦀á".repeat(400);
        journal
            .append("codex", "first", "assistant", &text)
            .unwrap();
        activate(&journal, "second");
        let snapshot = load(&db, Some(first)).unwrap();
        let empty = snapshot
            .conversations
            .iter()
            .find(|c| c.id != first)
            .unwrap();
        assert!(empty.last_message.is_none());
        let preview = snapshot
            .conversations
            .iter()
            .find(|c| c.id == first)
            .unwrap()
            .last_message
            .as_ref()
            .unwrap();
        assert_eq!(preview.chars().count(), 512);
        assert!(text.starts_with(preview));
        assert_eq!(snapshot.selected_messages.last().unwrap().text, text);
        assert_eq!(snapshot.active_id, Some(empty.id));
    }
    #[test]
    fn migration_keeps_legacy_rows_without_inventing_a_provider_link() {
        let db = Database::memory().unwrap();
        let module = crate::PresenceModule::default();
        db.migrate(module.name(), &module.migrations()[..2])
            .unwrap();
        db.with_connection(|connection| {
            for (speaker,text) in [("user","Recuerdo guardado · #1\nOrigen: Chat"),("system","Nuevo chat iniciado"),("system","Recuperado (1) · #1: Dato\nOrigen: Chat"),("assistant","Respuesta normal")] {
                connection.execute("INSERT INTO presence_history(speaker,text,created_at) VALUES(?1,?2,'2026-10-12T10:00:00.000Z')",params![speaker,text])?;
            }
            Ok(())
        }).unwrap();
        migrate(&db);
        let snapshot = load(&db, None).unwrap();
        assert_eq!(snapshot.conversations.len(), 1);
        assert_eq!(snapshot.conversations[0].title, "Historial anterior");
        assert_eq!(snapshot.selected_messages.len(), 3);
        assert!(snapshot.messages.is_empty());
        assert_eq!(snapshot.selected_messages[0].speaker, "user");
        assert_eq!(snapshot.selected_messages[1].text, "Nuevo chat iniciado");
        assert_eq!(snapshot.selected_messages[2].text, "Respuesta normal");
        assert_eq!(
            snapshot.selected_messages[2].created_at,
            "2026-10-12T10:00:00.000Z"
        );
        db.with_connection(|connection| {
            assert!(connection.query_row("SELECT provider IS NULL AND provider_thread_id IS NULL FROM presence_conversations",[],|row|row.get::<_,bool>(0))?);
            assert!(connection.execute("INSERT INTO presence_history(conversation_id,speaker,text) VALUES(999,'user','Invalid')",[]).is_err());
            Ok(())
        }).unwrap();
        migrate(&db);
        assert_eq!(load(&db, None).unwrap().conversations.len(), 1);
    }
    #[test]
    fn independent_chats_titles_and_reading_do_not_switch_the_active_chat() {
        let db = Database::memory().unwrap();
        migrate(&db);
        assert!(load(&db, None).unwrap().conversations.is_empty());
        let journal = journal(&db);
        activate(&journal, "first");
        journal
            .append("codex", "first", "user", "  Organiza\n mi día  ")
            .unwrap();
        journal
            .append(
                "codex",
                "first",
                "assistant",
                "[Agenda](https://example.com)",
            )
            .unwrap();
        let first = load(&db, None).unwrap().selected_id.unwrap();
        activate(&journal, "second");
        journal
            .append("codex", "second", "user", "Otra conversación")
            .unwrap();
        journal
            .append("codex", "second", "system", "Error: sin conexión")
            .unwrap();
        let second = load(&db, None).unwrap().selected_id.unwrap();
        let snapshot = load(&db, Some(first)).unwrap();
        assert_eq!(snapshot.conversations.len(), 2);
        assert_eq!(snapshot.conversations[0].id, second);
        assert_eq!(snapshot.selected_messages.len(), 2);
        assert_eq!(
            snapshot.selected_messages[1].text,
            "[Agenda](https://example.com)"
        );
        assert_eq!(snapshot.messages[0].text, "Otra conversación");
        assert_eq!(snapshot.messages[1].speaker, "system");
        assert_eq!(load(&db, None).unwrap().selected_id, Some(second));
        activate(&journal, "first");
        journal
            .append("codex", "first", "user", "No cambia el título")
            .unwrap();
        let snapshot = load(&db, None).unwrap();
        assert_eq!(
            snapshot
                .conversations
                .iter()
                .find(|chat| chat.id == first)
                .unwrap()
                .title,
            "Nuevo chat"
        );
        assert_eq!(snapshot.messages.len(), 3);
        assert!(
            journal
                .append("codex", "missing", "user", "Sin chat")
                .is_err()
        );
        assert!(
            journal
                .append("codex", "first", "tool", "No permitido")
                .is_err()
        );
        assert_eq!(load(&db, None).unwrap().messages.len(), 3);
    }
    #[test]
    fn messages_and_selection_survive_reopen_and_worker_loads_off_main() {
        let path = std::env::temp_dir().join(format!(
            "gennie-chat-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Database::open(&path, 500).unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "thread");
        journal
            .append("codex", "thread", "user", &"ñ".repeat(90))
            .unwrap();
        journal
            .append("codex", "thread", "assistant", "Respuesta final")
            .unwrap();
        let store = Store::start(db.clone(), journal.revision.clone()).unwrap();
        assert!(
            store
                .poll()
                .iter()
                .any(|event| matches!(event, Event::Changed))
        );
        assert!(store.poll().is_empty());
        store.load(None).unwrap();
        match store
            .events
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
        {
            Event::Loaded(None, Ok(snapshot)) => {
                assert_eq!(snapshot.messages.len(), 2);
                assert_eq!(snapshot.conversations[0].title, "Nuevo chat");
            }
            _ => panic!("Lectura fallida"),
        }
        drop(store);
        db.close().unwrap();
        let reopened = Database::open(&path, 500).unwrap();
        migrate(&reopened);
        let snapshot = load(&reopened, None).unwrap();
        assert_eq!(snapshot.messages[0].text, "ñ".repeat(90));
        assert_eq!(snapshot.messages[1].text, "Respuesta final");
        let revision = journal.revision.load(Ordering::Acquire);
        assert!(
            journal
                .append("codex", "thread", "user", "Fallará")
                .is_err()
        );
        assert_eq!(journal.revision.load(Ordering::Acquire), revision);
        reopened.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn title_generation_claim_is_once_and_respects_names_and_deleted_chats() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = Journal {
            db: db.clone(),
            revision: Arc::new(AtomicU64::new(0)),
        };
        for thread in ["greeting", "renamed", "provider", "deleted"] {
            activate(&journal, thread);
            journal
                .append("codex", thread, "system", "Aviso previo")
                .unwrap();
            assert!(
                journal
                    .claim_title_input("codex", thread)
                    .unwrap()
                    .is_none()
            );
            journal
                .append("codex", thread, "user", "Qué onda pa")
                .unwrap();
            assert!(
                journal
                    .claim_title_input("codex", thread)
                    .unwrap()
                    .is_none()
            );
            journal
                .append("codex", thread, "assistant", "¡Qué onda! ¿Cómo vas?")
                .unwrap();
            let id = load(&db, None).unwrap().active_id.unwrap();
            match thread {
                "renamed" => rename(&db, id, "Mi chat").unwrap(),
                "provider" => journal
                    .update_title("codex", thread, "Nombre recibido")
                    .unwrap(),
                "deleted" => delete(&db, id).unwrap(),
                _ => {}
            }
        }
        assert_eq!(
            journal.claim_title_input("codex", "greeting").unwrap(),
            Some(("Qué onda pa".into(), "¡Qué onda! ¿Cómo vas?".into()))
        );
        for thread in ["greeting", "renamed", "provider", "deleted", "missing"] {
            assert!(
                journal
                    .claim_title_input("codex", thread)
                    .unwrap()
                    .is_none()
            );
        }
        // Un renombrado durante la petición gana al resultado posterior.
        let id: i64 = db
            .with_connection(|c| {
                Ok(c.query_row(
                    "SELECT id FROM presence_conversations WHERE provider_thread_id='greeting'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        rename(&db, id, "Nombre elegido mientras se generaba").unwrap();
        journal
            .update_title("codex", "greeting", "Saludo informal")
            .unwrap();
        assert_eq!(
            load(&db, Some(id))
                .unwrap()
                .conversations
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .title,
            "Nombre elegido mientras se generaba"
        );
    }

    #[test]
    fn title_generation_attempt_survives_reopening() {
        let path =
            std::env::temp_dir().join(format!("gennie-title-{}.sqlite3", std::process::id()));
        let db = Database::open(&path, 50).unwrap();
        migrate(&db);
        let journal = Journal {
            db: db.clone(),
            revision: Arc::new(AtomicU64::new(0)),
        };
        activate(&journal, "once");
        journal.append("codex", "once", "user", "Hola").unwrap();
        journal
            .append("codex", "once", "assistant", "Hola de nuevo")
            .unwrap();
        let before = load(&db, None).unwrap();
        assert!(
            journal
                .claim_title_input("codex", "once")
                .unwrap()
                .is_some()
        );
        assert_eq!(
            load(&db, None).unwrap().conversations[0].updated_at,
            before.conversations[0].updated_at
        );
        db.close().unwrap();
        let reopened = Database::open(&path, 50).unwrap();
        migrate(&reopened);
        let journal = Journal {
            db: reopened.clone(),
            revision: Arc::new(AtomicU64::new(0)),
        };
        assert!(
            journal
                .claim_title_input("codex", "once")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            load(&reopened, None).unwrap().conversations[0].title,
            "Nuevo chat"
        );
        reopened.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn user_message_never_becomes_the_chat_title() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "title");
        journal
            .append(
                "codex",
                "title",
                "user",
                "Hola, cuéntame sobre el cohete más grande de la historia",
            )
            .unwrap();
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Nuevo chat"
        );
        // Los títulos provisionales de versiones anteriores tampoco deben mostrarse.
        db.with_connection(|connection| {
            connection.execute("UPDATE presence_conversations SET title='Primer mensaje antiguo',automatic_title=0", [])?;
            Ok(())
        }).unwrap();
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Nuevo chat"
        );
        journal
            .update_title("codex", "title", "Historia de los cohetes")
            .unwrap();
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Historia de los cohetes"
        );
        let id = load(&db, None).unwrap().selected_id.unwrap();
        rename(&db, id, "Mi investigación").unwrap();
        journal
            .update_title("codex", "title", "Otro título")
            .unwrap();
        journal
            .append("codex", "title", "user", "Otra pregunta")
            .unwrap();
        activate(&journal, "title");
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Mi investigación"
        );
    }
    #[test]
    fn provider_names_and_local_renames_preserve_messages_and_activity() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "named");
        journal
            .append("codex", "named", "user", "Título provisional")
            .unwrap();
        journal
            .append("codex", "named", "assistant", "Contenido intacto")
            .unwrap();
        let before = load(&db, None).unwrap();
        let id = before.selected_id.unwrap();
        journal
            .update_title("codex", "named", "Nombre de Codex")
            .unwrap();
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Nombre de Codex"
        );
        rename(&db, id, "  Mi nombre\n personal  ").unwrap();
        journal
            .update_title("codex", "named", "Otro nombre de Codex")
            .unwrap();
        journal.update_title("codex", "named", "   ").unwrap();
        let after = load(&db, None).unwrap();
        assert_eq!(after.conversations[0].title, "Mi nombre personal");
        assert_eq!(before.messages, after.messages);
        assert_eq!(
            before.conversations[0].updated_at,
            after.conversations[0].updated_at
        );
        assert!(rename(&db, id, "  ").is_err());
        assert!(rename(&db, id, &"ñ".repeat(201)).is_err());
        assert!(rename(&db, 999, "Desaparecido").is_err());
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Mi nombre personal"
        );
    }
    #[test]
    fn local_deletion_erases_messages_and_ignores_a_late_reply_without_touching_provider() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let module = gennie_llm::LlmModule::default();
        db.migrate(module.name(), module.migrations()).unwrap();
        db.with_connection(|c| {
            c.execute(
                "INSERT INTO codex_conversation(id,thread_id) VALUES(1,'active')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let journal = journal(&db);
        activate(&journal, "old");
        journal.append("codex", "old", "user", "Conservar").unwrap();
        let old = load(&db, None).unwrap().selected_id.unwrap();
        activate(&journal, "active");
        journal.append("codex", "active", "user", "Borrar").unwrap();
        let id = load(&db, None).unwrap().selected_id.unwrap();
        rename(&db, id, "Privado").unwrap();
        delete(&db, id).unwrap();
        journal
            .append("codex", "active", "assistant", "Respuesta tardía")
            .unwrap();
        journal
            .update_title("codex", "active", "Nombre tardío")
            .unwrap();
        activate(&journal, "active");
        let snapshot = load(&db, Some(id)).unwrap();
        assert_eq!(snapshot.conversations.len(), 1);
        assert_eq!(snapshot.selected_id, Some(old));
        assert!(snapshot.messages.is_empty());
        assert_eq!(snapshot.selected_messages[0].text, "Conservar");
        assert!(delete(&db, id).is_err());
        db.with_connection(|c| {
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM presence_history WHERE conversation_id=?1",
                    [id],
                    |r| r.get::<_, i64>(0)
                )?,
                0
            );
            assert_eq!(
                c.query_row(
                    "SELECT thread_id FROM codex_conversation WHERE id=1",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "active"
            );
            Ok(())
        })
        .unwrap();
        journal
            .append("codex", "active", "user", "Un mensaje nuevo")
            .unwrap();
        let snapshot = load(&db, None).unwrap();
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].text, "Un mensaje nuevo");
        assert_eq!(
            snapshot
                .conversations
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .title,
            "Nuevo chat"
        );
    }
    #[test]
    fn rename_worker_confirms_commit_and_keeps_changes_after_reopen() {
        let path = std::env::temp_dir().join(format!(
            "gennie-rename-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Database::open(&path, 500).unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "chat");
        journal.append("codex", "chat", "user", "Mensaje").unwrap();
        let id = load(&db, None).unwrap().selected_id.unwrap();
        let store = Store::start(db.clone(), journal.revision.clone()).unwrap();
        store.rename(id, "Guardado local").unwrap();
        assert!(matches!(
            store
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            Event::Modified(_, Mutation::Rename, Ok(()))
        ));
        assert_eq!(
            load(&db, None).unwrap().conversations[0].title,
            "Guardado local"
        );
        drop(store);
        db.close().unwrap();
        let reopened = Database::open(&path, 500).unwrap();
        migrate(&reopened);
        assert_eq!(
            load(&reopened, None).unwrap().conversations[0].title,
            "Guardado local"
        );
        let store = Store::start(reopened.clone(), Arc::default()).unwrap();
        reopened.close().unwrap();
        store.rename(id, "Fallará").unwrap();
        assert!(matches!(
            store
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            Event::Modified(_, Mutation::Rename, Err(_))
        ));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_delete_rolls_back_the_catalogue_and_retains_messages() {
        let db = Database::memory().unwrap();
        migrate(&db);
        let journal = journal(&db);
        activate(&journal, "retained");
        journal
            .append("codex", "retained", "user", "Mensaje importante")
            .unwrap();
        let before = load(&db, None).unwrap();
        db.with_connection(|c| {c.execute_batch("CREATE TRIGGER reject_history_delete BEFORE DELETE ON presence_history BEGIN SELECT RAISE(ABORT,'delete failed'); END;")?;Ok(())}).unwrap();
        assert!(delete(&db, before.selected_id.unwrap()).is_err());
        let after = load(&db, None).unwrap();
        assert_eq!(before.conversations, after.conversations);
        assert_eq!(before.messages, after.messages);
    }
}
