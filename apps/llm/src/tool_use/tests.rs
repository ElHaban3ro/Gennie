use super::*;
use serde_json::json;
fn event(method: &str) -> Value {
    json!({"method":method,"params":{"threadId":"active","turnId":"turn","item":{
        "id":"call-1","type":"mcpToolCall","server":"gennie","tool":"browser_plugin_run","status":"inProgress",
        "arguments":{"action":"run","plugin_id":"mail","function_name":"list","arguments":{"token":"PRIVATE_ARGUMENT","recipient":"PRIVATE_RECIPIENT"}}
    }}})
}
#[test]
fn reads_only_correlated_mcp_lifecycle_and_does_not_copy_payloads() {
    let mut value = event("item/started");
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(usage.outcome, ToolOutcome::Running);
    assert_eq!(usage.plugin_id.as_deref(), Some("mail"));
    assert_eq!(usage.function_name.as_deref(), Some("list"));
    assert!(!format!("{usage:?}").contains("PRIVATE"));
    assert!(
        from_notification(&value, "foreign", "turn")
            .unwrap()
            .is_none()
    );
    assert!(
        from_notification(&value, "active", "other")
            .unwrap()
            .is_none()
    );
    value["params"]["item"]["type"] = json!("agentMessage");
    assert!(
        from_notification(&value, "active", "turn")
            .unwrap()
            .is_none()
    );
}
#[test]
fn plugin_identity_comes_from_matching_metadata_and_domain_failure_is_not_success() {
    let mut value = event("item/completed");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["result"] = json!({"structuredContent":{"status":"ok","plugin_id":"mail","function_name":"list","plugin_name":"Correo","data":"PRIVATE_RESULT"},"content":[{"text":"PRIVATE_RESULT"}]});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(usage.outcome, ToolOutcome::Completed);
    assert_eq!(usage.display_name.as_deref(), Some("Correo"));
    assert!(!format!("{usage:?}").contains("PRIVATE"));
    for code in [
        "authentication_required",
        "permission_required",
        "site_changed",
        "access_denied",
        "revision_conflict",
        "input_required",
        "invalid_arguments",
    ] {
        value["params"]["item"]["result"]["structuredContent"]["status"] = json!(code);
        assert_eq!(
            from_notification(&value, "active", "turn")
                .unwrap()
                .unwrap()
                .outcome,
            ToolOutcome::Failed
        );
    }
    for code in [
        "result_uncertain",
        "timeout",
        "response_too_large",
        "future_status",
    ] {
        value["params"]["item"]["result"]["structuredContent"]["status"] = json!(code);
        assert_eq!(
            from_notification(&value, "active", "turn")
                .unwrap()
                .unwrap()
                .outcome,
            ToolOutcome::Unknown
        );
    }
    value["params"]["item"]["result"]["structuredContent"]["plugin_id"] = json!("other");
    assert!(
        from_notification(&value, "active", "turn")
            .unwrap()
            .unwrap()
            .display_name
            .is_none()
    );
}
#[test]
fn mcp_errors_are_structural_and_invalid_metadata_does_not_leak_error_messages() {
    let mut value = event("item/completed");
    value["params"]["item"]["server"] = json!("calendar");
    value["params"]["item"]["tool"] = json!("list_events");
    value["params"]["item"]["status"] = json!("failed");
    value["params"]["item"]["error"] = json!({"message":"PRIVATE_ERROR"});
    value["params"]["item"]["appContext"] = json!({"appName":"Calendario","linkId":"PRIVATE_LINK"});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(usage.outcome, ToolOutcome::Failed);
    assert_eq!(usage.display_name.as_deref(), Some("Calendario"));
    assert!(usage.plugin_id.is_none());
    assert!(!format!("{usage:?}").contains("PRIVATE"));
    value["params"]["item"]["id"] = json!("\nPRIVATE_ID");
    assert!(from_notification(&value, "active", "turn").is_err());
}

