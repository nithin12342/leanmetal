//! src/interfaces/http/actix_handlers.rs
//! File ID: FILE-020
//! Responsibility: Actix Web HTTP endpoints for EdgeFlag and Social Benchmark
//! Must Never: Block Actix event loops with synchronous disk I/O.

use crate::domain::engine::evaluator::evaluate_flag;
use crate::domain::engine::model::FlagDefinition;
use crate::domain::social::model::{CreatePostRequest, TimelineQuery, TimelineResponse};
use crate::domain::social::store::MmapSocialStore;
use crate::domain::social::SocialEngine;
use crate::interfaces::http::handlers::{AppState, EvaluateRequest, EvaluateResponse};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use std::sync::Arc;
use std::time::Instant;

/// GET /health for EdgeFlag Daemon (Actix Web)
pub async fn actix_edgeflag_health(data: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "runtime": "actix-web",
        "active_websockets": data.ring.active_clients(),
        "valkey_connected": data.mesh.is_cluster_connected()
    }))
}

/// GET /metrics for Prometheus OpenMetrics exposition (Actix Web)
pub async fn actix_edgeflag_metrics(data: web::Data<AppState>) -> impl Responder {
    let body = data.metrics.render_prometheus(
        data.async_wal.pending_writes(),
        data.ring.active_clients(),
        data.mesh.is_cluster_connected(),
    );
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(body)
}

/// POST /v1/evaluate (Actix Web)
pub async fn actix_evaluate(
    data: web::Data<AppState>,
    payload: web::Json<EvaluateRequest>,
) -> impl Responder {
    let t0 = Instant::now();

    // 1. Fast-path: Check static L1 cache (< 10 µs)
    if payload.context.attributes.is_empty() {
        if let Some(cached) = data.l1_cache.get(&payload.flag_key) {
            let eval_time_us = t0.elapsed().as_micros() as u64;
            data.metrics.record_evaluation(eval_time_us, true);
            let of_res = cached.to_openfeature();
            return HttpResponse::Ok().json(EvaluateResponse {
                result: cached,
                openfeature: Some(of_res),
                eval_time_us,
                served_from_l1: true,
            });
        }
    }

    // 2. Dynamic path: Read from heed mmap and evaluate (coalesced via Singleflight)
    let store_clone = data.store.clone();
    let l1_clone = data.l1_cache.clone();
    let flag_key_clone = payload.flag_key.clone();
    let context_clone = payload.context.clone();

    let exec_res = data
        .singleflight
        .execute(&payload.flag_key, || async move {
            let rtxn = store_clone.read_txn().map_err(|e| {
                (
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Storage read txn error: {}", e),
                )
            })?;

            let flag = match store_clone.get_flag(&rtxn, &flag_key_clone) {
                Ok(Some(f)) => f,
                Ok(None) => {
                    return Err((
                        http::StatusCode::NOT_FOUND,
                        format!("Flag '{}' not found", flag_key_clone),
                    ));
                }
                Err(e) => {
                    return Err((
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Storage read error: {}", e),
                    ));
                }
            };

            let res = evaluate_flag(&flag, &context_clone);

            if flag.rules.is_empty() && context_clone.attributes.is_empty() {
                l1_clone.put(&flag_key_clone, res.clone());
            }

            Ok(res)
        })
        .await;

    match exec_res {
        Ok(result) => {
            let eval_time_us = t0.elapsed().as_micros() as u64;
            data.metrics.record_evaluation(eval_time_us, false);
            let of_res = result.to_openfeature();
            HttpResponse::Ok().json(EvaluateResponse {
                result,
                openfeature: Some(of_res),
                eval_time_us,
                served_from_l1: false,
            })
        }
        Err((code, msg)) => {
            let actix_code = actix_web::http::StatusCode::from_u16(code.as_u16())
                .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
            HttpResponse::build(actix_code).json(serde_json::json!({ "error": msg }))
        }
    }
}

