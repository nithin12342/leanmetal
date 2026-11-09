//! src/domain/ingress/mod.rs
//! Ingress, L1 caching, and WebSocket connection ring

pub mod af_xdp;
pub mod backpressure;
pub mod connection_ring;
pub mod kernel_bypass;
pub mod l1_cache;

pub use af_xdp::{AfXdpConfig, AfXdpError, NativeXskEngine, XdpMode, XdpStats};

pub use backpressure::{BackpressureHub, ClientSession, PushResult};
pub use connection_ring::{ClientGuard, ConnectionRing};
pub use kernel_bypass::{KernelBypassEngine, PacketDescriptor, UmemPool, XdpDescriptorRing};
pub use l1_cache::{L1StaticCache, MokaL1Cache};

