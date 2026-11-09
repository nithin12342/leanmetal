//! src/interfaces/http/router.rs
//! Axum router configuration for EdgeFlag Daemon

use crate::interfaces::http::handlers::{
    delete_flag_handler, evaluate_handler, get_flag_handler, health_handler, list_flags_handler,
    metrics_handler, openfeature_evaluate_handler, put_flag_handler, ws_stream_handler, AppState,
};
use axum::routing::{get, post};
use axum::Router;

/// Builds the production Axum HTTP and WebSocket router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/v1/evaluate", post(evaluate_handler))
        .route("/v1/openfeature/evaluate", post(openfeature_evaluate_handler))
        .route("/v1/flags", get(list_flags_handler))
        .route(
            "/v1/flags/{id}",
            get(get_flag_handler)
                .put(put_flag_handler)
                .delete(delete_flag_handler),
        )
        .route("/v1/stream", get(ws_stream_handler))
        .with_state(state)
}
