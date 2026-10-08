//! src/interfaces/http/social_handlers.rs
//! File ID: FILE-016
//! Responsibility: Axum HTTP handlers for benchmark social application (<=7 words)
//! Must Never: Perform synchronous blocking I/O on Tokio worker threads.

use crate::domain::social::model::{CreatePostRequest, TimelineQuery, TimelineResponse};
use crate::domain::social::store::MmapSocialStore;
use crate::domain::social::SocialEngine;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

/// Standalone health handler for the benchmark service.
pub async fn benchmark_health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "healthy",
            "service": "social-concurrency-benchmark",
            "storage": "heed-lmdb-mmap",
            "caching": "tinyufo-s3-fifo",
            "ordering": "bplus-tree-reverse-chronological"
        })),
    )
}

/// Handler for `GET /users/:id` (Authenticate / Fetch Profile).
/// Single-row indexed lookup querying credentials, username, and account metadata.
pub async fn get_user_handler(
    State(engine): State<Arc<SocialEngine<MmapSocialStore>>>,
    Path(user_id): Path<u64>,
) -> impl IntoResponse {
    match engine.get_user_profile(user_id).await {
        Ok(Some(user)) => (StatusCode::OK, Json(user)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("User {} not found", user_id) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Handler for `GET /posts` (Fetch User Timeline Feed with pagination).
/// Retrieves recent posts joined with author profile metadata (`ORDER BY created_at DESC LIMIT 20`).
pub async fn get_timeline_handler(
    State(engine): State<Arc<SocialEngine<MmapSocialStore>>>,
    Query(params): Query<TimelineQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    match engine.get_timeline(limit, offset).await {
        Ok(posts) => {
            let count = posts.len();
            let resp = TimelineResponse { posts, count };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Handler for `POST /posts` (Create / Like a Post).
/// Persists record to LMDB, updates author counter, and commits to async WAL.
pub async fn create_post_handler(
    State(engine): State<Arc<SocialEngine<MmapSocialStore>>>,
    Json(payload): Json<CreatePostRequest>,
) -> impl IntoResponse {
    if payload.content.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Post content cannot be empty" })),
        )
            .into_response();
    }

    match engine.create_post(payload.author_id, payload.content).await {
        Ok(post) => (StatusCode::CREATED, Json(post)).into_response(),
        Err(crate::domain::social::SocialEngineError::Store(msg)) if msg.contains("User with ID") => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Handler for `GET /posts/:id` (Fetch single post by ID).
pub async fn get_post_handler(
    State(engine): State<Arc<SocialEngine<MmapSocialStore>>>,
    Path(post_id): Path<u64>,
) -> impl IntoResponse {
    match engine.get_post(post_id).await {
        Ok(Some(post)) => (StatusCode::OK, Json(post)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Post {} not found", post_id) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Handler for `POST /posts/:id/like` (Like a post).
pub async fn like_post_handler(
    State(engine): State<Arc<SocialEngine<MmapSocialStore>>>,
    Path(post_id): Path<u64>,
) -> impl IntoResponse {
    // Defaulting to user 1 for anonymous / token-less bench callers
    match engine.like_post(post_id, 1).await {
        Ok(post) => (StatusCode::OK, Json(post)).into_response(),
        Err(crate::domain::social::SocialEngineError::Store(msg)) if msg.contains("Post with ID") => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Builds the production zero-overhead Axum router for the benchmark application.
/// Exposes the exact 4 endpoints tested in the K6 benchmark:
/// - GET /feed
/// - GET /posts/{id}
/// - POST /posts/{id}/like
/// - POST /posts
/// Along with backwards-compatible GET /posts and GET /users/{id}.
pub fn build_benchmark_router(engine: Arc<SocialEngine<MmapSocialStore>>) -> Router {
    Router::new()
        .route("/health", get(benchmark_health_handler))
        .route("/feed", get(get_timeline_handler))
        .route("/users/{id}", get(get_user_handler))
        .route(
            "/posts",
            get(get_timeline_handler).post(create_post_handler),
        )
        .route("/posts/{id}", get(get_post_handler))
        .route("/posts/{id}/like", axum::routing::post(like_post_handler))
        .with_state(engine)
}
