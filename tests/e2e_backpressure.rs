use edgeflag::BackpressureHub;
use std::sync::Arc;

#[tokio::test]
async fn test_backpressure_and_slow_consumer_pruning() {
    let hub = BackpressureHub::new();

    // 1. Register 50 fast clients
    let mut fast_receivers = Vec::new();
    for i in 0..50 {
        let (rx, tracker) = hub.register(&format!("fast_client_{}", i));
        fast_receivers.push((rx, tracker));
    }

    // 2. Register 5 slow clients (who will never read from their rx queue)
    let mut _slow_receivers = Vec::new();
    for i in 0..5 {
        let (rx, tracker) = hub.register(&format!("slow_client_{}", i));
        _slow_receivers.push((rx, tracker));
    }

    assert_eq!(hub.active_count(), 55);

    // 3. Broadcast 70 frames (each 1 KB)
    // Fast clients will drain their queues; slow clients will exceed 64 frames / 64 KB
    let frame_payload: Arc<[u8]> = Arc::from(vec![0xAA; 1024].into_boxed_slice());

    let mut total_pruned = 0;
    for _ in 0..70 {
        // Fast clients drain one frame
        for (rx, tracker) in &mut fast_receivers {
            if let Ok(frame) = rx.try_recv() {
                tracker.fetch_sub(frame.len(), std::sync::atomic::Ordering::Relaxed);
            }
        }

        // Broadcast shared Arc frame
        let (_delivered, pruned) = hub.broadcast(frame_payload.clone());
        total_pruned += pruned;
    }

    // 4. Verify all 5 slow clients were pruned due to exceeding backpressure depth
    assert_eq!(total_pruned, 5, "All 5 slow consumers must be auto-pruned");
    assert_eq!(
        hub.active_count(),
        50,
        "Active count must drop from 55 down to exactly 50"
    );

    println!("Backpressure Hub successfully pruned 5 slow consumers while preserving 50 fast sessions!");
}
