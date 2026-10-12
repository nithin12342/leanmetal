//! packages/manifest-udp-transport/src/lib.rs
//! Sequential Zero-Padding Manifesto UDP Transport Engine.
//!
//! Universal Slotted Architecture:
//! - Combines HTTP RPC correlation, WebSocket virtual streaming, and Webhook event delivery.
//! - Generic over any arbitrary user payload `P: ZeroCopyRecord` via `ManifestEnvelope<P>`.
//! - Enforces strict 64-byte CPU cache-line alignment with zero hidden padding.

pub mod frame;
pub mod session;
pub mod multiplexer;

pub use frame::{
    CacheLineSlot, DenseManifestNackPacket, ManifestEnvelope, ManifestUdpHeader, SequenceGap,
    TransportPattern, ZeroCopyRecord,
};
pub use session::{UdpSessionRegistry, DEFAULT_MAX_SESSIONS};
pub use multiplexer::ManifestoUdpMultiplexer;

#[cfg(test)]
mod tests {
    use super::*;
    use engine_layout::ConnectionSlot;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    #[repr(C, align(64))]
    struct MockOrderPayload {
        pub order_id: u64,
        pub user_id: u64,
        pub price: u64,
        pub quantity: u64,
        pub side: u8,
        pub _pad: [u8; 31],
    }

    impl ZeroCopyRecord for MockOrderPayload {
        const SIZE: usize = core::mem::size_of::<Self>();
        fn validate_bytes(slice: &[u8]) -> bool {
            slice.len() >= Self::SIZE
        }
    }

    #[test]
    fn test_universal_slotted_envelope() {
        assert_eq!(core::mem::size_of::<ManifestEnvelope<MockOrderPayload>>(), 128); // 64B header + 64B payload
        assert_eq!(core::mem::align_of::<ManifestEnvelope<MockOrderPayload>>(), 64);

        let mut slot = CacheLineSlot::<128>::default();
        let envelope = ManifestEnvelope::<MockOrderPayload> {
            header: ManifestUdpHeader {
                sequence: 1,
                correlation_id: 100,
                session_token: 200,
                timestamp_ns: 300,
                routing_tag: 1,
                payload_len: 64,
                flags: 0,
                pattern: TransportPattern::HttpRequest as u8,
                channel_id: 0,
                reserved: [0u8; 20],
            },
            payload: MockOrderPayload {
                order_id: 555,
                user_id: 999,
                price: 15000,
                quantity: 10,
                side: 1,
                _pad: [0u8; 31],
            },
        };

        // Write directly into contiguous raw memory
        let bytes = unsafe {
            core::slice::from_raw_parts(&envelope as *const _ as *const u8, 128)
        };
        slot.raw.copy_from_slice(bytes);

        // Read back in-place with zero memory copying
        let parsed = ManifestEnvelope::<MockOrderPayload>::from_bytes(&slot.raw).unwrap();
        assert_eq!(parsed.header.correlation_id, 100);
        assert_eq!(parsed.payload.order_id, 555);
        assert_eq!(parsed.payload.price, 15000);
    }

    #[test]
    fn test_zero_padding_header_alignment() {
        assert_eq!(core::mem::size_of::<ManifestUdpHeader>(), 64);
        assert_eq!(core::mem::align_of::<ManifestUdpHeader>(), 64);
    }

    #[test]
    fn test_dense_nack_packet_alignment() {
        assert_eq!(core::mem::size_of::<DenseManifestNackPacket>(), 192);
        assert_eq!(core::mem::align_of::<DenseManifestNackPacket>(), 64);
    }

    #[test]
    fn test_multiplexer_encode_decode_roundtrip() {
        let mux = ManifestoUdpMultiplexer::new(1000);
        let header = ManifestUdpHeader {
            sequence: 1042,
            correlation_id: 998877,
            session_token: 554433,
            timestamp_ns: 1700000000,
            routing_tag: 42,
            payload_len: 12,
            flags: 0,
            pattern: TransportPattern::HttpRequest as u8,
            channel_id: 1,
            reserved: [0u8; 20],
        };

        let payload = b"GET /flags/1";
        let mut packet_buf = [0u8; 256];

        let encoded_len = ManifestoUdpMultiplexer::encode_datagram(&header, payload, &mut packet_buf).unwrap();
        assert_eq!(encoded_len, 64 + 12);

        let (decoded_hdr, decoded_payload) = mux.decode_datagram(&packet_buf[..encoded_len]).unwrap();
        assert_eq!(decoded_hdr.sequence, 1042);
        assert_eq!(decoded_hdr.correlation_id, 998877);
        assert_eq!(decoded_hdr.session_token, 554433);
        assert_eq!(decoded_hdr.pattern, TransportPattern::HttpRequest as u8);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_session_registry_slab() {
        let mut reg = UdpSessionRegistry::new(100);
        let slot = ConnectionSlot {
            socket_fd: 1001,
            last_rx_ns: 1700000000,
            expected_seq: 10,
            session_token: 887766,
            ip_addr_v4: 0x7F000001,
            port: 9000,
            state_flags: 0,
        };

        assert!(reg.register(887766, slot));
        let retrieved = reg.get(887766).unwrap();
        assert_eq!(retrieved.socket_fd, 1001);
        assert_eq!(retrieved.expected_seq, 10);

        assert!(reg.touch(887766, 1700000050, 10));
        assert_eq!(reg.get(887766).unwrap().expected_seq, 11);

        assert!(reg.remove(887766));
        assert!(reg.get(887766).is_none());
    }
}
