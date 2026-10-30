//! Archivo humano por familia; páginas estables sin perder borradores o recibos.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub created_at: String,
    pub review_id: String,
    pub instance_id: String,
    pub archived: bool,
}

pub(super) fn root(c: &SqliteConnection, id: &str) -> Result<String> {
    Ok(c.query_row("SELECT COALESCE(f.root_review_id,r.id) FROM assignment_reviews r LEFT JOIN assignment_review_refinements f ON f.review_id=r.id WHERE r.id=?1",[id],|r|r.get(0))?)
}

pub(super) fn archived_at(c: &SqliteConnection, id: &str) -> Result<Option<String>> {
    let root = root(c, id)?;
    let mut query =
        c.prepare("SELECT archived_at FROM assignment_review_archives WHERE root_review_id=?1")?;
    Ok(query
        .query_map([root], |r| r.get::<_, String>(0))?
        .next()
        .transpose()?)
}

pub(super) fn require_open(c: &SqliteConnection, id: &str) -> Result<()> {
    if archived_at(c, id)?.is_some() {
        return Err(
            "Esta revisión está archivada; solicita una revisión nueva para continuar".into(),
        );
    }
    Ok(())
}

impl Store {
    /// API humana, ausente del catálogo MCP. Archiva la familia sin resolver borradores.
    pub fn archive_review(&self, id: &str) -> Result<Value> {
        let result = self.db.with_connection(|c| {
            let tx = c.transaction()?;
            let root = root(&tx, id)?;
            let evaluating: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignment_reviews r LEFT JOIN assignment_review_refinements f ON f.review_id=r.id WHERE COALESCE(f.root_review_id,r.id)=?1 AND r.state='evaluating')",[&root],|r|r.get(0))?;
            if evaluating {
                return Err("Espera a que termine la valoración antes de archivar".into());
            }
            tx.execute("INSERT OR IGNORE INTO assignment_review_archives(root_review_id) VALUES(?1)", [&root])?;
            let result = load(&tx,id)?;
            tx.commit()?;
            Ok(result)
        })?;
        self.revision
            .fetch_add(1, std::sync::atomic::Ordering::Release);
        Ok(result)
    }

    pub fn reviews_page(
        &self,
        assignment_id: i64,
        archived: bool,
        before: Option<&Cursor>,
    ) -> Result<Value> {
        self.db.with_connection(|c| {
            let identity = read(c, assignment_id)?.instance_id;
            if let Some(cursor) = before
                && (cursor.instance_id != identity || cursor.archived != archived || cursor.created_at.is_empty() || cursor.created_at.len()>64 || cursor.review_id.is_empty() || cursor.review_id.len()>120) {
                    return Err("La página pertenece a otra instancia o vista de revisiones".into());
            }
            let predicate="EXISTS(SELECT 1 FROM json_each(r.scope,'$.assignments') a WHERE json_extract(a.value,'$.id')=?1 AND json_extract(a.value,'$.instance_id')=?2) AND EXISTS(SELECT 1 FROM assignment_review_archives ar WHERE ar.root_review_id=COALESCE((SELECT f.root_review_id FROM assignment_review_refinements f WHERE f.review_id=r.id),r.id))=?3";
            let total: i64=c.query_row(&format!("SELECT count(*) FROM assignment_reviews r WHERE {predicate}"),params![assignment_id,identity,archived],|r|r.get(0))?;
            let mut query=c.prepare(&format!("SELECT r.id,r.created_at FROM assignment_reviews r WHERE {predicate} AND (?4 IS NULL OR (r.created_at,r.id)<(?4,?5)) ORDER BY r.created_at DESC,r.id DESC LIMIT 11"))?;
            let ids=query.query_map(params![assignment_id,identity,archived,before.map(|v|&v.created_at),before.map(|v|&v.review_id)],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let mut reviews = Vec::new();
            let mut bytes = 0;
            let mut cursor = None;
            for (id,created_at) in &ids {
                if reviews.len()==10 { break; }
                let value=load(c,id)?;
                let length=serde_json::to_vec(&value)?.len();
                if !reviews.is_empty() && bytes+length>512*1024 { break; }
                bytes+=length;
                reviews.push(value);
                cursor=Some(Cursor{created_at:created_at.clone(),review_id:id.clone(),instance_id:identity.clone(),archived});
            }
            let more=ids.len()>reviews.len();
            Ok(json!({"reviews":reviews,"total":total,"has_more":more,"next_cursor":if more {cursor}else{None}}))
        })
    }
}

#[cfg(test)]
mod tests;
