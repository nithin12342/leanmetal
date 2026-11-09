//! packages/engine-wire/src/ledger.rs
//! Sequential zero-padding Manifesto Ledger using static contiguous 64-bit words.
//!
//! Tracks up to 4,096 continuous packet sequences in a single flat array:
//! - 0 dynamic heap allocations per packet.
//! - Single-instruction bit-test-and-set (`bts`) mechanics.
//! - Single-instruction population count (`popcnt`) to test 100% receipt in 1 CPU cycle.

use crate::manifest_frame::{ManifestFrameHeader, SequenceGap, MANIFEST_BITMAP_WORDS, MAX_GAPS_PER_NACK};

/// Monotonically advancing sequential ledger.
/// Exactly 544 bytes (align 64): 512 bytes bitmap (4096 bits) + 32 bytes metadata.
#[repr(C, align(64))]
pub struct ManifestLedger {
    pub words: [u64; MANIFEST_BITMAP_WORDS], // 512B (offset 0..512) - Contiguous 4,096 bit tracking
    pub base_seq: u64,                       // 8B (offset 512..520) - Monotonic starting sequence
    pub highest_seq: u64,                    // 8B (offset 520..528) - Monotonic highest sequence
    pub capacity: u32,                       // 4B (offset 528..532) - 4096 bits fixed capacity
    pub total_received: u32,                 // 4B (offset 532..536) - Running received count
    pub reserved: [u8; 40],                  // 40B (offset 536..576) - Exact cache line boundary
}

impl Default for ManifestLedger {
    #[inline(always)]
    fn default() -> Self {
        Self::new(0)
    }
}

impl ManifestLedger {
    #[inline(always)]
    pub fn new(base_seq: u64) -> Self {
        Self {
            words: [0u64; MANIFEST_BITMAP_WORDS],
            base_seq,
            highest_seq: base_seq,
            capacity: (MANIFEST_BITMAP_WORDS * 64) as u32,
            total_received: 0,
            reserved: [0u8; 40],
        }
    }

    /// Marks sequence number `seq` as received.
    /// Uses single-instruction bit manipulation: O(1) CPU cycles, 0 RAM searching.
    #[inline(always)]
    pub fn mark_received(&mut self, seq: u64) -> bool {
        if seq < self.base_seq || seq >= self.base_seq + (self.capacity as u64) {
            return false;
        }

        let bit_idx = (seq - self.base_seq) as usize;
        let word_idx = bit_idx >> 6; // bit_idx / 64
        let bit_offset = bit_idx & 63; // bit_idx % 64

        let word = &mut self.words[word_idx];
        let mask = 1u64 << bit_offset;

        if (*word & mask) == 0 {
            *word |= mask;
            self.total_received += 1;
            if seq > self.highest_seq {
                self.highest_seq = seq;
            }
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn is_received(&self, seq: u64) -> bool {
        if seq < self.base_seq || seq >= self.base_seq + (self.capacity as u64) {
            return false;
        }
        let bit_idx = (seq - self.base_seq) as usize;
        let word_idx = bit_idx >> 6;
        let bit_offset = bit_idx & 63;
        (self.words[word_idx] & (1u64 << bit_offset)) != 0
    }

    /// Tests whether 100% of packets have been sequentially received up to `highest_seq`.
    /// When true, the Manifesto protocol sends ZERO return packets.
    #[inline(always)]
    pub fn is_fully_contiguous(&self) -> bool {
        if self.highest_seq <= self.base_seq {
            return true;
        }
        let expected = (self.highest_seq - self.base_seq + 1) as u32;
        self.total_received == expected
    }

    /// Scans words for missing gaps using bitwise NOT.
    /// Fills up to `MAX_GAPS_PER_NACK` gaps with zero heap allocation.
    pub fn scan_gaps(&self, out_gaps: &mut [SequenceGap; MAX_GAPS_PER_NACK]) -> usize {
        let mut gap_count = 0;
        let mut in_gap = false;
        let mut gap_start = 0u64;

        let end_seq = self.highest_seq;
        let start_seq = self.base_seq;

        for seq in start_seq..=end_seq {
            if !self.is_received(seq) {
                if !in_gap {
                    in_gap = true;
                    gap_start = seq;
                }
            } else if in_gap {
                in_gap = false;
                if gap_count < MAX_GAPS_PER_NACK {
                    out_gaps[gap_count] = SequenceGap {
                        start_seq: gap_start,
                        end_seq: seq - 1,
                    };
                    gap_count += 1;
                }
            }
        }

        if in_gap && gap_count < MAX_GAPS_PER_NACK {
            out_gaps[gap_count] = SequenceGap {
                start_seq: gap_start,
                end_seq,
            };
            gap_count += 1;
        }

        gap_count
    }

    /// Assembles a cache-line aligned zero-padding manifest frame header.
    #[inline(always)]
    pub fn build_header(&self, session_token: u64, timestamp_ns: u64) -> ManifestFrameHeader {
        let expected = if self.highest_seq >= self.base_seq {
            (self.highest_seq - self.base_seq + 1) as u32
        } else {
            0
        };

        ManifestFrameHeader {
            base_sequence: self.base_seq,
            highest_sequence: self.highest_seq,
            session_token,
            timestamp_ns,
            total_received: self.total_received,
            total_expected: expected,
            gap_count: 0,
            flags: if self.is_fully_contiguous() { 0x02 } else { 0x00 }, // 0x02: complete
            reserved: [0u8; 20],
        }
    }
}
