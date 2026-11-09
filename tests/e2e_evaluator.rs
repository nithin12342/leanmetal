use edgeflag::{evaluate_flag, EvaluationContext, EvaluationResult, FlagDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct EvalFixture {
    flags: Vec<FlagDefinition>,
    evaluation_requests: Vec<EvalRequestCase>,
}

#[derive(Debug, Deserialize)]
struct EvalRequestCase {
    test_id: String,
    flag_key: String,
    context: EvaluationContext,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct VerifiedCaseOutput {
    test_id: String,
    flag_key: String,
    result: EvaluationResult,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct EvaluatorOutputArtifact {
    total_evaluated: usize,
    cases: Vec<VerifiedCaseOutput>,
}

#[test]
fn test_part2_evaluator_against_simulated_fixtures() {
    let input_path = Path::new("tests/fixtures/evaluator_input.json");
    let expected_path = Path::new("tests/expected/evaluator_output.json");

    assert!(input_path.exists(), "Input fixture must exist");

    let raw = fs::read_to_string(input_path).expect("Failed to read fixture");
    let fixture: EvalFixture = serde_json::from_str(&raw).expect("Invalid fixture JSON");

    let flag_map: HashMap<String, &FlagDefinition> =
        fixture.flags.iter().map(|f| (f.key.clone(), f)).collect();

    let mut actual_cases = Vec::new();

    for req in &fixture.evaluation_requests {
        let flag = flag_map
            .get(&req.flag_key)
            .unwrap_or_else(|| panic!("Flag '{}' not found in test definition", req.flag_key));

        let start = Instant::now();
        let result = evaluate_flag(flag, &req.context);
        let elapsed = start.elapsed();

        // Hard verification SLO: P99 sub-100us in release (sub-millisecond tolerance for debug profile on Windows)
        #[cfg(debug_assertions)]
        let max_allowed_us = 1_000;
        #[cfg(not(debug_assertions))]
        let max_allowed_us = 100;

        assert!(
            elapsed.as_micros() <= max_allowed_us,
            "Evaluation latency must be <= {} microseconds, took {}us",
            max_allowed_us,
            elapsed.as_micros()
        );

        actual_cases.push(VerifiedCaseOutput {
            test_id: req.test_id.clone(),
            flag_key: req.flag_key.clone(),
            result,
        });
    }

    let actual_artifact = EvaluatorOutputArtifact {
        total_evaluated: actual_cases.len(),
        cases: actual_cases,
    };

    if !expected_path.exists() {
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(&actual_artifact).unwrap();
        fs::write(expected_path, serialized).expect("Failed to write expected output artifact");
    }

    let expected_raw = fs::read_to_string(expected_path).expect("Failed to read expected output");
    let expected_artifact: EvaluatorOutputArtifact =
        serde_json::from_str(&expected_raw).expect("Invalid expected JSON");

    assert_eq!(
        actual_artifact, expected_artifact,
        "Actual evaluator output must strictly match expected artifact!"
    );
}

#[test]
fn test_part2_evaluator_benchmark_p99_latency() {
    let flag = FlagDefinition {
        key: "perf_test_flag".to_string(),
        enabled: true,
        rules: vec![
            edgeflag::TargetingRule {
                conditions: vec![
                    edgeflag::Condition {
                        field: "country".to_string(),
                        op: edgeflag::ComparisonOp::InSet(vec!["US".to_string(), "CA".to_string(), "GB".to_string()]),
                    },
                    edgeflag::Condition {
                        field: "app_version".to_string(),
                        op: edgeflag::ComparisonOp::SemverGte("2.5.0".to_string()),
                    },
                    edgeflag::Condition {
                        field: "cart_value".to_string(),
                        op: edgeflag::ComparisonOp::GreaterThan(100.0),
                    },
                ],
                rollout_percentage: 100,
                variant: Some("opt_in_v2".to_string()),
            },
        ],
        default_variant: Some("default_v1".to_string()),
    };

    let mut attrs = HashMap::new();
    attrs.insert("country".to_string(), edgeflag::AttributeValue::String("US".to_string()));
    attrs.insert("app_version".to_string(), edgeflag::AttributeValue::String("3.1.2".to_string()));
    attrs.insert("cart_value".to_string(), edgeflag::AttributeValue::Float(150.0));

    let ctx = EvaluationContext {
        user_id: "usr_bench_001".to_string(),
        attributes: attrs,
    };

    const ITERATIONS: usize = 10_000;
    let mut latencies_us = Vec::with_capacity(ITERATIONS);

    for _ in 0..ITERATIONS {
        let t0 = Instant::now();
        let res = evaluate_flag(&flag, &ctx);
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        assert!(res.enabled);
        latencies_us.push(us);
    }

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencies_us[ITERATIONS * 50 / 100];
    let p99 = latencies_us[ITERATIONS * 99 / 100];

    println!("Benchmark Results: P50 = {:.2} µs, P99 = {:.2} µs", p50, p99);

    // Assert that P99 is well within the 100 µs SLO budget
    assert!(p99 <= 100.0, "P99 latency ({:.2} µs) exceeded 100 µs", p99);
}
