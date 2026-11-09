use crate::assert_dense_layout;

/// Exactly 64 bytes aligned to 64 bytes (1 CPU cacheline).
/// Packed domain event structure for state changes with strictly descending alignments.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: u64,            // 8B (offset 0..8)
    pub aggregate_id: u64,        // 8B (offset 8..16)
    pub revision: u64,            // 8B (offset 16..24)
    pub timestamp_ns: u64,        // 8B (offset 24..32)
    pub schema_version: u32,      // 4B (offset 32..36)
    pub payload_len: u32,         // 4B (offset 36..40)
    pub origin_node_id: u32,      // 4B (offset 40..44)
    pub event_type: u16,          // 2B (offset 44..46)
    pub flags: u16,               // 2B (offset 46..48)
    pub payload_inline: [u8; 16], // 16B (offset 48..64)
}

assert_dense_layout!(EventEnvelope, 64, 64);

impl Default for EventEnvelope {
    #[inline(always)]
    fn default() -> Self {
        Self {
            event_id: 0,
            aggregate_id: 0,
            revision: 0,
            timestamp_ns: 0,
            schema_version: 0,
            payload_len: 0,
            origin_node_id: 0,
            event_type: 0,
            flags: 0,
            payload_inline: [0u8; 16],
        }
    }
}
