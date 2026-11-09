use axum::body::Body;
use axum::http::{Request, StatusCode};
use edgeflag::domain::ingress::{ConnectionRing, L1StaticCache};
use edgeflag::domain::invalidation::ValkeyMeshBus;
use edgeflag::domain::storage::{AuditWal, MmapFlagStore};
use edgeflag::interfaces::http::{build_router, AppState, DaemonMetrics, EvaluateResponse};
use edgeflag::{AsyncWal, FlagDefinition, Singleflight};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tower::ServiceExt;

#[derive(Debug, Deserialize)]
struct IngressFixture {
    flag_to_create: FlagDefinition,
    evaluations: Vec<EvalCase>,
}

#[derive(Debug, Deserialize)]
struct EvalCase {
    test_case: String,
    flag_key: String,
    context: edgeflag::EvaluationContext,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct VerifiedEndpointRecord {
    test_case: String,
    status_code: u16,
    enabled: bool,
    variant: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct IngressOutputArtifact {
    health_status: String,
    mutation_status_code: u16,
    evaluated_cases: Vec<VerifiedEndpointRecord>,
}

#[tokio::test]
async fn test_part5_ingress_router_against_simulated_fixtures() {
    let input_path = Path::new("tests/fixtures/ingress_input.json");
    let expected_path = Path::new("tests/expected/ingress_output.json");

    assert!(input_path.exists(), "Input fixture must exist");

    let raw = fs::read_to_string(input_path).expect("Failed to read fixture");
    let fixture: IngressFixture = serde_json::from_str(&raw).expect("Invalid JSON");

    // 1. Initialize temporary backing services
    let temp_lmdb = tempdir().unwrap();
    let temp_wal = tempdir().unwrap();

    let store = MmapFlagStore::open(temp_lmdb.path()).unwrap();
    let wal = Arc::new(AuditWal::open(temp_wal.path()).unwrap());
    let l1_cache = L1StaticCache::new(10_000);
    let ring = ConnectionRing::new(1_000);
    let mesh = ValkeyMeshBus::new("test-ingress-node", None).await.unwrap();

    let admin_secret = "test-admin-secret".to_string();
    let async_wal = AsyncWal::new(wal.clone(), 1_000, Duration::from_millis(15), 64);
    let singleflight = Singleflight::new();
    let state = AppState {
        store,
        wal,
        async_wal,
        l1_cache,
        mesh,
        ring,
        singleflight,
        admin_token: Some(admin_secret.clone()),
        metrics: DaemonMetrics::new(),
    };
    let router = build_router(state);

    // 2. Test GET /health
    let health_req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let health_resp = router.clone().oneshot(health_req).await.unwrap();
    assert_eq!(health_resp.status(), StatusCode::OK);
    let health_bytes = health_resp.into_body().collect().await.unwrap().to_bytes();
    let health_json: serde_json::Value = serde_json::from_slice(&health_bytes).unwrap();
    assert_eq!(health_json["status"], "healthy");

    // 3. Test Unauthorized PUT /v1/flags/checkout_express (REQ-011 Bearer Auth Check)
    let create_body = serde_json::to_vec(&fixture.flag_to_create).unwrap();
    let unauth_put_req = Request::builder()
        .uri(format!("/v1/flags/{}", fixture.flag_to_create.key))
        .method("PUT")
        .header("Content-Type", "application/json")
        .body(Body::from(create_body.clone()))
        .unwrap();

    let unauth_resp = router.clone().oneshot(unauth_put_req).await.unwrap();
    assert_eq!(
        unauth_resp.status(),
        StatusCode::UNAUTHORIZED,
        "Unauthenticated admin PUT must be rejected with 401 Unauthorized"
    );

    // 4. Test Authorized PUT /v1/flags/checkout_express
    let auth_put_req = Request::builder()
        .uri(format!("/v1/flags/{}", fixture.flag_to_create.key))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_secret))
        .body(Body::from(create_body))
        .unwrap();

    let put_resp = router.clone().oneshot(auth_put_req).await.unwrap();
    let mutation_status = put_resp.status().as_u16();
    assert_eq!(mutation_status, 200);

