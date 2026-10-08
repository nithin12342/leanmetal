//! src/main.rs
//! Production executable runner for EdgeFlag Daemon
//! High-Throughput, Low-Latency Feature Flag & Real-Time Configuration Engine

use edgeflag::domain::ingress::{ConnectionRing, L1StaticCache};
use edgeflag::domain::invalidation::ValkeyMeshBus;
use edgeflag::domain::storage::{AsyncWal, AuditWal, MmapFlagStore};
use edgeflag::interfaces::http::{build_router, AppState, DaemonMetrics};
use edgeflag::Singleflight;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured production logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,edgeflag=debug".into()),
        )
        .init();

    info!("============================================================");
    info!("Starting EdgeFlag Daemon (Bare-Metal Feature Flag Engine)");
    info!("Architecture: Sub-100µs P99, Zero-Copy Mmap, Linux Foundation Valkey");
    info!("============================================================");

    // 2. Open persistent storage directories
    let data_dir = env::var("EDGEFLAG_DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let lmdb_path = format!("{}/flags_lmdb", data_dir);
    let wal_path = format!("{}/audit_wal", data_dir);

    info!("Opening zero-copy LMDB mmap store at: {}", lmdb_path);
    let store = MmapFlagStore::open(&lmdb_path)?;

    info!("Opening persistent LSM WAL at: {}", wal_path);
    let wal = Arc::new(AuditWal::open(&wal_path)?);

    // 3. Initialize Ingress L1 Cache (TinyUFO) and Connection Ring (Calibrated to 25,000 sockets)
    let l1_cache = L1StaticCache::new(50_000);
    let ring = ConnectionRing::new(25_000);

    // 4. Initialize Linux Foundation Valkey Invalidation & Replication Mesh
    let valkey_url = env::var("VALKEY_URL").ok();
    let node_id = env::var("EDGEFLAG_NODE_ID").unwrap_or_else(|_| "edgeflag-node-01".to_string());

    info!("Initializing Replication Mesh for node: {}", node_id);
    let mesh = ValkeyMeshBus::new(&node_id, valkey_url.as_deref()).await?;

    // 5. Spawn background task linking replication mesh to local heed store and L1 cache purge
    let mut replication_rx = mesh.subscribe();
    let l1_purge_handle = l1_cache.clone();
    let store_replica = store.clone();
    tokio::spawn(async move {
        while let Ok(delta) = replication_rx.recv().await {
            info!(
                "Replication delta received for flag '{}' (rev {}) from '{}'",
                delta.flag_key, delta.revision, delta.source_node_id
            );
            if let Some(ref def) = delta.definition {
                if let Err(e) = store_replica.put_flag(def) {
                    tracing::error!("Failed to replicate flag to local store: {}", e);
                }
            } else {
                let _ = store_replica.delete_flag(&delta.flag_key);
            }
            l1_purge_handle.remove(&delta.flag_key);
        }
    });

    // 6. Build Axum HTTP and WebSocket Router
    let admin_token = env::var("EDGEFLAG_ADMIN_TOKEN")
        .ok()
        .or_else(|| Some("edgeflag-admin-secret".to_string()));

    let async_wal = AsyncWal::new(wal.clone(), 10_000, Duration::from_millis(10), 256);
    let singleflight = Singleflight::new();
    let metrics = DaemonMetrics::new();

    // 5b. Optional native AF_XDP packet ingest (REQ-020 / SPEC-020).
    // Opt-in via EDGEFLAG_XDP_IFACE (Linux + `--features af-xdp` build).
    // Any failure degrades to the simulated UMEM engine; boot never aborts.
    if let Some(xdp) = edgeflag::domain::ingress::af_xdp::NativeXskEngine::try_open_from_env()
    {
        let xdp_metrics = metrics.clone();
        tokio::task::spawn_blocking(move || {
            let mut batch = Vec::with_capacity(64);
            loop {
                batch.clear();
                match xdp.poll_rx(&mut batch) {
                    Ok(0) => {}
                    Ok(_) => {
                        let bytes: u64 = batch.iter().map(|p| p.len() as u64).sum();
                        xdp_metrics.record_xdp_rx(batch.len() as u64, bytes);
                    }
                    Err(e) => {
                        tracing::warn!("AF_XDP poll loop exiting: {}", e);
                        break;
                    }
                }
            }
        });
    }

    let app_state = AppState {
        store,
        wal,
        async_wal,
        l1_cache,
        mesh,
        ring,
        singleflight,
        admin_token,
        metrics,
    };
    let app = build_router(app_state);

    // 7. Bind and listen
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = TcpListener::bind(addr).await?;
    info!("EdgeFlag Daemon actively listening on http://{}", addr);
    info!("Endpoints available:");
    info!("  - POST   /v1/evaluate            (Programmatic contextual flag evaluation)");
    info!("  - POST   /v1/openfeature/evaluate (CNCF OpenFeature provider compliant endpoint)");
    info!("  - PUT    /v1/flags/:id           (Admin Bearer auth flag mutation & replication)");
    info!("  - DELETE /v1/flags/:id           (Admin Bearer auth flag deletion & replication)");
    info!("  - GET    /v1/flags/:id           (Programmatic flag inspection)");
    info!("  - GET    /v1/flags               (List registered flag keys)");
    info!("  - GET    /v1/stream              (Persistent WebSocket broadcast stream)");
    info!("  - GET    /health                 (Health & active socket metrics)");
    info!("  - GET    /metrics                (Prometheus OpenMetrics exposition)");

    axum::serve(listener, app).await?;

    Ok(())
}
