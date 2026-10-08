use edgeflag::{AsyncWal, AuditWal};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[tokio::test]
async fn test_async_wal_ring_buffered_burst() {
    let temp_dir = tempdir().unwrap();
    let durable_wal = Arc::new(AuditWal::open(temp_dir.path()).expect("Failed to open AuditWal"));

    // Ring capacity 2,000, 15ms batch interval, max batch 128
    let async_wal = AsyncWal::new(
        durable_wal.clone(),
        2_000,
        Duration::from_millis(15),
        128,
    );

    const BURST_SIZE: usize = 500;
    let mut acks = Vec::with_capacity(BURST_SIZE);

    let mut latencies = Vec::with_capacity(BURST_SIZE);
    let t0 = Instant::now();
    for i in 0..BURST_SIZE {
        let submission_start = Instant::now();
        let ack_rx = async_wal
            .append_async(
                "admin_tester",
                format!("flag_key_{}", i),
                "UPDATE_ROLLOUT",
                format!("{{\"rollout\": {}}}", i % 100),
            )
            .await
            .expect("Failed to enqueue write to ring buffer");

        latencies.push(submission_start.elapsed().as_micros());
        acks.push(ack_rx);
    }
    let total_enqueue_time = t0.elapsed();
    let avg_us = total_enqueue_time.as_micros() as f64 / BURST_SIZE as f64;
    assert!(
        avg_us < 250.0,
        "Average enqueue latency must be sub-250us, was {:.2}us",
        avg_us
    );

    latencies.sort_unstable();
    let p95_us = latencies[(BURST_SIZE as f64 * 0.95) as usize];
    #[cfg(not(debug_assertions))]
    assert!(
        p95_us <= 300,
        "P95 enqueue must be sub-300us, was {}us",
        p95_us
    );

    println!(
        "Enqueued {} writes in {:?} (average: {:.2} µs/write, P95: {} µs)",
        BURST_SIZE,
        total_enqueue_time,
        avg_us,
        p95_us
    );

    // Now wait for all batch commits
    let mut committed_seqs = Vec::with_capacity(BURST_SIZE);
    for ack_rx in acks {
        let res = ack_rx.await.expect("Channel dropped");
        let seq = res.expect("WAL append failed");
        committed_seqs.push(seq);
    }

    assert_eq!(committed_seqs.len(), BURST_SIZE);
    // Sequences must be strictly monotonic
    for window in committed_seqs.windows(2) {
        assert!(window[0] < window[1], "WAL sequence numbers must be monotonically increasing");
    }

    println!("All {} writes successfully committed via async batching!", BURST_SIZE);
}
