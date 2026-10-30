//! Condiciones publicadas del lote; Store conserva la validación previa al commit.
use serde_json::{Value, json};

pub(super) fn submission() -> Value {
    json!({
        "type":"object", "additionalProperties":false,
        "required":["review_id","proposals"],
        "properties":{
            "review_id":{"type":"string"},
            "proposals":{"type":"array","minItems":1,"maxItems":6,"items":{"anyOf":[
                candidate(crate::tools::document_schema(), 0),
                candidate(json!({"type":"null"}), 1)
            ]}}
        }
    })
}

fn candidate(document: Value, minimum_questions: usize) -> Value {
    // Cada alternativa publica el objeto entero: los consumidores no necesitan
    // combinar propiedades/requeridos de un padre con condiciones parciales.
    json!({
        "type":"object", "additionalProperties":false,
        "required":["assignment_id","title","rationale","expected_benefit","related_ids","questions","document"],
        "properties":{
            "assignment_id":{"type":"integer","minimum":1},
            "title":{"type":"string","minLength":1,"maxLength":200,"pattern":"\\S"},
            "rationale":{"type":"string","minLength":1,"maxLength":2000,"pattern":"\\S"},
            "expected_benefit":{"type":"string","minLength":1,"maxLength":2000,"pattern":"\\S"},
            "related_ids":{"type":"array","maxItems":20,"uniqueItems":true,"items":{"type":"integer","minimum":1}},
            "questions":{"type":"array","minItems":minimum_questions,"maxItems":8,"items":{"type":"string","minLength":1,"maxLength":500,"pattern":"\\S"}},
            "document":document
        }
    })
}
