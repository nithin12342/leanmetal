use axum::body::Body;
use axum::http::{Request, StatusCode};
use edgeflag::domain::ingress::{ConnectionRing, L1StaticCache};
use edgeflag::domain::invalidation::ValkeyMeshBus;
use edgeflag::domain::storage::{AuditWal, MmapFlagStore};
use edgeflag::interfaces::http::{build_router, AppState, DaemonMetrics};
use edgeflag::{AsyncWal, Singleflight};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn test_metrics_endpoint_exposes_prometheus_series() {
    let temp_lmdb = tempdir().unwrap();
    let temp_wal = tempdir().unwrap();
    let store = MmapFlagStore::open(temp_lmdb.path()).unwrap();
    let wal = Arc::new(AuditWal::open(temp_wal.path()).unwrap());
    let state = AppState {
        store,
        wal: wal.clone(),
        async_wal: AsyncWal::new(wal, 100, Duration::from_millis(15), 16),
        l1_cache: L1StaticCache::new(100),
        mesh: ValkeyMeshBus::new("metrics-node", None).await.unwrap(),
        ring: ConnectionRing::new(100),
        singleflight: Singleflight::new(),
        admin_token: None,
        metrics: DaemonMetrics::new(),
    };
    // Seed one evaluation so counters are non-zero.
    state.metrics.record_evaluation(12, true);
    let router = build_router(state);

    let req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("text/plain"));
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    for series in [
        "edgeflag_evaluations_total 1",
        "edgeflag_l1_cache_hits_total",
        "edgeflag_wal_queue_depth",
        "edgeflag_active_websockets",
        "edgeflag_valkey_connected",
        "edgeflag_slow_consumers_pruned_total",
    ] {
        assert!(body.contains(series), "missing {series}\n{body}");
    }
}
