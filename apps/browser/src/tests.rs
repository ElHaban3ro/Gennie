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
