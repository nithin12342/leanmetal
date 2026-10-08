use edgeflag::{
    evaluate_flag, from_slice_simd, to_vec_simd, EvaluationContext, FlagDefinition,
    KernelBypassEngine, TargetingRule,
};
use std::time::Instant;

#[test]
fn test_true_kernel_bypass_af_xdp_umem_ring_pipeline() {
    // 1. Initialize True Kernel Bypass Engine with 1,024 UMEM frames of 2048 bytes
    const NUM_FRAMES: usize = 1_024;
    const FRAME_SIZE: usize = 2_048;
    let mut bypass = KernelBypassEngine::new(NUM_FRAMES, FRAME_SIZE);

    let flag = FlagDefinition {
        key: "kernel_bypass_flag".to_string(),
        enabled: true,
        rules: vec![TargetingRule {
            conditions: vec![],
            rollout_percentage: 100,
            variant: Some("dma_zerocopy_variant".to_string()),
        }],
        default_variant: Some("fallback".to_string()),
    };

    let serialized_payload = to_vec_simd(&flag).expect("SIMD serialization failed");

    // 2. Inject 500 incoming packets directly into user-space UMEM DMA RAM (0 sk_buff allocations)
    const PACKET_COUNT: usize = 500;
    for _ in 0..PACKET_COUNT {
        let desc = bypass
            .inject_packet(&serialized_payload)
            .expect("UMEM frame allocation must succeed");
        assert!(desc.len > 0);
    }

    assert_eq!(bypass.rx_ring.available(), PACKET_COUNT);

    // 3. Process packet descriptors from RX ring in zero-copy mode without kernel syscalls
    let mut processed = 0;
    let mut cycle_times = Vec::with_capacity(PACKET_COUNT);
    let t0 = Instant::now();

    let ctx = EvaluationContext::new("usr_kernel_bypass_01");

    while let Some(rx_desc) = bypass.poll_rx() {
        let cycle_start = Instant::now();

        // Direct zero-copy borrow from UMEM physical frame
        bypass.process_packet(&rx_desc, |raw_bytes| {
            let mut scratch = raw_bytes.to_vec();
            let parsed_flag: FlagDefinition =
                from_slice_simd(&mut scratch).expect("SIMD parse inside UMEM frame failed");

            let eval_result = evaluate_flag(&parsed_flag, &ctx);
            assert!(eval_result.enabled);
            assert_eq!(
                eval_result.variant,
                Some("dma_zerocopy_variant".to_string())
            );
        });

        // 4. Recycle frame address back to UMEM pool
        bypass.complete_packet(rx_desc);
        processed += 1;
        cycle_times.push(cycle_start.elapsed().as_nanos() as f64 / 1_000.0);
    }

    let total_elapsed = t0.elapsed();
    let avg_cycle_us = total_elapsed.as_nanos() as f64 / (PACKET_COUNT as f64 * 1_000.0);

    cycle_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_cycle_us = cycle_times[(PACKET_COUNT as f64 * 0.95) as usize];

    println!(
        "Kernel Bypass (AF_XDP UMEM) Engine processed {} packets in {:?} (Average: {:.2} µs/req, P95: {:.2} µs)",
        processed, total_elapsed, avg_cycle_us, p95_cycle_us
    );

    assert_eq!(processed, PACKET_COUNT);

    #[cfg(not(debug_assertions))]
    assert!(
        p95_cycle_us < 25.0,
        "AF_XDP UMEM P95 cycle ({:.2} µs) exceeded 25 µs target",
        p95_cycle_us
    );

    #[cfg(not(debug_assertions))]
    assert!(
        avg_cycle_us < 25.0,
        "AF_XDP UMEM average cycle ({:.2} µs) exceeded 25 µs target",
        avg_cycle_us
    );
    #[cfg(debug_assertions)]
    assert!(
        avg_cycle_us < 100.0,
        "AF_XDP UMEM average cycle ({:.2} µs) exceeded 100 µs debug target",
        avg_cycle_us
    );
}
