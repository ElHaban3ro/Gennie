use super::*;
use gennie_core::AppModule;
use gennie_llm::ConversationJournal;
fn activate(j: &dyn ConversationJournal, db: &Database, thread: &str) {
    db.with_connection(|c| {
        let tx = c.transaction()?;
        j.activate(&tx, "codex", thread, None)?;
        tx.commit()?;
        Ok(())
    })
    .unwrap();
}
#[test]
fn proposal_input_is_the_current_literal_user_turn_not_the_read_selection_or_other_speakers() {
    let db = Database::memory().unwrap();
    let module = crate::PresenceModule::default();
    db.migrate("presence", module.migrations()).unwrap();
    let j = module.conversation_journal(db.clone());
    activate(j.as_ref(), &db, "old");
    j.append("codex", "old", "user", "OTHER_PRIVATE_INPUT")
        .unwrap();
    let old = conversation_for_thread(&db, "old").unwrap();
    activate(j.as_ref(), &db, "active");
    let input = "🦀é".repeat(3000);
    j.append("codex", "active", "user", &input).unwrap();
    j.append("codex", "active", "assistant", "NOT_A_USER_REPLY")
        .unwrap();
    load(&db, Some(old)).unwrap();
    let source = user_turn_for_thread(&db, "active").unwrap();
    assert_eq!(source.text.chars().count(), 4000);
    assert!(input.starts_with(&source.text));
    assert!(source.truncated);
    assert!(!source.text.contains("OTHER_PRIVATE"));
    assert!(user_turn_for_thread(&db, "old").is_err());
}
#[test]
fn deletion_requires_a_new_message_and_cannot_reuse_its_identity_or_epoch() {
    let db = Database::memory().unwrap();
    let module = crate::PresenceModule::default();
    db.migrate("presence", module.migrations()).unwrap();
    let j = module.conversation_journal(db.clone());
    activate(j.as_ref(), &db, "active");
    j.append("codex", "active", "user", "Primero").unwrap();
    let first = user_turn_for_thread(&db, "active").unwrap();
    delete(&db, first.conversation_id).unwrap();
    assert!(user_turn_for_thread(&db, "active").is_err());
    j.append("codex", "active", "user", "Respuesta nueva")
        .unwrap();
    let next = user_turn_for_thread(&db, "active").unwrap();
    assert!(next.message_id > first.message_id);
    assert!(next.epoch > first.epoch);
    assert_eq!(next.text, "Respuesta nueva");
}
