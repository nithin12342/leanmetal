use crate::assert_dense_layout;

/// Exactly 40 bytes aligned to 8 bytes.
/// Contiguous session slab entry holding network descriptor, timestamps,
/// sequence numbers, IPv4 address, and session flags with 0 hidden padding.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionSlot {
    pub socket_fd: u64,      // 8B (offset 0..8)
    pub last_rx_ns: u64,     // 8B (offset 8..16)
    pub expected_seq: u64,   // 8B (offset 16..24)
    pub session_token: u64,  // 8B (offset 24..32)
    pub ip_addr_v4: u32,     // 4B (offset 32..36)
    pub port: u16,           // 2B (offset 36..38)
    pub state_flags: u16,    // 2B (offset 38..40)
}

assert_dense_layout!(ConnectionSlot, 40, 8);

impl Default for ConnectionSlot {
    #[inline(always)]
    fn default() -> Self {
        Self {
            socket_fd: 0,
            last_rx_ns: 0,
            expected_seq: 0,
            session_token: 0,
            ip_addr_v4: 0,
            port: 0,
            state_flags: 0,
        }
    }
}
