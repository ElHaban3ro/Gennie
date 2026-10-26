//! Registro de planes del usuario; no ejecuta trabajos ni entrega notificaciones.
mod follow_up;
mod tools;
pub use follow_up::FollowUpPolicy;
use gennie_core::{AppModule, AppState, Migration, Result};
use gennie_db::{Database, params};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
pub use tools::tools;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    OneTime,
    Recurring,
    Goal,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Paused,
    Completed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    Desktop,
    Web,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub name: String,
    pub kind: AppKind,
    pub url: Option<String>,
    #[serde(default)]
    pub plugin_available: bool,
    #[serde(default = "yes")]
    pub manual_required: bool,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    Weekly,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recurrence {
    pub frequency: Frequency,
    pub local_time: String,
    /// ISO: lunes=1, domingo=7. Vacío para frecuencia diaria.
    #[serde(default)]
    pub weekdays: Vec<u8>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preparation {
    pub interval_minutes: Option<u32>,
    pub rationale: Option<String>,
    #[serde(default)]
    pub steps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub title: String,
    pub description: String,
    pub context: Option<String>,
    pub reason: Option<String>,
    pub kind: Kind,
    pub priority: Priority,
    pub status: Status,
    /// Zona IANA para recurrencias; instantes puntuales siempre ISO 8601 UTC.
    pub timezone: String,
    pub notify_at: Option<String>,
    pub recurrence: Option<Recurrence>,
    #[serde(default)]
    pub preparation: Preparation,
    #[serde(default)]
    pub applications: Vec<Application>,
    // Ausente en registros anteriores; no cambia su representación canónica/idempotencia.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_up: Option<FollowUpPolicy>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    pub id: i64,
    pub revision: u32,
    pub document: Document,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Default)]
