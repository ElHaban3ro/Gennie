use super::*;
use gennie_mcp::Tool;
pub fn tools(controller: Controller) -> Vec<Tool> {
    let start = controller.clone();
    vec![
        Tool {
            definition: json!({"name":"plugin_build_start","description":"Lanza la creación o actualización autorizada de un plugin web en un Codex técnico aislado. Conserva automáticamente el chat emisor del turno y le envía avances; no uses el chat seleccionado en pantalla. Reutiliza request_key solo al repetir la misma solicitud. Identifica plugin_id y origin observados; objective describe funciones, cobertura y límites delegados sin añadir requisitos no acordados. El constructor automatiza la interfaz oficial: no exijas una API externa ni resultados individuales desagrupados si el usuario no lo pidió; describe fielmente la unidad que muestra la app y aclara diferencias que cambien el resultado. La construcción no tiene un plazo ni un límite de turnos; informa bloqueos reales cuando no haya alternativas pertinentes. No programa revisiones periódicas ni pide login de antemano.","inputSchema":{"type":"object","properties":{"request_key":{"type":"string","minLength":1,"maxLength":200},"plugin_id":{"type":"string","minLength":1,"maxLength":64},"origin":{"type":"string","minLength":1,"maxLength":4000},"objective":{"type":"string","minLength":1,"maxLength":8000},"max_attempts":{"type":"integer","minimum":1,"maximum":8}},"required":["request_key","plugin_id","origin","objective"],"additionalProperties":false}}),
            call: Box::new(move |value| {
                let request: BuildRequest = serde_json::from_value(value)
                    .map_err(|_| "Solicitud de construcción no válida")?;
                Ok(json!({"job":start.submit(request,None)?}))
            }),
        },
        Tool {
            definition: json!({"name":"plugin_build_jobs","description":"Consulta trabajos de plugins y permite cancelar o continuar un trabajo esperando acceso/interrumpido. Los avances siguen en su chat de origen. action=list/get/cancel/resume; salvo list requiere job_id. Nunca afirma verificación por guardar un encargo.","inputSchema":{"type":"object","properties":{"action":{"type":"string","enum":["list","get","cancel","resume"]},"job_id":{"type":"integer","minimum":1}},"required":["action"],"additionalProperties":false}}),
            call: Box::new(move |value| match value["action"].as_str() {
                Some("list") => Ok(json!({"jobs":controller.jobs()?})),
                Some("get") => Ok(
                    json!({"job":controller.0.store.get(value["job_id"].as_i64().ok_or("Falta job_id")?)?}),
                ),
                Some("cancel") => Ok(
                    json!({"job":controller.cancel(value["job_id"].as_i64().ok_or("Falta job_id")?)?}),
                ),
                Some("resume") => Ok(
                    json!({"job":controller.resume(value["job_id"].as_i64().ok_or("Falta job_id")?)?}),
                ),
                _ => Err("Operación de trabajo no válida".into()),
            }),
        },
    ]
}
