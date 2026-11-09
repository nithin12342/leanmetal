use edgeflag::{evaluate_flag, AuditWal, EvaluationContext, FlagDefinition, MmapFlagStore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[derive(Debug, Deserialize)]
struct StorageFixture {
    flags_to_store: Vec<FlagDefinition>,
    audit_actions: Vec<AuditActionCase>,
}

#[derive(Debug, Deserialize)]
struct AuditActionCase {
    actor: String,
    flag_key: String,
    operation: String,
    payload: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct VerifiedStorageRecord {
    flag_key: String,
    stored_byte_len: usize,
    read_variant_from_mmap: Option<String>,
    evaluated_variant: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct StorageOutputArtifact {
    total_stored_flags: usize,
    flags: Vec<VerifiedStorageRecord>,
    total_wal_entries: usize,
    wal_keys: Vec<String>,
}

#[test]
fn test_part3_storage_against_simulated_fixtures() {
    let input_path = Path::new("tests/fixtures/storage_input.json");
    let expected_path = Path::new("tests/expected/storage_output.json");

    assert!(input_path.exists(), "Input fixture must exist");

    let raw = fs::read_to_string(input_path).expect("Failed to read fixture");
    let fixture: StorageFixture = serde_json::from_str(&raw).expect("Invalid JSON");

    // Create isolated temporary directories for heed and fjall
    let lmdb_dir = tempdir().expect("Failed to create temp dir for LMDB");
    let wal_dir = tempdir().expect("Failed to create temp dir for WAL");

    // 1. Initialize MmapFlagStore and AuditWal
    let store = MmapFlagStore::open(lmdb_dir.path()).expect("Failed to open MmapFlagStore");
    let wal = AuditWal::open(wal_dir.path()).expect("Failed to open AuditWal");

    let mut verified_flags = Vec::new();

    // 2. Persist simulated flags and verify zero-copy read
    for flag in &fixture.flags_to_store {
        store.put_flag(flag).expect("Failed to put flag");

        let rtxn = store.read_txn().expect("Failed to get read transaction");

        // Zero-copy read: gets raw byte slice mapped directly from virtual memory
        let raw_bytes = store
            .get_raw(&rtxn, &flag.key)
            .expect("Failed to read raw bytes")
            .expect("Flag must exist in store");

        let byte_len = raw_bytes.len();
        assert!(byte_len > 0, "Stored binary must be non-empty");

        // Deserialize from mmap slice
        let read_back: FlagDefinition =
            serde_json::from_slice(raw_bytes).expect("Failed to deserialize mmap slice");

        assert_eq!(read_back.key, flag.key);

        // Evaluate using the rule engine
        let mut ctx = EvaluationContext::new("user_us_01");
        ctx = ctx.with_attribute("country", edgeflag::AttributeValue::String("US".to_string()));
        ctx = ctx.with_attribute(
            "traffic_tier",
            edgeflag::AttributeValue::String("tier_1".to_string()),
        );

        let eval_res = evaluate_flag(&read_back, &ctx);

        verified_flags.push(VerifiedStorageRecord {
            flag_key: flag.key.clone(),
            stored_byte_len: byte_len,
            read_variant_from_mmap: read_back.default_variant,
            evaluated_variant: eval_res.variant,
        });
    }

    // 3. Append audit actions to LSM WAL
    let mut wal_keys = Vec::new();
    for action in &fixture.audit_actions {
        let entry = wal
            .append(&action.actor, &action.flag_key, &action.operation, &action.payload)
            .expect("Failed to append WAL entry");
        wal_keys.push(format!("{}:{}", entry.sequence_id, entry.flag_key));
    }

    wal.flush().expect("Failed to flush WAL");

    let all_wal = wal.read_all().expect("Failed to read all WAL entries");
    assert_eq!(all_wal.len(), fixture.audit_actions.len());

    let actual_artifact = StorageOutputArtifact {
        total_stored_flags: verified_flags.len(),
        flags: verified_flags,
        total_wal_entries: all_wal.len(),
        wal_keys,
    };

    if !expected_path.exists() {
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(&actual_artifact).unwrap();
        fs::write(expected_path, serialized).expect("Failed to write expected storage artifact");
    }

    let expected_raw = fs::read_to_string(expected_path).expect("Failed to read expected output");
    let expected_artifact: StorageOutputArtifact =
        serde_json::from_str(&expected_raw).expect("Invalid expected JSON");

    assert_eq!(
        actual_artifact, expected_artifact,
        "Actual storage output must strictly match expected artifact!"
    );
}
