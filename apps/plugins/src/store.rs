use gennie_core::Result;
use gennie_db::{Database, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub events: Vec<BuildEvent>,
    pub id: i64,
    pub plugin_id: String,
    pub origin: String,
    pub objective: String,
    pub trigger_conversation_id: i64,
    pub base_revision: u64,
    pub refresh_base: bool,
    pub retry_of: Option<i64>,
    pub recommended_route: Option<String>,
    pub state: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub max_seconds: u64,
    pub spent_seconds: u64,
    pub thread_id: Option<String>,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildEvent {
    pub id: i64,
    pub message: String,
    pub created_at: String,
}
#[derive(Clone)]
pub(crate) struct Store(pub Database);
impl Store {
    pub fn assignment_job(&self, key: &str) -> Result<Option<(i64, String)>> {
        self.0.with_connection(|c| {
            let mut s = c.prepare("WITH RECURSIVE related(id, state, plugin_id, origin, objective, trigger_conversation_id, base_revision) AS (
                SELECT id,state,plugin_id,origin,objective,trigger_conversation_id,base_revision FROM plugin_build_jobs WHERE request_key=?1
                UNION ALL
                SELECT j.id,j.state,j.plugin_id,j.origin,j.objective,j.trigger_conversation_id,j.base_revision
                FROM plugin_build_jobs j JOIN related parent ON j.id > parent.id
                AND (j.retry_of=parent.id OR (j.retry_of IS NULL AND (j.request_key LIKE 'plugin-rebuild-' || parent.id || '-%' OR j.request_key LIKE 'plugin-chat-rebuild-' || parent.id || '-%')))
                AND j.plugin_id=parent.plugin_id AND j.origin=parent.origin AND j.objective=parent.objective
                AND j.trigger_conversation_id=parent.trigger_conversation_id AND j.base_revision=parent.base_revision
            ) SELECT id,state FROM related ORDER BY id DESC LIMIT 1")?;
            Ok(s.query_map([key], |r| Ok((r.get(0)?, r.get(1)?)))?
                .next()
                .transpose()?)
        })
    }
    pub fn list(&self) -> Result<Vec<Job>> {
        let mut jobs = self.0.with_connection(|c| {
            let mut statement = c.prepare("SELECT id,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,attempts,max_attempts,max_seconds,spent_seconds,thread_id,summary,created_at,updated_at,refresh_base,retry_of,recommended_route FROM plugin_build_jobs ORDER BY state IN ('queued','running','waiting_user','interrupted') DESC,id DESC LIMIT 100")?;
            Ok(statement.query_map([], |row| Ok(Job { events:Vec::new(),id:row.get(0)?,plugin_id:row.get(1)?,origin:row.get(2)?,objective:row.get(3)?,trigger_conversation_id:row.get(4)?,base_revision:row.get::<_,u32>(5)? as u64,state:row.get(6)?,attempts:row.get(7)?,max_attempts:row.get(8)?,max_seconds:row.get::<_,u32>(9)? as u64,spent_seconds:row.get::<_,u32>(10)? as u64,thread_id:row.get(11)?,summary:row.get(12)?,created_at:row.get(13)?,updated_at:row.get(14)?,refresh_base:row.get(15)?,retry_of:row.get(16)?,recommended_route:row.get(17)? }))?.collect::<std::result::Result<Vec<_>,_>>()?)
        })?;
        for job in &mut jobs {
            job.events = self.events(job.id)?;
        }
        Ok(jobs)
    }
    fn events(&self, id: i64) -> Result<Vec<BuildEvent>> {
        self.0.with_connection(|c| {
            let mut query = c.prepare("SELECT id,message,created_at FROM (SELECT id,message,created_at FROM plugin_build_events WHERE job_id=?1 ORDER BY id DESC LIMIT 100) ORDER BY id")?;
            Ok(query.query_map([id], |r| Ok(BuildEvent {id:r.get(0)?,message:r.get(1)?,created_at:r.get(2)?}))?.collect::<std::result::Result<Vec<_>,_>>()?)
        })
    }
    pub fn get(&self, id: i64) -> Result<Job> {
        // El catálogo de trabajos visibles se acota; los trabajos vivos siempre se conservan consultables.
        let mut job = self.0.with_connection(|c| {
            Ok(c.query_row("SELECT id,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,attempts,max_attempts,max_seconds,spent_seconds,thread_id,summary,created_at,updated_at,refresh_base,retry_of,recommended_route FROM plugin_build_jobs WHERE id=?1", [id], |row| Ok(Job { events:Vec::new(),id:row.get(0)?,plugin_id:row.get(1)?,origin:row.get(2)?,objective:row.get(3)?,trigger_conversation_id:row.get(4)?,base_revision:row.get::<_,u32>(5)? as u64,state:row.get(6)?,attempts:row.get(7)?,max_attempts:row.get(8)?,max_seconds:row.get::<_,u32>(9)? as u64,spent_seconds:row.get::<_,u32>(10)? as u64,thread_id:row.get(11)?,summary:row.get(12)?,created_at:row.get(13)?,updated_at:row.get(14)?,refresh_base:row.get(15)?,retry_of:row.get(16)?,recommended_route:row.get(17)? }))?)
        })?;
        job.events = self.events(id)?;
        Ok(job)
    }
    pub fn create(&self, request: &crate::BuildRequest, origin: i64, revision: u64) -> Result<Job> {
        self.create_inner(request, origin, revision, false)
    }
    pub fn queue(&self, request: &crate::BuildRequest, origin: i64) -> Result<Job> {
        self.create_inner(request, origin, 0, true)
    }
    fn create_inner(
        &self,
        request: &crate::BuildRequest,
        origin: i64,
        revision: u64,
        queue: bool,
    ) -> Result<Job> {
        let id=self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let existing={let mut s=tx.prepare("SELECT id,plugin_id,origin,objective,trigger_conversation_id,retry_of FROM plugin_build_jobs WHERE request_key=?1")?;s.query_map([&request.request_key], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?)))?.next().transpose()?};
            if let Some((id,plugin_id,url,objective,chat,retry_of))=existing {
                if plugin_id!=request.plugin_id || url!=request.origin || objective!=request.objective || chat!=origin || retry_of!=request.retry_of { return Err("La clave del trabajo ya pertenece a otra solicitud".into()); }
                return Ok(id);
            }
            if let Some(source) = request.retry_of {
                let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM plugin_build_jobs WHERE id=?1 AND plugin_id=?2 AND origin=?3 AND objective=?4 AND trigger_conversation_id=?5 AND base_revision=?6 AND state IN ('blocked','cancelled'))",params![source,request.plugin_id,request.origin,request.objective,origin,i64::try_from(revision)?],|r|r.get(0))?;
                if !valid || queue { return Err("El trabajo anterior no corresponde a esta solicitud o la revisión cambió".into()); }
            }
            let count:i64=tx.query_row("SELECT count(*) FROM plugin_build_jobs WHERE state IN ('queued','running','waiting_user','interrupted')", [], |r|r.get(0))?;
            if count>=8 { return Err("Hay demasiados trabajos pendientes. Cancela o termina uno antes de crear otro".into()); }
            let live:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM plugin_build_jobs WHERE plugin_id=?1 AND state IN ('queued','running','waiting_user','interrupted'))",[&request.plugin_id],|r|r.get(0))?;
            if live && !queue { return Err("Ese plugin ya tiene un trabajo pendiente".into()); }
            let message="He delegado la preparación o mejora del plugin para reutilizar sus funciones y reducir el coste de futuras revisiones. Te informaré de los avances.";
            tx.execute("INSERT INTO plugin_build_jobs(request_key,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,max_attempts,max_seconds,summary,retry_of) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7,?8,?9,?10)", params![request.request_key,request.plugin_id,request.origin,request.objective,origin,i64::try_from(revision)?,request.max_attempts,i64::try_from(request.max_seconds)?,message,request.retry_of])?;
            let id=tx.last_insert_rowid();
            if queue { tx.execute("UPDATE plugin_build_jobs SET refresh_base=1 WHERE id=?1",[id])?; }
            tx.execute("INSERT INTO plugin_build_events(job_id,message) VALUES(?1,?2)",params![id,message])?;
            tx.commit()?; Ok(id)
        })?;
        self.get(id)
    }
    pub fn transition(&self, id: i64, state: &str, message: &str) -> Result<bool> {
        self.transition_with_route(id, state, message, None)
    }
    pub fn transition_with_route(
        &self,
        id: i64,
        state: &str,
        message: &str,
        route: Option<&str>,
    ) -> Result<bool> {
        if route
            .is_some_and(|r| state != "blocked" || !matches!(r, "http" | "web" | "computer_use"))
        {
            return Err("La vía propuesta no es válida para este estado".into());
        }
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE plugin_build_jobs SET state=?2,summary=?3,recommended_route=?4,retry_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state NOT IN ('cancelled','completed')",params![id,state,message,route])?;
            if changed>0 { tx.execute("INSERT INTO plugin_build_events(job_id,message) VALUES(?1,?2)",params![id,message])?; }
            tx.commit()?; Ok(changed>0)
        })
    }
    pub fn report(&self, id: i64, message: &str) -> Result<()> {
        self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let changed=tx.execute("UPDATE plugin_build_jobs SET summary=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='running'",params![id,message])?;
            if changed>0 { tx.execute("INSERT INTO plugin_build_events(job_id,message) VALUES(?1,?2)",params![id,message])?; }
            tx.commit()?; Ok(())
        })
    }
    pub fn claim(&self) -> Result<Option<Job>> {
        let id=self.0.with_connection(|c| {
            let tx=c.transaction()?;
            let mut rows=tx.prepare("SELECT id FROM plugin_build_jobs WHERE state='queued' AND (retry_at IS NULL OR retry_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')) ORDER BY id LIMIT 1")?;
            let id=rows.query_map([],|r|r.get::<_,i64>(0))?.next().transpose()?;
            drop(rows);
            if let Some(id)=id { tx.execute("UPDATE plugin_build_jobs SET state='running',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='queued'",[id])?; }
            tx.commit()?; Ok(id)
        })?;
        id.map(|id| self.get(id)).transpose()
    }
    pub fn set_initial_base(&self, id: i64, revision: u64) -> Result<()> {
        self.0.with_connection(|c| {
            c.execute("UPDATE plugin_build_jobs SET base_revision=?2,refresh_base=0 WHERE id=?1 AND state='running' AND refresh_base=1",params![id,i64::try_from(revision)?])?;
            Ok(())
        })
    }
    pub fn begin_attempt(&self, id: i64) -> Result<bool> {
        self.0.with_connection(|c| {
            Ok(c.execute(
                "UPDATE plugin_build_jobs SET attempts=attempts+1 WHERE id=?1 AND state='running'",
                [id],
            )? > 0)
        })
    }
    pub fn account(&self, id: i64, seconds: u64, thread: Option<&str>) -> Result<()> {
        self.0.with_connection(|c| { c.execute("UPDATE plugin_build_jobs SET spent_seconds=spent_seconds+?2,thread_id=coalesce(?3,thread_id) WHERE id=?1",params![id,i64::try_from(seconds)?,thread])?; Ok(()) })
    }
    pub fn schedule_provider_retry(&self, id: i64) -> Result<bool> {
        self.0.with_connection(|c| {
            let tx = c.transaction()?;
            let message = "Se interrumpió la conexión con Codex. Conservaré el trabajo y volveré a intentarlo en unos cinco minutos; si cierras Gennie, continuaré al abrirlo.";
            let changed = tx.execute("UPDATE plugin_build_jobs SET state='queued', retry_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','+5 minutes'), summary=?2, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state='running'",params![id,message])?;
            if changed > 0 { tx.execute("INSERT INTO plugin_build_events(job_id,message) VALUES(?1,?2)",params![id,message])?; }
            tx.commit()?; Ok(changed > 0)
        })
    }
    pub fn recover(&self) -> Result<()> {
        self.0.with_connection(|c| {
            c.execute("UPDATE plugin_build_jobs SET state='queued',retry_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state IN ('running','queued') AND retry_at IS NOT NULL", [])?;
            Ok(())
        })?;
        let ids = self.0.with_connection(|c| {
            let mut s =
                c.prepare("SELECT id FROM plugin_build_jobs WHERE state IN ('running','queued') AND retry_at IS NULL")?;
            Ok(s.query_map([], |r| r.get::<_, i64>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?)
        })?;
        for id in ids {
            self.transition(
                id,
                "interrupted",
                "La construcción se interrumpió al cerrar Gennie. Puedes continuar desde Plugins.",
            )?;
        }
        Ok(())
    }
    pub fn deliver(&self, journal: &dyn crate::BuildJournal) -> Result<()> {
        let events=self.0.with_connection(|c| {
            let mut s=c.prepare("SELECT e.id,j.trigger_conversation_id,e.message,j.origin FROM plugin_build_events e JOIN plugin_build_jobs j ON j.id=e.job_id WHERE e.delivered=0 ORDER BY e.id LIMIT 100")?;
            Ok(s.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<std::result::Result<Vec<_>,_>>()?)
        })?;
        for (event, chat, message, origin) in events {
            let host = reqwest::Url::parse(&origin)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned));
            let label = host
                .map(|host| format!("Plugin · {host}"))
                .unwrap_or_else(|| "Plugin".into());
            journal.deliver(
                chat,
                &format!("plugin-build:{event}"),
                &format!("{label}: {message}"),
            )?;
            self.0.with_connection(|c| {
                c.execute(
                    "UPDATE plugin_build_events SET delivered=1 WHERE id=?1",
                    [event],
                )?;
                Ok(())
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildJournal, BuildRequest};
    use std::sync::Mutex;
    fn fixture() -> Store {
        fixture_db(Database::memory().unwrap())
    }
    fn fixture_db(db: Database) -> Store {
        db.migrate(
            "plugins",
            &[
                gennie_core::Migration {
                    version: 1,
                    sql: include_str!("../migrations/0001_build_jobs.sql"),
                },
                gennie_core::Migration {
                    version: 2,
                    sql: include_str!("../migrations/0002_assignment_queue.sql"),
                },
                gennie_core::Migration {
                    version: 3,
                    sql: include_str!("../migrations/0003_retry_source.sql"),
                },
                gennie_core::Migration {
                    version: 4,
                    sql: include_str!("../migrations/0004_recommended_route.sql"),
                },
                gennie_core::Migration {
                    version: 5,
                    sql: include_str!("../migrations/0005_retry_schedule.sql"),
                },
            ],
        )
        .unwrap();
        Store(db)
    }
    fn request(id: &str) -> BuildRequest {
        BuildRequest {
            request_key: format!("request-{id}"),
            plugin_id: id.into(),
            origin: "https://example.com".into(),
            objective: "Consultar datos".into(),
            max_attempts: 2,
            max_seconds: 30,
            retry_of: None,
        }
    }
    #[test]
    fn retry_preserves_source_and_rejects_changed_scope_or_revision() {
        let store = fixture();
        let original = request("mail");
        let first = store.create(&original, 7, 0).unwrap();
        store.transition(first.id, "blocked", "Bloqueado").unwrap();
        let mut retry = original.clone();
        retry.request_key = "retry".into();
        retry.retry_of = Some(first.id);
        assert!(store.create(&retry, 7, 1).is_err());
        let mut changed = retry.clone();
        changed.objective = "Otro alcance".into();
        assert!(store.create(&changed, 7, 0).is_err());
        assert!(store.create(&retry, 8, 0).is_err());
        let second = store.create(&retry, 7, 0).unwrap();
        assert_eq!(second.retry_of, Some(first.id));
        assert_eq!(store.create(&retry, 7, 0).unwrap().id, second.id);
        retry.retry_of = None;
        assert!(store.create(&retry, 7, 0).is_err());
    }
    #[test]
    fn assignment_source_tracks_legacy_and_explicit_retries_only_for_same_scope() {
        let store = fixture();
        let mut first = request("mail");
        first.request_key = "assignment_9_1_0".into();
        let root = store.create(&first, 7, 0).unwrap();
        store.transition(root.id, "cancelled", "Cancelado").unwrap();
        let mut old = first.clone();
        old.request_key = format!("plugin-chat-rebuild-{}-123", root.id);
        let legacy = store.create(&old, 7, 0).unwrap();
        store.transition(legacy.id, "blocked", "Bloqueado").unwrap();
        let mut retry = first.clone();
        retry.request_key = "new-retry".into();
        retry.retry_of = Some(legacy.id);
        let latest = store.create(&retry, 7, 0).unwrap();
        store.transition(latest.id, "blocked", "Bloqueado").unwrap();
        assert_eq!(
            store.assignment_job(&first.request_key).unwrap(),
            Some((latest.id, "blocked".into()))
        );
        let mut unrelated = first.clone();
        unrelated.request_key = format!("plugin-rebuild-{}-456", root.id);
        unrelated.objective = "Otro alcance".into();
        let other = store.create(&unrelated, 7, 0).unwrap();
        store.transition(other.id, "blocked", "Otro").unwrap();
        assert_eq!(
            store.assignment_job(&first.request_key).unwrap(),
            Some((latest.id, "blocked".into()))
        );
    }
    #[derive(Default)]
    struct Journal {
        events: Mutex<Vec<(i64, String)>>,
        messages: Mutex<Vec<String>>,
        fail: Mutex<bool>,
    }
    impl BuildJournal for Journal {
        fn origin(&self) -> Result<i64> {
            Ok(999)
        }
        fn exists(&self, _: i64) -> Result<bool> {
            Ok(true)
        }
        fn deliver(&self, chat: i64, key: &str, text: &str) -> Result<()> {
            let mut events = self.events.lock().unwrap();
            if !events.iter().any(|(_, old)| old == key) {
                events.push((chat, key.into()));
                self.messages.lock().unwrap().push(text.to_owned());
            }
            if std::mem::take(&mut *self.fail.lock().unwrap()) {
                return Err("Se perdió la confirmación después del guardado".into());
            }
            Ok(())
        }
    }
    #[test]
    fn startup_reconciliation_fills_queue_and_retries_only_missing_sources() {
        let store = fixture();
        let module = crate::PluginsModule::new(
            store.0.clone(),
            gennie_browser::BrowserModule::default().controller(),
            std::sync::Arc::new(Journal::default()),
            std::sync::Arc::new(|| Err("sin proveedor".into())),
            Default::default(),
            Default::default(),
        );
        let controller = module.controller();
        let assignments: Vec<_> = (1..=10).map(|id| serde_json::json!({"id":id,"revision":1,"document":{"status":"active","applications":[{"name":"Web","kind":"web","url":"https://example.com"}]}})).collect();
        let prepare = |assignment: &serde_json::Value| -> Result<()> {
            let id = assignment["id"].as_i64().unwrap();
            let mut build = request(&format!("site_{id}"));
            build.request_key = format!("assignment_{id}_1_0");
            store.queue(&build, 7)?;
            Ok(())
        };
        crate::worker::reconcile_pending(&controller, assignments.clone(), prepare);
        assert_eq!(store.list().unwrap().len(), 8);
        let first = store.list().unwrap()[0].id;
        store.transition(first, "cancelled", "Cancelado").unwrap();
        crate::worker::reconcile_pending(&controller, assignments.clone(), prepare);
        assert_eq!(store.list().unwrap().len(), 9);
        assert_eq!(
            controller.assignment_sources(&assignments[0]).unwrap()[0]["status"],
            "queued"
        );
        assert_eq!(
            controller.assignment_sources(&assignments[9]).unwrap()[0]["status"],
            "not_investigated"
        );
    }
    #[test]
    fn assignment_sources_track_exact_revision_and_desktop_without_restarting_jobs() {
        let store = fixture();
        let module = crate::PluginsModule::new(
            store.0.clone(),
            gennie_browser::BrowserModule::default().controller(),
            std::sync::Arc::new(Journal::default()),
            std::sync::Arc::new(|| Err("sin proveedor".into())),
            Default::default(),
            Default::default(),
        );
        let controller = module.controller();
        let assignment = serde_json::json!({"id":9,"revision":2,"document":{"applications":[
            {"name":"Web","kind":"web","url":"https://example.com/path"},
            {"name":"Escritorio","kind":"desktop","url":null},
            {"name":"Sin URL","kind":"web","url":null}
        ]}});
        let states = controller.assignment_sources(&assignment).unwrap();
        assert_eq!(states[0]["status"], "not_investigated");
        assert_eq!(states[1]["status"], "desktop_unsupported");
        assert_eq!(states[2]["status"], "origin_required");
        let mut build = request("mail");
        build.request_key = "assignment_9_2_0".into();
        let job = store.queue(&build, 7).unwrap();
        store
            .transition(job.id, "waiting_user", "Acceso requerido")
            .unwrap();
        assert_eq!(
            controller.assignment_sources(&assignment).unwrap()[0]["status"],
            "waiting_user"
        );
        store
            .transition_with_route(
                job.id,
                "blocked",
                "La receta no puede archivar",
                Some("computer_use"),
            )
            .unwrap();
        let resolved = controller.assignment_sources(&assignment).unwrap();
        assert_eq!(resolved[0]["status"], "blocked");
        assert_eq!(resolved[0]["recommended_route"], "computer_use");
        assert!(
            store
                .transition_with_route(job.id, "running", "No válido", Some("computer_use"))
                .is_err()
        );
        let mut new_revision = assignment.clone();
        new_revision["revision"] = serde_json::json!(3);
        assert_eq!(
            controller.assignment_sources(&new_revision).unwrap()[0]["status"],
            "not_investigated"
        );
    }
    #[test]
    fn provider_retry_survives_database_close_and_reopen() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("gennie-retry-{}-{unique}", std::process::id()));
        let path = dir.join("test.sqlite3");
        let store = fixture_db(Database::open(&path, 50).unwrap());
        let job = store.create(&request("reopen"), 7, 0).unwrap();
        store.claim().unwrap();
        store.schedule_provider_retry(job.id).unwrap();
        store.0.close().unwrap();
        drop(store);
        let reopened = fixture_db(Database::open(&path, 50).unwrap());
        assert!(reopened.claim().unwrap().is_none());
        reopened.recover().unwrap();
        assert_eq!(reopened.claim().unwrap().unwrap().id, job.id);
        assert!(
            reopened
                .get(job.id)
                .unwrap()
                .events
                .iter()
                .any(|e| e.message.contains("cinco minutos"))
        );
        reopened.0.close().unwrap();
        drop(reopened);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn provider_retry_survives_recovery_waits_and_can_be_cancelled() {
        let store = fixture();
        let job = store.create(&request("retry"), 7, 0).unwrap();
        store.claim().unwrap();
        assert!(store.schedule_provider_retry(job.id).unwrap());
        assert_eq!(store.get(job.id).unwrap().state, "queued");
        assert!(store.claim().unwrap().is_none());
        store.recover().unwrap();
        assert_eq!(store.get(job.id).unwrap().state, "queued");
        assert_eq!(store.claim().unwrap().unwrap().id, job.id);
        store.recover().unwrap();
        assert_eq!(store.get(job.id).unwrap().state, "queued");
        store.transition(job.id, "cancelled", "Cancelado").unwrap();
        store.recover().unwrap();
        assert!(store.claim().unwrap().is_none());
        assert_eq!(store.get(job.id).unwrap().state, "cancelled");
        assert_eq!(
            store
                .get(job.id)
                .unwrap()
                .events
                .iter()
                .filter(|e| e.message.contains("cinco minutos"))
                .count(),
            1
        );
    }

    #[test]
    fn retry_deadline_does_not_block_other_jobs_and_notice_is_delivered_once() {
        #[derive(Default)]
        struct Notices(Mutex<Vec<(i64, String, String)>>);
        impl BuildJournal for Notices {
            fn origin(&self) -> Result<i64> {
                Ok(7)
            }
            fn exists(&self, _: i64) -> Result<bool> {
                Ok(true)
            }
            fn deliver(&self, id: i64, key: &str, text: &str) -> Result<()> {
                let mut notices = self.0.lock().unwrap();
                if !notices.iter().any(|(_, existing, _)| existing == key) {
                    notices.push((id, key.into(), text.into()));
                }
                Ok(())
            }
        }
        let store = fixture();
        let job = store.create(&request("offline"), 7, 0).unwrap();
        store.account(job.id, 0, Some("preserved-thread")).unwrap();
        store.claim().unwrap();
        store.begin_attempt(job.id).unwrap();
        assert!(store.schedule_provider_retry(job.id).unwrap());
        assert!(!store.schedule_provider_retry(job.id).unwrap());
        let seconds: i64 = store
            .0
            .with_connection(|c| {
                Ok(c.query_row(
            "SELECT unixepoch(retry_at)-unixepoch('now') FROM plugin_build_jobs WHERE id=?1",
            [job.id], |r| r.get(0)
        )?)
            })
            .unwrap();
        assert!((250..=305).contains(&seconds));
        assert!(store.claim().unwrap().is_none());
        let other = store.create(&request("other"), 9, 0).unwrap();
        assert_eq!(store.claim().unwrap().unwrap().id, other.id);
        store
            .transition(other.id, "completed", "Prueba completada")
            .unwrap();
        let notices = Notices::default();
        store.deliver(&notices).unwrap();
        store.deliver(&notices).unwrap();
        let messages = notices.0.lock().unwrap();
        let retries: Vec<_> = messages
            .iter()
            .filter(|(_, _, text)| text.contains("cinco minutos"))
            .collect();
        assert_eq!(retries.len(), 1);
        assert_eq!(retries[0].0, 7);
        assert!(retries[0].2.contains("Codex"));
        drop(messages);
        // Adelanta solo el reloj persistido de la DB temporal: no espera cinco minutos.
        store
            .0
            .with_connection(|c| {
                c.execute(
                    "UPDATE plugin_build_jobs SET retry_at='2000-01-01T00:00:00.000Z' WHERE id=?1",
                    [job.id],
                )?;
                Ok(())
            })
            .unwrap();
        let retried = store.claim().unwrap().unwrap();
        assert_eq!(retried.id, job.id);
        assert_eq!(retried.thread_id.as_deref(), Some("preserved-thread"));
        assert_eq!(retried.trigger_conversation_id, 7);
        assert!(store.claim().unwrap().is_none());
    }
    #[test]
    fn legacy_elapsed_budget_does_not_stop_attempts() {
        let store = fixture();
        let job = store.create(&request("estimate"), 1, 0).unwrap();
        assert_eq!(job.max_seconds, 30);
        assert_eq!(store.claim().unwrap().unwrap().max_seconds, 30);
        store.account(job.id, 599, None).unwrap();
        assert!(store.begin_attempt(job.id).unwrap());
        store.account(job.id, 1, None).unwrap();
        assert!(store.begin_attempt(job.id).unwrap());
    }
    #[test]
    fn origin_idempotency_outbox_and_recovery_are_durable() {
        let store = fixture();
        let first = store.create(&request("mail"), 1, 4).unwrap();
        assert_eq!(store.create(&request("mail"), 1, 4).unwrap().id, first.id);
        assert!(store.create(&request("mail"), 2, 4).is_err());
        let second = store.create(&request("calendar"), 2, 0).unwrap();
        let journal = Journal::default();
        *journal.fail.lock().unwrap() = true;
        assert!(store.deliver(&journal).is_err());
        store.deliver(&journal).unwrap();
        store.deliver(&journal).unwrap();
        let events = journal.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, 1);
        assert_eq!(events[1].0, 2);
        assert!(
            journal
                .messages
                .lock()
                .unwrap()
                .iter()
                .all(|message| message.starts_with("Plugin · example.com:")
                    && !message.contains("trabajo 1"))
        );
        drop(events);
        assert_eq!(store.claim().unwrap().unwrap().id, first.id);
        assert!(store.begin_attempt(first.id).unwrap());
        assert!(store.begin_attempt(first.id).unwrap());
        assert!(store.begin_attempt(first.id).unwrap());
        store
            .account(first.id, 12, Some("isolated-thread"))
            .unwrap();
        store.recover().unwrap();
        assert_eq!(store.get(first.id).unwrap().state, "interrupted");
        assert_eq!(store.get(second.id).unwrap().state, "interrupted");
        assert_eq!(
            store.get(first.id).unwrap().thread_id.as_deref(),
            Some("isolated-thread")
        );
        assert_eq!(store.get(first.id).unwrap().spent_seconds, 12);
        store.deliver(&journal).unwrap();
        assert_eq!(journal.events.lock().unwrap().len(), 4);
    }
    #[test]
    fn cancelled_or_completed_jobs_cannot_be_reactivated_by_late_progress() {
        let store = fixture();
        let job = store.create(&request("mail"), 1, 0).unwrap();
        store.claim().unwrap();
        store.transition(job.id, "cancelled", "Cancelado").unwrap();
        assert!(
            !store
                .transition(job.id, "completed", "Respuesta tardía")
                .unwrap()
        );
        store.report(job.id, "Avance tardío").unwrap();
        assert_eq!(store.get(job.id).unwrap().summary, "Cancelado");
        let mut next = request("mail");
        next.request_key = "second".into();
        assert!(store.create(&next, 1, 0).is_ok());
    }
    #[test]
    fn queue_migration_preserves_existing_jobs_and_evidence() {
        let db = Database::memory().unwrap();
        let migrations = [
            gennie_core::Migration {
                version: 1,
                sql: include_str!("../migrations/0001_build_jobs.sql"),
            },
            gennie_core::Migration {
                version: 2,
                sql: include_str!("../migrations/0002_assignment_queue.sql"),
            },
            gennie_core::Migration {
                version: 3,
                sql: include_str!("../migrations/0003_retry_source.sql"),
            },
            gennie_core::Migration {
                version: 4,
                sql: include_str!("../migrations/0004_recommended_route.sql"),
            },
            gennie_core::Migration {
                version: 5,
                sql: include_str!("../migrations/0005_retry_schedule.sql"),
            },
        ];
        db.migrate("plugins", &migrations[..1]).unwrap();
        db.with_connection(|c| {
            c.execute("INSERT INTO plugin_build_jobs(request_key,plugin_id,origin,objective,trigger_conversation_id,base_revision,state,max_attempts,max_seconds,summary) VALUES('legacy','mail','https://example.com','Consultar',7,4,'waiting_user',4,600,'Esperando acceso')", [])?;
            c.execute("INSERT INTO plugin_build_events(job_id,message,delivered) VALUES(1,'Antes de migrar',1)", [])?;
            Ok(())
        }).unwrap();
        db.migrate("plugins", &migrations).unwrap();
        let store = Store(db);
        let job = store.get(1).unwrap();
        assert_eq!(job.state, "waiting_user");
        assert_eq!(job.base_revision, 4);
        assert!(!job.refresh_base);
        assert_eq!(job.events[0].message, "Antes de migrar");
        let mut next = request("mail");
        next.request_key = "assignment_1_2_0".into();
        assert_eq!(store.queue(&next, 8).unwrap().state, "queued");
    }
    #[test]
    fn assignment_revisions_queue_separately_and_keep_their_source_chat() {
        let store = fixture();
        let mut first_request = request("mail");
        first_request.request_key = "assignment_9_1_0".into();
        let first = store.queue(&first_request, 7).unwrap();
        let mut second_request = first_request.clone();
        second_request.request_key = "assignment_9_2_0".into();
        second_request.objective = "Objetivo editado".into();
        let second = store.queue(&second_request, 8).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(store.queue(&second_request, 8).unwrap().id, second.id);
        assert_eq!(store.claim().unwrap().unwrap().id, first.id);
        store.set_initial_base(first.id, 4).unwrap();
        assert_eq!(store.get(first.id).unwrap().base_revision, 4);
        assert!(!store.get(first.id).unwrap().refresh_base);
        store
            .transition(first.id, "completed", "Terminado")
            .unwrap();
        assert_eq!(store.claim().unwrap().unwrap().id, second.id);
        store.set_initial_base(second.id, 5).unwrap();
        let second = store.get(second.id).unwrap();
        assert_eq!(second.base_revision, 5);
        assert_eq!(second.trigger_conversation_id, 8);
        assert_eq!(second.objective, "Objetivo editado");
    }
    #[test]
    fn duplicate_live_targets_are_rejected_after_legacy_budget() {
        let store = fixture();
        let job = store.create(&request("mail"), 1, 0).unwrap();
        let mut duplicate = request("mail");
        duplicate.request_key = "another".into();
        assert!(store.create(&duplicate, 1, 0).is_err());
        store.claim().unwrap();
        store.account(job.id, 600, None).unwrap();
        assert!(store.begin_attempt(job.id).unwrap());
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    #[test]
    fn investigation_steps_survive_delivery_and_terminal_state() {
        let db = Database::memory().unwrap();
        db.migrate(
            "plugins",
            &[
                gennie_core::Migration {
                    version: 1,
                    sql: include_str!("../migrations/0001_build_jobs.sql"),
                },
                gennie_core::Migration {
                    version: 2,
                    sql: include_str!("../migrations/0002_assignment_queue.sql"),
                },
                gennie_core::Migration {
                    version: 3,
                    sql: include_str!("../migrations/0003_retry_source.sql"),
                },
                gennie_core::Migration {
                    version: 4,
                    sql: include_str!("../migrations/0004_recommended_route.sql"),
                },
                gennie_core::Migration {
                    version: 5,
                    sql: include_str!("../migrations/0005_retry_schedule.sql"),
                },
            ],
        )
        .unwrap();
        let store = Store(db);
        let job = store
            .create(
                &crate::BuildRequest {
                    request_key: "steps".into(),
                    plugin_id: "mail".into(),
                    origin: "https://example.com".into(),
                    objective: "Consultar".into(),
                    max_attempts: 4,
                    max_seconds: 600,
                    retry_of: None,
                },
                7,
                0,
            )
            .unwrap();
        store.claim().unwrap();
        store
            .report(job.id, "Investigando la petición de la lista")
            .unwrap();
        store.report(job.id, "Verificando extracción web").unwrap();
        store.transition(job.id, "completed", "Verificado").unwrap();
        let job = store.get(job.id).unwrap();
        assert_eq!(job.events.len(), 4);
        assert_eq!(
            job.events[1].message,
            "Investigando la petición de la lista"
        );
        assert!(job.events.windows(2).all(|w| w[0].id < w[1].id));
        assert!(job.events.iter().all(|e| e.created_at.ends_with('Z')));
        assert_eq!(store.list().unwrap()[0].events.len(), 4);
    }
}
