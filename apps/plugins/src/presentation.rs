//! Proyección local para la ficha; el informe técnico no sustituye al mensaje del bot.
use super::*;

impl Controller {
    fn confirmed_messages(
        &self,
        job: &Job,
    ) -> Result<(&'static str, Vec<gennie_llm::delegation::BotMessage>)> {
        let Some((origin, legacy)) = self.0.store.presentation_origin(job.id)? else {
            return Ok(("unattributed", Vec::new()));
        };
        let Some(sequence) = job.events.last().map(|event| event.id) else {
            return Ok(("ready", Vec::new()));
        };
        let Some(messages) =
            self.0
                .journal
                .messages(&origin, job.id, &job.plugin_id, legacy, sequence)?
        else {
            return Ok(("origin_unavailable", Vec::new()));
        };
        if messages.len() > 12
            || messages
                .iter()
                .map(|m| m.text.chars().count())
                .sum::<usize>()
                > 8000
        {
            return Err("Actualizaciones del bot demasiado grandes".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut previous = 0;
        for message in &messages {
            message.validate()?;
            if message.sequence <= previous
                || message.sequence > sequence
                || !ids.insert(message.message_id)
            {
                return Err("Secuencia de mensajes del bot inválida".into());
            }
            previous = message.sequence;
        }
        Ok(("ready", messages))
    }

    pub(crate) fn present_job(&self, job: &Job) -> Result<Value> {
        let mut value = serde_json::to_value(job)?;
        value["web_pending"] = self
            .0
            .browser
            .web_attempts(&job.plugin_id, None)
            .unwrap_or_else(|_| json!({"status":"unavailable"}));
        // Los campos técnicos permanecen para el bot y las referencias históricas.
        // La vista humana solo presenta esta sección y estados fijos del host.
        value["bot_presentation"] = match self.confirmed_messages(job) {
            Ok((status, messages)) => json!({"status":status,"messages":messages}),
            Err(_) => json!({"status":"unavailable","messages":[]}),
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gennie_llm::delegation::{BotMessage, Origin, Update};
    use std::sync::atomic::AtomicU8;
    struct Journal(Arc<AtomicU8>);
    impl BuildJournal for Journal {
        fn origin(&self) -> Result<i64> {
            Ok(99)
        }
        fn exists(&self, _: i64) -> Result<bool> {
            Ok(true)
        }
        fn target(&self, _: i64) -> Result<Origin> {
            Err("No debe consultar el chat actual".into())
        }
        fn deliver(&self, _: &Update) -> Result<()> {
            Ok(())
        }
        fn messages(
            &self,
            origin: &Origin,
            job: i64,
            plugin: &str,
            legacy: bool,
            sequence: i64,
        ) -> Result<Option<Vec<BotMessage>>> {
            assert_eq!(origin.conversation_id, 1);
            assert_eq!(job, 1);
            assert_eq!(plugin, "fixture");
            assert!(!legacy);
            match self.0.load(Ordering::SeqCst) {
                1 => Err("PRIVATE_QUERY_ERROR".into()),
                2 => Ok(None),
                3 => Ok(Some(vec![])),
                mode => Ok(Some(vec![BotMessage {
                    message_id: 9,
                    sequence: sequence + i64::from(mode == 4),
                    text: "Mensaje confirmado del bot".into(),
                    created_at: "2026-10-29T13:00:00Z".into(),
                }])),
            }
        }
    }
    #[test]
    fn presentation_is_read_only_rechecks_source_and_never_uses_helper_as_bot() {
        let db = Database::memory().unwrap();
        db.migrate("plugins", MIGRATIONS).unwrap();
        let mode = Arc::new(AtomicU8::new(0));
        let root = std::env::temp_dir().join("gennie-presentation-no-runtime");
        let browser = gennie_browser::BrowserModule::new(root.clone());
        let module = PluginsModule::new(
            db.clone(),
            browser.controller(),
            Arc::new(Journal(mode.clone())),
            AgentSettings::shared(Arc::new(|| Ok(Settings::default()))),
            root.join("unused"),
            root,
        );
        let controller = module.controller();
        let origin = Origin {
            conversation_id: 1,
            bot_id: "gennie".into(),
            epoch: 1,
            provider_thread: None,
        };
        let request = BuildRequest {
            request_key: "presentation".into(),
            plugin_id: "fixture".into(),
            origin: "https://fixture.invalid".into(),
            objective: "Preparar consulta".into(),
            max_attempts: 8,
            max_seconds: 600,
            retry_of: None,
        };
        let job = controller
            .0
            .store
            .create_target(&request, &origin, 0, false)
            .unwrap();
        controller.0.store.claim().unwrap().unwrap();
        controller
            .0
            .store
            .report(job.id, "PRIVATE_HELPER_REPORT")
            .unwrap();
        let job = controller.0.store.get(job.id).unwrap();
        let ready = controller.present_job(&job).unwrap();
        assert_eq!(
            ready["bot_presentation"]["messages"][0]["text"],
            "Mensaje confirmado del bot"
        );
        assert!(
            !ready["bot_presentation"]
                .to_string()
                .contains("PRIVATE_HELPER")
        );
        for (state, expected) in [
            (1, "unavailable"),
            (2, "origin_unavailable"),
            (3, "ready"),
            (4, "unavailable"),
        ] {
            mode.store(state, Ordering::SeqCst);
            let value = controller.present_job(&job).unwrap();
            assert_eq!(value["bot_presentation"]["status"], expected);
            assert!(
                value["bot_presentation"]["messages"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(!value["bot_presentation"].to_string().contains("PRIVATE"));
        }
        mode.store(0, Ordering::SeqCst);
        assert_eq!(
            controller.present_job(&job).unwrap()["bot_presentation"]["messages"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            controller.0.store.get(job.id).unwrap().summary,
            "PRIVATE_HELPER_REPORT"
        );
        db.with_connection(|c| {
            c.execute(
                "UPDATE plugin_build_jobs SET delegation_origin=NULL WHERE id=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            controller.present_job(&job).unwrap()["bot_presentation"]["status"],
            "unattributed"
        );
        db.close().unwrap();
    }
}
