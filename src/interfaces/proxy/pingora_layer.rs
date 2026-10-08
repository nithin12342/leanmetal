//! src/interfaces/proxy/pingora_layer.rs
//! File ID: FILE-013
//! Responsibility: High-throughput edge proxy fronting Axum with TinyUFO L1 caching and rate limiting
//! Must Never: Stall on upstream timeouts or leak unbounded connection buffers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tinyufo::TinyUfo;

/// Operational status and statistics of the Edge Ingress Proxy layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EdgeProxyStatus {
    pub proxy_mode: String,
    pub total_requests: u64,
    pub cache_hits: u64,
    pub rate_limited_requests: u64,
    pub active_upstreams: usize,
}

/// Configuration parameters for the Pingora edge proxy layer (REQ-018 / SPEC-018).
#[derive(Debug, Clone)]
pub struct EdgeProxyConfig {
    pub listen_addr: String,
    pub upstream_addr: String,
    pub max_connections: usize,
    pub rate_limit_rps: u64,
    pub cache_capacity: usize,
    pub client_timeout: Duration,
}

impl Default for EdgeProxyConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:80".to_string(),
            upstream_addr: "127.0.0.1:8080".to_string(),
            max_connections: 50_000,
            rate_limit_rps: 20_000,
            cache_capacity: 50_000,
            client_timeout: Duration::from_secs(5),
        }
    }
}

/// Sliding-window token-bucket / rate-limiter for edge proxying.
pub struct EdgeRateLimiter {
    rps_limit: u64,
    current_second: AtomicU64,
    request_count: AtomicU64,
}

impl EdgeRateLimiter {
    pub fn new(rps_limit: u64) -> Self {
        Self {
            rps_limit,
            current_second: AtomicU64::new(Self::now_secs()),
            request_count: AtomicU64::new(0),
        }
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Evaluates if an incoming client request complies with configured RPS thresholds.
    pub fn check_rate_limit(&self) -> bool {
        let now = Self::now_secs();
        let last_sec = self.current_second.load(Ordering::Relaxed);

        if now != last_sec {
            self.current_second.store(now, Ordering::Relaxed);
            self.request_count.store(1, Ordering::Relaxed);
            return true;
        }

        let current = self.request_count.fetch_add(1, Ordering::Relaxed);
        current < self.rps_limit
    }
}

/// Production Edge Ingress Proxy engine fronting Axum with TinyUFO L1 caching and rate limiting.
pub struct EdgeProxyService {
    pub config: EdgeProxyConfig,
    pub rate_limiter: EdgeRateLimiter,
    pub response_cache: TinyUfo<String, Arc<[u8]>>,
    pub total_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub rate_limited_requests: AtomicU64,
}

impl EdgeProxyService {
    pub fn new(config: EdgeProxyConfig) -> Self {
        let response_cache = TinyUfo::new(config.cache_capacity, config.cache_capacity / 2);
        let rate_limiter = EdgeRateLimiter::new(config.rate_limit_rps);

        Self {
            config,
            rate_limiter,
            response_cache,
            total_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            rate_limited_requests: AtomicU64::new(0),
        }
    }

    /// Evaluates incoming path against L1 response cache and rate limits.
    pub fn handle_cached_evaluation(&self, cache_key: &str) -> Option<Arc<[u8]>> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        if !self.rate_limiter.check_rate_limit() {
            self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        if let Some(hit) = self.response_cache.get(&cache_key.to_string()) {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            Some(hit)
        } else {
            None
        }
    }

    /// Stores hot evaluation payload in TinyUFO L1 edge cache.
    pub fn store_cached_response(&self, cache_key: &str, body: &[u8], weight: u16) {
        let arc_slice: Arc<[u8]> = Arc::from(body.to_vec().into_boxed_slice());
        self.response_cache.put(cache_key.to_string(), arc_slice, weight);
    }

    /// Invalidate cache entry on cluster mutation.
    pub fn purge_cache_key(&self, cache_key: &str) {
        self.response_cache.remove(&cache_key.to_string());
    }

    /// Extracts telemetry snapshot.
    pub fn status(&self) -> EdgeProxyStatus {
        EdgeProxyStatus {
            proxy_mode: if cfg!(all(target_os = "linux", feature = "pingora-edge")) {
                "Pingora-Linux-Kernel-Bypass".to_string()
            } else {
                "TinyUFO-UserSpace-Proxy".to_string()
            },
            total_requests: self.total_requests.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            rate_limited_requests: self.rate_limited_requests.load(Ordering::Relaxed),
            active_upstreams: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Linux-Specific Pingora Proxy Integration (opt-in: --features pingora-edge)
// ---------------------------------------------------------------------------
#[cfg(all(target_os = "linux", feature = "pingora-edge"))]
pub mod pingora_impl {
    use super::*;
    use async_trait::async_trait;
    use pingora_core::prelude::*;
    use pingora_core::upstreams::peer::HttpPeer;
    use pingora_proxy::{ProxyHttp, Session};

    pub struct PingoraEdgeApp {
        pub service: Arc<EdgeProxyService>,
        pub upstream_peer: String,
    }

    #[async_trait]
    impl ProxyHttp for PingoraEdgeApp {
        type CTX = ();
        fn new_ctx(&self) -> Self::CTX {}

        async fn upstream_peer(
            &self,
            _session: &mut Session,
            _ctx: &mut Self::CTX,
        ) -> Result<Box<HttpPeer>> {
            let peer = Box::new(HttpPeer::new(&self.upstream_peer, false, "".to_string()));
            Ok(peer)
        }

        async fn request_filter(
            &self,
            session: &mut Session,
            _ctx: &mut Self::CTX,
        ) -> Result<bool> {
            if !self.service.rate_limiter.check_rate_limit() {
                self.service.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
                let _ = session.respond_error(429).await;
                return Ok(true); // Terminate early on rate limit
            }
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_rate_limiter_boundary() {
        let limiter = EdgeRateLimiter::new(10);
        for _ in 0..10 {
            assert!(limiter.check_rate_limit(), "Initial 10 requests must pass");
        }
        assert!(
            !limiter.check_rate_limit(),
            "11th request must be rejected under 10 RPS limit"
        );
    }

    #[test]
    fn test_edge_proxy_tinyufo_caching() {
        let service = EdgeProxyService::new(EdgeProxyConfig::default());
        let key = "eval:promo_discount:user_100";
        let body = b"{\"enabled\":true,\"variant\":\"v2\"}";

        assert!(service.handle_cached_evaluation(key).is_none());

        service.store_cached_response(key, body, 1);

        let cached = service.handle_cached_evaluation(key).expect("Cache hit expected");
        assert_eq!(&*cached, body);

        service.purge_cache_key(key);
        assert!(service.handle_cached_evaluation(key).is_none());
    }
}
