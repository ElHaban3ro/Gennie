//! La generación del evento manda sobre su orden de llegada y la selección de lectura.
use super::*;
use gennie_llm::{ReplyInterruption, SpeechEvent};

#[test]
fn late_speech_events_cannot_erase_acknowledgement_or_text_of_a_new_turn() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.turn = Some(TurnProgress {
        id: "new".into(),
        stopping: false,
    });
    backend.snapshot.reply = "Respuesta nueva".into();
    backend.snapshot.acknowledged_text = Some("Entrada nueva".into());
    let mut errors = Vec::new();
    for event in [
        SpeechEvent::Delta("Tardío".into()),
        SpeechEvent::UserSaved("Entrada anterior".into()),
        SpeechEvent::Complete("Texto anterior".into()),
        SpeechEvent::Interrupted {
            text: "Fragmento anterior".into(),
            saved: false,
            reason: ReplyInterruption::Cancelled,
        },
        SpeechEvent::Finished {
            cancelled: true,
            error: None,
        },
    ] {
        assert!(!backend.observe_speech("old", event, &mut errors));
    }
    assert_eq!(backend.snapshot.reply, "Respuesta nueva");
    assert_eq!(
        backend.snapshot.acknowledged_text.as_deref(),
        Some("Entrada nueva")
    );
    assert_eq!(backend.snapshot.turn.as_ref().unwrap().id, "new");
    assert!(backend.speech_queue.is_empty());
    assert!(errors.is_empty());
    backend.close().unwrap();
}

#[test]
fn unsaved_interrupted_text_survives_refresh_and_is_not_narrated_or_confused_with_old_text() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    append_conversation(&backend, "active", "Mismo texto");
    backend.snapshot.turn = Some(TurnProgress {
        id: "current".into(),
        stopping: true,
    });
    backend.speech_queue.push_back("En cola".into());
    let mut errors = Vec::new();
    assert!(backend.observe_speech(
        "current",
        SpeechEvent::Interrupted {
            text: "Mismo texto".into(),
            saved: false,
            reason: ReplyInterruption::Cancelled
        },
        &mut errors
    ));
    assert!(backend.speech_queue.is_empty());
    assert_eq!(errors.len(), 1);
    backend.observe_speech(
        "current",
        SpeechEvent::Finished {
            cancelled: true,
            error: None,
        },
        &mut errors,
    );
    backend.refresh().unwrap();
    assert_eq!(backend.snapshot.reply, "Mismo texto");
    assert_eq!(
        backend.snapshot.reply_interruption,
        Some(MessageInterruption::Cancelled)
    );
    assert!(backend.snapshot.turn.is_none());
    assert_eq!(
        backend.snapshot.confirmation.as_deref(),
        Some("Respuesta detenida")
    );
    backend.close().unwrap();
}
