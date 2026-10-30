//! Reserva previa al IO web; no conserva argumentos, resultados ni errores libres.
use super::*;
use gennie_db::{Database, params};

pub(crate) const MIGRATIONS: &[gennie_core::Migration] = &[gennie_core::Migration {
    version: 1,
    sql: include_str!("../migrations/0001_web_attempts.sql"),
}];
const COLUMNS: &str = "id,plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation,state,reason,resolution,created_at,updated_at,reviewed_at";
macro_rules! attempt_row { () => { |r| {
    Ok(json!({"id":r.get::<_,i64>(0)?,"plugin_id":r.get::<_,String>(1)?,"server_name":r.get::<_,String>(2)?,"tool":r.get::<_,String>(3)?,"source_revision":r.get::<_,i64>(4)?,"candidate_id":r.get::<_,Option<String>>(5)?,"operation":r.get::<_,String>(6)?,"state":r.get::<_,String>(7)?,"reason":r.get::<_,String>(8)?,"resolution":r.get::<_,Option<String>>(9)?,"created_at":r.get::<_,String>(10)?,"updated_at":r.get::<_,String>(11)?,"reviewed_at":r.get::<_,Option<String>>(12)?}))
} }; }
#[derive(Clone)]
pub(crate) struct Store(pub Database);
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}
impl Store {
    pub fn pending(&self, plugin: &str, function: Option<&str>) -> Result<Option<Value>> {
        if !identifier(plugin) || function.is_some_and(|name| !identifier(name)) {
            return Err("Referencia web inválida".into());
        }
        self.0.with_connection(|c|{
            let mut q=c.prepare(&format!("SELECT {COLUMNS} FROM browser_web_attempts WHERE plugin_id=?1 AND (?2 IS NULL OR function_name=?2) AND state IN ('running','uncertain') ORDER BY id LIMIT 1"))?;
            Ok(q.query_map(params![plugin,function],attempt_row!())?.next().transpose()?)
        })
    }
    pub fn recover(&self) -> Result<()> {
        self.0.with_connection(|c|{c.execute("UPDATE browser_web_attempts SET state='uncertain',reason='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='running'",[])?;Ok(())})
    }
    pub fn page(&self, plugin: &str, after: Option<i64>) -> Result<Value> {
        if !identifier(plugin) || after.is_some_and(|id| id < 1) {
            return Err("Referencia de intentos web inválida".into());
        }
        self.0.with_connection(|c|{
            let total:i64=c.query_row("SELECT count(*) FROM browser_web_attempts WHERE plugin_id=?1 AND state IN ('running','uncertain')",[plugin],|r|r.get(0))?;
            let mut q=c.prepare(&format!("SELECT {COLUMNS} FROM browser_web_attempts WHERE plugin_id=?1 AND id>COALESCE(?2,0) AND state IN ('running','uncertain') ORDER BY id LIMIT 51"))?;
            let mut rows=q.query_map(params![plugin,after],attempt_row!())?.collect::<std::result::Result<Vec<_>,_>>()?;
            let more=rows.len()>50; if more {rows.pop();}
            let next=if more {rows.last().map(|r|r["id"].clone())} else {None};
            Ok(json!({"attempts":rows,"total":total,"next_cursor":next}))
        })
    }
    pub fn reviews(&self, plugin: &str) -> Result<Vec<Value>> {
        self.0.with_connection(|c|{
            let mut q=c.prepare(&format!("SELECT {COLUMNS} FROM browser_web_attempts a WHERE plugin_id=?1 AND state='reviewed' AND id=(SELECT max(id) FROM browser_web_attempts b WHERE b.plugin_id=a.plugin_id AND b.function_name=a.function_name AND b.state='reviewed') ORDER BY id DESC LIMIT 20"))?;
            Ok(q.query_map([plugin],attempt_row!())?.collect::<std::result::Result<Vec<_>,_>>()?)
        })
    }
    pub fn reserve(
        &self,
        metadata: &Value,
        operation: &str,
        candidate: Option<&str>,
    ) -> Result<Value> {
        let plugin = metadata["plugin_id"].as_str().ok_or("Falta plugin web")?;
        let function = metadata["function_name"]
            .as_str()
            .ok_or("Falta función web")?;
        let name = metadata["plugin_name"].as_str().ok_or("Falta nombre web")?;
        let revision = metadata["revision"]
            .as_i64()
            .filter(|r| *r > 0)
            .ok_or("Falta revisión web")?;
        if !identifier(plugin)
            || !identifier(function)
            || name.trim().is_empty()
            || name.chars().count() > 200
            || name.chars().any(char::is_control)
            || !matches!(operation, "run" | "verify")
            || candidate.is_some_and(|id| {
                !id.starts_with("job_")
                    || id.len() > 64
                    || !id[4..].bytes().all(|c| c.is_ascii_digit())
                    || id.len() == 4
            })
        {
            return Err("Identidad de ejecución web inválida".into());
        }
        self.0.with_connection(|c|{
            let tx=c.transaction()?;
            let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM browser_web_attempts WHERE plugin_id=?1 AND function_name=?2 AND state IN ('running','uncertain'))",params![plugin,function],|r|r.get(0))?;
            if pending {return Err("Revisa el resultado web pendiente antes de continuar".into());}
            tx.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation) VALUES(?1,?2,?3,?4,?5,?6)",params![plugin,name,function,revision,candidate,operation])?;
            let attempt=tx.query_row(&format!("SELECT {COLUMNS} FROM browser_web_attempts WHERE id=?1"),[tx.last_insert_rowid()],attempt_row!())?;
            tx.commit()?; Ok(attempt)
        })
    }
    pub fn finish(&self, id: i64, reason: &str) -> Result<Value> {
        if !matches!(
            reason,
            "reported_success" | "reported_failure" | "transport_error"
        ) {
            return Err("Resultado web inválido".into());
        }
        self.0.with_connection(|c|{
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE browser_web_attempts SET state=?2,reason=?3,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='running'",params![id,if reason=="reported_success"{"completed"}else{"uncertain"},reason])?;
            if changed!=1 {return Err("El intento web ya no está en curso".into());}
            let attempt=tx.query_row(&format!("SELECT {COLUMNS} FROM browser_web_attempts WHERE id=?1"),[id],attempt_row!())?;
            tx.commit()?;Ok(attempt)
        })
    }
    pub fn reconcile(&self, plugin: &str, id: i64, revision: i64, resolution: &str) -> Result<()> {
        if !identifier(plugin)
            || id < 1
            || revision < 1
            || !matches!(resolution, "applied" | "not_applied")
        {
            return Err("Indica el intento y si comprobaste que la acción se aplicó o no".into());
        }
        self.0.with_connection(|c|{
            let changed=c.execute("UPDATE browser_web_attempts SET state='reviewed',resolution=?4,reviewed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND plugin_id=?2 AND plugin_revision=?3 AND state IN ('running','uncertain')",params![id,plugin,revision,resolution])?;
            if changed!=1 {return Err("El intento web cambió o pertenece a otra revisión".into());} Ok(())
        })
    }
}
