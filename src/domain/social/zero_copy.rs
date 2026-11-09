//! src/domain/social/zero_copy.rs
//! File ID: FILE-017
//! Responsibility: Strict descending alignment zero-padding social records (<=7 words)
//! Must Never: Permit hidden compiler padding bytes or runtime heap allocations.

use engine_layout::assert_dense_layout;

/// Fixed 128-byte cache-line aligned zero-padding post record.
/// Enforces strictly descending field sizes:
/// 8B (u64) -> 8B (u64) -> 8B (u64) -> 8B (u64) -> 2B (u16) -> 2B (u16) -> 4B explicit pad -> 84B content buffer
/// Total = 8 + 8 + 8 + 8 + 2 + 2 + 4 + 84 = 128 bytes (exactly 2 cache lines).
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DensePostRecord {
    pub id: u64,             // 8B (offset 0..8)
    pub author_id: u64,      // 8B (offset 8..16)
    pub created_at_ms: u64,  // 8B (offset 16..24)
    pub likes_count: u64,    // 8B (offset 24..32)
    pub content_len: u16,    // 2B (offset 32..34)
    pub flags: u16,          // 2B (offset 34..36)
    pub reserved_pad: u32,   // 4B (offset 36..40) - explicit zero pad
    pub content: [u8; 88],   // 88B (offset 40..128) - inline static content
}

assert_dense_layout!(DensePostRecord, 128, 64);

impl Default for DensePostRecord {
    #[inline(always)]
    fn default() -> Self {
        Self {
            id: 0,
            author_id: 0,
            created_at_ms: 0,
            likes_count: 0,
            content_len: 0,
            flags: 0,
            reserved_pad: 0,
            content: [0u8; 88],
        }
    }
}

impl DensePostRecord {
    #[inline(always)]
    pub fn new(id: u64, author_id: u64, created_at_ms: u64, text: &str) -> Self {
        let bytes = text.as_bytes();
        let copy_len = bytes.len().min(88);
        let mut content = [0u8; 88];
        content[..copy_len].copy_from_slice(&bytes[..copy_len]);

        Self {
            id,
            author_id,
            created_at_ms,
            likes_count: 0,
            content_len: copy_len as u16,
            flags: 0,
            reserved_pad: 0,
            content,
        }
    }

    #[inline(always)]
    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.content[..self.content_len as usize]).unwrap_or("")
    }
}

/// 16-Byte composite inverted key for natural reverse-chronological B+Tree sorting:
/// `(u64::MAX - event_timestamp_ms, entity_id)`
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InvertedTimelineKey {
    pub inverted_ts: u64, // 8B (offset 0..8)
    pub post_id: u64,     // 8B (offset 8..16)
}

assert_dense_layout!(InvertedTimelineKey, 16, 8);

impl InvertedTimelineKey {
    #[inline(always)]
    pub fn new(timestamp_ms: u64, post_id: u64) -> Self {
        Self {
            inverted_ts: u64::MAX - timestamp_ms,
            post_id,
        }
    }

    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.inverted_ts.to_be_bytes());
        buf[8..16].copy_from_slice(&self.post_id.to_be_bytes());
        buf
    }

    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }
        let inverted_ts = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let post_id = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
        Some(Self {
            inverted_ts,
            post_id,
        })
    }
}
