// Acumula frases completas para voz mientras el proveedor envía fragmentos.
#[derive(Default)]
pub struct Narration {
    pending: String,
}

impl Narration {
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.pending.push_str(delta);
        let mut ready = Vec::new();
        loop {
            let boundary = self.pending.char_indices().find_map(|(index, ch)| {
                let end = index + ch.len_utf8();
                let next = self.pending[end..].chars().next();
                ((matches!(ch, '.' | '!' | '?' | ';' | ':' | '\n'))
                    && next.is_some_and(char::is_whitespace))
                .then_some(end)
            });
            let Some(end) = boundary else { break };
            let sentence = self.pending[..end].trim().to_owned();
            self.pending.drain(..end);
            self.pending = self.pending.trim_start().to_owned();
            if !sentence.is_empty() {
                ready.push(sentence);
            }
        }
        ready
    }

    pub fn finish(&mut self) -> Option<String> {
        let text = std::mem::take(&mut self.pending);
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrates_complete_sentences_once_as_deltas_arrive() {
        let mut narration = Narration::default();
        assert!(narration.push("Voy a consul").is_empty());
        assert_eq!(narration.push("tar. El uso es 5.2"), ["Voy a consultar."]);
        assert_eq!(narration.push(" % hoy. "), ["El uso es 5.2 % hoy."]);
        assert_eq!(narration.finish(), None);
    }

    #[test]
    fn finalizes_unfinished_sentence_without_repeating_spoken_text() {
        let mut narration = Narration::default();
        assert_eq!(narration.push("Primera. Segunda"), ["Primera."]);
        assert_eq!(narration.finish().as_deref(), Some("Segunda"));
    }
}
