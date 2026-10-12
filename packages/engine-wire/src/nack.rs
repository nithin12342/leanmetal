//! packages/engine-wire/src/nack.rs
//! Zero-allocation Manifesto NACK Evaluator and Dispatcher.
//!
//! Enforces the core Manifesto protocol invariant:
//! 1. 100% Contiguous Receipt -> Return `None` (0 packets emitted on wire, 0 bytes moved).
//! 2. Detected Gap -> Encodes dense, cache-line aligned `DenseNackFrame` (192 bytes, 0 padding).

use crate::ledger::ManifestLedger;
use crate::manifest_frame::{DenseNackFrame, SequenceGap, MAX_GAPS_PER_NACK};

pub struct NackDispatcher;

impl NackDispatcher {
    /// Evaluates the ledger state.
    /// If all packets up to `highest_seq` are sequentially present, returns None.
    /// If missing gaps exist, populates a zero-padding `DenseNackFrame` without heap allocation.
    #[inline(always)]
    pub fn evaluate_manifest(
        ledger: &ManifestLedger,
        session_token: u64,
        timestamp_ns: u64,
    ) -> Option<DenseNackFrame> {
        if ledger.is_fully_contiguous() {
            // Core Manifesto invariant: Zero wire frames generated on 100% receipt!
            return None;
        }

        let mut gaps = [SequenceGap::default(); MAX_GAPS_PER_NACK];
        let gap_count = ledger.scan_gaps(&mut gaps);

        if gap_count == 0 {
            return None;
        }

        let mut header = ledger.build_header(session_token, timestamp_ns);
        header.gap_count = gap_count as u16;
        header.flags = 0x01; // 0x01: is_nack

        Some(DenseNackFrame {
            header,
            gaps,
            reserved: [0u8; 16],
        })
    }
}