/// POST /v1/openfeature/evaluate (Actix Web)
pub async fn actix_openfeature_evaluate(
    data: web::Data<AppState>,
    payload: web::Json<EvaluateRequest>,
) -> impl Responder {
    let t0 = Instant::now();
    let flag_key = payload.flag_key.clone();
    let flag_key_inner = flag_key.clone();
    let context = payload.context.clone();

    let store_clone = data.store.clone();
    let exec_res = data
        .singleflight
        .execute(&flag_key, || async move {
            let rtxn = store_clone.read_txn().map_err(|e| {
                (
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Storage read txn error: {}", e),
                )
            })?;

            let flag = match store_clone.get_flag(&rtxn, &flag_key_inner) {
                Ok(Some(f)) => f,
                Ok(None) => {
                    return Err((
                        http::StatusCode::NOT_FOUND,
                        format!("Flag '{}' not found", flag_key_inner),
                    ));
                }
                Err(e) => {
                    return Err((
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Storage read error: {}", e),
                    ));
                }
            };

            Ok(evaluate_flag(&flag, &context))
        })
        .await;

    match exec_res {
        Ok(res) => {
            let eval_time_us = t0.elapsed().as_micros() as u64;
            data.metrics.record_evaluation(eval_time_us, false);
            HttpResponse::Ok().json(res.to_openfeature())
        }
        Err((code, msg)) => {
            let actix_code = actix_web::http::StatusCode::from_u16(code.as_u16())
                .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
            HttpResponse::build(actix_code).json(serde_json::json!({ "error": msg }))
        }
    }
}

/// GET /v1/flags (Actix Web)
pub async fn actix_list_flags(data: web::Data<AppState>) -> impl Responder {
    let rtxn = match data.store.read_txn() {
        Ok(tx) => tx,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() }))
        }
    };

    match data.store.list_keys(&rtxn) {
        Ok(keys) => HttpResponse::Ok().json(keys),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// GET /v1/flags/{id} (Actix Web)
pub async fn actix_get_flag(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let flag_id = path.into_inner();
    let rtxn = match data.store.read_txn() {
        Ok(tx) => tx,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() }))
        }
    };

    match data.store.get_flag(&rtxn, &flag_id) {
        Ok(Some(flag)) => HttpResponse::Ok().json(flag),
        Ok(None) => HttpResponse::NotFound()
            .json(serde_json::json!({ "error": format!("Flag '{}' not found", flag_id) })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// PUT /v1/flags/{id} (Actix Web)
pub async fn actix_put_flag(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
    mut flag: web::Json<FlagDefinition>,
) -> impl Responder {
    if let Some(ref required_token) = data.admin_token {
        let auth_hdr = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok());
        let expected = format!("Bearer {}", required_token);
        if auth_hdr != Some(&expected) {
            return HttpResponse::Unauthorized()
                .json(serde_json::json!({ "error": "Unauthorized: Bearer token required" }));
        }
    }

    let flag_id = path.into_inner();
    flag.key = flag_id.clone();

    if let Err(e) = data.store.put_flag(&flag) {
        return HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": format!("Failed to persist flag: {}", e) }));
    }

    let payload_str = serde_json::to_string(&*flag).unwrap_or_default();
    let _ = data
        .async_wal
        .append_async("admin_api", &flag.key, "PUT_FLAG", &payload_str)
        .await;

    data.l1_cache.remove(&flag.key);
    let _ = data.mesh.publish(&flag.key, 1, Some(flag.clone())).await;

    let broadcast_frame: Arc<[u8]> = Arc::from(payload_str.into_bytes().into_boxed_slice());
    let notified_clients = data.ring.broadcast(broadcast_frame);

    HttpResponse::Ok().json(serde_json::json!({
        "status": "success",
        "flag_key": flag.key,
        "notified_websockets": notified_clients
    }))
}

/// DELETE /v1/flags/{id} (Actix Web)
pub async fn actix_delete_flag(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> impl Responder {
    if let Some(ref required_token) = data.admin_token {
        let auth_hdr = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok());
        let expected = format!("Bearer {}", required_token);
        if auth_hdr != Some(&expected) {
            return HttpResponse::Unauthorized()
                .json(serde_json::json!({ "error": "Unauthorized: Bearer token required" }));
        }
    }

    let flag_id = path.into_inner();
    match data.store.delete_flag(&flag_id) {
        Ok(true) => {
            let _ = data
                .async_wal
                .append_async("admin_api", &flag_id, "DELETE_FLAG", "{}")
                .await;
            data.l1_cache.remove(&flag_id);
            let _ = data.mesh.publish(&flag_id, 1, None).await;

            let tombstone =
                serde_json::json!({ "deleted": true, "flag_key": flag_id }).to_string();
            let frame: Arc<[u8]> = Arc::from(tombstone.into_bytes().into_boxed_slice());
            let notified = data.ring.broadcast(frame);

            HttpResponse::Ok().json(serde_json::json!({
                "status": "deleted",
                "flag_key": flag_id,
                "notified_websockets": notified
            }))
        }
        Ok(false) => HttpResponse::NotFound()
            .json(serde_json::json!({ "error": format!("Flag '{}' not found", flag_id) })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

// =========================================================================
// Benchmark / Social Endpoints (Actix Web)
// =========================================================================

/// GET /health for Social Benchmark
pub async fn actix_social_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "social-concurrency-benchmark",
        "framework": "actix-web",
        "storage": "heed-lmdb-mmap",
        "caching": "tinyufo-s3-fifo",
        "ordering": "bplus-tree-reverse-chronological"
    }))
}

