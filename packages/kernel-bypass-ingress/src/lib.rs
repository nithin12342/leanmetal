//! packages/kernel-bypass-ingress/src/lib.rs
//! Pure zero-dependency DMA ring buffer, contiguous user-space memory pool,
//! and descriptor recycle engine.
//!
//! Works across all operating systems (Windows, Linux, macOS, BSD, bare-metal).
//! Completely eliminates sk_buff overhead and OS copies by managing pre-pinned
//! contiguous RAM buffers directly in user space.

pub mod pool;
pub mod ring;

pub use pool::{DmaMemoryPool, PacketDescriptor, DEFAULT_FRAME_SIZE};
pub use ring::{DmaDescriptorRing, DEFAULT_RING_SIZE};
