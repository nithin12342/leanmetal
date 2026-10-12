#![no_std]

pub mod ring_entry;
pub mod connection_slot;
pub mod frame_header;
pub mod event_envelope;

pub use ring_entry::RingEntry;
pub use connection_slot::ConnectionSlot;
pub use frame_header::{UnifiedFrameHeader, OpCode};
pub use event_envelope::EventEnvelope;

/// Compile-time constant assertion macro verifying that a type has the exact
/// specified byte size and memory alignment with zero hidden padding.
#[macro_export]
macro_rules! assert_dense_layout {
    ($t:ty, $size:expr, $align:expr) => {
        const _: () = {
            assert!(
                core::mem::size_of::<$t>() == $size,
                "Dense memory layout violation: struct size mismatch!"
            );
            assert!(
                core::mem::align_of::<$t>() == $align,
                "Dense memory layout violation: struct alignment mismatch!"
            );
        };
    };
}
