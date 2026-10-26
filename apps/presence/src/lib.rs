// Datos del escritorio. La presentación vive en frontend y no posee SQLite.
mod answer;
pub mod history;
pub mod narration;
use gennie_core::{AppModule, AppState, Migration, Result};
use std::sync::{Arc, atomic::AtomicU64};
#[derive(Default)]
pub struct PresenceModule {
    revision: Arc<AtomicU64>,
}
impl PresenceModule {
    pub fn revision(&self) -> Arc<AtomicU64> {
        self.revision.clone()
    }
    pub fn conversation_journal(
        &self,
        db: gennie_db::Database,
    ) -> Arc<dyn gennie_llm::ConversationJournal> {
        Arc::new(history::Journal {
            db,
            revision: self.revision.clone(),
        })
    }
}
impl AppModule for PresenceModule {
    fn name(&self) -> &'static str {
        "presence"
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        &["transcription", "llm", "tts"]
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_preferences.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_history.sql"),
            },
            Migration {
                version: 3,
                sql: include_str!("../migrations/0003_remove_memory_activity.sql"),
            },
            Migration {
                version: 4,
                sql: include_str!("../migrations/0004_appearance.sql"),
            },
            Migration {
                version: 5,
                sql: include_str!("../migrations/0005_appearance_defaults.sql"),
            },
            Migration {
                version: 6,
                sql: include_str!("../migrations/0006_appearance_defaults.sql"),
            },
            Migration {
                version: 7,
                sql: include_str!("../migrations/0007_floating_visibility.sql"),
            },
            Migration {
                version: 8,
                sql: include_str!("../migrations/0008_conversations.sql"),
            },
            Migration {
                version: 9,
                sql: include_str!("../migrations/0009_conversation_actions.sql"),
            },
            Migration {
                version: 10,
                sql: include_str!("../migrations/0010_title_generation.sql"),
            },
            Migration {
                version: 11,
                sql: include_str!("../migrations/0011_plugin_messages.sql"),
            },
        ]
    }
    fn start(&self, _: &AppState) -> Result<()> {
        Ok(())
    }
    fn stop(&self, _: &AppState) -> Result<()> {
        Ok(())
    }
}
pub fn speech_text(raw: &str) -> String {
    answer::prepare(raw).text
}

pub fn reduced_motion(db: &gennie_db::Database) -> Result<bool> {
    db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT reduced_motion FROM presence_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    })
}
pub fn save_reduced_motion(db: &gennie_db::Database, value: bool) -> Result<()> {
    db.with_connection(|c|{c.execute("UPDATE presence_preferences SET reduced_motion=?1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",[value])?;Ok(())})
}
