use soc_db::{Database, Result, params};
pub fn load(db: &Database) -> Result<bool> {
    db.with_connection(|c| {
        Ok(c.query_row(
            "SELECT reduced_motion FROM presence_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    })
}
pub fn save(db: &Database, reduced: bool) -> Result<()> {
    db.with_connection(|c|{c.execute("UPDATE presence_preferences SET reduced_motion=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1",params![reduced])?;Ok(())})
}
#[cfg(test)]
mod tests {
    use super::*;
    use soc_core::AppModule;
    #[test]
    fn preference_survives_reopening() {
        let path = std::env::temp_dir().join(format!(
            "soc-preferences-{}-{:?}.sqlite3",
            std::process::id(),
            std::thread::current().id()
        ));
        let db = Database::open(&path, 50).unwrap();
        db.migrate("presence", crate::PresenceModule::default().migrations())
            .unwrap();
        assert!(!load(&db).unwrap());
        save(&db, true).unwrap();
        db.close().unwrap();
        let db = Database::open(&path, 50).unwrap();
        db.migrate("presence", crate::PresenceModule::default().migrations())
            .unwrap();
        assert!(load(&db).unwrap());
        db.close().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
