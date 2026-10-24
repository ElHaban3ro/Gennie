use crate::{ChatBubble, ChatSource, answer};
use slint::{Model, VecModel};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

const LIMIT: usize = 5;
struct Entry {
    id: u64,
    expires: Option<Instant>,
    sources: Vec<ChatSource>,
    pending_audio: usize,
}
#[derive(Default)]
pub(crate) struct Chat {
    pub model: Rc<VecModel<ChatBubble>>,
    pub sources: Rc<VecModel<ChatSource>>,
    entries: Vec<Entry>,
    status: Option<u64>,
    decision: Option<u64>,
    speech: Option<u64>,
    next: u64,
    sleep_after_dialogue: bool,
    pending_audio: usize,
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
        self.refresh_sources();
    }
    fn refresh_sources(&self) {
        let latest = self
            .entries
            .iter()
            .rev()
            .find(|entry| !entry.sources.is_empty());
        self.sources.set_vec(
            latest
                .map(|entry| entry.sources.clone())
                .unwrap_or_default(),
        );
    }
    fn push(&mut self, title: &str, text: &str) -> u64 {
        if self.entries.len() == LIMIT {
            self.remove(0);
        }
        self.next += 1;
        self.entries.push(Entry {
            id: self.next,
            expires: None,
            sources: Vec::new(),
            pending_audio: 0,
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
    pub fn decision(&mut self, text: &str, finished: bool, ttl: Duration, now: Instant) -> u64 {
        let answer = answer::prepare(text);
        let id = self
            .decision
            .unwrap_or_else(|| self.push("Gennie", &answer.text));
        self.update(id, "Gennie", &answer.text, finished.then_some(now + ttl));
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            if entry.pending_audio > 0 {
                entry.expires = None;
            }
            entry.sources = answer
                .sources
                .into_iter()
                .map(|source| ChatSource {
                    label: source.label.into(),
                    url: source.url.into(),
                })
                .collect();
            self.refresh_sources();
        }
        self.decision = (!finished).then_some(id);
        id
    }
    pub fn begin_audio(&mut self, bubble_id: Option<u64>) {
        self.pending_audio += 1;
        self.sleep_after_dialogue = true;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Some(entry.id) == bubble_id)
        {
            entry.pending_audio += 1;
            entry.expires = None;
        }
    }
    pub fn finish_audio(&mut self, bubble_id: Option<u64>, ttl: Duration, now: Instant) {
        self.pending_audio = self.pending_audio.saturating_sub(1);
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Some(entry.id) == bubble_id)
        {
            entry.pending_audio = entry.pending_audio.saturating_sub(1);
            if entry.pending_audio == 0 && self.decision != Some(entry.id) {
                entry.expires = Some(now + ttl);
            }
        }
    }
    pub fn loaded(&mut self, ttl: Duration, now: Instant) {
        // Una carga fría durante dictado no añade otro globo encima del usuario.
        if self.speech.is_none() || self.status.is_some() {
            self.status("Gennie", "Reconocimiento de voz listo", true, ttl, now);
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
            self.speech("Gennie", text, false, ttl, now);
        } else {
            self.status("Gennie", text, false, ttl, now);
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
    #[allow(dead_code)]
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
        if self.sleep_after_dialogue && self.entries.is_empty() && self.pending_audio == 0 {
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
    fn completed_llm_messages_get_separate_bubbles() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(15);
        chat.decision("Pensando…", false, ttl, now);
        chat.decision("Voy a consultar CPU y memoria.", true, ttl, now);
        chat.decision("CPU ocupada: 42 %.", true, ttl, now);
        assert_eq!(chat.model.row_count(), 2);
        assert_eq!(
            chat.model.row_data(0).unwrap().text,
            "Voy a consultar CPU y memoria."
        );
        assert_eq!(chat.model.row_data(1).unwrap().text, "CPU ocupada: 42 %.");
    }
    #[test]
    fn streaming_answer_waits_for_all_spoken_chunks_before_expiring() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(2);
        let id = chat.decision("Primera. Segunda", false, ttl, now);
        chat.begin_audio(Some(id));
        chat.begin_audio(Some(id));
        chat.finish_audio(Some(id), ttl, now + ttl);
        assert!(!chat.expire(now + ttl + ttl));
        chat.decision("Primera. Segunda.", true, ttl, now + ttl);
        assert!(!chat.expire(now + ttl + ttl));
        chat.finish_audio(Some(id), ttl, now + ttl + ttl);
        assert!(chat.expire(now + ttl + ttl + ttl));
    }
    #[test]
    fn final_answer_keeps_citations_out_of_spoken_bubble_text() {
        let mut chat = Chat::default();
        chat.decision(
            "La **serpiente más larga** es la pitón. [Guinness World Records](https://www.guinnessworldrecords.com/world-records/782113-longest-wild-snake-specimen) [Museo de Historia Natural de Londres](https://www.nhm.ac.uk/discover/what-is-the-biggest-snake-in-the-world.html)",
            true,
            Duration::from_secs(15),
            Instant::now(),
        );
        assert_eq!(
            chat.model.row_data(0).unwrap().text,
            "La serpiente más larga es la pitón."
        );
        assert_eq!(chat.sources.row_count(), 2);
        assert_eq!(
            chat.sources.row_data(0).unwrap().label,
            "Guinness World Records"
        );
    }
    #[test]
    fn source_panel_expires_with_its_answer() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(5);
        chat.decision("Respuesta. [Fuente](https://example.com/1)", true, ttl, now);
        assert_eq!(chat.sources.row_count(), 1);
        chat.expire(now + ttl);
        assert_eq!(chat.sources.row_count(), 0);
    }
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
    fn answer_stays_awake_and_visible_until_after_audio_finishes() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(3);
        let answer = chat.decision("Respuesta leída", true, ttl, now);
        chat.begin_audio(Some(answer));
        assert!(!chat.expire(now + Duration::from_secs(30)));
        assert_eq!(chat.model.row_count(), 1);
        chat.finish_audio(Some(answer), ttl, now + Duration::from_secs(30));
        assert!(!chat.expire(now + Duration::from_secs(32)));
        assert_eq!(chat.model.row_count(), 1);
        assert!(chat.expire(now + Duration::from_secs(33)));
        assert_eq!(chat.model.row_count(), 0);
    }
    #[test]
    fn multiple_audio_requests_keep_face_awake_until_all_finish() {
        let mut chat = Chat::default();
        let now = Instant::now();
        let ttl = Duration::from_secs(2);
        chat.begin_audio(None);
        chat.begin_audio(None);
        chat.finish_audio(None, ttl, now);
        assert!(!chat.expire(now));
        chat.finish_audio(None, ttl, now);
        assert!(chat.expire(now));
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
