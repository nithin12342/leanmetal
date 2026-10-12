//! packages/engine-wire/src/lib.rs
//! Core Manifesto-Driven Transport Protocol Engine with Sequential Zero-Padding Architecture.

pub mod manifest_frame;
pub mod ledger;
pub mod nack;
pub mod codec;
pub mod registry;

pub use manifest_frame::{DenseNackFrame, ManifestFrameHeader, SequenceGap, MAX_GAPS_PER_NACK};
pub use ledger::ManifestLedger;
pub use nack::NackDispatcher;
pub use codec::UnifiedCodec;
pub use registry::SessionRegistry;
