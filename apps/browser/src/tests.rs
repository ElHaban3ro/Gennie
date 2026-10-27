use super::*;

#[test]
fn screenshot_output_transports_pixels_as_an_mcp_image() {
    let mut registry = gennie_mcp::Registry::default();
    registry.register([gennie_mcp::Tool {
        definition:json!({"name":"browser_screenshot"}),
        call:Box::new(|_| tool_output("screenshot", json!({"page_id":"task-page","artifact":"fixture.jpg","image":{"data":"aA==","mime_type":"image/jpeg"}})))
    }]).unwrap();
    let result = registry.respond(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"browser_screenshot","arguments":{}}})).unwrap();
    assert_eq!(result["result"]["content"][1]["type"], "image");
    assert_eq!(result["result"]["content"][1]["mimeType"], "image/jpeg");
    assert_eq!(result["result"]["content"][1]["data"], "aA==");
    assert_eq!(
        result["result"]["structuredContent"]["page_id"],
        "task-page"
    );
    assert!(result["result"]["structuredContent"]["image"].is_null());
    assert!(
        !result["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("aA==")
    );
    assert!(tool_output("screenshot", json!({"page_id":"task-page"})).is_err());
}

#[test]
fn disabled_and_busy_calls_never_start_a_process() {
    let controller = Controller::new("/missing/gennie-browser-test".into());
    assert!(
        controller
            .automate(json!({"method":"open"}))
            .unwrap_err()
            .to_string()
            .contains("control de aplicaciones")
    );
    assert!(
        controller
            .open()
            .unwrap_err()
            .to_string()
            .contains("desactivado")
    );
    assert!(controller.inner.session.lock().unwrap().is_none());
    controller.inner.enabled.store(true, Ordering::Release);
    let guard = controller.inner.session.lock().unwrap();
    assert!(controller.open().unwrap_err().is::<BusyError>());
    drop(guard);
    assert!(
        controller
            .open()
            .unwrap_err()
            .to_string()
            .contains("setup_browser.sh")
    );
    assert!(controller.inner.session.lock().unwrap().is_none());
    controller.stop().unwrap();
    controller.stop().unwrap();
    assert!(!controller.inner.enabled.load(Ordering::Acquire));
}

#[test]
fn mcp_rejects_invalid_arguments_before_touching_the_browser() {
    let controller = Controller::new("/missing/gennie-browser-test".into());
    for tool in tools(controller.clone()) {
        assert!(tool.definition["inputSchema"]["additionalProperties"] == false);
        assert!((tool.call)(json!({"unknown":"ignored?"})).is_err());
        assert!((tool.call)(json!([])).is_err());
    }
    assert!(controller.inner.session.lock().unwrap().is_none());
}

#[test]
fn expanded_inventory_validates_structured_arguments_without_side_effects() {
    let controller = Controller::new("/missing/gennie-browser-test".into());
    let tools = tools(controller.clone());
    assert_eq!(tools.len(), 24);
    for (name, invalid) in [
        ("browser_wait_for", json!({"kind":"element"})),
        (
            "browser_wait_for",
            json!({"kind":"text","text":"ready","timeout_ms":9000}),
        ),
        ("browser_check", json!({"selector":"input","checked":"yes"})),
        (
            "browser_select_option",
            json!({"selector":"select","values":[]}),
        ),
        ("browser_tabs", json!({"action":"close"})),
        ("browser_download", json!({"action":"cancel"})),
        (
            "browser_screenshot",
            json!({"target":"p1s1e1","full_page":true}),
        ),
        (
            "browser_request",
            json!({"url":"https://example.com","http_method":"POST"}),
        ),
        ("browser_upload", json!({"files":[false]})),
        ("browser_pdf", json!({"format":"unbounded"})),
        ("browser_read", json!({"mode":"javascript"})),
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.definition["name"] == name)
            .unwrap();
        let error = (tool.call)(invalid).unwrap_err().to_string();
        assert!(
            !error.contains("desactivado"),
            "La validación debe ocurrir antes del acceso: {name}"
        );
    }
    controller.inner.enabled.store(true, Ordering::Release);
    controller.set_automation_allowed(true);
    assert!(
        controller
            .automate(json!({"method":"upload","files":["/tmp/explicit-upload.txt"]}))
            .unwrap_err()
            .to_string()
            .contains("lectura de archivos")
    );
    assert!(controller.inner.session.lock().unwrap().is_none());
}

#[test]
fn plugin_operations_share_permissions_and_validate_before_start() {
    let controller = Controller::new("/missing/gennie-browser-test".into());
    let inventory = tools(controller.clone());
    for (name, arguments) in [
        (
            "browser_plugins",
            json!({"action":"save","expected_revision":0}),
        ),
        ("browser_plugins", json!({"action":"get"})),
        ("browser_network", json!({"action":"start"})),
        (
            "browser_plugin_run",
            json!({"action":"run","plugin_id":"demo","function_name":"read","expected_revision":1,"arguments":[]}),
        ),
    ] {
        let tool = inventory
            .iter()
            .find(|tool| tool.definition["name"] == name)
            .unwrap();
        assert!(
            !(tool.call)(arguments)
                .unwrap_err()
                .to_string()
                .contains("desactivado")
        );
    }
    assert!(
        controller
            .run_web_plugin("demo", "read", 1, json!({}))
            .unwrap_err()
            .to_string()
            .contains("control de aplicaciones")
    );
    assert!(controller.inner.session.lock().unwrap().is_none());
}

#[test]
fn plugin_schema_accepts_the_builder_manifest_without_browser_access() {
    let schema = crate::tools::plugin_schema();
    crate::validate_arguments(&schema,&json!({"id":"fixture","name":"Prueba local","origin":"http://127.0.0.1:1234","description":"Consulta local de prueba","capabilities":["Listar elementos"],"limitations":["Solo lectura local"],"functions":[{"name":"list_items","description":"Consultar elementos","parameters":{},"transport":"http","path":"/items","result_type":"array","result_pointer":"/items","fields":{"id":"/id","subject":"/subject"}}]})).unwrap();
}

#[test]
fn plugin_mcp_schema_accepts_program_blocks() {
    let schema = crate::tools::plugin_schema();
    crate::validate_arguments(
        &schema,
        &json!({
            "id":"fixture", "name":"Prueba", "origin":"https://example.com",
            "functions":[{"name":"write", "description":"Acción", "parameters":{"id":"string"},
                "transport":"program", "path":"/", "result_type":"object",
                "code":"return {ok: true, id: args.id};"}]
        }),
    )
    .unwrap();
}

#[test]
fn plugin_mcp_accepts_many_functions_and_parameters() {
    let parameters = (0..25)
        .map(|i| (format!("p{i}"), json!("string")))
        .collect::<serde_json::Map<_, _>>();
    let functions = (0..25).map(|i| json!({"name":format!("f{i}"),"description":"Prueba","parameters":parameters,"transport":"program","path":"/","result_type":"object","code":"return args;"})).collect::<Vec<_>>();
    let schema = crate::tools::plugin_schema();
    crate::validate_arguments(
        &schema,
        &json!({"id":"many","name":"Prueba","origin":"https://example.com","functions":functions}),
    )
    .unwrap();
}
