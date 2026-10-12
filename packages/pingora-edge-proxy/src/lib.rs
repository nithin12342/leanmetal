//! packages/pingora-edge-proxy/src/lib.rs
//! High-Throughput Edge Ingress Proxy Layer with TinyUFO Caching and Rate Limiting.

pub mod pingora_layer;

pub use pingora_layer::{
    EdgeProxyConfig, EdgeProxyStatus, EdgeRateLimiter,
};