    // 5. Test POST /v1/openfeature/evaluate (REQ-012 OpenFeature Compliance)
    let of_payload = serde_json::json!({
        "flag_key": fixture.flag_to_create.key,
        "context": fixture.evaluations[0].context
    });
    let of_req = Request::builder()
        .uri("/v1/openfeature/evaluate")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&of_payload).unwrap()))
        .unwrap();

    let of_resp = router.clone().oneshot(of_req).await.unwrap();
    assert_eq!(of_resp.status(), StatusCode::OK);
    let of_bytes = of_resp.into_body().collect().await.unwrap().to_bytes();
    let of_json: serde_json::Value = serde_json::from_slice(&of_bytes).unwrap();
    assert_eq!(of_json["flagKey"], fixture.flag_to_create.key);
    assert_eq!(of_json["reason"], "TARGETING_MATCH");
    assert_eq!(of_json["variant"], "one_click_vip");

    // 6. Test POST /v1/evaluate for simulated requests
    let mut verified_cases = Vec::new();

    for tc in &fixture.evaluations {
        let req_payload = serde_json::json!({
            "flag_key": tc.flag_key,
            "context": tc.context
        });
        let eval_req = Request::builder()
            .uri("/v1/evaluate")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
            .unwrap();

        let eval_resp = router.clone().oneshot(eval_req).await.unwrap();
        let status_code = eval_resp.status().as_u16();
        assert_eq!(status_code, 200);

        let eval_bytes = eval_resp.into_body().collect().await.unwrap().to_bytes();
        let eval_data: EvaluateResponse = serde_json::from_slice(&eval_bytes).unwrap();

        verified_cases.push(VerifiedEndpointRecord {
            test_case: tc.test_case.clone(),
            status_code,
            enabled: eval_data.result.enabled,
            variant: eval_data.result.variant,
        });
    }

    let actual_artifact = IngressOutputArtifact {
        health_status: health_json["status"].as_str().unwrap().to_string(),
        mutation_status_code: mutation_status,
        evaluated_cases: verified_cases,
    };

    if !expected_path.exists() {
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(&actual_artifact).unwrap();
        fs::write(expected_path, serialized).expect("Failed to write expected ingress artifact");
    }

    let expected_raw = fs::read_to_string(expected_path).expect("Failed to read expected output");
    let expected_artifact: IngressOutputArtifact =
        serde_json::from_str(&expected_raw).expect("Invalid expected JSON");

    assert_eq!(
        actual_artifact, expected_artifact,
        "Actual HTTP ingress output must strictly match expected artifact!"
    );
}

#[tokio::test]
async fn test_delete_flag_and_auth_protection() {
    let temp_lmdb = tempdir().unwrap();
    let temp_wal = tempdir().unwrap();

    let store = MmapFlagStore::open(temp_lmdb.path()).unwrap();
    let wal = Arc::new(AuditWal::open(temp_wal.path()).unwrap());
    let l1_cache = L1StaticCache::new(1_000);
    let ring = ConnectionRing::new(100);
    let mesh = ValkeyMeshBus::new("test-delete-node", None).await.unwrap();

    let test_flag = FlagDefinition {
        key: "temporary_promo".to_string(),
        enabled: true,
        rules: vec![],
        default_variant: Some("promo_10".to_string()),
    };
    store.put_flag(&test_flag).unwrap();

    let async_wal = AsyncWal::new(wal.clone(), 100, Duration::from_millis(15), 16);
    let singleflight = Singleflight::new();
    let state = AppState {
        store,
        wal,
        async_wal,
        l1_cache,
        mesh,
        ring,
        singleflight,
        admin_token: Some("secret-token-123".to_string()),
        metrics: DaemonMetrics::new(),
    };
    let router = build_router(state);

    // 1. Unauthenticated DELETE must fail with 401
    let unauth_del = Request::builder()
        .uri("/v1/flags/temporary_promo")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(unauth_del).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authenticated DELETE must succeed with 200
    let auth_del = Request::builder()
        .uri("/v1/flags/temporary_promo")
        .method("DELETE")
        .header("Authorization", "Bearer secret-token-123")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(auth_del).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. GET /v1/flags/temporary_promo should now be 404 NOT_FOUND
    let get_req = Request::builder()
        .uri("/v1/flags/temporary_promo")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
