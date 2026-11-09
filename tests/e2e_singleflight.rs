use edgeflag::{
    evaluate_flag, from_slice_simd, to_vec_simd, EvaluationContext, EvaluationResult, FlagDefinition,
    Singleflight, TargetingRule,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_stampede_singleflight_5000_concurrent_requests() {
    let sf: Singleflight<String, EvaluationResult> = Singleflight::new();
    let actual_evaluations = Arc::new(AtomicUsize::new(0));

    let flag = FlagDefinition {
        key: "stampede_heavy_feature".to_string(),
        enabled: true,
        rules: vec![TargetingRule {
            conditions: vec![],
            rollout_percentage: 100,
            variant: Some("instant_response".to_string()),
        }],
        default_variant: Some("fallback".to_string()),
    };

    let ctx = EvaluationContext::new("usr_stampede_target");

    const CONCURRENT_CLIENTS: usize = 5_000;
    let mut handles = Vec::with_capacity(CONCURRENT_CLIENTS);

    for _ in 0..CONCURRENT_CLIENTS {
        let sf_clone = sf.clone();
        let eval_counter = actual_evaluations.clone();
        let flag_clone = flag.clone();
        let ctx_clone = ctx.clone();

        handles.push(tokio::spawn(async move {
            sf_clone
                .execute(&"stampede_heavy_feature".to_string(), || async move {
                    // Simulate non-trivial computation or cold disk read
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    eval_counter.fetch_add(1, Ordering::SeqCst);
                    evaluate_flag(&flag_clone, &ctx_clone)
                })
                .await
        }));
    }

    let mut success_count = 0;
    for h in handles {
        let res = h.await.unwrap();
        assert!(res.enabled);
        assert_eq!(res.variant, Some("instant_response".to_string()));
        success_count += 1;
    }

    assert_eq!(success_count, CONCURRENT_CLIENTS);

    // Hard Singleflight assertion: Exactly 1 evaluation occurred across 5,000 concurrent requests!
    let total_evals = actual_evaluations.load(Ordering::SeqCst);
    println!(
        "Singleflight Anti-Stampede Result: {} concurrent clients served via {} storage evaluation",
        CONCURRENT_CLIENTS, total_evals
    );
    assert_eq!(
        total_evals, 1,
        "Singleflight must coalesce 5,000 concurrent requests into 1 execution"
    );
}

#[test]
fn test_simd_accelerated_serialization_vector() {
    let flag = FlagDefinition {
        key: "simd_vector_flag".to_string(),
        enabled: true,
        rules: vec![TargetingRule {
            conditions: vec![],
            rollout_percentage: 75,
            variant: Some("vector_v2".to_string()),
        }],
        default_variant: Some("scalar_v1".to_string()),
    };

    // Serialize to vector bytes
    let mut serialized = to_vec_simd(&flag).expect("SIMD serialization failed");
    assert!(!serialized.is_empty());

    // In-situ vector parsing directly in CPU registers
    let parsed: FlagDefinition =
        from_slice_simd(&mut serialized).expect("SIMD register parsing failed");

    assert_eq!(parsed.key, "simd_vector_flag");
    assert_eq!(parsed.rules[0].rollout_percentage, 75);
    assert_eq!(parsed.default_variant, Some("scalar_v1".to_string()));
}
