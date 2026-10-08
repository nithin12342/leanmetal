//! tests/e2e_social_benchmark.rs
//! End-to-end integration and concurrency benchmark tests for the $12 server social API.
//! Verifies zero-cost abstractions, sub-millisecond latencies, and lock-free concurrent scalability.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use edgeflag::domain::social::model::{CreatePostRequest, TimelineResponse, UserProfile};
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

/// Helper to spin up a clean test instance of SocialEngine and Axum benchmark router.
fn setup_test_benchmark_app() -> (
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
async fn test_social_benchmark_functional() {
    let (_lmdb, _wal, engine, router) = setup_test_benchmark_app();

    // 1. Verify GET /health
    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Seed test users
    let user1 = UserProfile::new(1, "alice", "alice@example.com", "Distributed systems engineer");
    let user2 = UserProfile::new(2, "bob", "bob@example.com", "Kernel & systems developer");
    engine.seed_user(user1.clone()).expect("seed user1");
    engine.seed_user(user2.clone()).expect("seed user2");

    // 3. Test GET /users/:id (Authenticate / Profile Read)
    let req = Request::builder()
        .uri("/users/1")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let profile: UserProfile = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(profile.username, "alice");
    assert_eq!(profile.id, 1);

    // Non-existent user returns 404 Not Found
    let req_404 = Request::builder()
        .uri("/users/99999")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp_404 = router.clone().oneshot(req_404).await.unwrap();
    assert_eq!(resp_404.status(), StatusCode::NOT_FOUND);

    // 4. Test POST /posts (Create / Mutation)
    let post_req = CreatePostRequest {
        author_id: 1,
        content: "Rust with heed mmap achieves zero-copy speed!".to_string(),
    };
    let req = Request::builder()
        .uri("/posts")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&post_req).unwrap()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a second post by bob
    let post_req2 = CreatePostRequest {
        author_id: 2,
        content: "Eliminating TCP sockets removes the database tax.".to_string(),
    };
    let req2 = Request::builder()
        .uri("/posts")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&post_req2).unwrap()))
        .unwrap();
    let resp2 = router.clone().oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::CREATED);

    // 5. Test GET /posts (Fetch User Timeline Feed with author join)
    let req = Request::builder()
        .uri("/posts?limit=10&offset=0")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let timeline: TimelineResponse = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(timeline.count, 2);
    // Verifies natural reverse-chronological ordering: post 2 (bob) is newer than post 1 (alice)
    assert_eq!(timeline.posts[0].author_username, "bob");
    assert_eq!(timeline.posts[1].author_username, "alice");

    // 6. Verify user posts_count incremented in profile
    let updated_user1 = engine.get_user_profile(1).await.unwrap().unwrap();
    assert_eq!(updated_user1.posts_count, 1);
}

