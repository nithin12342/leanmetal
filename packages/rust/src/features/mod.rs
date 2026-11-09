//! packages/rust/src/features/mod.rs
//! Isolated Advanced Zero-Overhead Feature Modules.

pub mod kernel_bypass;
pub mod l1_cache;
pub mod singleflight;

pub use kernel_bypass::{PacketDescriptor, UmemPool, XdpDescriptorRing};
pub use l1_cache::ZeroOverheadL1Cache;
pub use singleflight::ZeroOverheadSingleflight;
