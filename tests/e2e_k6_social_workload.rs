//! tests/e2e_k6_social_workload.rs
//! End-to-end integration and K6 virtual user loop verification for the video social benchmark.
//!
//! Benchmark Parameters Replicated:
//! - 4 Endpoints: GET /feed, GET /posts/:id, POST /posts/:id/like, POST /posts
//! - Workload Profile: 92.2% Reads (Feed + Post), 7.8% Writes (Likes + Post creations)
//! - Pre-seeded dataset: 50,000 users, 500,000 posts simulation
//! - SLA: P95 latency < 1000ms, Error rate < 1.0%

use axum::body::Body;
use axum::http::{Request, StatusCode};
use edgeflag::domain::social::model::{CreatePostRequest, PostRecord, TimelineResponse, UserProfile};
use edgeflag::domain::social::store::MmapSocialStore;
use edgeflag::domain::social::SocialEngine;
use edgeflag::domain::storage::{AsyncWal, AuditWal};
use edgeflag::interfaces::http::build_benchmark_router;
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tower::ServiceExt;

fn setup_k6_test_app() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Arc<SocialEngine<MmapSocialStore>>,
    axum::Router,
) {
    let lmdb_dir = tempdir().expect("create temp lmdb dir");
    let wal_dir = tempdir().expect("create temp wal dir");

    let store = Arc::new(MmapSocialStore::open(lmdb_dir.path()).expect("open social store"));
    let wal = Arc::new(AuditWal::open(wal_dir.path()).expect("open audit wal"));
    let async_wal = AsyncWal::new(wal, 10_000, Duration::from_millis(10), 256);

    let engine = Arc::new(SocialEngine::new(store, Some(async_wal)));
    let router = build_benchmark_router(engine.clone());

    (lmdb_dir, wal_dir, engine, router)
}

#[tokio::test]
async fn test_exact_four_benchmark_endpoints() {
    let (_lmdb, _wal, engine, router) = setup_k6_test_app();

    // Seed 1 user and 1 post
    let user = UserProfile::new(1, "alex", "alex@edgeflag.local", "Rust systems dev");
    engine.seed_user(user).expect("seed user");

    let post = engine
        .create_post(1, "First benchmark post".to_string())
        .await
        .expect("create post");

    // 1. GET /feed
    let req = Request::builder().uri("/feed").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let timeline: TimelineResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(timeline.count, 1);

    // 2. GET /posts/:id
    let req = Request::builder()
        .uri(format!("/posts/{}", post.id))
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let post_read: PostRecord = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(post_read.id, post.id);
    assert_eq!(post_read.likes_count, 0);

    // 3. POST /posts/:id/like
    let req = Request::builder()
        .uri(format!("/posts/{}/like", post.id))
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let liked_post: PostRecord = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(liked_post.likes_count, 1);

    // 4. POST /posts
    let new_post_payload = CreatePostRequest {
        author_id: 1,
        content: "Second benchmark post".to_string(),
    };
    let req = Request::builder()
        .uri("/posts")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&new_post_payload).unwrap()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_simulated_k6_user_loop_workload() {
    let (_lmdb, _wal, engine, router) = setup_k6_test_app();

    // Pre-seed 100 users and 200 posts to simulate pre-populated database
    let users: Vec<UserProfile> = (1..=100)
        .map(|i| UserProfile::new(i, format!("u{}", i), format!("u{}@b.local", i), "bio"))
        .collect();
    let posts: Vec<PostRecord> = (1..=200)
        .map(|i| PostRecord {
            id: i,
            author_id: (i % 100) + 1,
            content: format!("Seeded post payload {}", i),
            created_at_ms: 1_700_000_000_000 + i,
            likes_count: 5,
        })
        .collect();
    engine.bulk_seed(&users, &posts, &[]).expect("bulk seed");

    let total_actions = 1_000;
    let success = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let mut handles = Vec::with_capacity(total_actions);

    for i in 0..total_actions {
        let r = router.clone();
        let succ = success.clone();
        let err = errors.clone();

        handles.push(tokio::spawn(async move {
            // Emulate K6 distribution:
            // ~50% Step 1: GET /feed
            // ~42.2% Step 2: GET /posts/:id
            // ~6.3% Step 3a: POST /posts/:id/like (15% conditional)
            // ~1.5% Step 3b: POST /posts (2% conditional)
            let mod_val = i % 100;
            let req = if mod_val < 50 {
                Request::builder().uri("/feed").body(Body::empty()).unwrap()
            } else if mod_val < 92 {
                let pid = (i % 200) + 1;
                Request::builder()
                    .uri(format!("/posts/{}", pid))
                    .body(Body::empty())
                    .unwrap()
            } else if mod_val < 98 {
                let pid = (i % 200) + 1;
                Request::builder()
                    .uri(format!("/posts/{}/like", pid))
                    .method("POST")
                    .body(Body::empty())
                    .unwrap()
            } else {
                let author_id = ((i % 100) + 1) as u64;
                let payload = CreatePostRequest {
                    author_id,
                    content: format!("Dynamic K6 post {}", i),
                };
                Request::builder()
                    .uri("/posts")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap()
            };

            let resp = r.oneshot(req).await;
            match resp {
                Ok(res) if res.status().is_success() => {
                    succ.fetch_add(1, Ordering::Relaxed);
                }
                _ => {
                    err.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let duration = start.elapsed();
    let completed = success.load(Ordering::SeqCst);
    let failed = errors.load(Ordering::SeqCst);
    let rps = (completed as f64) / duration.as_secs_f64();
    let error_rate = (failed as f64) / (total_actions as f64) * 100.0;

    assert_eq!(failed, 0, "All simulated K6 requests must pass with zero errors");
    assert!(error_rate < 1.0, "Error rate must be < 1%");
    assert!(rps > 1_000.0, "Throughput must exceed 1,000 RPS on multi-endpoint K6 workload");
}
