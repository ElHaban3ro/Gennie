//! Experimento aislado: datos sintéticos, controles explícitos y ninguna migración de producto.
use super::*;
use gennie_assignments::proposals::{Begin, Candidate, Origin, Reference};
use gennie_assignments::{AssignmentsModule, Document, Record, Store};
use gennie_core::AppModule;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

mod live;

const CORPUS: &str = include_str!("../../../tests/fixtures/assignment-evaluation-v1.json");
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u32,
    seed: u64,
    clock: String,
    base_document: Value,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    family: String,
    request: String,
    documents: Vec<Value>,
    constraints: Vec<String>,
    rubric: Vec<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Representation {
    FlatV1,
    GroupedV1,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Strategy {
    LocalRules,
    Bot,
    BotGuide,
}

fn merge(base: &mut Value, overlay: &Value) {
    if let (Some(target), Some(source)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, value) in source {
            if value.is_object() && target.get(key).is_some_and(Value::is_object) {
                merge(target.get_mut(key).unwrap(), value);
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay.clone();
    }
}
fn group(key: &str) -> Result<&'static str> {
    match key {
        "title" | "description" | "context" | "reason" | "kind" | "priority" | "status"
        | "applications" => Ok("definition"),
        "timezone" | "notify_at" | "recurrence" | "preparation" => Ok("timing"),
        "authorization" | "method" | "follow_up" => Ok("control"),
        _ => {
            Err("La representación experimental desconoce un campo; no se permite perderlo".into())
        }
    }
}
fn project(document: &Value, representation: Representation) -> Result<Value> {
    if representation == Representation::FlatV1 {
        return Ok(document.clone());
    }
    let mut grouped = json!({"definition":{},"timing":{},"control":{}});
    for (key, value) in document.as_object().ok_or("Documento no válido")? {
        grouped[group(key)?][key] = value.clone();
    }
    Ok(grouped)
}
fn ungroup(value: &Value) -> Result<Value> {
    let mut flat = serde_json::Map::new();
    for (name, fields) in value.as_object().ok_or("Vista agrupada no válida")? {
        if !["definition", "timing", "control"].contains(&name.as_str()) {
            return Err("Grupo desconocido".into());
        }
        for (key, value) in fields.as_object().ok_or("Grupo no válido")? {
            if group(key)? != name || flat.insert(key.clone(), value.clone()).is_some() {
                return Err("Campo duplicado o en otro grupo".into());
            }
        }
    }
    Ok(Value::Object(flat))
}
fn project_payload(value: &mut Value, representation: Representation) -> Result<()> {
    if representation == Representation::FlatV1 {
        return Ok(());
    }
    // Solo cambia documentos conocidos, no texto libre, metadatos o resultados ajenos.
    if let Some(rows) = value
        .pointer_mut("/scope/assignments")
        .and_then(Value::as_array_mut)
    {
        for row in rows {
            row["document"] = project(&row["document"], representation)?;
        }
    }
    if let Some(rows) = value.get_mut("assignments").and_then(Value::as_array_mut) {
        for row in rows {
            row["document"] = project(&row["document"], representation)?;
        }
    }
    if let Some(rows) = value.get_mut("proposals").and_then(Value::as_array_mut) {
        for row in rows {
            let candidate = if row.get("candidate").is_some() {
                &mut row["candidate"]
            } else {
                row
            };
            if candidate.get("document").is_some_and(Value::is_object) {
                candidate["document"] = project(&candidate["document"], representation)?;
            }
        }
    }
    if let Some(rows) = value
        .pointer_mut("/version/previous_proposals")
        .and_then(Value::as_array_mut)
    {
        for row in rows {
            if row["candidate"]["document"].is_object() {
                row["candidate"]["document"] =
                    project(&row["candidate"]["document"], representation)?;
            }
        }
    }
    Ok(())
}

struct Cell {
    db: Database,
    store: Store,
    records: Vec<Record>,
    review: Value,
    origin: Origin,
}
fn setup(corpus: &Corpus, case: &Case, guide: bool) -> Result<Cell> {
    let db = Database::memory()?;
    db.migrate("events", AssignmentsModule.migrations())?;
    let store = Store::new(db.clone());
    let case_index = corpus
        .cases
        .iter()
        .position(|value| value.id == case.id)
        .ok_or("Caso desconocido")?;
    let mut records = Vec::new();
    for (n, overlay) in case.documents.iter().enumerate() {
        let mut document = corpus.base_document.clone();
        merge(&mut document, overlay);
        let document: Document = serde_json::from_value(document)?;
        let record = store.create(&format!("fixture-{n}"), document)?;
        let instance = format!("{:016x}{:016x}", corpus.seed, case_index * 100 + n + 1);
        // Valida por la API y siembra la identidad reproducible al insertar;
        // nunca desactiva el trigger de inmutabilidad ni usa una DB personal.
        db.with_connection(|c| {
            let tx=c.transaction()?;
            tx.execute("DELETE FROM assignments WHERE id=?1",[record.id])?;
            tx.execute("INSERT INTO assignments(id,request_key,document,revision,instance_id,created_at,updated_at) VALUES(?1,?2,?3,1,?4,?5,?5)",gennie_db::params![record.id,format!("fixture-{n}"),serde_json::to_string(&record.document)?,instance,corpus.clock])?;
            tx.commit()?;Ok(())
        })?;
        records.push(store.get(record.id)?);
    }
    let origin = Origin {
        conversation_id: 1,
        epoch: 1,
    };
    let review = store.begin_review(Begin {
        request_key: "fixture-review".into(),
        assignments: records
            .iter()
            .map(|row| Reference {
                id: row.id,
                instance_id: row.instance_id.clone(),
                revision: row.revision,
            })
            .collect(),
        origin: origin.clone(),
        instructions: case.request.clone(),
        use_guide: guide,
    })?;
    let id = review["id"].as_str().ok_or("Revisión sin ID")?.to_owned();
    db.with_connection(|c|{c.execute("UPDATE assignment_reviews SET id='evaluation-review',created_at=?2,updated_at=?2 WHERE id=?1",gennie_db::params![id,corpus.clock])?;Ok(())})?;
    let review = store.review("evaluation-review")?;
    Ok(Cell {
        db,
        store,
        records,
        review,
        origin,
    })
}

fn baseline(records: &[Record]) -> Result<Vec<Candidate>> {
    let mut candidates = Vec::new();
    let mut matched = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        if record.document.status != gennie_assignments::Status::Active {
            continue;
        }
        let mut current = serde_json::to_value(&record.document)?;
        current.as_object_mut().unwrap().remove("title");
        for earlier in &records[..index] {
            let mut previous = serde_json::to_value(&earlier.document)?;
            previous.as_object_mut().unwrap().remove("title");
            if current == previous && matched.insert(record.id) {
                let mut document = record.document.clone();
                document.status = gennie_assignments::Status::Paused;
                candidates.push(Candidate{assignment_id:record.id,title:"Agrupar la revisión duplicada".into(),rationale:"La definición coincide exactamente con otra salvo el título; se propone pausar esta copia y conservar la otra".into(),expected_benefit:"Evitar revisiones duplicadas; requiere decisión humana".into(),related_ids:vec![earlier.id],questions:vec![],document:Some(document)});
                break;
            }
        }
    }
    candidates.truncate(6);
    Ok(candidates)
}
fn constraints(case: &Case, cell: &Cell, review: &Value) -> Result<Value> {
    let mut observations = serde_json::Map::new();
    let proposals = review["proposals"]
        .as_array()
        .ok_or("Propuestas no válidas")?;
    let unchanged = cell
        .records
        .iter()
        .map(|row| {
            cell.store
                .get(row.id)
                .map(|current| current.revision == row.revision && current.document == row.document)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .all(|value| value);
    observations.insert("plans_unchanged".into(), json!(unchanged));
    for constraint in &case.constraints {
        let result = match constraint.as_str() {
            "questions_required" => proposals.iter().any(|row| {
                row["candidate"]["questions"]
                    .as_array()
                    .is_some_and(|values| !values.is_empty())
            }),
            "preserve_status"
            | "preserve_recurrence"
            | "preserve_authorization_routes"
            | "method_absent" => proposals.iter().all(|row| {
                let candidate = &row["candidate"];
                let proposed = &candidate["document"];
                if proposed.is_null() {
                    return true;
                }
                let original = cell
                    .records
                    .iter()
                    .find(|record| json!(record.id) == candidate["assignment_id"]);
                let Some(original) = original else {
                    return false;
                };
                let original = serde_json::to_value(&original.document).unwrap();
                match constraint.as_str() {
                    "preserve_status" => proposed["status"] == original["status"],
                    "preserve_recurrence" => ["timezone", "recurrence", "notify_at"]
                        .iter()
                        .all(|key| proposed[key] == original[key]),
                    "preserve_authorization_routes" => {
                        proposed.pointer("/authorization/allowed_routes")
                            == original.pointer("/authorization/allowed_routes")
                    }
                    _ => proposed["method"].is_null(),
                }
            }),
            _ => return Err("Comprobación del corpus desconocida".into()),
        };
        // Sin salida, una abstención no demuestra que se conservarían permisos o datos.
        observations.insert(
            constraint.clone(),
            if proposals.is_empty() {
                Value::Null
            } else {
                json!(result)
            },
        );
    }
    let mut authorization_changed = false;
    for row in proposals {
        if let Some(original) = cell
            .records
            .iter()
            .find(|r| json!(r.id) == row["candidate"]["assignment_id"])
            && row["candidate"]["document"].is_object()
        {
            authorization_changed |= row["candidate"]["document"]["authorization"]
                != serde_json::to_value(&original.document)?["authorization"];
        }
    }
    Ok(
        json!({"checks":observations,"authorization_changed_needs_review":authorization_changed,"semantic_quality":null,"human_review":"pending"}),
    )
}

fn local_row(corpus: &Corpus, case: &Case, representation: Representation) -> Result<Value> {
    let started = std::time::Instant::now();
    let cell = setup(corpus, case, false)?;
    let mut decoded = Vec::new();
    let mut presented = Vec::new();
    for record in &cell.records {
        let view = project(&serde_json::to_value(&record.document)?, representation)?;
        let flat = if representation == Representation::GroupedV1 {
            ungroup(&view)?
        } else {
            view.clone()
        };
        let mut reconstructed = record.clone();
        reconstructed.document = serde_json::from_value(flat)?;
        decoded.push(reconstructed);
        presented.push(view);
    }
    let proposals = baseline(&decoded)?;
    let abstained = proposals.is_empty();
    if !abstained {
        let no_guide: gennie_assignments::proposals::GuideEvaluator =
            Arc::new(|_, _| Err("La línea base no permite guía".into()));
        cell.store.submit_proposals(
            None,
            "evaluation-review",
            &cell.origin,
            proposals,
            &no_guide,
        )?;
    }
    let review = cell.store.review("evaluation-review")?;
    let input = cell
        .records
        .iter()
        .map(|row| serde_json::to_value(&row.document))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let result = json!({"case_id":case.id,"family":case.family,"strategy":"local_rules","representation":representation,"status":if abstained{"abstained"}else{"completed"},"proposals":review["proposals"],"machine":constraints(case,&cell,&review)?,"input_documents":input,"presented_documents":presented,"cell_elapsed_ms":started.elapsed().as_millis(),"model_turns":0,"guide_calls":0,"usage_reported":null,"cost_reported":null});
    cell.db.close()?;
    Ok(result)
}

fn contract_row(corpus: &Corpus, case: &Case) -> Result<Value> {
    let cell = setup(corpus, case, case.id == "guide_failure")?;
    let original = &cell.records[0];
    let mut changed = original.document.clone();
    changed
        .description
        .push_str(". Agrupar novedades en una entrega");
    let proposal = Candidate {
        assignment_id: original.id,
        title: "Mejora ficticia".into(),
        rationale: "Control de contrato".into(),
        expected_benefit: "Conservar decisiones humanas".into(),
        related_ids: vec![],
        questions: vec![],
        document: Some(changed),
    };
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let guide: gennie_assignments::proposals::GuideEvaluator = Arc::new(move |_, _| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err("Fallo ficticio de guía".into())
    });
    let saved = cell.store.submit_proposals(
        None,
        "evaluation-review",
        &cell.origin,
        vec![proposal.clone()],
        &guide,
    )?;
    let checks = if case.id == "stale_context" {
        let mut human = original.document.clone();
        human.description = "Edición humana que debe conservarse".into();
        let human = cell.store.update(original.id, original.revision, human)?;
        let denied = cell
            .store
            .resolve_proposal(
                saved["proposals"][0]["id"]
                    .as_i64()
                    .ok_or("Propuesta sin ID")?,
                true,
            )
            .is_err();
        json!({"stale_application_rejected":denied,"human_edit_preserved":cell.store.get(original.id)?.document==human.document,"guide_calls":calls.load(std::sync::atomic::Ordering::SeqCst)})
    } else if case.id == "guide_failure" {
        cell.store.submit_proposals(
            None,
            "evaluation-review",
            &cell.origin,
            vec![proposal],
            &guide,
        )?;
        cell.store.recover_reviews()?;
        let after = cell.store.review("evaluation-review")?;
        json!({"draft_preserved":after["proposals"]==saved["proposals"],"failure_distinguished":after["state"]=="guide_failed","no_retry":calls.load(std::sync::atomic::Ordering::SeqCst)==1,"plans_unchanged":cell.store.get(original.id)?.revision==original.revision})
    } else {
        return Err("Control desconocido".into());
    };
    let result = json!({"case_id":case.id,"family":"contract","strategy":"contract_control","status":"completed","checks":checks,"model_turns":0,"remote_guide_calls":0});
    cell.db.close()?;
    Ok(result)
}

#[test]
fn corpus_is_valid_projection_is_lossless_and_unknown_fields_cannot_disappear() {
    let corpus: Corpus = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(corpus.version, 1);
    assert_eq!(corpus.cases.len(), 12);
    let mut names = BTreeSet::new();
    for case in &corpus.cases {
        assert!(names.insert(&case.id));
        assert!(!case.rubric.is_empty());
        assert!(["proposals", "contract"].contains(&case.family.as_str()));
        let cell = setup(&corpus, case, false).unwrap();
        for record in &cell.records {
            let flat = serde_json::to_value(&record.document).unwrap();
            assert_eq!(
                flat,
                ungroup(&project(&flat, Representation::GroupedV1).unwrap()).unwrap()
            );
        }
        cell.db.close().unwrap();
    }
    assert!(
        project(
            &json!({"new_field":"must_not_be_lost"}),
            Representation::GroupedV1
        )
        .is_err()
    );
    assert!(ungroup(&json!({"control":{"title":"wrong_group"}})).is_err());
}

#[test]
fn local_baseline_keeps_abstentions_unknown_quality_and_contract_failures_visible() {
    let corpus: Corpus = serde_json::from_str(CORPUS).unwrap();
    let mut completed = 0;
    let mut abstained = 0;
    for case in &corpus.cases {
        if case.family == "contract" {
            let row = contract_row(&corpus, case).unwrap();
            assert!(
                row["checks"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(|value| value == true || value == 0)
            );
            continue;
        }
        let flat = local_row(&corpus, case, Representation::FlatV1).unwrap();
        let grouped = local_row(&corpus, case, Representation::GroupedV1).unwrap();
        assert_eq!(flat["proposals"], grouped["proposals"]);
        assert_eq!(flat["machine"], grouped["machine"]);
        assert_eq!(flat["machine"]["checks"]["plans_unchanged"], true);
        assert!(flat["machine"]["semantic_quality"].is_null());
        assert!(flat["cost_reported"].is_null());
        if flat["status"] == "completed" {
            completed += 1;
        } else {
            abstained += 1;
            for (key, value) in flat["machine"]["checks"].as_object().unwrap() {
                if key != "plans_unchanged" {
                    assert!(value.is_null());
                }
            }
        }
    }
    assert_eq!((completed, abstained), (1, 9));
}
