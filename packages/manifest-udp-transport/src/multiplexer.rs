//! packages/manifest-udp-transport/src/multiplexer.rs
//! Unified UDP Datagram Codec & Multiplexer.
//!
//! Blends the 3 dominant communication patterns into a single zero-copy datagram:
//! 1. HTTP Semantics: Independent request/response multiplexing via `correlation_id`.
//! 2. WebSocket Semantics: Persistent session streaming without TCP Head-of-Line stalls.
//! 3. Webhook Semantics: High-throughput event delivery verified via Manifest NACKs.

use crate::frame::{DenseManifestNackPacket, ManifestUdpHeader, SequenceGap, TransportPattern};
use crate::session::UdpSessionRegistry;

pub struct ManifestoUdpMultiplexer {
    pub registry: UdpSessionRegistry,
}

impl Default for ManifestoUdpMultiplexer {
    fn default() -> Self {
        Self::new(100_000)
    }
}

impl ManifestoUdpMultiplexer {
    pub const HEADER_LEN: usize = core::mem::size_of::<ManifestUdpHeader>(); // 64 bytes

    pub fn new(capacity: usize) -> Self {
        Self {
            registry: UdpSessionRegistry::new(capacity),
        }
    }

    /// Decodes a Manifesto UDP datagram in-place with zero memory allocation.
    /// Returns the 64-byte typed header and an immutable slice pointing directly into the UDP packet payload.
    #[inline(always)]
    pub fn decode_datagram<'a>(&self, packet: &'a [u8]) -> Option<(ManifestUdpHeader, &'a [u8])> {
        if packet.len() < Self::HEADER_LEN {
            return None;
        }

        let header = unsafe {
            core::ptr::read_unaligned(packet.as_ptr() as *const ManifestUdpHeader)
        };

        let payload_len = header.payload_len as usize;
        if packet.len() < Self::HEADER_LEN + payload_len {
            return None;
        }

        let payload = &packet[Self::HEADER_LEN..Self::HEADER_LEN + payload_len];
        Some((header, payload))
    }

    /// Encodes a Manifesto UDP datagram directly into a pre-allocated packet buffer.
    /// Copies 0 extra bytes—places the 64-byte header and payload contiguously.
    #[inline(always)]
    pub fn encode_datagram(
        header: &ManifestUdpHeader,
        payload: &[u8],
        dst_packet_buffer: &mut [u8],
    ) -> Option<usize> {
        let total_len = Self::HEADER_LEN + payload.len();
        if dst_packet_buffer.len() < total_len {
            return None;
        }

        // Copy 64-byte header directly
        let hdr_bytes = unsafe {
            core::slice::from_raw_parts(
                header as *const ManifestUdpHeader as *const u8,
                Self::HEADER_LEN,
            )
        };
        dst_packet_buffer[..Self::HEADER_LEN].copy_from_slice(hdr_bytes);

        // Copy payload immediately contiguous to header
        if !payload.is_empty() {
            dst_packet_buffer[Self::HEADER_LEN..total_len].copy_from_slice(payload);
        }

        Some(total_len)
    }

    /// Evaluates if an incoming packet is an HTTP-style RPC request.
    #[inline(always)]
    pub fn is_http_request(header: &ManifestUdpHeader) -> bool {
        header.pattern == TransportPattern::HttpRequest as u8
    }

    /// Evaluates if an incoming packet belongs to a continuous WebSocket-style stream.
    #[inline(always)]
    pub fn is_websocket_stream(header: &ManifestUdpHeader) -> bool {
        header.pattern == TransportPattern::WebSocketStream as u8
    }

    /// Evaluates if an incoming packet is a Webhook push notification.
    #[inline(always)]
    pub fn is_webhook_push(header: &ManifestUdpHeader) -> bool {
        header.pattern == TransportPattern::WebhookPush as u8
    }

    /// Evaluates if an incoming packet is a NACK gap frame.
    #[inline(always)]
    pub fn is_nack_gap(header: &ManifestUdpHeader) -> bool {
        header.pattern == TransportPattern::NackGap as u8
    }

    /// Generates a NACK response packet directly into buffer when missing sequence gaps are detected.
    pub fn build_nack_packet(
        session_token: u64,
        gaps: &[SequenceGap],
        timestamp_ns: u64,
    ) -> DenseManifestNackPacket {
        let mut packet = DenseManifestNackPacket::default();
        packet.header.session_token = session_token;
        packet.header.timestamp_ns = timestamp_ns;
        packet.header.flags = 0x01; // is_nack

        let count = gaps.len().min(7);
        for i in 0..count {
            packet.gaps[i] = gaps[i];
        }
        packet.gap_count = count as u16;
        packet
    }
}
