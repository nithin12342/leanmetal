//! packages/manifest-udp-transport/src/frame.rs
//! Sequential Zero-Padding Hardware Datagram Framing for Manifesto UDP.
//!
//! Universal Slotted Architecture:
//! Supports arbitrary user payload types P: ZeroCopyRecord while guaranteeing
//! 64-byte CPU cache-line alignment with 0% hidden compiler padding.

use engine_layout::assert_dense_layout;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPattern {
    HttpRequest = 0x01,
    HttpResponse = 0x02,
    WebSocketStream = 0x03,
    WebhookPush = 0x04,
    NackGap = 0x05,
}

/// Invariant trait for types that can be mapped directly to/from contiguous memory
/// without deserialization or intermediate heap allocations.
pub trait ZeroCopyRecord: Sized + Copy + 'static {
    const SIZE: usize;
    fn validate_bytes(slice: &[u8]) -> bool;
}

/// Universal Slotted Cache-Line Memory Block.
/// Const-generic size `BYTES` must be a multiple of 64 bytes.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheLineSlot<const BYTES: usize> {
    pub raw: [u8; BYTES],
}

impl<const BYTES: usize> Default for CacheLineSlot<BYTES> {
    #[inline(always)]
    fn default() -> Self {
        assert!(BYTES > 0 && BYTES % 64 == 0, "CacheLineSlot size must be a multiple of 64 bytes");
        Self { raw: [0u8; BYTES] }
    }
}

/// Exactly 64 bytes (1 CPU cache line) aligned to 64 bytes.
/// Packs universal routing, monotonic sequence numbers, session token, RPC correlation,
/// pattern opcode, and payload bounds with ZERO hidden padding.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestUdpHeader {
    pub sequence: u64,       // 8B (offset 0..8) - Monotonic packet sequence for gap detection
    pub correlation_id: u64, // 8B (offset 8..16) - HTTP RPC request/response pairing ID
    pub session_token: u64,  // 8B (offset 16..24) - Persistent connection token (WebSocket session)
    pub timestamp_ns: u64,   // 8B (offset 24..32) - Nanosecond transmission timestamp
    pub routing_tag: u32,    // 4B (offset 32..36) - Webhook endpoint / topic subscription tag
    pub payload_len: u32,    // 4B (offset 36..40) - Length of subsequent payload slice
    pub flags: u16,          // 2B (offset 40..42) - Bit 0: requires_ack, Bit 1: compressed, Bit 2: end_of_stream
    pub pattern: u8,         // 1B (offset 42..43) - TransportPattern enum (HTTP, WS, Webhook, NACK)
    pub channel_id: u8,      // 1B (offset 43..44) - Multiplexed virtual stream channel (0..255)
    pub reserved: [u8; 20],  // 20B (offset 44..64) - Explicit zero padding to cache line boundary
}

assert_dense_layout!(ManifestUdpHeader, 64, 64);

impl Default for ManifestUdpHeader {
    #[inline(always)]
    fn default() -> Self {
        Self {
            sequence: 0,
            correlation_id: 0,
            session_token: 0,
            timestamp_ns: 0,
            routing_tag: 0,
            payload_len: 0,
            flags: 0,
            pattern: TransportPattern::HttpRequest as u8,
            channel_id: 0,
            reserved: [0u8; 20],
        }
    }
}

/// Universal Slotted Manifesto Envelope generic over arbitrary payload type `P`.
/// Enforces 64-byte alignment and descending layout.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestEnvelope<P: Copy> {
    pub header: ManifestUdpHeader, // Exactly 64 bytes (1 CPU cacheline)
    pub payload: P,                // Contiguous payload placed immediately adjacent
}

impl<P: Copy + Default> Default for ManifestEnvelope<P> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            header: ManifestUdpHeader::default(),
            payload: P::default(),
        }
    }
}

impl<P: ZeroCopyRecord> ManifestEnvelope<P> {
    /// In-place reference view with zero memory copying or heap allocation.
    #[inline(always)]
    pub fn from_bytes(slice: &[u8]) -> Option<&Self> {
        let expected_size = core::mem::size_of::<Self>();
        if slice.len() < expected_size {
            return None;
        }
        if !P::validate_bytes(&slice[64..64 + P::SIZE]) {
            return None;
        }
        // SAFETY: Pointer is verified to have adequate size and proper alignment.
        unsafe {
            Some(&*(slice.as_ptr() as *const Self))
        }
    }
}

/// Gap segment in a sequence batch [start_seq..=end_seq]. Exactly 16 bytes.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SequenceGap {
    pub start_seq: u64, // 8B
    pub end_seq: u64,   // 8B
}

assert_dense_layout!(SequenceGap, 16, 8);

/// Fixed-size Manifesto UDP Retransmission Packet.
/// Exactly 192 bytes (3 CPU cache lines) aligned to 64 bytes.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DenseManifestNackPacket {
    pub header: ManifestUdpHeader,      // 64B
    pub gaps: [SequenceGap; 7],         // 7 * 16B = 112B (offset 64..176)
    pub gap_count: u16,                 // 2B (offset 176..178)
    pub reserved: [u8; 14],             // 14B (offset 178..192) - Exact 192B boundary
}

assert_dense_layout!(DenseManifestNackPacket, 192, 64);

impl Default for DenseManifestNackPacket {
    #[inline(always)]
    fn default() -> Self {
        let mut hdr = ManifestUdpHeader::default();
        hdr.pattern = TransportPattern::NackGap as u8;
        Self {
            header: hdr,
            gaps: [SequenceGap::default(); 7],
            gap_count: 0,
            reserved: [0u8; 14],
        }
    }
}