pub struct AssignmentsModule;
impl AppModule for AssignmentsModule {
    fn name(&self) -> &'static str {
        "assignments"
    }
    fn migration_namespace(&self) -> &'static str {
        // Conserva checksums y fechas de las instalaciones previas al cambio de nombre.
        "events"
    }
    fn migrations(&self) -> &'static [Migration] {
        &[
            Migration {
                version: 1,
                sql: include_str!("../migrations/0001_events.sql"),
            },
            Migration {
                version: 2,
                sql: include_str!("../migrations/0002_assignments.sql"),
            },
            Migration {
                version: 3,
                sql: include_str!("../migrations/0003_active_status.sql"),
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
#[derive(Clone)]
pub struct Store {
    db: Database,
    revision: Arc<AtomicU64>,
}
impl Store {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            revision: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn clock_context(&self) -> Result<serde_json::Value> {
        self.db.with_connection(|c| {
            let now: String =
                c.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| {
                    r.get(0)
                })?;
            Ok(serde_json::json!({"now_utc":now,"timezone":system_timezone()}))
        })
    }
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }
    pub fn create(&self, request_key: &str, mut document: Document) -> Result<Record> {
        text(request_key, 120, "Identificador de petición")?;
        self.validate(&mut document)?;
        let encoded = serde_json::to_string(&document)?;
        let record = self.db.with_connection(|c| {
            let tx = c.transaction()?;
            let existing = {
                let mut statement = tx.prepare("SELECT id,document FROM assignments WHERE request_key=?1")?;
                let mut rows = statement.query([request_key])?;
                rows.next()?.map(|r| Ok::<_,gennie_db::Error>((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).transpose()?
            };
            let id = if let Some((id,saved)) = existing {
                if saved != encoded { return Err("La petición ya existe con otros datos; consulta el encargo antes de editarlo".into()); }
                id
            } else {
                tx.execute("INSERT INTO assignments(request_key,document) VALUES(?1,?2)", params![request_key,encoded])?;
                tx.last_insert_rowid()
            };
            let record = read(&tx, id)?;
            tx.commit()?;
            Ok(record)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(record)
    }
    pub fn update(&self, id: i64, revision: u32, mut document: Document) -> Result<Record> {
        self.validate(&mut document)?;
        let record = self.db.with_connection(|c| {
            let tx = c.transaction()?;
            let changed = tx.execute("UPDATE assignments SET document=?1,revision=revision+1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2 AND revision=?3",params![serde_json::to_string(&document)?,id,revision])?;
            if changed != 1 { return Err("El encargo cambió o no existe. Vuelve a consultarlo antes de guardar".into()); }
            let record = read(&tx, id)?;
            tx.commit()?;
            Ok(record)
        })?;
        self.revision.fetch_add(1, Ordering::Release);
        Ok(record)
    }
    pub fn get(&self, id: i64) -> Result<Record> {
        self.db.with_connection(|c| read(c, id))
    }
    pub fn list(&self, offset: u32, limit: u32) -> Result<Vec<Record>> {
        if !(1..=100).contains(&limit) {
            return Err("Límite entre 1 y 100".into());
        }
        self.db.with_connection(|c| {
            let mut q = c.prepare(
                "SELECT id FROM assignments ORDER BY updated_at DESC,id DESC LIMIT ?1 OFFSET ?2",
            )?;
            let ids = q
                .query_map(params![limit, offset], |r| r.get::<_, i64>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ids.into_iter().map(|id| read(c, id)).collect()
        })
    }
    fn validate(&self, d: &mut Document) -> Result<()> {
        text(&d.title, 200, "Título")?;
        if d.title.chars().any(char::is_control) {
            return Err("El título debe ocupar una sola línea".into());
        }
        text(&d.description, 4000, "Descripción")?;
        for (value, name) in [
            (&d.context, "Contexto"),
            (&d.reason, "Motivo"),
            (&d.preparation.rationale, "Razón de la preparación"),
        ] {
            if let Some(value) = value {
                text(value, 2000, name)?;
            }
        }
        let zone = std::path::Path::new(&d.timezone);
        if d.timezone.len() > 100
            || !zone
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            || d.timezone.is_empty()
            || !std::path::Path::new("/usr/share/zoneinfo")
                .join(zone)
                .is_file()
        {
            return Err(
                "Selecciona una zona horaria IANA válida, por ejemplo America/Bogota".into(),
            );
        }
        {
            use std::io::Read;
            let mut header = [0u8; 4];
            std::fs::File::open(std::path::Path::new("/usr/share/zoneinfo").join(zone))?
                .read_exact(&mut header)?;
            if &header != b"TZif" {
                return Err("La zona horaria no contiene reglas IANA válidas".into());
            }
        }
        if let Some(at) = &d.notify_at {
            let valid = self.db.with_connection(|c| {
                Ok(c.query_row(
                    "SELECT length(?1)=20 AND strftime('%Y-%m-%dT%H:%M:%SZ',?1,'+0 seconds')=?1",
                    [at],
                    |r| r.get::<_, Option<bool>>(0),
                )?
                .unwrap_or(false))
            })?;
            if !valid {
                return Err("La fecha debe ser válida y UTC: AAAA-MM-DDTHH:MM:SSZ".into());
            }
        }
        match d.kind {
            Kind::OneTime if d.notify_at.is_none() || d.recurrence.is_some() => {
                return Err(
                    "Un encargo puntual necesita fecha de aviso y no admite recurrencia".into(),
                );
            }
            Kind::Recurring if d.recurrence.is_none() || d.notify_at.is_some() => {
                return Err(
                    "Un encargo recurrente necesita recurrencia local y no fecha puntual".into(),
                );
            }
            Kind::Goal if d.recurrence.is_some() => return Err(
                "Un objetivo admite fecha límite opcional; usa preparación para definir revisiones"
                    .into(),
            ),
            _ => {}
        }
        if let Some(r) = &d.recurrence {
            let b = r.local_time.as_bytes();
            if b.len() != 5
                || b[2] != b':'
                || ![b[0], b[1], b[3], b[4]].iter().all(u8::is_ascii_digit)
                || &r.local_time[..2] > "23"
                || &r.local_time[3..] > "59"
            {
                return Err("La hora debe ser HH:mm (00:00–23:59)".into());
            }
            let days: std::collections::BTreeSet<_> = r.weekdays.iter().copied().collect();
            if days.len() != r.weekdays.len()
                || days.iter().any(|d| !(1..=7).contains(d))
                || (r.frequency == Frequency::Daily && !days.is_empty())
                || (r.frequency == Frequency::Weekly && days.is_empty())
            {
                return Err("La recurrencia semanal necesita días únicos entre 1 y 7; la diaria no admite días".into());
            }
        }
        if d.preparation
            .interval_minutes
            .is_some_and(|n| !(1..=525600).contains(&n))
        {
            return Err("El intervalo de preparación debe estar entre 1 minuto y un año".into());
        }
        if d.preparation.interval_minutes.is_some() && d.preparation.rationale.is_none() {
            return Err("Explica por qué elegiste ese intervalo de preparación".into());
        }
        if let Some(policy) = &d.follow_up {
            policy.validate(d.preparation.interval_minutes)?;
        }
        if d.preparation.steps.len() > 30 || d.applications.len() > 20 {
            return Err("Demasiados pasos o aplicaciones".into());
        }
        for step in &d.preparation.steps {
            text(step, 1000, "Paso de preparación")?;
        }
        for app in &mut d.applications {
            text(&app.name, 200, "Aplicación")?;
            if let Some(url) = &app.url {
                text(url, 2000, "URL")?;
                if app.kind != AppKind::Web
                    || !(url.starts_with("https://") || url.starts_with("http://"))
                    || url.chars().any(char::is_whitespace)
                {
                    return Err(
                        "La URL debe pertenecer a una aplicación web y usar HTTP o HTTPS".into(),
                    );
                }
            }
            // No hay catálogo de plugins todavía: el agente no puede inventar disponibilidad.
            app.plugin_available = false;
            app.manual_required = true;
        }
        if serde_json::to_vec(d)?.len() > 12000 {
            return Err("El encargo supera el tamaño máximo de 12 KB".into());
        }
        Ok(())
    }
}
fn text(value: &str, max: usize, label: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > max || value.contains('\0') {
        return Err(format!("{label}: escribe entre 1 y {max} caracteres").into());
    }
    Ok(())
}
fn read(c: &gennie_db::SqliteConnection, id: i64) -> Result<Record> {
    let (id, revision, document, created_at, updated_at) = c.query_row(
        "SELECT id,revision,document,created_at,updated_at FROM assignments WHERE id=?1",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, String>(2)?,
                r.get(3)?,
                r.get(4)?,
            ))
        },
    )?;
    Ok(Record {
        id,
        revision,
        document: serde_json::from_str(&document)?,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests;

/// Referencia local para el editor y el agente; sin crear procesos ni consultar la red.
pub fn system_timezone() -> String {
    std::fs::canonicalize("/etc/localtime")
        .ok()
        .and_then(|p| {
            p.to_str()
                .and_then(|p| p.split_once("/zoneinfo/").map(|(_, zone)| zone.to_owned()))
        })
        .unwrap_or_else(|| "UTC".into())
}
