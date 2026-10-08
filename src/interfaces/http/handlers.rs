use super::metrics::DaemonMetrics;
use crate::domain::engine::evaluator::evaluate_flag;
use crate::domain::engine::model::{
    EvaluationContext, EvaluationResult, FlagDefinition, OpenFeatureResolution,
};
use crate::domain::engine::singleflight::Singleflight;
use crate::domain::ingress::{ConnectionRing, L1StaticCache};
use crate::domain::invalidation::ValkeyMeshBus;
use crate::domain::storage::{AsyncWal, AuditWal, MmapFlagStore};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Shared application state injected into Axum router
#[derive(Clone)]
pub struct AppState {
    pub store: MmapFlagStore,
    pub wal: Arc<AuditWal>,
    pub async_wal: AsyncWal,
    pub l1_cache: L1StaticCache,
    pub mesh: ValkeyMeshBus,
    pub ring: ConnectionRing,
    pub singleflight: Singleflight<String, Result<EvaluationResult, (StatusCode, String)>>,
    pub admin_token: Option<String>,
    pub metrics: DaemonMetrics,
}

#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub flag_key: String,
    pub context: EvaluationContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateResponse {
    pub result: EvaluationResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openfeature: Option<OpenFeatureResolution>,
    pub eval_time_us: u64,
    pub served_from_l1: bool,
}

/// Validates Bearer token on administrative mutating endpoints (REQ-011 / SPEC-011).
fn check_admin_auth(headers: &HeaderMap, state: &AppState) -> Result<(), (StatusCode, String)> {
    if let Some(ref required_token) = state.admin_token {
        let auth_hdr = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());
        let expected = format!("Bearer {}", required_token);
        if auth_hdr != Some(&expected) {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Unauthorized: Valid Bearer token required for mutating administrative actions".to_string(),
            ));
        }
    }
    Ok(())
}

/// POST /v1/evaluate
/// Programmatically evaluates a dynamic feature flag against client context.
pub async fn evaluate_handler(
    State(state): State<AppState>,
    Json(payload): Json<EvaluateRequest>,
) -> Result<Json<EvaluateResponse>, (StatusCode, String)> {
    let t0 = Instant::now();

    // 1. Fast-path: Check static L1 cache (< 10 µs)
    if payload.context.attributes.is_empty()
        && let Some(cached) = state.l1_cache.get(&payload.flag_key) {
            let eval_time_us = t0.elapsed().as_micros() as u64;
            state.metrics.record_evaluation(eval_time_us, true);
            let of_res = cached.to_openfeature();
            return Ok(Json(EvaluateResponse {
                result: cached,
                openfeature: Some(of_res),
                eval_time_us,
                served_from_l1: true,
            }));
        }

    // 2. Dynamic path: Read from heed mmap and evaluate (coalesced via Singleflight)
    let store_clone = state.store.clone();
    let l1_clone = state.l1_cache.clone();
    let flag_key_clone = payload.flag_key.clone();
    let context_clone = payload.context.clone();

    let result = state
        .singleflight
        .execute(&payload.flag_key, || async move {
            let rtxn = store_clone.read_txn().map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage read txn error: {}", e))
            })?;

            let flag = match store_clone.get_flag(&rtxn, &flag_key_clone) {
                Ok(Some(f)) => f,
                Ok(None) => {
                    return Err((StatusCode::NOT_FOUND, format!("Flag '{}' not found", flag_key_clone)));
                }
                Err(e) => {
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Storage read error: {}", e)));
                }
            };

            let res = evaluate_flag(&flag, &context_clone);

            // Cache static flags in L1 if no targeting rules exist
            if flag.rules.is_empty() && context_clone.attributes.is_empty() {
                l1_clone.put(&flag_key_clone, res.clone());
            }

            Ok(res)
        })
        .await?;

    let eval_time_us = t0.elapsed().as_micros() as u64;
    state.metrics.record_evaluation(eval_time_us, false);
    let of_res = result.to_openfeature();

    Ok(Json(EvaluateResponse {
        result,
        openfeature: Some(of_res),
        eval_time_us,
        served_from_l1: false,
    }))
}

/// POST /v1/openfeature/evaluate
/// Directly evaluates dynamic feature flag conforming to CNCF OpenFeature provider specification.
pub async fn openfeature_evaluate_handler(
    State(state): State<AppState>,
    Json(payload): Json<EvaluateRequest>,
) -> Result<Json<crate::domain::engine::model::OpenFeatureResolution>, (StatusCode, String)> {
    let eval_resp = evaluate_handler(State(state), Json(payload)).await?;
    Ok(Json(eval_resp.result.to_openfeature()))
}

