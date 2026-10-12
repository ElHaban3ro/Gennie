use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
pub use rusqlite::{Connection as SqliteConnection, params};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct Database(Arc<Mutex<Option<Connection>>>);
#[derive(Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub sql: &'static str,
}

impl Database {
    pub fn open(path: &Path, busy_timeout_ms: u64) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        Self::configure(connection, busy_timeout_ms)
    }
    pub fn memory() -> Result<Self> {
        Self::configure(Connection::open_in_memory()?, 50)
    }
    fn configure(connection: Connection, busy_timeout_ms: u64) -> Result<Self> {
        connection.busy_timeout(Duration::from_millis(busy_timeout_ms))?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        Ok(Self(Arc::new(Mutex::new(Some(connection)))))
    }
    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        let mut guard = self.0.lock().map_err(|_| "Bloqueo SQLite inválido")?;
        operation(guard.as_mut().ok_or("SQLite ya cerrada")?)
    }
    pub fn close(&self) -> Result<()> {
        let mut guard = self.0.lock().map_err(|_| "Bloqueo SQLite inválido")?;
        if let Some(connection) = guard.take()
            && let Err((connection, error)) = connection.close()
        {
            *guard = Some(connection);
            return Err(error.into());
        }
        Ok(())
    }
    pub fn migrate(&self, module: &str, migrations: &[Migration]) -> Result<()> {
        if module.is_empty()
            || !module
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        {
            return Err("Identificador de migraciones inválido".into());
        }
        let table = format!("_soc_migrations_{module}");
        self.with_connection(|connection| {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(&format!("CREATE TABLE IF NOT EXISTS {table} (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')));"))?;
            let history: Vec<(i64,String)> = tx.prepare(&format!("SELECT version,checksum FROM {table} ORDER BY version"))?.query_map([], |row| Ok((row.get(0)?,row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            if migrations.windows(2).any(|m| m[0].version >= m[1].version) || migrations.iter().any(|m|m.version<=0) { return Err("Versiones de migración desordenadas".into()); }
            for (version, stored_checksum) in &history {
                let migration = migrations.iter().find(|m| m.version == *version).ok_or("El binario no reconoce una migración aplicada; downgrade no soportado")?;
                if *stored_checksum != checksum(migration.sql) { return Err(format!("Migración inmutable alterada: {module}/{version}").into()); }
            }
            for migration in migrations {
                let existing: Option<String> = tx.query_row(&format!("SELECT checksum FROM {table} WHERE version=?1"), [migration.version], |row|row.get(0)).optional()?;
                if existing.is_none() {
                    if history.last().is_some_and(|(v,_)| *v > migration.version) { return Err("Migración añadida fuera de orden".into()); }
                    tx.execute_batch(migration.sql)?;
                    tx.execute(&format!("INSERT INTO {table}(version,checksum) VALUES(?1,?2)"),params![migration.version, checksum(migration.sql)])?;
                    eprintln!("event=migration_applied module={module} version={}", migration.version);
                }
            }
            tx.commit()?; Ok(())
        })
    }
}

fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrations_are_atomic_immutable_and_namespaced() {
        let db = Database::memory().unwrap();
        let m = [Migration {
            version: 1,
            sql: "CREATE TABLE sample(id INTEGER PRIMARY KEY);",
        }];
        db.migrate("one", &m).unwrap();
        db.migrate("one", &m).unwrap();
        assert!(
            db.migrate(
                "one",
                &[Migration {
                    version: 1,
                    sql: "CREATE TABLE sample(id INTEGER);"
                }]
            )
            .is_err()
        );
        assert!(db.migrate("bad-name", &m).is_err());
        assert!(
            db.migrate(
                "two",
                &[Migration {
                    version: 1,
                    sql: "CREATE TABLE transient(id INTEGER); invalid SQL;"
                }]
            )
            .is_err()
        );
        db.with_connection(|c| { let count:i64=c.query_row("SELECT count(*) FROM sqlite_master WHERE name IN ('transient','_soc_migrations_two')",[],|r|r.get(0))?; assert_eq!(count,0); Ok(()) }).unwrap();
        db.migrate(
            "two",
            &[Migration {
                version: 1,
                sql: "CREATE TABLE other(id INTEGER);",
            }],
        )
        .unwrap();
        assert!(db.migrate("one", &[]).is_err());
    }
    #[test]
    fn handles_share_shutdown() {
        let db = Database::memory().unwrap();
        let clone = db.clone();
        db.close().unwrap();
        db.close().unwrap();
        assert!(clone.with_connection(|_| Ok(())).is_err());
    }
}
