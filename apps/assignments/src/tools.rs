use crate::{Document, Store};
#[cfg(test)]
use gennie_core::Result;
use gennie_mcp::Tool;
use serde::Deserialize;
use serde_json::{Value, json};

pub fn tools(store: Store) -> Vec<Tool> {
    let document = document_schema();
    let mut tools = Vec::new();
    let create = store.clone();
    tools.push(Tool {
        definition: definition("assignments_create", "Registra un encargo, no lo ejecuta. Para revisiones de aplicaciones necesita los datos del plan y la fuente, no que el usuario copie listas, notas o capturas al chat. Identifica aplicación, ámbito/resultado y si la revisión es puntual o recurrente; incluye applications cuando dependa de una app, aunque su acceso esté pendiente. Antes de guardar concreta el efecto observable y qué debe conservarse usando el contexto existente. Si hay interpretaciones que cambian acciones, destino o conservación, acláralas con una pregunta breve; no pidas diseño técnico ni añadas requisitos no pedidos. Conserva el resultado acordado en description y el ámbito en context/authorization. Separa objetivo (description), método (method) y autorización (authorization). method referencia un plugin y funciones alternativas equivalentes en orden; el constructor aislado determina sus transportes al explorarlas y verificarlas. No inventes vías verificadas ni amplíes permisos ante fallos; MCP es el canal, no un método. Consulta method_resolution para evidencia vigente; no prueba ejecución. Deja el método pendiente si falta integración. Para cada día a una hora usa kind=recurring, recurrence={frequency:daily,local_time:HH:mm,weekdays:[]} y notify_at=null u omitido. No conviertas la hora diaria en fecha UTC ni rellenes notify_at para recurrencias. Si el backend rechaza campos incompatibles, corrige y reintenta la misma request_key. Separa aviso y preparación. Si pide acompañamiento continuo, incluye follow_up con alcance, aspectos abiertos a valorar y criterio contextual de intervención; no impongas importancia ni umbrales fijos. Guardar la política no activa revisiones ni autoriza efectos. Reutiliza contexto y fecha/zona de assignments_list; no preguntes la zona si no hay contradicción. Redacta título/descripción y deduce tipo, prioridad, pasos y política sin pedir que el usuario rellene campos. Pregunta solo por el principal dato indispensable ausente, como un horario obligatorio o una fuente ambigua. No inventes horarios ni autorizaciones; motivo y otros datos opcionales pueden quedar vacíos. Usa una request_key estable para no duplicar al repetir la llamada. Confirma solo tras éxito y resume los datos guardados. Si el resultado incluye plugin_preparation, informa su estado real: started significa que se delegó la creación para reutilizar funciones y reducir el coste futuro; existing o not_started no significan que se inició una construcción. No ocultes bloqueos ni inventes disponibilidad. No añadas advertencias automáticas sobre implementación, ejecución o integración; esos metadatos no son texto de la confirmación. Tampoco afirman que una revisión se haya ejecutado.", json!({"type":"object","properties":{"request_key":{"type":"string","minLength":1,"maxLength":120},"document":document.clone()},"required":["request_key","document"],"additionalProperties":false}),false),
        call: Box::new(move |v| { let a:Create=serde_json::from_value(v)?; Ok(json!({"assignment":create.create(&a.request_key,a.document)?,"execution_enabled":false})) }),
    });
    let get = store.clone();
    tools.push(Tool {
        definition: definition("assignments_get","Consulta todos los datos de un encargo antes de editarlo.",json!({"type":"object","properties":{"id":{"type":"integer","minimum":1}},"required":["id"],"additionalProperties":false}),true),
        call:Box::new(move |v| { let a:Id=serde_json::from_value(v)?; Ok(json!({"assignment":get.get(a.id)?})) }),
    });
    let list = store.clone();
    tools.push(Tool {
        definition: definition("assignments_list","Consulta primero esta herramienta antes de crear: incluye fecha UTC y zona horaria del sistema para resolver expresiones relativas. Lista resúmenes de encargos registrados, incluidos pausados y cancelados. Usa assignments_get para consultar sus detalles. Paginación por offset.",json!({"type":"object","properties":{"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":50}},"additionalProperties":false}),true),
        call: Box::new(move |v| {
            let a:List=serde_json::from_value(v)?;
            if !(1..=50).contains(&a.limit) { return Err("Límite entre 1 y 50".into()); }
            let rows=list.list(a.offset,a.limit)?;
            let next=(rows.len()==a.limit as usize).then(|| a.offset.saturating_add(a.limit));
            let rows:Vec<_>=rows.into_iter().map(|r| json!({"id":r.id,"revision":r.revision,"title":r.document.title,"kind":r.document.kind,"status":r.document.status,"priority":r.document.priority,"updated_at":r.updated_at})).collect();
            Ok(json!({"assignments":rows,"next_offset":next,"execution_enabled":false,"clock":list.clock_context()?}))
        }),
    });
    tools.push(Tool {
        definition: definition("assignments_update","Sustituye el plan completo de un encargo después de consultarlo. expected_revision evita sobrescribir cambios concurrentes. También permite pausar, completar o cancelar el registro; no hay ejecución.",json!({"type":"object","properties":{"id":{"type":"integer","minimum":1},"expected_revision":{"type":"integer","minimum":1},"document":document},"required":["id","expected_revision","document"],"additionalProperties":false}),false),
        call:Box::new(move |v| { let a:Update=serde_json::from_value(v)?; Ok(json!({"assignment":store.update(a.id,a.expected_revision,a.document)?,"execution_enabled":false})) }),
    });
    tools
}
fn definition(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    request_key: String,
    document: Document,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    id: i64,
    expected_revision: u32,
    document: Document,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    id: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    #[serde(default)]
    offset: u32,
    #[serde(default = "limit")]
    limit: u32,
}
fn limit() -> u32 {
    20
}
fn document_schema() -> Value {
    let optional_text = json!({"type":["string","null"]});
    json!({"type":"object","additionalProperties":false,"required":["title","description","kind","priority","status","timezone"],"properties":{
        "title":{"type":"string","minLength":1,"maxLength":200},
        "description":{"type":"string","description":"Qué pidió el usuario; conserva el alcance y resultado esperado."},
        "context":{"type":["string","null"],"description":"Ámbito de la fuente: nombre de lista/carpeta/cuenta/sección cuando sea necesario, sin copiar su contenido al chat. Conserva las suposiciones identificadas y el acceso pendiente."},"reason":{"type":["string","null"],"description":"Para qué lo quiere. Pregunta solo si resulta útil y no puedes inferirlo; null si se desconoce. Identifica en el contexto cualquier motivo inferido."},
        "kind":{"enum":["one_time","recurring","goal"],"description":"Cada día o semana usa recurring; entonces notify_at debe ser null u omitirse y recurrence contiene la hora local."},"priority":{"enum":["low","normal","high","critical"]},
        "status":{"enum":["active","paused","completed","cancelled"]},
        "timezone":{"type":"string","description":"Zona IANA confirmada o inferida del contexto, por ejemplo America/Bogota. No inventar."},
        "notify_at":{"type":["string","null"],"description":"Instante UTC AAAA-MM-DDTHH:MM:SSZ para puntual; fecha límite opcional de un objetivo. Para recurring DEBE ser null u omitirse; nunca pongas aquí la hora diaria ni una próxima fecha calculada."},
        "recurrence":{"type":["object","null"],"additionalProperties":false,"required":["frequency","local_time"],"properties":{"frequency":{"enum":["daily","weekly"]},"local_time":{"type":"string","description":"HH:mm del aviso al usuario, no de la preparación"},"weekdays":{"type":"array","items":{"type":"integer","minimum":1,"maximum":7},"description":"Lunes=1; obligatorio en semanal, vacío en diaria"}}},
        "preparation":{"type":"object","additionalProperties":false,"properties":{"interval_minutes":{"type":["integer","null"],"minimum":1,"maximum":525600,"description":"Cadencia interna sugerida por el agente según importancia; NO genera avisos"},"rationale":optional_text,"steps":{"type":"array","maxItems":30,"items":{"type":"string"}}}},
        "follow_up":{"type":["object","null"],"additionalProperties":false,"required":["scope","decision_guidance"],"description":"Política de seguimiento aceptada por el usuario; no ejecuta revisiones. Omitir si solo pide una entrega puntual/recurrente sin acompañamiento. La cadencia está en preparation.interval_minutes y necesita rationale; la hora de entrega permanece en recurrence/notify_at.","properties":{
            "scope":{"type":"string","minLength":1,"maxLength":2000,"description":"Qué observar y qué cambios conservar en el futuro historial, aunque no se avise: completadas, reabiertas u otros cambios propios de esta necesidad."},
            "signals":{"type":"array","maxItems":24,"items":{"type":"string","minLength":1,"maxLength":500},"description":"Aspectos o preguntas pertinentes que propone el agente para este encargo. Son libres, sin catálogo obligatorio de importancia/dificultad ni umbrales predeterminados. Vacío si basta con la orientación general; no inventar valoraciones de tareas que no se han leído."},
            "decision_guidance":{"type":"string","minLength":1,"maxLength":4000,"description":"Cómo decidir en cada revisión si registrar sin interrumpir, avisar, preguntar, proponer ayuda o considerar una acción delegada, usando cambios, contexto, relevancia e historial. No equivale a una notificación por cada revisión. El agente propone una orientación breve adecuada al encargo: agrupar asuntos relacionados, evitar repetir la misma pregunta mientras se espera respuesta y respetar aplazamientos. Sin reglas ni plazos universales."},
            "action_scope":{"type":["string","null"],"minLength":1,"maxLength":2000,"description":"Solo acciones delegadas expresamente por el usuario y sus límites. null si no hay delegación de ejecución; no inventar autorización ni reemplazar permisos del sistema. El texto no ejecuta acciones."}
        }},
        "method":{"type":["object","null"],"additionalProperties":false,"required":["plugin_id","functions"],"description":"Referencia al plugin; las funciones son alternativas equivalentes para el objetivo en orden de preferencia. El constructor descubre HTTP/web; no guardar resultados ni transportes inventados. Ausente significa método pendiente.","properties":{"plugin_id":{"type":"string"},"functions":{"type":"array","minItems":1,"maxItems":20,"uniqueItems":true,"items":{"type":"string"}}}},
        "authorization":{"type":["object","null"],"additionalProperties":false,"required":["scope","allowed_routes"],"description":"Solo alcance y vías aceptados por el usuario; no inferir permisos nuevos por un fallo. Ausente no concede autorización.","properties":{"scope":{"type":"string","minLength":1,"maxLength":2000},"allowed_routes":{"type":"array","minItems":1,"maxItems":3,"uniqueItems":true,"items":{"enum":["http","web","computer_use"]}}}},
        "applications":{"type":"array","description":"Aplicaciones de las que depende el trabajo. Si el usuario pide revisar una app, identifica y registra esa fuente; no requiere leerla, conectarla ni pedir capturas para guardar el encargo. Vacío para planes que no dependan de una aplicación.","maxItems":20,"items":{"type":"object","additionalProperties":false,"required":["name","kind"],"properties":{"name":{"type":"string","description":"Nombre de la aplicación o servicio confirmado o inequívoco en la petición. No pedir al usuario datos técnicos que no hagan falta para identificarlo."},"kind":{"enum":["desktop","web"]},"url":optional_text,"plugin_available":{"type":"boolean","description":"Indicador histórico normalizado a false; consultar method_resolution para evidencia vigente"},"manual_required":{"type":"boolean","description":"Indicador histórico normalizado a true; no implica acceso autorizado ni ejecución"}}}}
    }})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_request_reports_the_conflicting_field_and_can_be_retried() -> Result<()> {
        use gennie_core::AppModule;
        let db = gennie_db::Database::memory()?;
        db.migrate(
            crate::AssignmentsModule.migration_namespace(),
            crate::AssignmentsModule.migrations(),
        )?;
        let store = Store::new(db);
        let mut registry = gennie_mcp::Registry::default();
        registry.register(tools(store.clone()))?;
        let mut document = json!({"title":"Revisar Gmail","description":"Resumir y archivar según autorización", "kind":"recurring", "priority":"normal", "status":"active", "timezone":"America/Bogota", "notify_at":"09:00", "recurrence":{"frequency":"daily","local_time":"09:00"}});
        for bad_date in ["09:00", "2026-10-27T14:00:00Z"] {
            document["notify_at"] = json!(bad_date);
            let result=registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"assignments_create","arguments":{"request_key":"daily-mail","document":document}}})).unwrap();
            assert_eq!(result["result"]["isError"], true);
            let error = result["result"]["content"][0]["text"].as_str().unwrap();
            assert!(
                error.contains("notify_at") && error.contains("recurrence"),
                "error no orienta la corrección: {error}"
            );
            assert!(store.list(0, 20)?.is_empty());
        }
        document["notify_at"] = Value::Null;
        let result=registry.respond(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"assignments_create","arguments":{"request_key":"daily-mail","document":document}}})).unwrap();
        assert_eq!(result["result"]["isError"], false);
        let row = store.list(0, 20)?.pop().unwrap();
        assert_eq!(row.document.recurrence.unwrap().local_time, "09:00");
        assert!(row.document.notify_at.is_none());
        Ok(())
    }
    #[test]
    fn schema_and_dispatch_share_real_store() -> Result<()> {
        use gennie_core::AppModule;
        let db = gennie_db::Database::memory()?;
        db.migrate(
            crate::AssignmentsModule.migration_namespace(),
            crate::AssignmentsModule.migrations(),
        )?;
        let store = Store::new(db);
        let mut registry = gennie_mcp::Registry::default();
        registry.register(tools(store))?;
        let schema = registry
            .respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .unwrap();
        assert_eq!(schema["result"]["tools"].as_array().unwrap().len(), 4);
        let invalid=registry.respond(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"assignments_create","arguments":{}}})).unwrap();
        assert_eq!(invalid["result"]["isError"], true);
        let document = json!({"title":"Acompañar tareas", "description":"Resumen de mañana y seguimiento contextual", "kind":"recurring", "priority":"normal", "status":"active", "timezone":"America/Bogota", "recurrence":{"frequency":"daily","local_time":"07:30"}, "preparation":{"interval_minutes":10,"rationale":"Detectar cambios entre entregas"}, "follow_up":{"scope":"Tareas abiertas, cerradas y reabiertas", "signals":["¿Hay un bloqueo que podamos resolver?"], "decision_guidance":"Valorar contexto e historial; registrar sin interrumpir si no aporta ayuda", "action_scope":null}});
        let created = registry.respond(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"assignments_create","arguments":{"request_key":"policy-mcp","document":document}}})).unwrap();
        let result: Value =
            serde_json::from_str(created["result"]["content"][0]["text"].as_str().unwrap())?;
        assert_eq!(result["execution_enabled"], false);
        let id = result["assignment"]["id"].as_i64().unwrap();
        let fetched = registry.respond(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"assignments_get","arguments":{"id":id}}})).unwrap();
        let fetched: Value =
            serde_json::from_str(fetched["result"]["content"][0]["text"].as_str().unwrap())?;
        assert_eq!(
            fetched["assignment"]["document"]["follow_up"],
            document["follow_up"]
        );
        Ok(())
    }
}
