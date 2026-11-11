//! La generación del evento manda sobre su orden de llegada y la selección de lectura.
use super::*;
use gennie_llm::{ReplyInterruption, SpeechEvent};

#[test]
fn delegation_voice_only_queues_for_visible_active_origin_and_not_during_dictation() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    let first = append_conversation(&backend, "voice-first", "Primera")
        .parse::<i64>()
        .unwrap();
    let second = append_conversation(&backend, "voice-second", "Segunda")
        .parse::<i64>()
        .unwrap();
    backend.snapshot.selected_id = Some(second.to_string());
    let origin = gennie_presence::delegation::origin(&backend.state.db, second)
        .unwrap()
        .unwrap();
    let other = gennie_presence::delegation::origin(&backend.state.db, first)
        .unwrap()
        .unwrap();
    backend.observe_delegation_message(&other, "Aviso anterior".into());
    assert!(backend.speech_queue.is_empty());
    backend.voice_suppressed = true;
    backend.observe_delegation_message(&origin, "Durante dictado".into());
    assert!(backend.speech_queue.is_empty());
    backend.voice_suppressed = false;
    backend.observe_delegation_message(&origin, "He delegado la preparación".into());
    assert_eq!(
        backend.speech_queue.pop_front(),
        Some(("He delegado la preparación".into(), false))
    );
    backend.snapshot.selected_id = Some(first.to_string());
    backend.observe_delegation_message(&origin, "Otro chat visible".into());
    assert!(backend.speech_queue.is_empty());
    backend.close().unwrap();
}

#[test]
fn empty_dictation_allows_later_progress_messages_to_be_narrated() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.turn = Some(TurnProgress {
        id: "current".into(),
        stopping: false,
    });
    backend.voice_rewrite = true;
    let mut errors = Vec::new();
    let mut dictated = Vec::new();
    backend
        .speech_queue
        .push_back(("Voz anterior".into(), true));
    backend.observe_dictation_event(
        gennie_transcription::Event::DictationRequested,
        &mut dictated,
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "Durante dictado".into(),
            adapted: true,
        },
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    backend.observe_dictation_event(
        gennie_transcription::Event::NoSpeech("Sin palabras".into()),
        &mut dictated,
        &mut errors,
    );
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "Sigo trabajando".into(),
            adapted: true,
        },
        &mut errors,
    );
    assert_eq!(
        backend.speech_queue.pop_front(),
        Some(("Sigo trabajando".into(), true))
    );
    backend.close().unwrap();
}

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
            error_recorded: false,
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
    backend.speech_queue.push_back(("En cola".into(), false));
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
            error_recorded: false,
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

#[test]
fn voice_starts_on_first_complete_sentence_and_interruption_stays_silent() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.turn = Some(TurnProgress {
        id: "current".into(),
        stopping: false,
    });
    let mut errors = Vec::new();
    backend.observe_speech(
        "current",
        SpeechEvent::Delta("Una respuesta completa. ".into()),
        &mut errors,
    );
    assert_eq!(
        backend.speech_queue.pop_front(),
        Some(("Una respuesta completa.".into(), false))
    );
    backend.observe_speech(
        "current",
        SpeechEvent::Complete("Una respuesta completa.".into()),
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    backend.voice_suppressed = true;
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "Tardío".into(),
            adapted: true,
        },
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    backend.close().unwrap();
}

#[test]
fn expressive_voice_keeps_written_text_separate_and_preserves_guidance() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.turn = Some(TurnProgress {
        id: "current".into(),
        stopping: false,
    });
    backend.voice_rewrite = true;
    let mut errors = Vec::new();
    backend.observe_speech(
        "current",
        SpeechEvent::Delta("Texto para el chat. ".into()),
        &mut errors,
    );
    assert_eq!(backend.snapshot.reply, "Texto para el chat. ");
    assert!(backend.speech_queue.is_empty());
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "[warmly] Texto para hablar.".into(),
            adapted: true,
        },
        &mut errors,
    );
    assert_eq!(
        backend.speech_queue.pop_front(),
        Some(("[warmly] Texto para hablar.".into(), true))
    );
    backend.observe_speech(
        "current",
        SpeechEvent::Complete("Texto para el chat.".into()),
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    assert!(errors.is_empty());
    backend.close().unwrap();
}

#[test]
fn progress_message_is_queued_for_speech_without_error() {
    let fixture = Fixture::new();
    fixture.seed_selection();
    let mut backend = fixture.open();
    backend.snapshot.turn = Some(TurnProgress {
        id: "current".into(),
        stopping: false,
    });
    backend.voice_rewrite = true;
    let mut errors = Vec::new();
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "Voy a comprobarlo.".into(),
            adapted: false,
        },
        &mut errors,
    );
    assert_eq!(
        backend.speech_queue.pop_front(),
        Some(("Voy a comprobarlo.".into(), false))
    );
    assert!(errors.is_empty());
    backend.voice_suppressed = true;
    backend.observe_speech(
        "current",
        SpeechEvent::Spoken {
            text: "Mensaje tardío".into(),
            adapted: false,
        },
        &mut errors,
    );
    assert!(backend.speech_queue.is_empty());
    backend.close().unwrap();
}
