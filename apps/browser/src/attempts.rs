//! Reserva previa al IO web; no conserva argumentos, resultados ni errores libres.
use super::*;
use gennie_db::{Database, params};

pub(crate) const MIGRATIONS: &[gennie_core::Migration] = &[
    gennie_core::Migration {
        version: 1,
        sql: include_str!("../migrations/0001_web_attempts.sql"),
    },
    gennie_core::Migration {
        version: 2,
        sql: include_str!("../migrations/0002_attempt_diagnostics.sql"),
    },
    gennie_core::Migration {
        version: 3,
        sql: include_str!("../migrations/0003_observation_attempts.sql"),
    },
    gennie_core::Migration {
        version: 4,
        sql: include_str!("../migrations/0004_extended_attempt_diagnostics.sql"),
    },
    gennie_core::Migration {
        version: 5,
        sql: include_str!("../migrations/0005_quarantined_attempts.sql"),
    },
    gennie_core::Migration {
        version: 6,
        sql: include_str!("../migrations/0006_gmail_archive_evidence.sql"),
    },
];
const COLUMNS: &str = "id,plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation,state,reason,CASE WHEN closed_unconfirmed_at IS NOT NULL THEN 'unknown' ELSE resolution END,created_at,updated_at,reviewed_at,COALESCE(failure_code_extended,failure_code),COALESCE(failure_phase_extended,failure_phase),observation_only,closed_unconfirmed_at,(SELECT auto_resolution FROM browser_gmail_archive_evidence WHERE attempt_id=id)";
macro_rules! attempt_row { () => { |r| {
    Ok(json!({"id":r.get::<_,i64>(0)?,"plugin_id":r.get::<_,String>(1)?,"server_name":r.get::<_,String>(2)?,"tool":r.get::<_,String>(3)?,"source_revision":r.get::<_,i64>(4)?,"candidate_id":r.get::<_,Option<String>>(5)?,"operation":r.get::<_,String>(6)?,"state":r.get::<_,String>(7)?,"reason":r.get::<_,String>(8)?,"resolution":r.get::<_,Option<String>>(9)?,"created_at":r.get::<_,String>(10)?,"updated_at":r.get::<_,String>(11)?,"reviewed_at":r.get::<_,Option<String>>(12)?,"failure_code":r.get::<_,Option<String>>(13)?,"failure_phase":r.get::<_,Option<String>>(14)?,"observation_only":r.get::<_,bool>(15)?,"closed_unconfirmed_at":r.get::<_,Option<String>>(16)?,"automatic_resolution":r.get::<_,Option<String>>(17)?}))
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
    pub fn gmail_evidence(&self, id: i64) -> Result<Option<Value>> {
        self.0.with_connection(|c| {
            let mut q = c.prepare("SELECT e.account_hash,e.thread_id,e.unread_before,a.plugin_id,a.function_name,a.plugin_revision,a.candidate_id FROM browser_gmail_archive_evidence e JOIN browser_web_attempts a ON a.id=e.attempt_id WHERE a.id=?1 AND a.state='uncertain' AND e.auto_resolution IS NULL")?;
            Ok(q.query_map([id], |r| Ok(json!({"account_hash":r.get::<_,String>(0)?,"thread_id":r.get::<_,String>(1)?,"unread_before":r.get::<_,bool>(2)?,"plugin_id":r.get::<_,String>(3)?,"function_name":r.get::<_,String>(4)?,"revision":r.get::<_,i64>(5)?,"candidate_id":r.get::<_,Option<String>>(6)?})))?.next().transpose()?)
        })
    }
    pub fn reconcile_gmail(&self, id: i64, resolution: &str) -> Result<()> {
        if resolution != "applied" {
            return Err("Resolución automática inválida".into());
        }
        self.0.with_connection(|c| {
            let tx = c.transaction()?;
            let n = tx.execute("UPDATE browser_web_attempts SET state='reviewed',resolution=?2,reviewed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='uncertain' AND EXISTS(SELECT 1 FROM browser_gmail_archive_evidence WHERE attempt_id=?1 AND auto_resolution IS NULL)", params![id,resolution])?;
            if n != 1 { return Err("El intento ya no está pendiente".into()); }
            tx.execute("UPDATE browser_gmail_archive_evidence SET auto_resolution=?2,observed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE attempt_id=?1",params![id,resolution])?;
            tx.commit()?; Ok(())
        })
    }
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
        self.0.with_connection(|c|{c.execute("UPDATE browser_web_attempts SET state=CASE WHEN observation_only=1 THEN 'completed' ELSE 'uncertain' END,reason='interrupted',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='running'",[])?;Ok(())})
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
            let mut q=c.prepare(&format!("SELECT {COLUMNS} FROM browser_web_attempts a WHERE plugin_id=?1 AND (state='reviewed' OR closed_unconfirmed_at IS NOT NULL) AND id=(SELECT max(id) FROM browser_web_attempts b WHERE b.plugin_id=a.plugin_id AND b.function_name=a.function_name AND (b.state='reviewed' OR b.closed_unconfirmed_at IS NOT NULL)) ORDER BY id DESC LIMIT 20"))?;
            Ok(q.query_map([plugin],attempt_row!())?.collect::<std::result::Result<Vec<_>,_>>()?)
        })
    }
    pub fn close_unconfirmed(&self, plugin: &str, id: i64, revision: i64) -> Result<()> {
        if !identifier(plugin) || id < 1 || revision < 1 {
            return Err("Indica el intento y su revisión".into());
        }
        self.0.with_connection(|c| {
            let changed=c.execute("UPDATE browser_web_attempts SET state='completed',closed_unconfirmed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND plugin_id=?2 AND plugin_revision=?3 AND state='uncertain' AND closed_unconfirmed_at IS NULL",params![id,plugin,revision])?;
            if changed!=1 {return Err("El intento cambió o pertenece a otra revisión".into());} Ok(())
        })
    }
    pub fn quarantined(&self, plugin: &str, function: &str) -> Result<bool> {
        self.0.with_connection(|c| Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM browser_web_attempts WHERE plugin_id=?1 AND function_name=?2 AND closed_unconfirmed_at IS NOT NULL)",params![plugin,function],|r|r.get(0))?))
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
            tx.execute("INSERT INTO browser_web_attempts(plugin_id,plugin_name,function_name,plugin_revision,candidate_id,operation,observation_only) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![plugin,name,function,revision,candidate,operation,matches!(metadata["transport"].as_str(),Some("observe" | "gmail_inbox" | "gmail_print" | "gmail_importance"))]) ?;
            if let Some(evidence) = metadata.get("gmail_evidence") {
                let account = evidence["account_hash"].as_str().filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("Identidad de cuenta inválida")?;
                let thread = evidence["thread_id"].as_str().filter(|s| !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("Identidad de conversación inválida")?;
                let unread = evidence["unread_before"].as_bool().ok_or("Estado de lectura inválido")?;
                tx.execute("INSERT INTO browser_gmail_archive_evidence(attempt_id,account_hash,thread_id,unread_before) VALUES(?1,?2,?3,?4)", params![tx.last_insert_rowid(),account,thread,unread])?;
            }
            let attempt=tx.query_row(&format!("SELECT {COLUMNS} FROM browser_web_attempts WHERE id=?1"),[tx.last_insert_rowid()],attempt_row!())?;
            tx.commit()?; Ok(attempt)
        })
    }
    #[cfg(test)]
    pub fn finish(&self, id: i64, reason: &str) -> Result<Value> {
        self.finish_with_diagnostic(id, reason, None)
    }
    #[cfg(test)]
    pub fn finish_with_diagnostic(
        &self,
        id: i64,
        reason: &str,
        diagnostic: Option<&Value>,
    ) -> Result<Value> {
        self.finish_with_outcome(id, reason, diagnostic, false)
    }
    pub fn finish_with_outcome(
        &self,
        id: i64,
        reason: &str,
        diagnostic: Option<&Value>,
        not_started: bool,
    ) -> Result<Value> {
        let code = diagnostic.and_then(|v| v["code"].as_str()).filter(|v| {
            matches!(
                *v,
                "timeout"
                    | "connection_failed"
                    | "type_error"
                    | "reference_error"
                    | "program_error"
                    | "postcondition_failed"
            )
        });
        let phase = diagnostic
            .and_then(|v| v["phase"].as_str())
            .filter(|v| matches!(*v, "prepare" | "read" | "action" | "verify"));
        let extended_code = diagnostic
            .and_then(|v| v["code"].as_str())
            .filter(|v| *v == "result_type_mismatch");
        let extended_phase = diagnostic
            .and_then(|v| v["phase"].as_str())
            .filter(|v| *v == "execute");
        if !matches!(
            reason,
            "reported_success" | "reported_failure" | "transport_error"
        ) {
            return Err("Resultado web inválido".into());
        }
        self.0.with_connection(|c|{
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE browser_web_attempts SET state=CASE WHEN observation_only=1 THEN 'completed' ELSE ?2 END,reason=?3,failure_code=?4,failure_phase=?5,failure_code_extended=?6,failure_phase_extended=?7,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='running'",params![id,if reason=="reported_success" || not_started {"completed"}else{"uncertain"},reason,code,phase,extended_code,extended_phase])?;
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
