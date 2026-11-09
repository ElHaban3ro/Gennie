//! Evidencia de llamadas MCP del protocolo; nunca se conserva texto de argumentos o resultados.
use gennie_core::Result;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolOutcome {
    Running,
    Completed,
    Failed,
    Unknown,
}
impl ToolOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err("Estado de herramienta desconocido".into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUse {
    pub call_id: String,
    pub server: String,
    pub tool: String,
    pub plugin_id: Option<String>,
    pub function_name: Option<String>,
    pub display_name: Option<String>,
    pub operation: Option<String>,
    pub outcome: ToolOutcome,
    /// Código fijo del adaptador, nunca mensaje libre de un error.
    pub detail: Option<String>,
}

fn text(value: Option<&Value>, max: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|s| {
            !s.trim().is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
        })
        .map(str::to_owned)
}
fn plugin_identifier(value: Option<&Value>) -> Option<String> {
    text(value, 64).filter(|s| {
        s.starts_with(|c: char| c.is_ascii_lowercase())
            && s.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    })
}

// Solo diagnósticos locales conocidos: nunca persistir datos arbitrarios de servidores.
fn local_failure_reason(item: &Value, server: &str, tool: &str) -> Option<String> {
    if server != "gennie" || !tool.starts_with("assignments_") {
        return None;
    }
    let reason = item
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| {
            item.pointer("/result/content/0/text")
                .and_then(Value::as_str)
        })?;
    const SAFE: &[&str] = &[
        "Un encargo recurrente usa recurrence para la hora local; elimina notify_at o usa null. No conviertas esa hora en una fecha UTC",
        "Un encargo recurrente usa recurrence para la hora local; elimina notify_at o usa null.",
        "La zona horaria no contiene reglas IANA válidas",
        "La fecha debe ser válida y UTC: AAAA-MM-DDTHH:MM:SSZ",
        "La hora debe ser HH:mm (00:00–23:59)",
        "La recurrencia semanal necesita días únicos entre 1 y 7; la diaria no admite días",
        "El encargo cambió o no existe. Vuelve a consultarlo antes de guardar",
        "La petición ya existe con otros datos; consulta el encargo antes de editarlo",
        "El título debe ocupar una sola línea",
        "El intervalo de preparación debe estar entre 1 minuto y un año",
        "Explica por qué elegiste ese intervalo de preparación",
        "Demasiados pasos o aplicaciones",
        "El encargo supera el tamaño máximo de 12 KB",
    ];
    SAFE.contains(&reason).then(|| format!("cause:{reason}"))
}

