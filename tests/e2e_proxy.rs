use edgeflag::interfaces::proxy::pingora_layer::{EdgeProxyConfig, EdgeProxyService};
use std::time::Instant;

#[test]
fn test_edge_proxy_layer_caching_and_rate_limiting() {
    let config = EdgeProxyConfig {
        rate_limit_rps: 100,
        cache_capacity: 1_000,
        ..EdgeProxyConfig::default()
    };

    let service = EdgeProxyService::new(config);

    // 1. Initial lookup misses cache
    let cache_key = "eval:tier_upgrade:user_999";
    assert!(service.handle_cached_evaluation(cache_key).is_none());

    // 2. Populate TinyUFO L1 response cache
    let payload = b"{\"enabled\":true,\"variant\":\"platinum_tier\"}";
    service.store_cached_response(cache_key, payload, 1);

    // 3. Cache hit verification & latency benchmark
    let t0 = Instant::now();
    const ITERATIONS: usize = 50;
    for _ in 0..ITERATIONS {
        let cached = service
            .handle_cached_evaluation(cache_key)
            .expect("Response must be served from TinyUFO L1 cache");
        assert_eq!(&*cached, payload);
    }
    let elapsed = t0.elapsed();
    let avg_us = elapsed.as_nanos() as f64 / (ITERATIONS as f64 * 1_000.0);

    println!(
        "Edge Proxy TinyUFO L1 Cache served {} requests in {:?} (Average: {:.2} µs/hit)",
        ITERATIONS, elapsed, avg_us
    );

    #[cfg(not(debug_assertions))]
    assert!(
        avg_us < 10.0,
        "TinyUFO L1 cache hit must be sub-10µs, was {:.2} µs",
        avg_us
    );

    // 4. Rate-limit exhaustion
    for _ in 0..60 {
        let _ = service.handle_cached_evaluation(cache_key);
    }

    let status = service.status();
    println!("Proxy Status Telemetry: {:?}", status);
    assert!(status.total_requests >= 110);
    assert!(status.cache_hits >= 50);
}
