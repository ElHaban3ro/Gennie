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
            detail: result
                .filter(|r| r["review_required"] == true || r["status"] == "review_required")
                .map(|_| "result_uncertain".into()),
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
    let mut detail = None;
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
                    Some("input_required" | "invalid_arguments") => outcome = ToolOutcome::Failed,
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
