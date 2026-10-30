//! Finalización del informe de la sonda; sin texto libre del proveedor.
use gennie_core::Result;
use serde_json::{Value, json};

pub(super) fn finish(
    report: &mut Value,
    result: &Result<()>,
    elapsed_ms: u128,
    profile_removed: bool,
) {
    report["status"] = json!(if result.is_ok() && profile_removed {
        "completed"
    } else {
        "failed"
    });
    report["failure"] = match result {
        Err(error) => diagnostic(error.as_ref()),
        Ok(()) if !profile_removed => json!({"code":"fixture_cleanup_failed"}),
        Ok(()) => Value::Null,
    };
    report["elapsed_ms"] = json!(elapsed_ms);
    report["profile_removed"] = json!(profile_removed);
}

fn diagnostic(error: &(dyn std::error::Error + 'static)) -> Value {
    if error.is::<super::super::RetryableProviderError>() {
        return json!({"code":"provider_transport_interrupted"});
    }
    if error.is::<gennie_mcp::Cancelled>() {
        return json!({"code":"cancelled"});
    }
    if let Some(error) = error.downcast_ref::<std::io::Error>() {
        return json!({"code":"io_failure","kind":format!("{:?}", error.kind())});
    }
    if error.is::<serde_json::Error>() {
        return json!({"code":"malformed_json"});
    }
    // Solo clasificar textos públicos propios. Nunca persistir Display/Debug libre,
    // que puede contener datos de la cuenta, rutas, argumentos o credenciales.
    let message = error.to_string();
    if let Some(code) = message
        .strip_prefix("El proveedor rechazó la operación (código ")
        .and_then(|value| value.strip_suffix(')'))
        .and_then(|value| value.parse::<i64>().ok())
    {
        return json!({"code":"provider_rpc_rejected","rpc_code":code});
    }
    let code = match message.as_str() {
        "El modelo elegido no está disponible en el catálogo de Codex" => "model_unavailable",
        "El modelo elegido no admite el esfuerzo configurado" => "model_effort_unavailable",
        "El proveedor no devolvió modelos" => "model_catalog_missing",
        "Codex no está enlazado con ChatGPT. Ejecuta `codex login` y pulsa Conectar" => {
            "chatgpt_session_unavailable"
        }
        "El proveedor no completó la respuesta" => "provider_turn_not_completed",
        "El proveedor terminó sin texto de respuesta" => "provider_no_final_text",
        "El proveedor solicitó una acción no disponible"
        | "El proveedor solicitó una acción no disponible para la conversación por voz" => {
            "unavailable_native_action"
        }
        "El proveedor entrelazó mensajes de respuesta" => "interleaved_response_items",
        "El proveedor finalizó un ítem distinto del mensaje en curso" => {
            "mismatched_response_item"
        }
        "La respuesta excedió el límite" => "reply_size_limit",
        "El diario no admite evidencia de herramientas" => "tool_journal_unsupported",
        "Contrato MCP descubierto distinto" => "mcp_schema_mismatch",
        "Actuó antes de aclarar el destinatario" => "premature_execution",
        "Conducta no verificada; revisar intentos y respuesta, sin repetir" => {
            "behaviour_not_verified"
        }
        "Cancelación no comprobada; no repetir muestra" => "local_stop_not_verified",
        "Socket propio no retirado" => "fixture_cleanup_failed",
        "Sesiones independientes no comprobadas" => "fixture_thread_isolation_failed",
        _ => "unclassified_error",
    };
    json!({"code":code})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Value {
        json!({"mode":"live", "stage":"ambiguous_recipient:first_turn", "model_turns":1, "guide_calls":0})
    }

    #[test]
    fn probe_failure_report_preserves_typed_transport_cause_and_counters() {
        let mut r = report();
        let error: Result<()> = Err(Box::new(super::super::super::RetryableProviderError));
        finish(&mut r, &error, 12, true);
        assert_eq!(r["status"], "failed");
        assert_eq!(r["failure"]["code"], "provider_transport_interrupted");
        assert_eq!(r["stage"], "ambiguous_recipient:first_turn");
        assert_eq!(r["model_turns"], 1);
        assert_eq!(r["guide_calls"], 0);
        assert_eq!(r["profile_removed"], true);
    }

    #[test]
    fn probe_failure_report_does_not_persist_free_error_text() {
        let mut r = report();
        let error: Result<()> = Err("Bearer fake-secret /Users/private?token=hidden".into());
        finish(&mut r, &error, 12, true);
        assert_eq!(r["failure"]["code"], "unclassified_error");
        let text = r.to_string();
        for private in ["Bearer", "fake-secret", "/Users/private", "token=hidden"] {
            assert!(!text.contains(private));
        }
    }

    #[test]
    fn probe_failure_report_marks_cleanup_failure_even_after_successful_exercise() {
        let mut r = report();
        finish(&mut r, &Ok(()), 12, false);
        assert_eq!(r["status"], "failed");
        assert_eq!(r["failure"]["code"], "fixture_cleanup_failed");
        assert_eq!(r["profile_removed"], false);
    }

    #[test]
    fn probe_failure_report_distinguishes_success_from_an_absent_diagnostic() {
        let mut r = report();
        finish(&mut r, &Ok(()), 12, true);
        assert_eq!(r["status"], "completed");
        assert_eq!(r.get("failure"), Some(&Value::Null));
    }

    #[test]
    fn probe_failure_report_keeps_only_the_public_rpc_code() {
        let mut r = report();
        let rejected: Result<()> = Err("El proveedor rechazó la operación (código -32603)".into());
        finish(&mut r, &rejected, 1, true);
        assert_eq!(
            r["failure"],
            json!({"code":"provider_rpc_rejected","rpc_code":-32603})
        );
        let private: Result<()> =
            Err("El proveedor rechazó la operación (código -32603) token=hidden".into());
        finish(&mut r, &private, 1, true);
        assert_eq!(r["failure"], json!({"code":"unclassified_error"}));
        assert!(!r.to_string().contains("hidden"));
    }

    #[test]
    fn probe_failure_report_separates_availability_cancellation_and_io_without_values() {
        let mut r = report();
        let cases: Vec<(Result<()>, &str)> = vec![
            (
                Err("El diario no admite evidencia de herramientas".into()),
                "tool_journal_unsupported",
            ),
            (
                Err("El modelo elegido no está disponible en el catálogo de Codex".into()),
                "model_unavailable",
            ),
            (
                Err("El modelo elegido no admite el esfuerzo configurado".into()),
                "model_effort_unavailable",
            ),
            (Err(Box::new(gennie_mcp::Cancelled)), "cancelled"),
            (
                Err(
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Bearer fake-secret")
                        .into(),
                ),
                "io_failure",
            ),
        ];
        for (result, code) in cases {
            finish(&mut r, &result, 1, true);
            assert_eq!(r["failure"]["code"], code);
            assert!(!r.to_string().contains("fake-secret"));
        }
        assert_eq!(r["failure"]["kind"], "BrokenPipe");
    }
}
