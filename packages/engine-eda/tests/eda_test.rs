use engine_eda::{CommandIntent, CommandProcessor, MokaL1Cache, ProjectionStore, SnapshotReplayer};
use engine_layout::EventEnvelope;
use engine_log::{SegmentConfig, SegmentLog};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_command_processor_and_projection() {
    let intent = CommandIntent {
        aggregate_id: 101,
        expected_revision: 0,
        event_type: 1,
        origin_node_id: 42,
        payload: [1u8; 16],
    };

    let envelope = CommandProcessor::process_command(intent, 5001, 1_000_000).unwrap();
    assert_eq!(envelope.aggregate_id, 101);
    assert_eq!(envelope.revision, 1);

    let store = ProjectionStore::new();
    store.apply_event(&envelope);

    let view = store.load_view();
    assert_eq!(view.latest_event_id, 5001);
    assert_eq!(*view.aggregate_versions.get(&101).unwrap(), 1);
}

#[test]
fn test_snapshot_replayer_from_segment() {
    let dir = tempdir().unwrap();
    let config = SegmentConfig {
        segment_size: 1024 * 1024,
    };
    let mut log = SegmentLog::open(dir.path(), config).unwrap();

    let envelope = EventEnvelope {
        event_id: 99,
        aggregate_id: 555,
        revision: 3,
        timestamp_ns: 2000,
        schema_version: 1,
        payload_len: 16,
        origin_node_id: 1,
        event_type: 2,
        flags: 0,
        payload_inline: [7u8; 16],
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &envelope as *const EventEnvelope as *const u8,
            core::mem::size_of::<EventEnvelope>(),
        )
    };
    log.append(bytes).unwrap();
    log.flush().unwrap();

    let seg_path = SegmentLog::segment_path(dir.path(), 0);
    let store = ProjectionStore::new();
    let replayed = SnapshotReplayer::replay_segment(seg_path, &store).unwrap();
    assert!(replayed >= 1);

    let view = store.load_view();
    assert_eq!(*view.aggregate_versions.get(&555).unwrap(), 3);
}

#[test]
fn test_moka_l1_cache() {
    let cache = MokaL1Cache::default();
    let data: Arc<[u8]> = Arc::from(b"CACHED_VALUE".as_ref());
    cache.insert("flag_alpha".to_string(), data.clone());

    let hit = cache.get("flag_alpha").unwrap();
    assert_eq!(&*hit, b"CACHED_VALUE");

    cache.invalidate("flag_alpha");
    assert!(cache.get("flag_alpha").is_none());
}
