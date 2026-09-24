use crate::interfaces::http::handlers::{
    events_by_tx, health, list_events, metrics_handler, ready, status,
};
use crate::interfaces::http::state::AppState;
use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/status", get(status))
        .route("/events", get(list_events))
        .route("/events/{transaction_hash}", get(events_by_tx))
        .route("/metrics", get(metrics_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
