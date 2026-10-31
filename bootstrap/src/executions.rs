//! Composición del ejecutor con los mismos contratos/permisos de Plugins.
use gennie_assignments::{
    Route,
    execution::{Runtime, Target},
};
use gennie_core::Result;
use serde_json::{Value, json};
pub(crate) struct NoticeGuide {
    pub store: gennie_assignments::Store,
    pub guide: gennie_proactivity::Controller,
}
impl gennie_llm::notices::guidance::Guide for NoticeGuide {
    fn permitted(&self, update: &gennie_llm::notices::Update) -> Result<bool> {
        self.store.contextual_notice_permitted(
            update.reference.run_id,
            &gennie_assignments::execution::notices::Delivery {
                conversation_id: update.origin.conversation_id,
                bot_id: update.origin.bot_id.clone(),
                epoch: update.origin.epoch,
                provider_thread: update.origin.provider_thread.clone(),
            },
        )
    }
    fn prepare(
        &self,
        update: &gennie_llm::notices::Update,
    ) -> Result<gennie_llm::notices::guidance::GuideInvocation> {
        if !self.permitted(update)? {
            return Err("El consentimiento de valoración ya no está vigente".into());
        }
        let prepared = self.guide.prepare()?;
        let store = self.store.clone();
        let run = update.reference.run_id;
        let destination = gennie_assignments::execution::notices::Delivery {
            conversation_id: update.origin.conversation_id,
            bot_id: update.origin.bot_id.clone(),
            epoch: update.origin.epoch,
            provider_thread: update.origin.provider_thread.clone(),
        };
        Ok(gennie_llm::notices::guidance::GuideInvocation {
            model: prepared.model().into(),
            revision: prepared.revision(),
            evaluate: Box::new(move |state, questions| {
                if !store.contextual_notice_permitted(run, &destination)? {
                    return Err(
                        "El consentimiento de valoración se retiró antes de consultar".into(),
                    );
                }
                prepared.evaluate(state, questions)
            }),
        })
    }
}
pub(crate) struct Host {
    pub plugins: gennie_plugins::Controller,
    pub execution_available: bool,
    pub bot: Option<gennie_llm::delegation::Receiver>,
    pub db: gennie_db::Database,
    pub guide: Option<gennie_proactivity::Controller>,
}
fn request(target: &Target, arguments: &Value, prepare: bool) -> Value {
    match target {
        Target::Web {
            plugin_id,
            function,
            revision,
        } => {
            json!({"action":if prepare{"prepare"}else{"run"},"plugin_id":plugin_id,"function_name":function,"expected_revision":revision,"arguments":arguments})
        }
        Target::Mcp {
            server_id,
            tool,
            revision,
        } => {
            json!({"action":if prepare{"mcp_prepare"}else{"mcp_run"},"server_id":server_id,"tool":tool,"expected_revision":revision,"arguments":arguments})
        }
    }
}
impl Runtime for Host {
    fn guide_available(&self) -> Result<bool> {
        self.guide.as_ref().map_or(Ok(false), |g| g.available())
    }
    fn notice_details(&self, run_id: i64) -> Result<Option<Value>> {
        gennie_llm::notices::decision_details(&self.db, run_id)
    }
    fn execution_available(&self) -> bool {
        self.execution_available
    }
    fn notice_origin(
        &self,
        conversation: Option<i64>,
    ) -> Result<gennie_assignments::execution::notices::Delivery> {
        let receiver = self
            .bot
            .as_ref()
            .ok_or("Activa la conversación para recibir resultados en el chat")?;
        let id = conversation
            .or(gennie_presence::history::active_conversation(&self.db)?)
            .ok_or("Inicia una conversación con Gennie antes de elegir avisos en el chat")?;
        let origin = receiver
            .origin(id)?
            .ok_or("El chat de destino ya no está disponible")?;
        Ok(gennie_assignments::execution::notices::Delivery {
            conversation_id: origin.conversation_id,
            bot_id: origin.bot_id,
            epoch: origin.epoch,
            provider_thread: origin.provider_thread,
        })
    }
    fn accept_notice(&self, notice: &gennie_assignments::execution::notices::Notice) -> Result<()> {
        notice.validate()?;
        self.bot
            .as_ref()
            .ok_or("El bot no está disponible; el aviso sigue guardado")?
            .accept_execution_notice(&gennie_llm::notices::Update {
                guidance: notice.guidance.as_ref().map(|g| {
                    gennie_llm::notices::guidance::Guidance {
                        objective: g.objective.clone(),
                        scope: g.scope.clone(),
                        signals: g.signals.clone(),
                        decision_guidance: g.decision_guidance.clone(),
                    }
                }),
                event_key: notice.event_key.clone(),
                origin: gennie_llm::delegation::Origin {
                    conversation_id: notice.destination.conversation_id,
                    bot_id: notice.destination.bot_id.clone(),
                    epoch: notice.destination.epoch,
                    provider_thread: notice.destination.provider_thread.clone(),
                },
                reference: gennie_llm::notices::Reference {
                    assignment_id: notice.assignment_id,
                    instance_id: notice.instance_id.clone(),
                    policy_id: notice.policy_id.clone(),
                    policy_revision: notice.policy_revision,
                    run_id: notice.run_id,
                    state: notice.state.clone(),
                },
                title: notice.title.clone(),
                scheduled_at: notice.scheduled_at.clone(),
                state: notice.state.clone(),
                code: notice.code.clone(),
                observation: notice.observation.clone(),
                observation_truncated: notice.observation_truncated,
            })
    }
    fn prepare(&self, target: &Target, arguments: &Value) -> Result<Route> {
        let prepared = self.plugins.request(request(target, arguments, true))?;
        if prepared["status"] != "ready" {
            return Err("La función necesita parámetros, permisos o una revisión vigente antes de programarse".into());
        }
        match prepared["transport"].as_str() {
            Some("http") => Ok(Route::Http),
            Some("page" | "program") => Ok(Route::Web),
            Some("mcp") => Ok(Route::Mcp),
            _ => Err("La función no tiene una vía de ejecución admitida".into()),
        }
    }
    fn execute(&self, target: &Target, arguments: &Value) -> Result<Value> {
        self.plugins.request(request(target, arguments, false))
    }
}

#[cfg(test)]
mod tests;
