//! src/interfaces/proxy/mod.rs
//! File ID: FILE-012
//! Responsibility: Edge ingress proxy layer definitions and Pingora integration
//! Must Never: Expose unauthenticated proxy internals or bypass rate-limiting guarantees.

pub mod pingora_layer;

pub use pingora_layer::{EdgeProxyConfig, EdgeProxyStatus};
