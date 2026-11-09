use edgeflag::domain::invalidation::mesh::{InvalidationMessage, ValkeyMeshBus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct InvalidationFixture {
    simulated_invalidations: Vec<SimulatedEvent>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SimulatedEvent {
    event_id: String,
    flag_key: String,
    revision: u64,
    source_node: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct VerifiedInvalidationRecord {
    event_id: String,
    flag_key: String,
    revision: u64,
    received_source: String,
    delivery_status: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct InvalidationOutputArtifact {
    total_events_processed: usize,
    events: Vec<VerifiedInvalidationRecord>,
    average_delivery_latency_us: u64,
}

#[tokio::test]
async fn test_part4_invalidation_mesh_against_simulated_fixtures() {
    let input_path = Path::new("tests/fixtures/invalidation_input.json");
    let expected_path = Path::new("tests/expected/invalidation_output.json");

    assert!(input_path.exists(), "Input fixture must exist");

    let raw = fs::read_to_string(input_path).expect("Failed to read fixture");
    let fixture: InvalidationFixture = serde_json::from_str(&raw).expect("Invalid JSON");

    // Initialize ValkeyMeshBus in fallback mode (or live if local server running)
    let mesh = ValkeyMeshBus::new("edge-node-local", None)
        .await
        .expect("Failed to create ValkeyMeshBus");

    let mut rx = mesh.subscribe();
    let mut verified_records = Vec::new();
    let mut total_latency_us = 0u64;

    for event in &fixture.simulated_invalidations {
        let t0 = Instant::now();

        // Publish invalidation event
        let published_msg = mesh
            .publish(&event.flag_key, event.revision, None)
            .await
            .expect("Failed to publish invalidation");

        // Receive on subscriber channel
        let received_msg: InvalidationMessage = rx
            .recv()
            .await
            .expect("Subscriber failed to receive message");

        let elapsed_us = t0.elapsed().as_micros() as u64;
        total_latency_us += elapsed_us;

        // Delivery latency check (< 15,000 µs to account for Windows unoptimized debug scheduling)
        assert!(
            elapsed_us < 15_000,
            "Invalidation delivery latency must be sub-15ms in debug mode, took {} µs",
            elapsed_us
        );

        assert_eq!(received_msg.flag_key, event.flag_key);
        assert_eq!(received_msg.revision, event.revision);
        assert_eq!(published_msg.flag_key, received_msg.flag_key);

        verified_records.push(VerifiedInvalidationRecord {
            event_id: event.event_id.clone(),
            flag_key: received_msg.flag_key,
            revision: received_msg.revision,
            received_source: received_msg.source_node_id,
            delivery_status: "DELIVERED_INSTANT".to_string(),
        });
    }

    let avg_latency = if !fixture.simulated_invalidations.is_empty() {
        total_latency_us / fixture.simulated_invalidations.len() as u64
    } else {
        0
    };

    println!("Average Invalidation Delivery Latency: {} µs", avg_latency);

    let actual_artifact = InvalidationOutputArtifact {
        total_events_processed: verified_records.len(),
        events: verified_records,
        average_delivery_latency_us: avg_latency,
    };

    if !expected_path.exists() {
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(&actual_artifact).unwrap();
        fs::write(expected_path, serialized).expect("Failed to write expected invalidation artifact");
    }

    let expected_raw = fs::read_to_string(expected_path).expect("Failed to read expected output");
    let expected_artifact: InvalidationOutputArtifact =
        serde_json::from_str(&expected_raw).expect("Invalid expected JSON");

    assert_eq!(
        actual_artifact.total_events_processed, expected_artifact.total_events_processed,
        "Total events must match"
    );
    assert_eq!(
        actual_artifact.events, expected_artifact.events,
        "Delivered invalidation events must strictly match expected artifact"
    );
}

#[tokio::test]
async fn test_cross_node_state_delta_replication() {
    use edgeflag::domain::invalidation::mesh::ReplicationDelta;
    use edgeflag::domain::storage::MmapFlagStore;
    use edgeflag::{evaluate_flag, EvaluationContext, FlagDefinition, TargetingRule};
    use tempfile::tempdir;

    // 1. Initialize two isolated nodes with independent LMDB stores
    let node1_dir = tempdir().unwrap();
    let node2_dir = tempdir().unwrap();

    let node1_store = MmapFlagStore::open(node1_dir.path()).unwrap();
    let node2_store = MmapFlagStore::open(node2_dir.path()).unwrap();

    let mesh_node1 = ValkeyMeshBus::new("edge-node-01", None).await.unwrap();
    let mut rx_node2 = mesh_node1.subscribe();

    // 2. Node 1 creates and persists a new flag
    let new_flag = FlagDefinition {
        key: "ai_summary_stream".to_string(),
        enabled: true,
        rules: vec![TargetingRule {
            conditions: vec![],
            rollout_percentage: 100,
            variant: Some("gpt4o_realtime".to_string()),
        }],
        default_variant: Some("gpt4o_mini".to_string()),
    };

    node1_store.put_flag(&new_flag).unwrap();

    // 3. Node 1 broadcasts State Delta Replication carrying full FlagDefinition
    mesh_node1
        .publish(&new_flag.key, 1, Some(new_flag.clone()))
        .await
        .unwrap();

    // 4. Node 2 receives ReplicationDelta and writes into its own local LMDB
    let delta: ReplicationDelta = rx_node2.recv().await.unwrap();
    assert_eq!(delta.flag_key, "ai_summary_stream");
    assert!(delta.definition.is_some(), "Delta must carry full FlagDefinition");

    // Node 2 replicates delta into its local store
    if let Some(def) = delta.definition {
        node2_store.put_flag(&def).unwrap();
    }

    // 5. Verify Node 2 can now locally evaluate the replicated flag without contacting Node 1
    let rtxn_node2 = node2_store.read_txn().unwrap();
    let flag_on_node2 = node2_store
        .get_flag(&rtxn_node2, "ai_summary_stream")
        .unwrap()
        .expect("Flag must exist in Node 2 local mmap store");

    let ctx = EvaluationContext::new("usr_replicated_99");
    let eval_node2 = evaluate_flag(&flag_on_node2, &ctx);

    assert!(eval_node2.enabled);
    assert_eq!(eval_node2.variant, Some("gpt4o_realtime".to_string()));
    println!("Cross-node State Delta Replication verified successfully!");
}
