use super::*;

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
    assert!(
        controller
            .open()
            .unwrap_err()
            .to_string()
            .contains("ocupado")
    );
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
    assert_eq!(tools.len(), 21);
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