#[test]
fn external_mcp_proxy_keeps_actual_server_tool_and_only_matching_display_metadata() {
    let mut value = event("item/completed");
    value["params"]["item"]["tool"] = json!("plugin_mcp_run");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["arguments"] = json!({"server_id":"calendar","tool":"events/list","arguments":{"token":"PRIVATE_ARGUMENT"}});
    value["params"]["item"]["result"] = json!({"structuredContent":{"status":"ok","result":{"content":[{"text":"PRIVATE_RESULT"}]},"source":{"server_id":"calendar","tool":"events/list","server_name":"Calendario"}}});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(usage.server, "mcp:calendar");
    assert_eq!(usage.tool, "events/list");
    assert_eq!(usage.display_name.as_deref(), Some("Calendario"));
    assert!(!format!("{usage:?}").contains("PRIVATE"));
    for (status, outcome) in [
        ("tool_failed", ToolOutcome::Failed),
        ("busy", ToolOutcome::Failed),
        ("review_required", ToolOutcome::Failed),
        ("invalid_arguments", ToolOutcome::Failed),
        ("result_uncertain", ToolOutcome::Unknown),
    ] {
        value["params"]["item"]["result"]["structuredContent"]["status"] = json!(status);
        assert_eq!(
            from_notification(&value, "active", "turn")
                .unwrap()
                .unwrap()
                .outcome,
            outcome
        );
    }
    value["params"]["item"]["result"]["structuredContent"]["review_required"] = json!(true);
    assert_eq!(
        from_notification(&value, "active", "turn")
            .unwrap()
            .unwrap()
            .detail
            .as_deref(),
        Some("result_uncertain")
    );
    value["params"]["item"]["result"]["structuredContent"]["source"]["server_id"] =
        json!("foreign");
    assert!(
        from_notification(&value, "active", "turn")
            .unwrap()
            .unwrap()
            .display_name
            .is_none()
    );
}

#[test]
fn failed_assignment_keeps_the_reported_reason_without_arguments() {
    let mut value = event("item/completed");
    value["params"]["item"]["tool"] = json!("assignments_create");
    value["params"]["item"]["status"] = json!("failed");
    value["params"]["item"]["error"] = json!({"message":"Un encargo recurrente usa recurrence para la hora local; elimina notify_at o usa null."});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert!(usage.detail.unwrap().contains("recurrence"));
}

#[test]
fn assignment_validation_failures_keep_safe_specific_causes() {
    for (error, expected) in [
        (
            "unknown field `type`, expected one of `name`, `kind`, `url`, `plugin_available`, `manual_required`",
            "campo type",
        ),
        ("missing field `timezone`", "zona horaria"),
        ("Selecciona vías autorizadas únicas", "vías autorizadas"),
    ] {
        let mut value = event("item/completed");
        value["params"]["item"]["tool"] = json!("assignments_create");
        value["params"]["item"]["status"] = json!("failed");
        value["params"]["item"]["result"] =
            json!({"isError":true,"content":[{"type":"text","text":error}]});
        let usage = from_notification(&value, "active", "turn")
            .unwrap()
            .unwrap();
        assert!(usage.detail.unwrap().contains(expected), "{error}");
    }
}

#[test]
fn external_errors_keep_a_safe_diagnostic_without_sensitive_values() {
    let mut value = event("item/completed");
    value["params"]["item"]["server"] = json!("calendar");
    value["params"]["item"]["tool"] = json!("events");
    value["params"]["item"]["status"] = json!("failed");
    value["params"]["item"]["error"] =
        json!({"message":"Connection timed out; token=PRIVATE_SECRET; email=private@example.com"});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.detail.as_deref(),
        Some("cause:El servidor informó que se agotó el tiempo de espera.")
    );
    assert!(!format!("{usage:?}").contains("PRIVATE_SECRET"));
}

#[test]
fn external_proxy_preserves_structured_failure_category_without_remote_content() {
    let mut value = event("item/completed");
    value["params"]["item"]["tool"] = json!("plugin_mcp_run");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["arguments"] = json!({"server_id":"calendar","tool":"events/list"});
    value["params"]["item"]["result"] = json!({"structuredContent":{"status":"permission_required","result":{"content":[{"type":"text","text":"PRIVATE_SECRET"}]}}});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.detail.as_deref(),
        Some("cause:La herramienta informó que faltan permisos.")
    );
    assert!(!format!("{usage:?}").contains("PRIVATE_SECRET"));
}