/// GET /users/{id} (Authenticate / Fetch Profile)
pub async fn actix_social_get_user(
    engine: web::Data<Arc<SocialEngine<MmapSocialStore>>>,
    path: web::Path<u64>,
) -> impl Responder {
    let user_id = path.into_inner();
    match engine.get_user_profile(user_id).await {
        Ok(Some(user)) => HttpResponse::Ok().json(user),
        Ok(None) => HttpResponse::NotFound()
            .json(serde_json::json!({ "error": format!("User {} not found", user_id) })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// GET /posts (Fetch User Timeline Feed with pagination)
pub async fn actix_social_get_timeline(
    engine: web::Data<Arc<SocialEngine<MmapSocialStore>>>,
    params: web::Query<TimelineQuery>,
) -> impl Responder {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    match engine.get_timeline(limit, offset).await {
        Ok(posts) => HttpResponse::Ok().json(TimelineResponse {
            count: posts.len(),
            posts,
        }),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// POST /posts (Submit New Post)
pub async fn actix_social_create_post(
    engine: web::Data<Arc<SocialEngine<MmapSocialStore>>>,
    body: web::Json<CreatePostRequest>,
) -> impl Responder {
    if body.content.trim().is_empty() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "error": "Post content cannot be empty" }));
    }

    match engine.create_post(body.author_id, body.content.clone()).await {
        Ok(post) => HttpResponse::Created().json(post),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// POST /posts/{id}/like (Like / Vote on Post)
pub async fn actix_social_like_post(
    engine: web::Data<Arc<SocialEngine<MmapSocialStore>>>,
    path: web::Path<u64>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let post_id = path.into_inner();
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    match engine.like_post(post_id, user_id).await {
        Ok(new_count) => HttpResponse::Ok().json(serde_json::json!({
            "post_id": post_id,
            "likes_count": new_count
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Configures EdgeFlag routes on an Actix Web `App` or `Scope`
pub fn configure_edgeflag_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(actix_edgeflag_health))
        .route("/metrics", web::get().to(actix_edgeflag_metrics))
        .route("/v1/evaluate", web::post().to(actix_evaluate))
        .route(
            "/v1/openfeature/evaluate",
            web::post().to(actix_openfeature_evaluate),
        )
        .route("/v1/flags", web::get().to(actix_list_flags))
        .route("/v1/flags/{id}", web::get().to(actix_get_flag))
        .route("/v1/flags/{id}", web::put().to(actix_put_flag))
        .route("/v1/flags/{id}", web::delete().to(actix_delete_flag));
}

/// Configures Social Benchmark routes on an Actix Web `App` or `Scope`
pub fn configure_social_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(actix_social_health))
        .route("/users/{id}", web::get().to(actix_social_get_user))
        .route("/posts", web::get().to(actix_social_get_timeline))
        .route("/posts", web::post().to(actix_social_create_post))
        .route("/posts/{id}/like", web::post().to(actix_social_like_post));
}
