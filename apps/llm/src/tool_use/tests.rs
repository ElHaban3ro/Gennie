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
