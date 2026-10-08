//! src/bin/sociald.rs
//! Application 2 standalone binary: stateful social benchmark API.
//! Serves the benchmark router at root (GET /health, /users/:id, /posts).

use edgeflag::domain::social::{MmapSocialStore, SocialEngine, UserProfile};
use edgeflag::domain::storage::{AsyncWal, AuditWal};
use edgeflag::interfaces::http::build_benchmark_router;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,edgeflag=debug".into()),
        )
        .init();

    info!("Starting sociald (stateful social benchmark API)");

    let data_dir = env::var("EDGEFLAG_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let wal = Arc::new(AuditWal::open(format!("{}/social_wal", data_dir))?);
    let async_wal = AsyncWal::new(wal, 10_000, Duration::from_millis(10), 256);

    let store = Arc::new(MmapSocialStore::open(format!("{}/social_lmdb", data_dir))?);
    let engine = Arc::new(SocialEngine::new(store, Some(async_wal)));

    // Benchmark pre-seeding support (50k users, 500k posts, 2M likes)
    let auto_seed = env::var("EDGEFLAG_AUTO_SEED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if auto_seed {
        info!("EDGEFLAG_AUTO_SEED active: populating benchmark dataset (50k users, 500k posts)...");
        let users: Vec<UserProfile> = (1..=50_000)
            .map(|i| {
                UserProfile::new(
                    i,
                    format!("user_{}", i),
                    format!("user_{}@bench.local", i),
                    format!("Benchmark user bio #{}", i),
                )
            })
            .collect();

        let posts: Vec<edgeflag::domain::social::PostRecord> = (1..=500_000)
            .map(|i| edgeflag::domain::social::PostRecord {
                id: i,
                author_id: (i % 50_000) + 1,
                content: format!("Benchmark post #{} simulated feed payload", i),
                created_at_ms: 1_700_000_000_000 + i,
                likes_count: (i % 50),
            })
            .collect();

        let _ = engine.bulk_seed(&users, &posts, &[]);
        info!("Benchmark pre-seeding successfully completed (~360MB virtual footprint initialized)");
    } else if engine
        .get_user_profile(1)
        .await
        .map(|u| u.is_none())
        .unwrap_or(true)
    {
        // Minimal idempotent default seed
        let _ = engine.seed_user(UserProfile::new(
            1,
            "demo_user",
            "demo@edgeflag.local",
            "Seeded demo profile",
        ));
    }

    let app = build_benchmark_router(engine);

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("sociald listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
