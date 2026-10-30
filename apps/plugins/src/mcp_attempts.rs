//! Diario mínimo de efectos MCP; ninguna transacción abarca el transporte.
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    pub id: i64,
    pub source_id: String,
    pub source_revision: i64,
    pub server_name: String,
    pub tool: String,
    pub state: String,
    pub reason: String,
    pub resolution: Option<String>,
    pub reviewed_at: Option<String>,
    pub created_at: String,
}
#[derive(Clone)]
pub struct Store(pub Database);
pub enum Reservation {
    Reserved(Attempt),
    Blocked(Attempt),
    Rejected(&'static str),
}
const COLUMNS: &str =
    "id,source_id,source_revision,server_name,tool,state,reason,resolution,reviewed_at,created_at";
macro_rules! attempt_row {
    () => {
        |r| {
            Ok(Attempt {
                id: r.get(0)?,
                source_id: r.get(1)?,
                source_revision: r.get(2)?,
                server_name: r.get(3)?,
                tool: r.get(4)?,
                state: r.get(5)?,
                reason: r.get(6)?,
                resolution: r.get(7)?,
                reviewed_at: r.get(8)?,
                created_at: r.get(9)?,
            })
        }
    };
}
fn get(c: &SqliteConnection, id: i64) -> Result<Attempt> {
    Ok(c.query_row(
        &format!("SELECT {COLUMNS} FROM plugin_mcp_attempts WHERE id=?1"),
        [id],
        attempt_row!(),
    )?)
}
impl Store {
    pub fn recover(&self) -> Result<()> {
        self.0.with_connection(|c| { c.execute("UPDATE plugin_mcp_attempts SET state='uncertain',reason='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='running'",[])?; Ok(()) })
    }
    pub fn pending_tool(&self, source: &str, tool: &str) -> Result<Option<Attempt>> {
        self.0.with_connection(|c| {
            let mut s=c.prepare(&format!("SELECT {COLUMNS} FROM plugin_mcp_attempts WHERE source_id=?1 AND tool=?2 AND state IN ('running','uncertain')"))?;
            Ok(s.query_map(params![source,tool],attempt_row!())?.next().transpose()?)
        })
    }
    pub fn pending_page(&self, source: &str, after: Option<i64>) -> Result<(Vec<Attempt>, i64)> {
        if after.is_some_and(|id| id < 1) {
            return Err("Cursor de intentos MCP inválido".into());
        }
        self.0.with_connection(|c| {
            let count=c.query_row("SELECT count(*) FROM plugin_mcp_attempts WHERE source_id=?1 AND state IN ('running','uncertain')",[source],|r|r.get(0))?;
            let mut s=c.prepare(&format!("SELECT {COLUMNS} FROM plugin_mcp_attempts WHERE source_id=?1 AND id>COALESCE(?2,0) AND state IN ('running','uncertain') ORDER BY id LIMIT 51"))?;
            Ok((s.query_map(params![source,after],attempt_row!())?.collect::<std::result::Result<Vec<_>,_>>()?,count))
        })
    }
    pub fn reviews(&self, source: &str) -> Result<Vec<Attempt>> {
        self.0.with_connection(|c| {
            let mut s=c.prepare(&format!("SELECT {COLUMNS} FROM plugin_mcp_attempts a WHERE source_id=?1 AND state='reviewed' AND id=(SELECT max(id) FROM plugin_mcp_attempts b WHERE b.source_id=a.source_id AND b.tool=a.tool AND b.state='reviewed') ORDER BY id DESC LIMIT 20"))?;
            Ok(s.query_map([source],attempt_row!())?.collect::<std::result::Result<Vec<_>,_>>()?)
        })
    }
    /// El índice parcial y la comprobación de permisos participan en el mismo commit.
    pub fn reserve(&self, source: &str, tool: &str, revision: i64) -> Result<Reservation> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let (current,enabled,deleted,allowed,name):(i64,bool,bool,String,String)=tx.query_row("SELECT revision,enabled,deleted,allowed_tools,json_extract(document,'$.name') FROM plugin_mcp_sources WHERE id=?1",[source],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
            if current!=revision || deleted { return Ok(Reservation::Rejected("revision_conflict")); }
            let allowed:Vec<String>=serde_json::from_str(&allowed)?;
            if !enabled || !allowed.iter().any(|name|name==tool) { return Ok(Reservation::Rejected("permission_required")); }
            if tool.is_empty() || tool.chars().count()>200 || tool.chars().any(char::is_control) { return Err("Identidad de herramienta MCP inválida".into()); }
            let mut statement=tx.prepare(&format!("SELECT {COLUMNS} FROM plugin_mcp_attempts WHERE source_id=?1 AND tool=?2 AND state IN ('running','uncertain')"))?;
            let pending=statement.query_map(params![source,tool],attempt_row!())?.next().transpose()?;
            drop(statement);
            if let Some(attempt)=pending { return Ok(Reservation::Blocked(attempt)); }
            tx.execute("INSERT INTO plugin_mcp_attempts(source_id,source_revision,server_name,tool) VALUES(?1,?2,?3,?4)",params![source,revision,name,tool])?;
            let attempt=get(&tx,tx.last_insert_rowid())?;
            tx.commit()?; Ok(Reservation::Reserved(attempt))
        })
    }
    pub fn finish(&self, id: i64, reason: &'static str) -> Result<Attempt> {
        if !matches!(
            reason,
            "reported_success" | "transport_error" | "tool_error"
        ) {
            return Err("Resultado MCP inválido".into());
        }
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE plugin_mcp_attempts SET state=?2,reason=?3,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='running'",params![id,if reason=="reported_success"{"completed"}else{"uncertain"},reason])?;
            if changed!=1 {return Err("El intento MCP ya terminó o fue revisado".into());}
            let attempt=get(&tx,id)?;tx.commit()?;Ok(attempt)
        })
    }
    /// Solo el servicio humano permite llamar a esta operación; el modelo no la publica.
    pub fn reconcile(
        &self,
        source: &str,
        revision: i64,
        attempt: i64,
        resolution: &str,
    ) -> Result<()> {
        if !matches!(resolution, "applied" | "not_applied") {
            return Err("Indica si comprobaste que la acción se aplicó o no se aplicó".into());
        }
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE plugin_mcp_sources SET revision=revision+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND revision=?2 AND deleted=0",params![source,revision])?;
            if changed!=1 {return Err("La conexión MCP cambió; recarga antes de revisar el intento".into());}
            let changed=tx.execute("UPDATE plugin_mcp_attempts SET state='reviewed',resolution=?3,reviewed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND source_id=?2 AND state IN ('running','uncertain')",params![attempt,source,resolution])?;
            if changed!=1 {return Err("El intento MCP cambió o pertenece a otra conexión".into());}
            tx.commit()?;Ok(())
        })
    }
}
