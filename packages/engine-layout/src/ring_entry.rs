use crate::assert_dense_layout;

/// Exactly 64 bytes (1 CPU cacheline) aligned to 64 bytes.
/// Enforces strictly descending field sizes (u64 -> u32 -> u16 -> u8 -> explicit trailing padding)
/// to eliminate any hidden compiler padding.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RingEntry {
    pub sequence: u64,       // 8B (offset 0..8)
    pub timestamp_ns: u64,   // 8B (offset 8..16)
    pub session_id: u64,     // 8B (offset 16..24)
    pub payload_ptr: u64,    // 8B (offset 24..32)
    pub payload_len: u32,    // 4B (offset 32..36)
    pub routing_tag: u32,    // 4B (offset 36..40)
    pub flags: u16,          // 2B (offset 40..42)
    pub event_type: u16,     // 2B (offset 42..44)
    pub opcode: u8,          // 1B (offset 44..45)
    pub reserved: [u8; 19],  // 19B (offset 45..64)
}

assert_dense_layout!(RingEntry, 64, 64);

impl Default for RingEntry {
    #[inline(always)]
    fn default() -> Self {
        Self {
            sequence: 0,
            timestamp_ns: 0,
            session_id: 0,
            payload_ptr: 0,
            payload_len: 0,
            routing_tag: 0,
            flags: 0,
            event_type: 0,
            opcode: 0,
            reserved: [0u8; 19],
        }
    }
}

impl RingEntry {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            sequence: 0,
            timestamp_ns: 0,
            session_id: 0,
            payload_ptr: 0,
            payload_len: 0,
            routing_tag: 0,
            flags: 0,
            event_type: 0,
            opcode: 0,
            reserved: [0u8; 19],
        }
    }
}