#[tokio::test]
async fn test_social_stampede_and_l1_cache() {
    let (_lmdb, _wal, engine, router) = setup_test_benchmark_app();

    let user = UserProfile::new(42, "stampede_user", "user42@test.com", "High load tester");
    engine.seed_user(user).expect("seed user");

    // Simulate 500 concurrent requests arriving for the same user profile
    const CONCURRENT_CLIENTS: usize = 500;
    let mut handles = Vec::with_capacity(CONCURRENT_CLIENTS);

    let t0 = Instant::now();
    for _ in 0..CONCURRENT_CLIENTS {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let req = Request::builder()
                .uri("/users/42")
                .method("GET")
                .body(Body::empty())
                .unwrap();
            let resp = r.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
    let elapsed = t0.elapsed();
    let avg_us = elapsed.as_micros() as f64 / CONCURRENT_CLIENTS as f64;
    println!(
        "Stampede test: {} concurrent requests completed in {:?} (Average: {:.2} µs/request)",
        CONCURRENT_CLIENTS, elapsed, avg_us
    );

    // Warm L1 Cache latency benchmark
    const ITERATIONS: usize = 1_000;
    let t_cache = Instant::now();
    for _ in 0..ITERATIONS {
        let profile = engine
            .get_user_profile(42)
            .await
            .expect("fetch cached user")
            .expect("user exists");
        assert_eq!(profile.id, 42);
    }
    let cache_elapsed = t_cache.elapsed();
    let cache_avg_us = cache_elapsed.as_micros() as f64 / ITERATIONS as f64;
    println!(
        "L1 Cache Benchmark: {} reads completed in {:?} (Average: {:.3} µs/hit)",
        ITERATIONS, cache_elapsed, cache_avg_us
    );
    assert!(
        cache_avg_us < 50.0,
        "L1 Cache hit must be sub-50µs, measured {:.3} µs",
        cache_avg_us
    );
}

#[tokio::test]
async fn test_social_benchmark_concurrent_load() {
    let (_lmdb, _wal, engine, router) = setup_test_benchmark_app();

    // Pre-seed 50 active users and 100 initial posts
    for i in 1..=50 {
        let user = UserProfile::new(
            i,
            format!("user_{}", i),
            format!("user{}@benchmark.com", i),
            "Benchmark test account",
        );
        engine.seed_user(user).expect("seed user");
    }

    for i in 1..=100 {
        let author_id = ((i % 50) + 1) as u64;
        engine
            .create_post(author_id, format!("Initial historical post #{}", i))
            .await
            .expect("create initial post");
    }

    // Heavy simulated load: 2,500 operations simulating realistic user journeys:
    // - 40% Authenticate / Profile reads (GET /users/:id)
    // - 40% Timeline feed reads (GET /posts?limit=20)
    // - 20% Create / mutations (POST /posts)
    const TOTAL_REQUESTS: usize = 2_500;
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::with_capacity(TOTAL_REQUESTS);
    let start_time = Instant::now();

    for i in 0..TOTAL_REQUESTS {
        let r = router.clone();
        let succ = success_count.clone();
        let err = error_count.clone();

        handles.push(tokio::spawn(async move {
            let req = match i % 10 {
                // 40% Profile reads
                0..=3 => {
                    let user_id = (i % 50) + 1;
                    Request::builder()
                        .uri(format!("/users/{}", user_id))
                        .method("GET")
                        .body(Body::empty())
                        .unwrap()
                }
                // 40% Timeline feed reads
                4..=7 => {
                    Request::builder()
                        .uri("/posts?limit=20&offset=0")
                        .method("GET")
                        .body(Body::empty())
                        .unwrap()
                }
                // 20% Post creations
                _ => {
                    let author_id = ((i % 50) + 1) as u64;
                    let payload = CreatePostRequest {
                        author_id,
                        content: format!("Concurrent load post index {}", i),
                    };
                    Request::builder()
                        .uri("/posts")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                        .unwrap()
                }
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

    let duration = start_time.elapsed();
    let completed = success_count.load(Ordering::SeqCst);
    let failed = error_count.load(Ordering::SeqCst);
    let rps = (completed as f64) / duration.as_secs_f64();
    let error_rate = (failed as f64) / (TOTAL_REQUESTS as f64) * 100.0;

    println!("============================================================");
    println!("Benchmark Concurrency Results ($12 Server Workload):");
    println!("  Total Requests:    {}", TOTAL_REQUESTS);
    println!("  Completed:         {}", completed);
    println!("  Failed:            {}", failed);
    println!("  Total Duration:    {:?}", duration);
    println!("  Throughput:        {:.2} requests/sec", rps);
    println!("  Error Rate:        {:.2}% (Criteria: < 1%)", error_rate);
    println!("============================================================");

    assert_eq!(failed, 0, "All benchmark requests must succeed");
    assert!(error_rate < 1.0, "Error rate must be < 1%");
    assert!(rps > 1_000.0, "Throughput must exceed 1,000 RPS on concurrent load");
}
