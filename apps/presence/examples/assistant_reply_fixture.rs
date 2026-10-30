//! Solo prepara la cita de la sonda FRB en un archivo temporal explícito.
use gennie_core::{AppModule, Result};
use gennie_db::Database;
use gennie_presence::PresenceModule;
use std::path::PathBuf;
fn main() -> Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("Falta fixture")?);
    if path.file_name().and_then(|v| v.to_str()) != Some("fixture.sqlite3")
        || !path
            .parent()
            .and_then(|v| v.file_name())
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.starts_with("gennie-assistant-bridge-"))
        || !path.is_file()
    {
        return Err("La sonda solo admite su fixture temporal".into());
    }
    let db = Database::open(&path, 50)?;
    let module = PresenceModule::default();
    db.migrate(module.name(), module.migrations())?;
    db.with_connection(|c| {c.execute("UPDATE presence_conversations SET provider='codex',provider_thread_id='fixture-thread' WHERE id=2",[])?;Ok(())})?;
    let journal = module.conversation_journal(db.clone());
    journal.append_user(
        "codex",
        "fixture-thread",
        "Respuesta citada de ejemplo",
        Some(2),
    )?;
    journal.begin_turn("codex", "fixture-thread", "fixture-turn")?;
    for (id, outcome, detail) in [
        (
            "fixture-completed",
            gennie_llm::ToolOutcome::Completed,
            None,
        ),
        (
            "fixture-failed",
            gennie_llm::ToolOutcome::Failed,
            Some("permission_required"),
        ),
        ("fixture-unknown", gennie_llm::ToolOutcome::Running, None),
    ] {
        journal.record_tool(
            "codex",
            "fixture-thread",
            "fixture-turn",
            &gennie_llm::ToolUse {
                call_id: id.into(),
                server: "gennie".into(),
                tool: "browser_plugin_run".into(),
                plugin_id: Some("fixture_mail".into()),
                function_name: Some("read_messages".into()),
                display_name: Some("Correo de prueba".into()),
                operation: Some("run".into()),
                outcome,
                detail: detail.map(str::to_owned),
            },
        )?;
    }
    journal.append_response(
        "codex",
        "fixture-thread",
        "fixture-turn",
        Some("fixture-response"),
        "Resultado de la consulta de ejemplo",
    )?;
    journal.finish_turn("codex", "fixture-thread", "fixture-turn")?;
    let image = image::RgbaImage::from_fn(120, 80, |x, y| {
        if x < 60 && y < 40 {
            image::Rgba([225, 155, 60, 255])
        } else {
            image::Rgba([45, 110, 170, 255])
        }
    });
    image.save(
        path.parent()
            .ok_or("Falta directorio temporal")?
            .join("avatar.png"),
    )?;
    db.close()?;
    Ok(())
}
