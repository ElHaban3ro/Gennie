use super::mcp_source::Specification;
use gennie_core::Result;
use gennie_db::{Database, SqliteConnection, params};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Serialize)]
pub struct Source {
    pub specification: Specification,
    pub revision: i64,
    pub enabled: bool,
    pub tools: Value,
    pub allowed_tools: Vec<String>,
    pub checked_at: Option<String>,
}
#[derive(Serialize)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub transport: &'static str,
    pub revision: i64,
    pub enabled: bool,
    pub tool_count: u32,
    pub allowed_count: u32,
    pub checked_at: Option<String>,
    pub pending_count: u32,
}
#[derive(Clone)]
pub struct Store(pub Database);
fn source(c: &SqliteConnection, id: &str) -> Result<Source> {
    let (document,revision,enabled,tools,allowed,checked):(String,i64,bool,String,String,Option<String>)=c.query_row("SELECT document,revision,enabled,tools,allowed_tools,checked_at FROM plugin_mcp_sources WHERE id=?1 AND deleted=0",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    let specification: Specification = serde_json::from_str(&document)?;
    specification.validate()?;
    if specification.id != id {
        return Err("La identidad de la conexión MCP guardada no coincide".into());
    }
    Ok(Source {
        specification,
        revision,
        enabled,
        tools: serde_json::from_str(&tools)?,
        allowed_tools: serde_json::from_str(&allowed)?,
        checked_at: checked,
    })
}
impl Store {
    pub fn is_current(&self, id: &str, revision: i64) -> Result<bool> {
        self.0.with_connection(|c|Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM plugin_mcp_sources WHERE id=?1 AND revision=?2 AND deleted=0)",params![id,revision],|r|r.get(0))?))
    }
    pub fn get(&self, id: &str) -> Result<Source> {
        self.0.with_connection(|c| source(c, id))
    }
    pub fn list(&self) -> Result<Vec<Summary>> {
        // La lista no carga los cuerpos de todos los esquemas en cada actualización de UI.
        let rows=self.0.with_connection(|c| {
            let mut s=c.prepare("SELECT document,revision,enabled,(SELECT count(*) FROM json_each(s.tools)),json_array_length(allowed_tools),checked_at,(SELECT count(*) FROM plugin_mcp_attempts WHERE source_id=s.id AND state IN ('running','uncertain')) FROM plugin_mcp_sources s WHERE deleted=0 ORDER BY id")?;
            Ok(s.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,bool>(2)?,r.get::<_,u32>(3)?,r.get::<_,u32>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,u32>(6)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)
        })?;
        rows.into_iter()
            .map(
                |(
                    document,
                    revision,
                    enabled,
                    tool_count,
                    allowed_count,
                    checked_at,
                    pending_count,
                )| {
                    let spec: Specification = serde_json::from_str(&document)?;
                    spec.validate()?;
                    Ok(Summary {
                        id: spec.id,
                        name: spec.name,
                        transport: match spec.connection {
                            super::mcp_source::Connection::Stdio { .. } => "stdio",
                            super::mcp_source::Connection::Http { .. } => "http",
                        },
                        revision,
                        enabled,
                        tool_count,
                        allowed_count,
                        checked_at,
                        pending_count,
                    })
                },
            )
            .collect()
    }
    pub fn save(&self, spec: &Specification, revision: i64) -> Result<Source> {
        spec.validate()?;
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let current = {
                let mut statement=tx.prepare("SELECT revision,deleted FROM plugin_mcp_sources WHERE id=?1")?;
                statement.query_map([&spec.id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,bool>(1)?)))?.next().transpose()?
            };
            let matches_revision = match current {
                None => revision == 0,
                Some((value,deleted)) => value == revision || deleted && revision == 0,
            };
            if !matches_revision {
                return Err("La conexión MCP cambió; recarga antes de guardar".into());
            }
            let count:u32=tx.query_row("SELECT count(*) FROM plugin_mcp_sources WHERE deleted=0 AND id<>?1",[&spec.id],|r|r.get(0))?;
            if count>=20 {return Err("El catálogo admite hasta 20 conexiones MCP".into());}
            tx.execute("INSERT INTO plugin_mcp_sources(id,document) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET document=excluded.document,revision=plugin_mcp_sources.revision+1,enabled=0,deleted=0,tools='{}',allowed_tools='[]',checked_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![spec.id,serde_json::to_string(spec)?])?;
            let result=source(&tx,&spec.id)?;tx.commit()?;Ok(result)
        })
    }
    pub fn catalogue(&self, id: &str, revision: i64, tools: &Value) -> Result<Source> {
        let encoded = serde_json::to_string(tools)?;
        if !tools.is_object() || encoded.len() > 2 * 1024 * 1024 {
            return Err("Catálogo MCP inválido o demasiado grande".into());
        }
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let current=source(&tx,id)?;
            if current.revision!=revision {return Err("La conexión MCP cambió durante el descubrimiento".into());}
            let allowed=current.allowed_tools.iter().filter(|name|tools.get(*name).is_some_and(|tool|current.tools.get(*name)==Some(tool))).collect::<Vec<_>>();
            tx.execute("UPDATE plugin_mcp_sources SET tools=?2,allowed_tools=?3,checked_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),revision=revision+?4,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,encoded,serde_json::to_string(&allowed)?,u32::from(current.tools!=*tools)])?;
            let result=source(&tx,id)?;tx.commit()?;Ok(result)
        })
    }
    pub fn permissions(
        &self,
        id: &str,
        revision: i64,
        allowed: &[String],
        enabled: bool,
    ) -> Result<Source> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;let current=source(&tx,id)?;
            if current.revision!=revision {return Err("La conexión MCP cambió; recarga sus permisos".into());}
            if allowed.iter().any(|name|current.tools.get(name).is_none()) || allowed.len()!=allowed.iter().collect::<std::collections::HashSet<_>>().len() {return Err("Solo se permiten herramientas descubiertas y sin duplicados".into());}
            tx.execute("UPDATE plugin_mcp_sources SET allowed_tools=?2,enabled=?3,revision=revision+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,serde_json::to_string(allowed)?,enabled])?;
            let result=source(&tx,id)?;tx.commit()?;Ok(result)
        })
    }
    pub fn delete(&self, id: &str, revision: i64) -> Result<()> {
        self.0.with_connection(|c| {
        let changed=c.execute("UPDATE plugin_mcp_sources SET deleted=1,enabled=0,document=json_object('id',id,'name','Eliminado','connection',json_object('type','http','url','https://invalid.example'),'timeout_seconds',60),tools='{}',allowed_tools='[]',checked_at=NULL,revision=revision+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND revision=?2 AND deleted=0 AND NOT EXISTS(SELECT 1 FROM plugin_mcp_attempts WHERE source_id=?1 AND state IN ('running','uncertain'))",params![id,revision])?;
        if changed!=1 {return Err("La conexión MCP cambió o tiene resultados pendientes; revísalos antes de eliminar".into());} Ok(())
    })
    }
}
