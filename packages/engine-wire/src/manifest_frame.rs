//! packages/engine-wire/src/manifest_frame.rs
//! Sequential zero-padding Manifesto NACK Protocol Frame.
//!
//! Formatted down to exact hardware bytes:
//! - 64-byte Header & Summary (Exactly 1 CPU Cache Line aligned to 64 bytes).
//! - Monotonically sequenced 64-bit word bitmask segments.
//! - Single-instruction bit-test-and-set (bts) and bitwise population count (popcnt).

use engine_layout::assert_dense_layout;

pub const MANIFEST_BITMAP_WORDS: usize = 64; // 64 * 64 bits = 4,096 sequence window
pub const MAX_GAPS_PER_NACK: usize = 7;

/// Gap segment representing a missing sequence window [start_seq..=end_seq].
/// Exactly 16 bytes aligned to 8 bytes.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SequenceGap {
    pub start_seq: u64, // 8B
    pub end_seq: u64,   // 8B
}

assert_dense_layout!(SequenceGap, 16, 8);

/// Fixed-size, zero-padding Manifesto Transport Frame.
/// Exactly 64 bytes (1 CPU Cache Line) aligned to 64 bytes.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestFrameHeader {
    pub base_sequence: u64,    // 8B (offset 0..8) - Starting sequence number of batch
    pub highest_sequence: u64, // 8B (offset 8..16) - Highest sequence received
    pub session_token: u64,    // 8B (offset 16..24) - Connection validation token
    pub timestamp_ns: u64,     // 8B (offset 24..32) - Transmission timestamp
    pub total_received: u32,   // 4B (offset 32..36) - Popcount of received packets
    pub total_expected: u32,   // 4B (offset 36..40) - Total packets in sequence window
    pub gap_count: u16,        // 2B (offset 40..42) - Number of missing gaps detected
    pub flags: u16,            // 2B (offset 42..44) - Protocol flags (0x01: is_nack, 0x02: complete)
    pub reserved: [u8; 20],    // 20B (offset 44..64) - Explicit zero padding to cache line
}

assert_dense_layout!(ManifestFrameHeader, 64, 64);

impl Default for ManifestFrameHeader {
    #[inline(always)]
    fn default() -> Self {
        Self {
            base_sequence: 0,
            highest_sequence: 0,
            session_token: 0,
            timestamp_ns: 0,
            total_received: 0,
            total_expected: 0,
            gap_count: 0,
            flags: 0,
            reserved: [0u8; 20],
        }
    }
}

/// Dense Manifesto NACK Payload (when gap_count > 0).
/// Exactly 128 bytes (2 CPU Cache Lines) aligned to 64 bytes.
/// Packs up to 7 explicit gaps with zero padding.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DenseNackFrame {
    pub header: ManifestFrameHeader,     // 64B
    pub gaps: [SequenceGap; MAX_GAPS_PER_NACK], // 7 * 16B = 112B (offset 64..176)
    pub reserved: [u8; 16],              // 16B (offset 176..192) - 3 cachelines total
}

assert_dense_layout!(DenseNackFrame, 192, 64);

impl Default for DenseNackFrame {
    #[inline(always)]
    fn default() -> Self {
        Self {
            header: ManifestFrameHeader::default(),
            gaps: [SequenceGap::default(); MAX_GAPS_PER_NACK],
            reserved: [0u8; 16],
        }
    }
}
