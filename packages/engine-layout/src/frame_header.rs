use crate::assert_dense_layout;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpCode {
    DirectCall = 0x01,
    DirectReply = 0x02,
    DuplexStream = 0x03,
    EventNotify = 0x04,
    NackGap = 0x05,
}

/// Exactly 24 bytes aligned to 8 bytes.
/// Network envelope combining RPC correlation, persistent session token,
/// routing tag, and 1-byte opcode with zero padding.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnifiedFrameHeader {
    pub correlation_id: u64, // 8B (offset 0..8)
    pub session_token: u64,  // 8B (offset 8..16)
    pub routing_tag: u32,    // 4B (offset 16..20)
    pub payload_len: u16,    // 2B (offset 20..22)
    pub opcode: u8,          // 1B (offset 22..23)
    pub flags: u8,           // 1B (offset 23..24)
}

assert_dense_layout!(UnifiedFrameHeader, 24, 8);

impl Default for UnifiedFrameHeader {
    #[inline(always)]
    fn default() -> Self {
        Self {
            correlation_id: 0,
            session_token: 0,
            routing_tag: 0,
            payload_len: 0,
            opcode: 0,
            flags: 0,
        }
    }
}
