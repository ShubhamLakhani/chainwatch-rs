use crate::application::ports::{ChainReader, EventStore};
use crate::bootstrap::config::Config;
use crate::infrastructure::telemetry::Metrics;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Arc<dyn EventStore>,
    pub rpc: Arc<dyn ChainReader>,
    pub metrics: Arc<Metrics>,
}