#[test]
fn external_mcp_error_content_never_copies_sensitive_or_unknown_text() {
    let mut value = event("item/completed");
    value["params"]["item"]["server"] = json!("calendar");
    value["params"]["item"]["tool"] = json!("lookup");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["result"] = json!({"isError":true,"content":[{"type":"text","text":"Unknown failure bearer SECRET_VALUE https://example.com/secret someone@example.com"}]});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(usage.outcome, ToolOutcome::Failed);
    assert_eq!(
        usage.detail.as_deref(),
        Some("cause:La herramienta informó un fallo; no se pudo determinar su causa.")
    );
    let diagnostic = format!("{usage:?}");
    for secret in ["SECRET_VALUE", "example.com", "someone@"] {
        assert!(!diagnostic.contains(secret));
    }
}

#[test]
fn deepwiki_invalid_repository_format_is_categorized() {
    let mut event = event("item/completed");
    event["params"]["item"]["server"] = json!("deepwiki");
    event["params"]["item"]["tool"] = json!("read_wiki_structure");
    event["params"]["item"]["status"] = json!("completed");
    event["params"]["item"]["result"] = json!({"isError":true,"content":[{"type":"text","text":"Invalid repoName format: \"\". Expected format is \"owner/repo\"."}]});
    let result = from_notification(&event, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        result.detail.as_deref(),
        Some("cause:El servidor informó que el formato de un dato no es válido.")
    );
}

#[test]
#[ignore = "Requiere una respuesta MCP real guardada temporalmente"]
fn observed_external_error_is_classified_without_echoed_sensitive_marker() {
    let path = std::env::var("GENNIE_REAL_MCP_RESULT").unwrap();
    let result: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(result["isError"], true);
    assert!(
        result
            .to_string()
            .contains("QA_TOKEN_SYNTHETIC_GENNIE_8721")
    );
    let mut event = event("item/completed");
    event["params"]["item"]["server"] = json!("deepwiki");
    event["params"]["item"]["tool"] = json!("read_wiki_structure");
    event["params"]["item"]["status"] = json!("completed");
    event["params"]["item"]["result"] = result;
    let usage = from_notification(&event, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.detail.as_deref(),
        Some("cause:El servidor informó que el formato de un dato no es válido.")
    );
    assert!(!format!("{usage:?}").contains("QA_TOKEN_SYNTHETIC_GENNIE_8721"));
    if let Ok(path) = std::env::var("GENNIE_SAFE_CAUSE_OUTPUT") {
        std::fs::write(path, usage.detail.as_deref().unwrap()).unwrap();
    }
}

#[test]
fn real_proxy_failure_keeps_reason_and_review_without_remote_data() {
    let mut value = event("item/completed");
    value["params"]["item"]["tool"] = json!("plugin_mcp_run");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["arguments"] =
        json!({"server_id":"deepwiki","tool":"read_wiki_structure"});
    value["params"]["item"]["result"] = json!({"isError":true,"structuredContent":{"status":"tool_failed","review_required":true,"result":{"content":[{"type":"text","text":"Invalid repoName format: QA_TOKEN_SYNTHETIC_GENNIE_8721"}],"isError":true}}});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.detail.as_deref(),
        Some("review_cause:El servidor informó que el formato de un dato no es válido.")
    );
    assert!(!format!("{usage:?}").contains("QA_TOKEN_SYNTHETIC_GENNIE_8721"));
}

#[test]
fn native_mcp_error_block_keeps_reason_with_review() {
    let mut value = event("item/completed");
    value["params"]["item"]["tool"] = json!("plugin_mcp_run");
    value["params"]["item"]["status"] = json!("completed");
    value["params"]["item"]["arguments"] =
        json!({"server_id":"deepwiki","tool":"read_wiki_structure"});
    value["params"]["item"]["result"] = json!({"isError":true,"structuredContent":{"status":"tool_failed","review_required":true,"content_delivery":"native","result":{"isError":true}},"content":[{"type":"text","text":"{\\\"status\\\":\\\"tool_failed\\\"}"},{"type":"text","text":"Invalid repoName format: QA_TOKEN_SYNTHETIC_GENNIE_8721"}]});
    let usage = from_notification(&value, "active", "turn")
        .unwrap()
        .unwrap();
    assert_eq!(
        usage.detail.as_deref(),
        Some("review_cause:El servidor informó que el formato de un dato no es válido.")
    );
    assert!(!format!("{usage:?}").contains("QA_TOKEN_SYNTHETIC_GENNIE_8721"));
}
