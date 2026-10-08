use edgeflag::{compute_bucket, is_in_rollout};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct InputTestCase {
    flag_key: String,
    user_id: String,
    rollout_percentage: u8,
}

#[derive(Debug, Deserialize)]
struct FixtureData {
    test_cases: Vec<InputTestCase>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OutputTestCase {
    flag_key: String,
    user_id: String,
    computed_bucket: u8,
    is_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct OutputArtifact {
    total_evaluated: usize,
    results: Vec<OutputTestCase>,
}

#[test]
fn test_part1_hasher_against_simulated_fixtures() {
    let input_path = Path::new("tests/fixtures/bucketing_input.json");
    let expected_path = Path::new("tests/expected/bucketing_output.json");

    assert!(input_path.exists(), "Input fixture file must exist");

    let input_raw = fs::read_to_string(input_path).expect("Failed to read input fixture");
    let fixture: FixtureData = serde_json::from_str(&input_raw).expect("Invalid JSON in input fixture");

    let mut actual_results = Vec::new();
    for tc in &fixture.test_cases {
        let bucket = compute_bucket(&tc.flag_key, &tc.user_id);
        let enabled = is_in_rollout(&tc.flag_key, &tc.user_id, tc.rollout_percentage);

        assert!(bucket < 100, "Bucket must be in range [0, 99]");
        assert_eq!(
            enabled,
            bucket < tc.rollout_percentage,
            "Rollout decision must strictly match bucket < percentage"
        );

        actual_results.push(OutputTestCase {
            flag_key: tc.flag_key.clone(),
            user_id: tc.user_id.clone(),
            computed_bucket: bucket,
            is_enabled: enabled,
        });
    }

    let actual_artifact = OutputArtifact {
        total_evaluated: actual_results.len(),
        results: actual_results,
    };

    // If expected output does not exist yet, generate it for verifiable baseline
    if !expected_path.exists() {
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(&actual_artifact).unwrap();
        fs::write(expected_path, serialized).expect("Failed to write expected output artifact");
    }

    let expected_raw = fs::read_to_string(expected_path).expect("Failed to read expected output");
    let expected_artifact: OutputArtifact = serde_json::from_str(&expected_raw).expect("Invalid expected JSON");

    assert_eq!(
        actual_artifact, expected_artifact,
        "Actual runtime output must strictly match expected artifact!"
    );
}

#[test]
fn test_part1_hasher_uniformity_distribution() {
    const SAMPLE_SIZE: usize = 100_000;
    let mut bucket_counts = [0usize; 100];

    for i in 0..SAMPLE_SIZE {
        let user_id = format!("user_{:08}", i);
        let bucket = compute_bucket("experiment_feature", &user_id);
        bucket_counts[bucket as usize] += 1;
    }

    // Expected per bucket in uniform distribution: 1,000 (1%)
    // Chi-square test: sum((observed - expected)^2 / expected)
    let expected = (SAMPLE_SIZE as f64) / 100.0;
    let mut chi_square = 0.0;
    for &count in &bucket_counts {
        let diff = (count as f64) - expected;
        chi_square += (diff * diff) / expected;
    }

    // With 99 degrees of freedom, chi-square critical value at alpha=0.001 is ~148
    // If chi_square < 140, we have very high statistical confidence of uniformity
    assert!(
        chi_square < 140.0,
        "Bucketing distribution chi-square statistic ({}) exceeds threshold",
        chi_square
    );
}
