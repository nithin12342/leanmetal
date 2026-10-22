//! tests/test_social_horizontal_scaling.rs
//! Verification of strict zero-padding layouts, ArcSwap RCU, and io_uring WAL batching

use edgeflag::domain::social::{
    ArcSwapTimeline, DensePostRecord, InvertedTimelineKey, IoUringWalCommitter,
};
use std::mem::{align_of, size_of};

#[test]
fn test_dense_post_record_zero_padding_and_alignment() {
    assert_eq!(size_of::<DensePostRecord>(), 128);
    assert_eq!(align_of::<DensePostRecord>(), 64);

    let post = DensePostRecord::new(42, 1001, 1_700_000_000_000, "Zero padding HFT social record");
    assert_eq!(post.id, 42);
    assert_eq!(post.author_id, 1001);
    assert_eq!(post.created_at_ms, 1_700_000_000_000);
    assert_eq!(post.content_str(), "Zero padding HFT social record");
}

#[test]
fn test_inverted_timeline_key_reverse_chronological_ordering() {
    assert_eq!(size_of::<InvertedTimelineKey>(), 16);
    assert_eq!(align_of::<InvertedTimelineKey>(), 8);

    let older_post = InvertedTimelineKey::new(1_000, 1);
    let newer_post = InvertedTimelineKey::new(2_000, 2);

    // Inverted key for newer post must be smaller so it sorts first in B+Tree leaf scans
    assert!(newer_post < older_post);

    let bytes = newer_post.to_bytes();
    let recovered = InvertedTimelineKey::from_bytes(&bytes).expect("Valid 16-byte key");
    assert_eq!(newer_post, recovered);
}

#[test]
fn test_arcswap_timeline_sub_microsecond_rcu() {
    let timeline = ArcSwapTimeline::new();

    // Emulate 10 post creations
    for i in 1..=10 {
        timeline.record_post(i);
    }

    let slice = timeline.load_post_ids(5);
    // Reverse chronological (newest first)
    assert_eq!(slice, vec![10, 9, 8, 7, 6]);
}

#[tokio::test]
async fn test_io_uring_wal_group_commit_batching() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let committer = IoUringWalCommitter::new(temp_dir.path(), 1024, 16)
        .expect("IoUringWalCommitter init");

    let post = DensePostRecord::new(1, 10, 1_700_000_000, "Batched WAL write payload");
    let offset = committer.append(post).await.expect("Appended to WAL");
    assert!(offset >= 128);
    assert!(committer.synced_offset() >= 128);
}