// Las cadenas externas son datos no confiables: solo se conservan categorías fijas.
fn failure_category(item: &Value, status: Option<&str>) -> String {
    let fixed = match status {
        Some("input_required" | "invalid_arguments") => {
            Some("La herramienta rechazó los datos de la solicitud.")
        }
        Some("schema_unavailable") => Some("No estaba disponible el esquema de la herramienta."),
        Some("permission_required") => Some("La herramienta informó que faltan permisos."),
        Some("revision_conflict") => Some("La revisión cambió antes de ejecutar la herramienta."),
        Some("function_not_found") => Some("No se encontró la función solicitada."),
        Some("busy") => Some("La herramienta estaba ocupada."),
        _ => None,
    };
    if let Some(reason) = fixed {
        return format!("cause:{reason}");
    }
    let native_error = (item
        .pointer("/result/structuredContent/content_delivery")
        .and_then(Value::as_str)
        == Some("native"))
    .then(|| item.pointer("/result/content").and_then(Value::as_array))
    .flatten()
    .and_then(|blocks| {
        blocks.iter().skip(1).find_map(|block| {
            (block["type"].as_str() == Some("text"))
                .then(|| block["text"].as_str())
                .flatten()
        })
    });
    let source = native_error
        .or_else(|| item.pointer("/error/message").and_then(Value::as_str))
        .or_else(|| {
            item.pointer("/result/structuredContent/result/content/0/text")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            item.pointer("/result/content/0/text")
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    // El texto solo se examina en memoria y nunca se copia a SQLite, logs o UI.
    let lower: String = source.chars().take(4096).collect::<String>().to_lowercase();
    let reason = if [
        "timed out",
        "timeout",
        "time out",
        "tiempo de espera agotado",
        "se agotó el tiempo",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        "El servidor informó que se agotó el tiempo de espera."
    } else if [
        "unauthorized",
        "authentication required",
        "unauthenticated",
        "authentication failed",
        "authentication_required",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        "El servidor informó un problema de autenticación."
    } else if [
        "permission denied",
        "permission_required",
        "insufficient permissions",
        "forbidden",
        "access denied",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        "El servidor informó que se denegó el acceso o faltan permisos."
    } else if [
        "invalid arguments",
        "invalid parameters",
        "invalid params",
        "invalid_arguments",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        "El servidor informó que los argumentos no son válidos."
    } else if lower.contains("invalid") && lower.contains("format") {
        "El servidor informó que el formato de un dato no es válido."
    } else if ["rate limit", "too many requests", "rate_limit"]
        .iter()
        .any(|v| lower.contains(v))
    {
        "El servidor informó que se alcanzó un límite de solicitudes."
    } else if [
        "connection refused",
        "connection reset",
        "connection closed",
        "broken pipe",
        "connection failed",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        "El servidor informó un fallo de conexión."
    } else {
        "La herramienta informó un fallo; no se pudo determinar su causa."
    };
    format!("cause:{reason}")
}

pub(crate) fn from_notification(
    event: &Value,
    thread: &str,
    turn: &str,
) -> Result<Option<ToolUse>> {
    if event.pointer("/params/threadId").and_then(Value::as_str) != Some(thread)
        || event.pointer("/params/turnId").and_then(Value::as_str) != Some(turn)
        || event.pointer("/params/item/type").and_then(Value::as_str) != Some("mcpToolCall")
    {
        return Ok(None);
    }
    let completed = match event["method"].as_str() {
        Some("item/started") => false,
        Some("item/completed") => true,
        _ => return Ok(None),
    };
    let item = &event["params"]["item"];
    let call_id = text(item.get("id"), 256).ok_or("Falta la identidad de la llamada MCP")?;
    let server = text(item.get("server"), 200).ok_or("Falta el servidor de la llamada MCP")?;
    let tool = text(item.get("tool"), 200).ok_or("Falta la herramienta de la llamada MCP")?;
    let mut outcome = if !completed {
        ToolOutcome::Running
    } else {
        match item["status"].as_str() {
            Some("failed") => ToolOutcome::Failed,
            Some("completed")
                if !item["error"].is_null()
                    || item.pointer("/result/isError") == Some(&Value::Bool(true)) =>
            {
                ToolOutcome::Failed
            }
            Some("completed") => ToolOutcome::Completed,
            _ => ToolOutcome::Unknown,
        }
    };
    let local_plugin = server == "gennie" && tool == "browser_plugin_run";
    if server == "gennie" && tool == "plugin_mcp_run" {
        let id = plugin_identifier(item.pointer("/arguments/server_id"))
            .ok_or("Falta identidad del servidor MCP externo")?;
        let remote_tool = text(item.pointer("/arguments/tool"), 200)
            .ok_or("Falta identidad de herramienta MCP externa")?;
        let result = item.pointer("/result/structuredContent");
        let display_name = result
            .filter(|r| {
                r.pointer("/source/server_id").and_then(Value::as_str) == Some(id.as_str())
                    && r.pointer("/source/tool").and_then(Value::as_str)
                        == Some(remote_tool.as_str())
            })
            .and_then(|r| text(r.pointer("/source/server_name"), 200));
        if completed && outcome == ToolOutcome::Completed {
            outcome = match result.and_then(|r| r["status"].as_str()) {
                Some("ok") => ToolOutcome::Completed,
                Some(
                    "input_required"
                    | "invalid_arguments"
                    | "schema_unavailable"
                    | "permission_required"
                    | "revision_conflict"
                    | "function_not_found"
                    | "tool_failed"
                    | "review_required"
                    | "busy",
                ) => ToolOutcome::Failed,
                _ => ToolOutcome::Unknown,
            };
        }
        return Ok(Some(ToolUse {
            call_id,
            server: format!("mcp:{id}"),
            tool: remote_tool,
            plugin_id: None,
            function_name: None,
            display_name,
            operation: None,
            outcome,
            detail: if result
                .is_some_and(|r| r["review_required"] == true || r["status"] == "review_required")
            {
                if result.is_some_and(|r| r["status"] == "tool_failed") {
                    let reason = failure_category(item, None);
                    Some(format!(
                        "review_cause:{}",
                        reason.trim_start_matches("cause:")
                    ))
                } else {
                    Some("result_uncertain".into())
                }
            } else if outcome == ToolOutcome::Failed {
                Some(failure_category(
                    item,
                    result.and_then(|r| r["status"].as_str()),
                ))
            } else {
                None
            },
        }));
    }
    let plugin_id = local_plugin
        .then(|| plugin_identifier(item.pointer("/arguments/plugin_id")))
        .flatten();
    let function_name = local_plugin
        .then(|| plugin_identifier(item.pointer("/arguments/function_name")))
        .flatten();
    let operation = local_plugin
        .then(|| {
            item.pointer("/arguments/action")
                .and_then(Value::as_str)
                .filter(|v| matches!(*v, "run" | "verify"))
                .map(str::to_owned)
        })
        .flatten();
    let structured = item.pointer("/result/structuredContent");
    let mut display_name = text(item.pointer("/appContext/appName"), 200);
    let mut detail = if outcome == ToolOutcome::Failed {
        local_failure_reason(item, &server, &tool).or_else(|| Some(failure_category(item, None)))
    } else {
        None
    };
    if local_plugin {
        display_name = None;
        if let Some(result) = structured {
            if plugin_id
                .as_deref()
                .is_some_and(|id| result["plugin_id"].as_str() == Some(id))
                && function_name
                    .as_deref()
                    .is_some_and(|name| result["function_name"].as_str() == Some(name))
            {
                display_name = text(result.get("plugin_name"), 200);
            }
            if completed && outcome == ToolOutcome::Completed {
                match result["status"].as_str() {
                    Some("ok") => {}
                    Some(status @ ("input_required" | "invalid_arguments")) => {
                        outcome = ToolOutcome::Failed;
                        detail = Some(failure_category(item, Some(status)));
                    }
                    Some(
                        code @ ("result_uncertain" | "timeout" | "browser_closed"
                        | "response_too_large"),
                    ) => {
                        outcome = ToolOutcome::Unknown;
                        detail = Some(code.to_owned());
                    }
                    Some(
                        code @ ("authentication_required"
                        | "access_denied"
                        | "site_changed"
                        | "redirect_blocked"
                        | "redirect_limit"
                        | "http_error"
                        | "execution_failed"
                        | "permission_required"
                        | "revision_conflict"
                        | "verification_required"
                        | "plugin_not_found"
                        | "function_not_found"),
                    ) => {
                        outcome = ToolOutcome::Failed;
                        detail = Some(code.to_owned());
                    }
                    _ => outcome = ToolOutcome::Unknown,
                }
            }
        } else if completed && outcome == ToolOutcome::Completed {
            outcome = ToolOutcome::Unknown;
        }
    }
    Ok(Some(ToolUse {
        call_id,
        server,
        tool,
        plugin_id,
        function_name,
        display_name,
        operation,
        outcome,
        detail,
    }))
}

#[cfg(test)]
mod tests;
