use soc_config::Config;
pub use soc_config::{Error, Result};
pub use soc_db::{Database, Migration};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub shutdown: Shutdown,
}
#[derive(Clone, Default)]
pub struct Shutdown(Arc<AtomicBool>);
impl Shutdown {
    pub fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
pub trait AppModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn active_by_default(&self) -> bool {
        true
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        &[]
    }
    fn migrations(&self) -> &'static [Migration] {
        &[]
    }
    fn start(&self, state: &AppState) -> Result<()>;
    fn stop(&self, state: &AppState) -> Result<()>;
}
