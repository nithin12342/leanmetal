//! tests/e2e_all_rust_caching.rs
//! End-to-end integration tests for The All-Rust Caching Stack (REQ-028)
//! and the Zero-Overhead Reusable EdgeFlag Guard SDK (REQ-029).

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use edgeflag::domain::engine::guard_sdk::{EdgeFlagGuard, EdgeFlagLayer};
use edgeflag::domain::engine::model::{EvaluationContext, FlagDefinition};
use edgeflag::domain::ingress::MokaL1Cache;
use edgeflag::domain::invalidation::GossipMeshBus;
use edgeflag::domain::storage::MmapFlagStore;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn test_tier1_moka_l1_cache_operations() {
    let cache: MokaL1Cache<Arc<[u8]>> = MokaL1Cache::new(10_000);

    // Initial read is a miss
    assert!(cache.get("user:101").await.is_none());
    assert!(cache.get_sync("user:101").is_none());

    // Insert pre-serialized payload
    let payload: Arc<[u8]> = Arc::from(b"{\"id\":101,\"name\":\"Alice\"}".as_slice());
    cache.insert("user:101", payload.clone()).await;

    // Sub-microsecond hit
    let hit_async = cache.get("user:101").await.expect("async cache hit");
    assert_eq!(&hit_async[..], b"{\"id\":101,\"name\":\"Alice\"}");

    let hit_sync = cache.get_sync("user:101").expect("sync cache hit");
    assert_eq!(&hit_sync[..], b"{\"id\":101,\"name\":\"Alice\"}");

    // Invalidation
    cache.invalidate("user:101").await;
    assert!(cache.get("user:101").await.is_none());
}

#[tokio::test]
async fn test_tier3_daemonless_mesh_invalidation() {
    let mesh = GossipMeshBus::new("node-alpha", None).await.expect("mesh start");
    let mut rx = mesh.subscribe();

    // Broadcast eviction signal
    mesh.publish_invalidation("post:9999", 1).await.expect("mesh broadcast");

    // Receiver picks up the eviction key
    let delta = rx.recv().await.expect("mesh receive");
    assert_eq!(delta.flag_key, "post:9999");
}

#[tokio::test]
async fn test_reusable_edgeflag_guard_sdk_inlined_evaluation() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(MmapFlagStore::open(dir.path()).expect("store open"));

    // Write a boolean feature flag to Tier 2 LMDB
    let flag = FlagDefinition {
        key: "dark_mode".to_string(),
        enabled: true,
        rules: vec![],
        default_variant: Some("on".to_string()),
    };
    store.put_flag(&flag).expect("put flag");

    // Initialize Guard in 1 line
    let guard = Arc::new(EdgeFlagGuard::new(store.clone()));

    // Evaluate in <= 3 lines
    let ctx = EvaluationContext::new("user_123");
    let enabled = guard.is_enabled("dark_mode", &ctx);
    assert!(enabled, "feature flag must evaluate to true");

    let disabled_check = guard.is_enabled("non_existent_flag", &ctx);
    assert!(!disabled_check, "missing flag must evaluate to false (safe fallback)");
}

#[tokio::test]
async fn test_edgeflag_layer_middleware_route_protection() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(MmapFlagStore::open(dir.path()).expect("store open"));

    // Flag is initially disabled
    let flag = FlagDefinition {
        key: "beta_feature".to_string(),
        enabled: false,
        rules: vec![],
        default_variant: None,
    };
    store.put_flag(&flag).expect("put flag");

    let guard = Arc::new(EdgeFlagGuard::new(store.clone()));

    // Protected router using EdgeFlagLayer
    let app = Router::new()
        .route("/beta", get(|| async { "welcome to beta" }))
        .layer(EdgeFlagLayer::require("beta_feature", guard.clone()));

    // 1. Request when flag is false -> 403 Forbidden
    let req = Request::builder().uri("/beta").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Enable flag -> Update in store & invalidate L1 cache -> Should allow 200 OK
    let updated_flag = FlagDefinition {
        key: "beta_feature".to_string(),
        enabled: true,
        rules: vec![],
        default_variant: None,
    };
    store.put_flag(&updated_flag).expect("put flag");
    guard.invalidate("beta_feature");

    let req2 = Request::builder().uri("/beta").body(Body::empty()).unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
}
