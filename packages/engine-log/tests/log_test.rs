use engine_layout::EventEnvelope;
use engine_log::{MmapStore, SegmentConfig, SegmentLog, WalBatcher};
use tempfile::tempdir;

#[test]
fn test_segment_append_and_mmap_read() {
    let dir = tempdir().unwrap();
    let config = SegmentConfig {
        segment_size: 1024 * 1024, // 1 MB for testing
    };

    let mut log = SegmentLog::open(dir.path(), config).unwrap();
    let sample_data = b"ENGINELOG_RECORD_001_HELLO_WORLD";
    let (seg_id, offset) = log.append(sample_data).unwrap();
    log.flush().unwrap();

    assert_eq!(seg_id, 0);
    assert_eq!(offset, 0);

    let seg_path = SegmentLog::segment_path(dir.path(), seg_id);
    let store = MmapStore::open(seg_path).unwrap();
    let read_slice = store.slice_at(offset, sample_data.len()).unwrap();
    assert_eq!(read_slice, sample_data);
}

#[test]
fn test_wal_batcher_flush() {
    let dir = tempdir().unwrap();
    let config = SegmentConfig {
        segment_size: 1024 * 1024,
    };
    let mut log = SegmentLog::open(dir.path(), config).unwrap();
    let mut batcher = WalBatcher::new(128);

    let envelope = EventEnvelope::default();
    let envelope_bytes = unsafe {
        core::slice::from_raw_parts(
            &envelope as *const EventEnvelope as *const u8,
            core::mem::size_of::<EventEnvelope>(),
        )
    };

    batcher.push(envelope_bytes);
    batcher.push(envelope_bytes);
    assert!(batcher.should_flush());

    let res = batcher.flush_to(&mut log).unwrap();
    assert!(res.is_some());
    assert_eq!(batcher.pending_bytes(), 0);
}
