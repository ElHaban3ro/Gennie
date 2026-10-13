use crate::ChatBubble;
use slint::{Model, VecModel};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

const LIMIT: usize = 5;
struct Entry {
    id: u64,
    expires: Option<Instant>,
}
#[derive(Default)]
pub(crate) struct Chat {
    pub model: Rc<VecModel<ChatBubble>>,
    entries: Vec<Entry>,
    status: Option<u64>,
    decision: Option<u64>,
    speech: Option<u64>,
    next: u64,
    sleep_after_dialogue: bool,
}
impl Chat {
    fn remove(&mut self, index: usize) {
        let id = self.entries.remove(index).id;
        self.model.remove(index);
        if self.status == Some(id) {
            self.status = None;
        }
        if self.decision == Some(id) {
            self.decision = None;
        }
        if self.speech == Some(id) {
            self.speech = None;
        }
    }
    fn push(&mut self, title: &str, text: &str) -> u64 {
        if self.entries.len() == LIMIT {
            self.remove(0);
        }
        self.next += 1;
        self.entries.push(Entry {
            id: self.next,
            expires: None,
        });
        self.model.push(ChatBubble {
            from_user: title.starts_with("Tú"),
            title: title.into(),
            text: text.into(),
        });
        self.next
    }
    fn update(&mut self, id: u64, title: &str, text: &str, expires: Option<Instant>) {
        if let Some(index) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries[index].expires = expires;
            self.model.set_row_data(
                index,
                ChatBubble {
                    from_user: title.starts_with("Tú"),
                    title: title.into(),
                    text: text.into(),
                },
            );
        }
    }
    pub fn status(&mut self, title: &str, text: &str, finished: bool, ttl: Duration, now: Instant) {
        let id = self.status.unwrap_or_else(|| self.push(title, text));
        self.update(id, title, text, finished.then_some(now + ttl));
        self.status = (!finished).then_some(id);
    }
    pub fn decision(&mut self, text: &str, finished: bool, ttl: Duration, now: Instant) {
        let id = self.decision.unwrap_or_else(|| self.push("SOC", text));
        self.update(id, "SOC", text, finished.then_some(now + ttl));
        self.decision = (!finished).then_some(id);
    }
    pub fn loaded(&mut self, ttl: Duration, now: Instant) {
        // Una carga fría durante dictado no añade otro globo encima del usuario.
        if self.speech.is_none() || self.status.is_some() {
            self.status("SOC", "Reconocimiento de voz listo", true, ttl, now);
        }
    }
    pub fn begin(&mut self, ttl: Duration, now: Instant) {
        self.sleep_after_dialogue = true;
        if let Some(id) = self.speech.take()
            && let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id)
        {
            entry.expires = Some(now + ttl);
        }
        self.speech = Some(self.push("Tú", "Iniciando dictado…"));
    }
    pub fn listening(&mut self, ttl: Duration, now: Instant) {
        if self.speech.is_none() {
            self.begin(ttl, now);
        }
        self.speech("Tú · escuchando", "Habla ahora", false, ttl, now);
    }
    pub fn speech(&mut self, title: &str, text: &str, finished: bool, ttl: Duration, now: Instant) {
        self.sleep_after_dialogue = true;
        let id = self.speech.unwrap_or_else(|| self.push(title, text));
        self.update(id, title, text, finished.then_some(now + ttl));
        self.speech = (!finished).then_some(id);
    }
    pub fn progress(&mut self, text: &str, ttl: Duration, now: Instant) {
        if self.speech.is_some() {
            self.speech("SOC", text, false, ttl, now);
        } else {
            self.status("SOC", text, false, ttl, now);
        }
    }
    pub fn error(&mut self, text: &str, ttl: Duration, now: Instant) {
        if self.speech.is_some() {
            if let Some(id) = self.status
                && let Some(index) = self.entries.iter().position(|entry| entry.id == id)
            {
                self.remove(index);
            }
            self.speech("Transcripción", text, true, ttl, now);
        } else {
            self.status("Transcripción", text, true, ttl, now);
        }
    }
    pub fn cancel(&mut self) {
        self.cancel_transcription();
        self.cancel_decision();
    }
    pub fn cancel_decision(&mut self) {
        if let Some(id) = self.decision
            && let Some(index) = self.entries.iter().position(|entry| entry.id == id)
        {
            self.remove(index);
        }
    }
    pub fn cancel_transcription(&mut self) {
        for id in [self.speech, self.status].into_iter().flatten() {
            if let Some(index) = self.entries.iter().position(|entry| entry.id == id) {
                self.remove(index);
            }
        }
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        for index in (0..self.entries.len()).rev() {
            if self.entries[index]
                .expires
                .is_some_and(|deadline| now >= deadline)
            {
                self.remove(index);
            }
        }
        if self.sleep_after_dialogue && self.entries.is_empty() {
            self.sleep_after_dialogue = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_readiness_and_transcription_cancellation_do_not_finish_a_pending_decision() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        chat.decision("Analizando la orden…", false, ttl, now);
        chat.progress("Cargando Nemotron", ttl, now);
        chat.loaded(ttl, now);
        chat.begin(ttl, now);
        chat.cancel_transcription();
        assert_eq!(chat.model.row_count(), 2);
        chat.expire(now + ttl);
        assert_eq!(chat.model.row_count(), 1);
        assert_eq!(chat.model.row_data(0).unwrap().text, "Analizando la orden…");
        chat.decision("Resultado comprobado", true, ttl, now + ttl);
        assert_eq!(chat.model.row_count(), 1);
        assert_eq!(chat.model.row_data(0).unwrap().text, "Resultado comprobado");
        assert!(chat.expire(now + ttl + ttl));
    }
    #[test]
    fn cancelling_command_mode_keeps_current_dictation_and_completed_results() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        chat.decision("Resultado anterior", true, ttl, now);
        chat.decision("Analizando…", false, ttl, now);
        chat.begin(ttl, now);
        chat.cancel_decision();
        assert_eq!(chat.model.row_count(), 2);
        assert_eq!(chat.model.row_data(0).unwrap().text, "Resultado anterior");
        assert_eq!(chat.model.row_data(1).unwrap().text, "Iniciando dictado…");
    }
    #[test]
    fn sleep_waits_for_speech_and_last_dialogue_then_fires_once() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        chat.loaded(ttl, now);
        assert!(!chat.expire(now + ttl)); // La precarga no arma el sueño automático.
        chat.begin(ttl, now + ttl);
        assert!(!chat.expire(now + Duration::from_secs(100))); // Voz activa sin vencimiento.
        chat.speech(
            "Tú",
            "Primer mensaje",
            true,
            ttl,
            now + Duration::from_secs(100),
        );
        chat.begin(ttl, now + Duration::from_secs(101));
        chat.speech(
            "Tú",
            "Segundo mensaje",
            true,
            ttl,
            now + Duration::from_secs(102),
        );
        assert!(!chat.expire(now + Duration::from_secs(105)));
        assert!(chat.expire(now + Duration::from_secs(107)));
        assert!(!chat.expire(now + Duration::from_secs(108)));
    }
    #[test]
    fn progress_updates_one_status_and_failure_expires_without_stale_loading() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        for _ in 0..100 {
            chat.progress("Cargando modelo", ttl, now);
        }
        assert_eq!(chat.model.row_count(), 1);
        chat.begin(ttl, now);
        chat.error("Error de carga", ttl, now);
        assert_eq!(chat.model.row_count(), 1);
        assert_eq!(chat.model.row_data(0).unwrap().text, "Error de carga");
        chat.expire(now + ttl);
        assert_eq!(chat.model.row_count(), 0);
    }
    #[test]
    fn partials_share_one_bubble_and_sessions_expire_independently() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        chat.loaded(ttl, now);
        chat.begin(ttl, now + Duration::from_secs(1));
        chat.speech("Tú", "Hola", false, ttl, now);
        chat.speech("Tú", "Hola mundo", true, ttl, now + Duration::from_secs(2));
        chat.begin(ttl, now + Duration::from_secs(3));
        chat.speech(
            "Tú",
            "Segundo mensaje",
            true,
            ttl,
            now + Duration::from_secs(4),
        );
        assert_eq!(chat.model.row_count(), 3);
        assert_eq!(chat.model.row_data(1).unwrap().text, "Hola mundo");
        chat.expire(now + Duration::from_secs(5));
        assert_eq!(chat.model.row_count(), 2);
        chat.expire(now + Duration::from_secs(7));
        assert_eq!(chat.model.row_count(), 1);
        assert_eq!(chat.model.row_data(0).unwrap().text, "Segundo mensaje");
        chat.expire(now + Duration::from_secs(9));
        assert_eq!(chat.model.row_count(), 0);
    }
    #[test]
    fn limit_discards_oldest_and_cancel_keeps_completed_bubbles() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(30);
        for i in 0..7 {
            chat.begin(ttl, now);
            chat.speech("Tú", &i.to_string(), true, ttl, now);
        }
        assert_eq!(chat.model.row_count(), 5);
        assert_eq!(chat.model.row_data(0).unwrap().text, "2");
        chat.begin(ttl, now);
        chat.cancel();
        assert_eq!(chat.model.row_count(), 4);
        assert_eq!(chat.model.row_data(3).unwrap().text, "6");
    }
}
