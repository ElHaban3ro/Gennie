//! Prepara únicamente mensajes ficticios de la sonda de lectura FRB.
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
            .is_some_and(|v| v.starts_with("gennie-stop-bridge-"))
        || !path.is_file()
    {
        return Err("La sonda solo admite su fixture temporal".into());
    }
    let reason = match std::env::args().nth(2).as_deref() {
        Some("cancelled") => gennie_llm::ReplyInterruption::Cancelled,
        Some("failed") => gennie_llm::ReplyInterruption::Failed,
        _ => return Err("Falta estado ficticio".into()),
    };
    let db = Database::open(&path, 50)?;
    let module = PresenceModule::default();
    db.migrate(module.name(), module.migrations())?;
    let journal = module.conversation_journal(db.clone());
    db.with_connection(|c| {
        let tx = c.transaction()?;
        journal.activate(&tx, "codex", "fixture-thread", None)?;
        tx.commit()?;
        Ok(())
    })?;
    journal.append_user("codex", "fixture-thread", "Una pregunta ficticia", None)?;
    journal.begin_turn("codex", "fixture-thread", "fixture-turn")?;
    journal.append_response(
        "codex",
        "fixture-thread",
        "fixture-turn",
        Some("complete"),
        "Este mensaje terminó antes de la interrupción.",
    )?;
    journal.append_interrupted_response(
        "codex",
        "fixture-thread",
        "fixture-turn",
        Some("fragment"),
        "Fragmento conservado con su estado de interrupción.",
        reason,
    )?;
    journal.finish_turn("codex", "fixture-thread", "fixture-turn")?;
    db.close()?;
    Ok(())
}
