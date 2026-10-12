use core::mem::{align_of, size_of};
use engine_layout::{ConnectionSlot, EventEnvelope, RingEntry, UnifiedFrameHeader};

#[test]
fn test_ring_entry_dense_layout() {
    assert_eq!(size_of::<RingEntry>(), 64, "RingEntry must be exactly 64 bytes");
    assert_eq!(align_of::<RingEntry>(), 64, "RingEntry must be 64-byte cacheline aligned");
    let entry = RingEntry::new();
    assert_eq!(entry.sequence, 0);
}

#[test]
fn test_connection_slot_dense_layout() {
    assert_eq!(size_of::<ConnectionSlot>(), 40, "ConnectionSlot must be exactly 40 bytes");
    assert_eq!(align_of::<ConnectionSlot>(), 8, "ConnectionSlot must be 8-byte aligned");
}

#[test]
fn test_unified_frame_header_dense_layout() {
    assert_eq!(size_of::<UnifiedFrameHeader>(), 24, "UnifiedFrameHeader must be exactly 24 bytes");
    assert_eq!(align_of::<UnifiedFrameHeader>(), 8, "UnifiedFrameHeader must be 8-byte aligned");
}

#[test]
fn test_event_envelope_dense_layout() {
    assert_eq!(size_of::<EventEnvelope>(), 64, "EventEnvelope must be exactly 64 bytes");
    assert_eq!(align_of::<EventEnvelope>(), 64, "EventEnvelope must be 64-byte cacheline aligned");
}
