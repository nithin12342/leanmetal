//! src/interfaces/http/mod.rs
//! HTTP/3 and WebSocket interface module

pub mod handlers;
pub mod metrics;
pub mod router;
pub mod social_handlers;

pub use handlers::{AppState, EvaluateRequest, EvaluateResponse};
pub use metrics::DaemonMetrics;
pub use router::build_router;
pub use social_handlers::build_benchmark_router;
