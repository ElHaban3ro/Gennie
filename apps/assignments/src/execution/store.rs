use super::*;
use crate::read;
pub(super) enum Due {
    Idle,
    Advanced,
    Run(i64, Box<Policy>),
}
use gennie_db::{SqliteConnection, params};
fn version(c: &SqliteConnection, id: &str, revision: u32) -> Result<Policy> {
    let text: String = c.query_row(
        "SELECT document FROM assignment_execution_versions WHERE policy_id=?1 AND revision=?2",
        params![id, revision],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&text)?)
}
fn current(c: &SqliteConnection, policy: &Policy) -> Result<bool> {
    let mut q = c.prepare("SELECT instance_id,revision,document FROM assignments WHERE id=?1")?;
    let row = q
        .query_map([policy.assignment.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .next()
        .transpose()?;
    Ok(row.is_some_and(|(id, revision, document)| {
        id == policy.assignment.instance_id
            && revision == policy.assignment.revision
            && serde_json::from_str::<crate::Document>(&document)
                .is_ok_and(|d| d.status == Status::Active && d == policy.assignment.document)
    }))
}
fn pending_target(c: &SqliteConnection, target: &Target) -> Result<bool> {
    let mut q=c.prepare("SELECT v.document FROM assignment_execution_runs r JOIN assignment_execution_versions v ON v.policy_id=r.policy_id AND v.revision=r.policy_revision WHERE r.state='uncertain' AND r.resolution IS NULL")?;
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let saved: Policy = serde_json::from_str(&row?)?;
        if saved.target.plugin_id() == target.plugin_id()
            && saved.target.function() == target.function()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn present(c: &SqliteConnection, id: &str) -> Result<Value> {
    let (revision, enabled, state, next): (u32, bool, String, Option<String>) = c.query_row(
        "SELECT revision,enabled,state,next_at FROM assignment_execution_policies WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let policy = version(c, id, revision)?;
    let running:i64=c.query_row("SELECT count(*) FROM assignment_execution_runs WHERE policy_id=?1 AND state IN ('reserved','running')",[id],|r|r.get(0))?;
    Ok(
        json!({"id":id,"revision":revision,"enabled":enabled,"state":state,"next_at":next,"current":current(c,&policy)?,"running":running>0,"configuration":policy}),
    )
}
fn trigger(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Delivery => "delivery",
        Trigger::Preparation => "preparation",
    }
}
impl Store {
    /// Próximas fechas ya guardadas de los encargos visibles; no prepara ni ejecuta funciones.
    pub fn upcoming_execution_times(
        &self,
        assignment_ids: &[i64],
    ) -> Result<std::collections::BTreeMap<i64, String>> {
        if assignment_ids.len() > 100 || assignment_ids.iter().any(|id| *id <= 0) {
            return Err(
                "Selecciona hasta cien encargos válidos para consultar sus próximas ejecuciones"
                    .into(),
            );
        }
        if assignment_ids.is_empty() {
            return Ok(Default::default());
        }
        self.db.with_connection(|c| {
            let mut q = c.prepare(
                "SELECT p.assignment_id,p.instance_id,p.id,p.revision,p.next_at FROM assignment_execution_policies p JOIN assignments a ON a.id=p.assignment_id AND a.instance_id=p.instance_id WHERE p.enabled=1 AND p.state='active' AND p.next_at IS NOT NULL AND p.assignment_id IN (SELECT value FROM json_each(?1))",
            )?;
            let rows = q.query_map([serde_json::to_string(assignment_ids)?], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, u32>(3)?, r.get::<_, String>(4)?))
            })?;
            let mut upcoming = std::collections::BTreeMap::<i64, (i64, String)>::new();
            for row in rows {
                let (id, instance, policy_id, revision, at) = row?;
                let policy = version(c, &policy_id, revision)?;
                if policy.assignment.id != id || policy.assignment.instance_id != instance || !current(c, &policy)? {
                    continue;
                }
                let epoch = schedule::epoch(&at)?;
                if upcoming.get(&id).is_none_or(|(previous, _)| epoch < *previous) {
                    upcoming.insert(id, (epoch, at));
                }
            }
            Ok(upcoming.into_iter().map(|(id, (_, at))| (id, at)).collect())
        })
    }

    /// Descubrir resultados compartidos, incluidos los guardados sin aviso; sin cargar cuerpos.
    pub fn execution_activity_for_origin(
        &self,
        assignment_id: i64,
        origin: &crate::proposals::Origin,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Value> {
        if !(1..=50).contains(&limit) || before.is_some_and(|id| id <= 0) {
            return Err("Página de actividad no válida".into());
        }
        self.db.with_connection(|c| {
            let record=read(c,assignment_id)?;
            let mut q=c.prepare("SELECT r.id,r.policy_id,r.policy_revision,r.scheduled_at,r.state,r.code,r.output IS NOT NULL,r.output_omitted,r.resolution FROM assignment_execution_runs r JOIN assignment_execution_policies p ON p.id=r.policy_id JOIN assignment_execution_versions v ON v.policy_id=r.policy_id AND v.revision=r.policy_revision WHERE p.instance_id=?1 AND json_extract(v.document,'$.delivery.conversation_id')=?2 AND json_extract(v.document,'$.delivery.epoch')=?3 AND (?4 IS NULL OR r.id<?4) ORDER BY r.id DESC LIMIT ?5")?;
            let rows=q.query_map(params![record.instance_id,origin.conversation_id,origin.epoch,before,limit+1],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,bool>(6)?,r.get::<_,bool>(7)?,r.get::<_,Option<String>>(8)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let more=rows.len()>limit as usize;
            let rows=rows.into_iter().take(limit as usize).map(|(id,policy,revision,at,state,code,output,omitted,resolution)|json!({"run_id":id,"policy_id":policy,"policy_revision":revision,"scheduled_at":at,"state":state,"code":code,"has_output":output,"output_omitted":omitted,"resolution":resolution})).collect::<Vec<_>>();
            Ok(json!({"executions":rows,"next_before":if more{rows.last().map(|r|r["run_id"].clone())}else{None},"read_only":true,"execution_activated":false}))
        })
    }
    /// Retirar consentimiento no depende del catálogo, disponibilidad o efecto de un plugin.
    pub(super) fn revoke_contextual_notice(&self, id: &str, expected: u32) -> Result<Value> {
        let value=self.db.with_connection(|c| {let tx=c.transaction()?;
            let revision:u32=tx.query_row("SELECT revision FROM assignment_execution_policies WHERE id=?1",[id],|r|r.get(0))?;
            if revision!=expected {return Err("La configuración cambió; vuelve a consultarla".into());}
            let mut policy=version(&tx,id,revision)?;policy.contextual_notice=false;policy.delivery=None;
            let next=revision.checked_add(1).ok_or("Revisión agotada")?;
            tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES(?1,?2,?3)",params![id,next,serde_json::to_string(&policy)?])?;
            tx.execute("UPDATE assignment_execution_policies SET revision=?2,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,next])?;
            let result=present(&tx,id)?;tx.commit()?;Ok(result)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(value)
    }
    /// Revalidar antes de cada envío remoto sin consultar tablas de otro propietario.
    pub fn contextual_notice_permitted(
        &self,
        run_id: i64,
        destination: &notices::Delivery,
    ) -> Result<bool> {
        self.db.with_connection(|c| {
            let (id, revision, state): (String, u32, String) = c.query_row(
                "SELECT policy_id,policy_revision,state FROM assignment_execution_runs WHERE id=?1",
                [run_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let original = version(c, &id, revision)?;
            let latest: u32 = c.query_row(
                "SELECT revision FROM assignment_execution_policies WHERE id=?1",
                [&id],
                |r| r.get(0),
            )?;
            let latest = version(c, &id, latest)?;
            Ok(state == "succeeded"
                && original.contextual_notice
                && original.delivery.as_ref() == Some(destination)
                && current(c, &original)?
                && serde_json::to_value(&original)? == serde_json::to_value(&latest)?)
        })
    }
    pub(super) fn execution_assignment(&self, reference: &Reference) -> Result<crate::Record> {
        self.db.with_connection(|c| {
            let record = read(c, reference.id)?;
            if record.instance_id != reference.instance_id || record.revision != reference.revision
            {
                return Err("El encargo cambió; vuelve a consultar su configuración".into());
            }
            Ok(record)
        })
    }
    pub(super) fn save_execution(
        &self,
        policy: Policy,
        expected: u32,
        enabled: bool,
        next: i64,
    ) -> Result<Value> {
        let result=self.db.with_connection(|c| {
            let tx=c.transaction()?;
            if !current(&tx,&policy)? {return Err("El encargo cambió antes de confirmar".into());}
            if pending_target(&tx,&policy.target)? {return Err("Revisa el resultado incierto de esta función antes de volver a programarla".into());}
            let existing={let mut q=tx.prepare("SELECT id,revision FROM assignment_execution_policies WHERE instance_id=?1 AND trigger=?2")?;q.query_map(params![policy.assignment.instance_id,trigger(policy.trigger)],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?)))?.next().transpose()?};
            let (id,revision)=if let Some((id,revision))=existing {
                if revision!=expected {return Err("La configuración de ejecución cambió; vuelve a cargarla".into());}
                let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignment_execution_runs WHERE policy_id=?1 AND state='uncertain' AND resolution IS NULL)",[&id],|r|r.get(0))?;
                if pending {return Err("Revisa los intentos inciertos de esta configuración antes de sustituir su función".into());}
                let running:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignment_execution_runs WHERE policy_id=?1 AND state IN ('reserved','running'))",[&id],|r|r.get(0))?;
                if running {return Err("Espera a que termine la ejecución antes de cambiar su función o parámetros".into());}
                (id,revision.checked_add(1).ok_or("Revisión agotada")?)
            }else{
                if expected!=0 {return Err("La configuración ya no existe".into());}
                (tx.query_row("SELECT lower(hex(randomblob(16)))",[],|r|r.get::<_,String>(0))?,1)
            };
            let state=if enabled {"active"}else{"paused"};
            tx.execute("INSERT INTO assignment_execution_policies(id,assignment_id,instance_id,trigger,revision,enabled,state,next_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,enabled=excluded.enabled,state=excluded.state,next_at=excluded.next_at,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",params![id,policy.assignment.id,policy.assignment.instance_id,trigger(policy.trigger),revision,enabled,state,if enabled{Some(schedule::utc(next)?)}else{None}])?;
            tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES(?1,?2,?3)",params![id,revision,serde_json::to_string(&policy)?])?;
            let result=present(&tx,&id)?;tx.commit()?;Ok(result)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(result)
    }
    pub fn execution_policies(&self, assignment_id: i64) -> Result<Value> {
        self.db.with_connection(|c| {
            let record=read(c,assignment_id)?;
            let ids={let mut q=c.prepare("SELECT id FROM assignment_execution_policies WHERE instance_id=?1 ORDER BY trigger")?;q.query_map([&record.instance_id],|r|r.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?};
            let pending:i64=c.query_row("SELECT count(*) FROM assignment_execution_notices n JOIN assignment_execution_runs r ON r.id=n.run_id JOIN assignment_execution_policies p ON p.id=r.policy_id WHERE p.instance_id=?1 AND n.state='pending'",[&record.instance_id],|r|r.get(0))?;
            Ok(json!({"policies":ids.iter().map(|id|present(c,id)).collect::<Result<Vec<_>>>()?,"notices_pending":pending}))
        })
    }
    pub(super) fn pause_execution(&self, id: &str, expected: u32) -> Result<Value> {
        let value=self.db.with_connection(|c| {
            let tx=c.transaction()?;
            let revision:u32=tx.query_row("SELECT revision FROM assignment_execution_policies WHERE id=?1",[id],|r|r.get(0))?;
            if revision!=expected {return Err("La ejecución cambió; vuelve a cargarla".into());}
            let next=revision.checked_add(1).ok_or("Revisión agotada")?;
            let policy=version(&tx,id,revision)?;
            tx.execute("INSERT INTO assignment_execution_versions(policy_id,revision,document) VALUES(?1,?2,?3)",params![id,next,serde_json::to_string(&policy)?])?;
            tx.execute("UPDATE assignment_execution_policies SET enabled=0,state=CASE WHEN state='review_required' THEN state ELSE 'paused' END,revision=?2,next_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,next])?;
            let result=present(&tx,id)?;tx.commit()?;Ok(result)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(value)
    }
    pub fn recover_executions(&self) -> Result<()> {
        self.db.with_connection(|c| {
            let tx=c.transaction()?;
            let ids={let mut q=tx.prepare("SELECT id FROM assignment_execution_runs WHERE state IN ('reserved','running')")?;q.query_map([],|r|r.get::<_,i64>(0))?.collect::<std::result::Result<Vec<_>,_>>()?};
            tx.execute("UPDATE assignment_execution_runs SET state='uncertain',code='interrupted_after_dispatch',completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='running'",[])?;
            tx.execute("UPDATE assignment_execution_runs SET state='cancelled',code='interrupted_before_dispatch',completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='reserved'",[])?;
            tx.execute("UPDATE assignment_execution_policies SET enabled=0,state='review_required',next_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id IN (SELECT policy_id FROM assignment_execution_runs WHERE state='uncertain' AND resolution IS NULL)",[])?;

            for id in ids { notices::queue(&tx,id)?; }
            tx.commit()?;Ok(())
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(())
    }
    pub(super) fn next_execution(&self) -> Result<Option<i64>> {
        self.db.with_connection(|c| {
            let next: Option<String> = c.query_row(
                "SELECT min(next_at) FROM assignment_execution_policies WHERE enabled=1",
                [],
                |r| r.get(0),
            )?;
            next.as_deref().map(schedule::epoch).transpose()
        })
    }
    pub(super) fn reserve_execution(&self, now: i64) -> Result<Due> {
        let due = self.db.with_connection(|c| {
            let tx=c.transaction()?;let now_utc=schedule::utc(now)?;
            let row={let mut q=tx.prepare("SELECT id,revision,next_at FROM assignment_execution_policies WHERE enabled=1 AND next_at<=?1 ORDER BY next_at,id LIMIT 1")?;q.query_map([&now_utc],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?)))?.next().transpose()?};
            let Some((id,revision,due))=row else{return Ok(Due::Idle);};
            let policy=version(&tx,&id,revision)?;
            if !current(&tx,&policy)? || pending_target(&tx,&policy.target)? {
                tx.execute("UPDATE assignment_execution_policies SET enabled=0,state='stale',next_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",[id])?;tx.commit()?;return Ok(Due::Advanced);
            }
            let due=schedule::epoch(&due)?;
            let slot=if policy.misfire==Misfire::RunLatest {policy.calendar.latest_at(now)?.unwrap_or(due).max(due)}else{due};
            let until=policy.until.as_deref().map(schedule::epoch).transpose()?;
            let skipped=now.saturating_sub(slot)>i64::from(policy.max_lateness_minutes)*60 || until.is_some_and(|at|now>at);
            let next=policy.calendar.next_after(now)?.filter(|at|until.is_none_or(|end|*at<=end));
            tx.execute("UPDATE assignment_execution_policies SET next_at=?2,enabled=?3,state=?4,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,next.map(schedule::utc).transpose()?,next.is_some(),if next.is_some(){"active"}else{"completed"}])?;
            let changed=tx.execute("INSERT OR IGNORE INTO assignment_execution_runs(policy_id,policy_revision,scheduled_at,state,code,completed_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,revision,schedule::utc(slot)?,if skipped{"skipped"}else{"reserved"},if skipped{Some("missed_or_expired")}else{None},if skipped{Some(now_utc)}else{None}])?;
            if changed==1 && skipped { notices::queue(&tx,tx.last_insert_rowid())?; }
            let run=(changed==1 && !skipped).then(||tx.last_insert_rowid());
            tx.commit()?;Ok(run.map(|id|Due::Run(id,Box::new(policy))).unwrap_or(Due::Advanced))
        })?;
        if !matches!(due, Due::Idle) {
            self.revision.fetch_add(1, Ordering::Release);
        }
        Ok(due)
    }
    pub(super) fn dispatch_execution(&self, run_id: i64, now: i64) -> Result<bool> {
        let dispatched = self.db.with_connection(|c| {
            let tx=c.transaction()?;
            let (id,revision,state,scheduled):(String,u32,String,String)=tx.query_row("SELECT policy_id,policy_revision,state,scheduled_at FROM assignment_execution_runs WHERE id=?1",[run_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
            let policy=version(&tx,&id,revision)?;
            // completed permite la última ocurrencia puntual, ya reservada.
            let valid:bool=tx.query_row("SELECT revision=?2 AND state IN ('active','completed') FROM assignment_execution_policies WHERE id=?1",params![id,revision],|r|r.get(0))?;
            if now.saturating_sub(schedule::epoch(&scheduled)?)>i64::from(policy.max_lateness_minutes)*60 || state!="reserved" || !valid || !current(&tx,&policy)? || pending_target(&tx,&policy.target)? || policy.until.as_deref().map(schedule::epoch).transpose()?.is_some_and(|at|now>at){return Ok(false);}
            tx.execute("UPDATE assignment_execution_runs SET state='running',started_at=?2 WHERE id=?1",params![run_id,schedule::utc(now)?])?;
            tx.commit()?;Ok(true)
        })?;
        if dispatched {
            self.revision.fetch_add(1, Ordering::Release);
        }
        Ok(dispatched)
    }
    pub(super) fn finish_execution(
        &self,
        run_id: i64,
        state: &str,
        code: &str,
        output: Option<Value>,
        pause: bool,
    ) -> Result<()> {
        let encoded = output.map(|v| serde_json::to_string(&v)).transpose()?;
        let omitted = encoded.as_ref().is_some_and(|s| s.len() > 32768);
        self.db.with_connection(|c| {
            let tx=c.transaction()?;
            let id:String=tx.query_row("SELECT policy_id FROM assignment_execution_runs WHERE id=?1",[run_id],|r|r.get(0))?;
            if tx.execute("UPDATE assignment_execution_runs SET state=?2,code=?3,output=?4,output_omitted=?5,completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND state IN ('reserved','running')",params![run_id,state,code,if omitted{None}else{encoded},omitted])?!=1{return Err("El intento ya terminó o cambió de estado".into());}
            if pause {tx.execute("UPDATE assignment_execution_policies SET enabled=0,state=?2,next_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",params![id,if state=="uncertain"{"review_required"}else{"paused"}])?;}
            notices::queue(&tx,run_id)?;
            tx.commit()?;Ok(())
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(())
    }
    pub(super) fn execution_runs(&self, id: &str, before: Option<i64>) -> Result<Value> {
        self.db.with_connection(|c| {
            let mut q=c.prepare("SELECT id,policy_revision,scheduled_at,state,code,output,output_omitted,started_at,completed_at,resolution FROM assignment_execution_runs WHERE policy_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT 50")?;
            let rows=q.query_map(params![id,before],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,bool>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let runs=rows.into_iter().map(|(id,revision,at,state,code,output,omitted,started,completed,resolution)|->Result<Value>{Ok(json!({"id":id,"policy_revision":revision,"scheduled_at":at,"state":state,"code":code,"has_output":output.is_some(),"output_omitted":omitted,"started_at":started,"completed_at":completed,"resolution":resolution}))}).collect::<Result<Vec<_>>>()?;
            Ok(json!({"next_cursor":if runs.len()==50{runs.last().map(|r|r["id"].clone())}else{None},"runs":runs}))
        })
    }
    pub(super) fn execution_run_details(&self, run_id: i64) -> Result<Value> {
        self.db.with_connection(|c| {
            let (id,revision,at,state,code,output,omitted,resolution)=c.query_row("SELECT policy_id,policy_revision,scheduled_at,state,code,output,output_omitted,resolution FROM assignment_execution_runs WHERE id=?1",[run_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,bool>(6)?,r.get::<_,Option<String>>(7)?)))?;
            Ok(json!({"id":run_id,"policy_id":id,"policy_revision":revision,"scheduled_at":at,"state":state,"code":code,"output":output.map(|text|serde_json::from_str::<Value>(&text)).transpose()?,"output_omitted":omitted,"resolution":resolution,"configuration":version(c,&id,revision)?}))
        })
    }
    pub(super) fn reconcile_execution(&self, run_id: i64, resolution: &str) -> Result<Value> {
        if !matches!(resolution, "applied" | "not_applied") {
            return Err("Selecciona el resultado que comprobaste".into());
        }
        let result = self.db.with_connection(|c| {
            let tx=c.transaction()?;
            let (id,state,saved):(String,String,Option<String>)=tx.query_row("SELECT policy_id,state,resolution FROM assignment_execution_runs WHERE id=?1",[run_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            if state!="uncertain" || saved.as_ref().is_some_and(|r|r!=resolution){return Err("El intento no admite esta revisión".into());}
            tx.execute("UPDATE assignment_execution_runs SET resolution=?2,reviewed_at=COALESCE(reviewed_at,strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE id=?1",params![run_id,resolution])?;
            tx.execute("UPDATE assignment_execution_policies SET state='paused' WHERE id=?1 AND state='review_required' AND NOT EXISTS(SELECT 1 FROM assignment_execution_runs WHERE policy_id=?1 AND state='uncertain' AND resolution IS NULL)",[&id])?;
            let result=present(&tx,&id)?;tx.commit()?;Ok(result)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(result)
    }
}

impl Store {
    /// Lectura para el bot: solo resultados cuyo aviso fue autorizado para esa sesión/época.
    pub fn execution_for_origin(
        &self,
        run_id: i64,
        origin: &crate::proposals::Origin,
    ) -> Result<Value> {
        let details = self.execution_run_details(run_id)?;
        let policy: Policy = serde_json::from_value(details["configuration"].clone())?;
        let permitted = policy.delivery.as_ref().is_some_and(|d| {
            d.conversation_id == origin.conversation_id && d.epoch == origin.epoch
        });
        if !permitted {
            return Err("El resultado no está compartido con esta sesión del bot".into());
        }
        Ok(
            json!({"execution":{"run_id":run_id,"assignment_id":policy.assignment.id,"instance_id":policy.assignment.instance_id,"assignment_revision":policy.assignment.revision,"policy_id":details["policy_id"],"policy_revision":details["policy_revision"],"title":policy.assignment.document.title,"scheduled_at":details["scheduled_at"],"state":details["state"],"code":details["code"],"resolution":details["resolution"],"target":policy.target,"authorization":policy.assignment.document.authorization,"output":details["output"],"output_omitted":details["output_omitted"]},"read_only":true,"execution_activated":false}),
        )
    }
}