/// PUT /v1/flags/:id
/// Programmatically creates or updates a feature flag definition (requires Bearer auth).
pub async fn put_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(flag_id): Path<String>,
    Json(mut flag): Json<FlagDefinition>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_admin_auth(&headers, &state)?;
    flag.key = flag_id.clone();

    // 1. Atomically write to mmap store
    state.store.put_flag(&flag).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to persist flag: {}", e))
    })?;

    // 2. Journal mutation to LSM WAL via lockless async ring buffer (< 100µs)
    let payload_str = serde_json::to_string(&flag).unwrap_or_default();
    let _ = state
        .async_wal
        .append_async("admin_api", &flag.key, "PUT_FLAG", &payload_str)
        .await;

    // 3. Purge local L1 cache
    state.l1_cache.remove(&flag.key);

    // 4. Publish full State Delta Replication event across Valkey mesh
    let _ = state.mesh.publish(&flag.key, 1, Some(flag.clone())).await;

    // 5. Broadcast to connected WebSocket clients via Arc<[u8]>
    let broadcast_frame: Arc<[u8]> = Arc::from(payload_str.into_bytes().into_boxed_slice());
    let notified_clients = state.ring.broadcast(broadcast_frame);

    Ok(Json(serde_json::json!({
        "status": "success",
        "flag_key": flag.key,
        "notified_websockets": notified_clients
    })))
}

/// DELETE /v1/flags/:id
/// Programmatically deletes a feature flag definition (requires Bearer auth).
pub async fn delete_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(flag_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_admin_auth(&headers, &state)?;

    // 1. Atomically delete from mmap store
    let deleted = state.store.delete_flag(&flag_id).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete flag: {}", e))
    })?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, format!("Flag '{}' not found", flag_id)));
    }

    // 2. Journal deletion to LSM WAL via lockless async ring buffer (< 100µs)
    let _ = state
        .async_wal
        .append_async("admin_api", &flag_id, "DELETE_FLAG", "{}")
        .await;

    // 3. Purge local L1 cache
    state.l1_cache.remove(&flag_id);

    // 4. Publish deletion delta across Valkey mesh
    let _ = state.mesh.publish(&flag_id, 1, None).await;

    // 5. Broadcast deletion to connected WebSockets
    let tombstone = serde_json::json!({ "deleted": true, "flag_key": flag_id }).to_string();
    let frame: Arc<[u8]> = Arc::from(tombstone.into_bytes().into_boxed_slice());
    let notified = state.ring.broadcast(frame);

    Ok(Json(serde_json::json!({
        "status": "deleted",
        "flag_key": flag_id,
        "notified_websockets": notified
    })))
}

/// GET /v1/flags/:id
/// Programmatically inspects a flag definition.
pub async fn get_flag_handler(
    State(state): State<AppState>,
    Path(flag_id): Path<String>,
) -> Result<Json<FlagDefinition>, (StatusCode, String)> {
    let rtxn = state.store.read_txn().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage read txn error: {}", e))
    })?;

    match state.store.get_flag(&rtxn, &flag_id) {
        Ok(Some(flag)) => Ok(Json(flag)),
        Ok(None) => Err((StatusCode::NOT_FOUND, format!("Flag '{}' not found", flag_id))),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Storage error: {}", e))),
    }
}

/// GET /v1/flags
/// Lists all registered flag keys.
pub async fn list_flags_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let rtxn = state.store.read_txn().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage error: {}", e))
    })?;

    let keys = state.store.list_keys(&rtxn).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Storage error: {}", e))
    })?;

    Ok(Json(keys))
}

/// GET /v1/stream
/// Upgrades HTTP connection to WebSocket for real-time flag push notifications.
pub async fn ws_stream_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state.ring))
}

async fn handle_socket(mut socket: WebSocket, ring: ConnectionRing) {
    let (mut rx, _guard) = ring.register_client();

    while let Ok(frame) = rx.recv().await {
        if socket.send(Message::Binary(frame.to_vec().into())).await.is_err() {
            break; // Client disconnected
        }
    }
}

/// GET /health
/// Health check and operational metrics endpoint.
pub async fn health_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "active_websockets": state.ring.active_clients(),
        "valkey_connected": state.mesh.is_cluster_connected()
    }))
}

/// GET /metrics
/// Prometheus OpenMetrics exposition (REQ-019 / SPEC-019).
/// Collects P99-relevant latencies, L1 hit ratios, WAL depth, pruned clients.
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics.render_prometheus(
        state.async_wal.pending_writes(),
        state.ring.active_clients(),
        state.mesh.is_cluster_connected(),
    );
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    )
}
