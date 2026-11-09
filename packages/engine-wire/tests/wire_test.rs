use engine_layout::{ConnectionSlot, OpCode};
use engine_wire::{ManifestLedger, NackDispatcher, SessionRegistry, UnifiedCodec};

#[test]
fn test_manifest_ledger_and_nack_dispatcher() {
    let mut ledger = ManifestLedger::new(100);

    // Mark 100..109 as received
    for seq in 100..=109 {
        ledger.mark_received(seq);
    }

    // Full receipt -> 0 NACK return frames (core Manifesto zero-wire cost)
    let nacks = NackDispatcher::evaluate_manifest(&ledger, 1234, 1_000_000);
    assert_eq!(nacks, None);

    // Create ledger with missing sequence 105
    let mut ledger_gap = ManifestLedger::new(100);
    for seq in 100..=109 {
        if seq != 105 {
            ledger_gap.mark_received(seq);
        }
    }

    let nack_frame = NackDispatcher::evaluate_manifest(&ledger_gap, 1234, 1_000_000).unwrap();
    assert_eq!(nack_frame.header.gap_count, 1);
    assert_eq!(nack_frame.gaps[0].start_seq, 105);
    assert_eq!(nack_frame.gaps[0].end_seq, 105);
}

#[test]
fn test_unified_codec_roundtrip() {
    let header = UnifiedCodec::make_header(999, 123456, 77, OpCode::DirectCall, 5);
    let mut buffer = [0u8; 32];
    assert!(UnifiedCodec::encode_header(&header, &mut buffer));
    buffer[24..29].copy_from_slice(b"HELLO");

    let (decoded_hdr, payload) = UnifiedCodec::decode_header(&buffer[..29]).unwrap();
    assert_eq!(decoded_hdr.correlation_id, 999);
    assert_eq!(decoded_hdr.session_token, 123456);
    assert_eq!(decoded_hdr.routing_tag, 77);
    assert_eq!(decoded_hdr.opcode, OpCode::DirectCall as u8);
    assert_eq!(payload, b"HELLO");
}

#[test]
fn test_session_registry_slab() {
    let mut reg = SessionRegistry::new(1000);
    assert_eq!(reg.capacity(), 1000);

    let slot = ConnectionSlot {
        socket_fd: 42,
        last_rx_ns: 1000,
        expected_seq: 1,
        session_token: 888,
        ip_addr_v4: 0x7F000001,
        port: 8080,
        state_flags: 0,
    };

    assert!(reg.register(10, slot));
    let retrieved = reg.get(10, 888).unwrap();
    assert_eq!(retrieved.socket_fd, 42);
    assert_eq!(retrieved.session_token, 888);

    assert!(reg.remove(10, 888));
    assert!(reg.get(10, 888).is_none());
}
