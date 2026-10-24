//! Sonda real de precarga y primera decisión; DB en memoria y ningún efecto.
use gennie_core::{AppModule, AppState, Shutdown};
use gennie_decisions::{DecisionsModule, ModelStatus};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> gennie_core::Result<()> {
    let db = gennie_db::Database::memory()?;
    let state = AppState {
        config: Arc::new(gennie_config::Config {
            database_path: "/tmp/gennie-preload-unused.sqlite3".into(),
            db_busy_timeout_ms: 500,
        }),
        db: db.clone(),
        shutdown: Shutdown::default(),
    };
    let module = DecisionsModule::default();
    db.migrate(module.name(), module.migrations())?;
    module.start(&state)?;
    let result = (|| -> gennie_core::Result<()> {
        let started = Instant::now();
        let controller = module.controller()?;
        loop {
            match controller.model_status() {
                ModelStatus::Ready => break,
                ModelStatus::Failed(error) => return Err(error.into()),
                _ => {}
            }
            if started.elapsed() > Duration::from_secs(125) {
                return Err("Precarga excedió el plazo".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let preload_ms = started.elapsed().as_millis();
        if !controller.poll().is_empty() {
            return Err("La precarga no debe producir una decisión ni una acción".into());
        }
        let started = Instant::now();
        controller.submit("Por favor, abre Discord.", false)?;
        let outcome = loop {
            if let Some(outcome) = controller.poll().into_iter().next() {
                break outcome;
            }
            if started.elapsed() > Duration::from_secs(125) {
                return Err("Decisión excedió el plazo".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        println!(
            "{}",
            serde_json::json!({"preload_ms":preload_ms,"first_decision_ms":started.elapsed().as_millis(),"execution":"none","context":"real_system","outcome":outcome})
        );
        let prediction = outcome
            .prediction
            .as_ref()
            .ok_or_else(|| outcome.message.clone())?;
        if outcome.verified
            || prediction.action.as_deref() != Some("open_app")
            || prediction.target.as_deref() != Some("com.hnc.Discord")
        {
            return Err("La regresión de Discord no pasó; consulta la predicción anterior".into());
        }
        Ok(())
    })();
    let stop = module.stop(&state);
    let close = db.close();
    result.and(stop).and(close)
}
