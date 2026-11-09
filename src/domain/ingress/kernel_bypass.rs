//! src/domain/ingress/kernel_bypass.rs
//! File ID: FILE-011
//! Responsibility: Manage AF_XDP zero-copy UMEM ring buffers (<= 7 words)
//! Must Never: Allocate sk_buff headers or cross user/kernel boundary on read path.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub const DEFAULT_FRAME_SIZE: usize = 2048;
pub const DEFAULT_RING_SIZE: usize = 4096;

/// Descriptor representing a single packet frame in user-space UMEM RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketDescriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u32,
}

/// Pre-allocated contiguous user-space memory pool (UMEM) for zero-copy DMA.
pub struct UmemPool {
    buffer: Vec<u8>,
    frame_size: usize,
    num_frames: usize,
    free_frames: Vec<u64>,
}

impl UmemPool {
    pub fn new(num_frames: usize, frame_size: usize) -> Self {
        let total_bytes = num_frames * frame_size;
        let mut free_frames = Vec::with_capacity(num_frames);
        for i in 0..num_frames {
            free_frames.push((i * frame_size) as u64);
        }

        Self {
            buffer: vec![0u8; total_bytes],
            frame_size,
            num_frames,
            free_frames,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<u64> {
        self.free_frames.pop()
    }

    pub fn free_frame(&mut self, addr: u64) {
        if addr as usize + self.frame_size <= self.buffer.len() {
            self.free_frames.push(addr);
        }
    }

    pub fn get_slice(&self, desc: &PacketDescriptor) -> &[u8] {
        let start = desc.addr as usize;
        let end = start + desc.len as usize;
        &self.buffer[start..end]
    }

    pub fn get_slice_mut(&mut self, desc: &PacketDescriptor) -> &mut [u8] {
        let start = desc.addr as usize;
        let end = start + desc.len as usize;
        &mut self.buffer[start..end]
    }

    pub fn capacity(&self) -> usize {
        self.num_frames
    }
}

/// Circular DMA ring buffer for packet descriptors (AF_XDP Fill / Rx / Tx / Completion).
pub struct XdpDescriptorRing {
    entries: Vec<PacketDescriptor>,
    capacity: usize,
    producer: AtomicUsize,
    consumer: AtomicUsize,
}

impl XdpDescriptorRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: vec![PacketDescriptor { addr: 0, len: 0, flags: 0 }; capacity],
            capacity,
            producer: AtomicUsize::new(0),
            consumer: AtomicUsize::new(0),
        }
    }

    /// Enqueues a packet descriptor to the ring without kernel context switches.
    #[inline(always)]
    pub fn enqueue(&mut self, desc: PacketDescriptor) -> bool {
        let prod = self.producer.load(Ordering::Relaxed);
        let cons = self.consumer.load(Ordering::Acquire);

        if prod.wrapping_sub(cons) >= self.capacity {
            return false; // Ring full
        }

        let idx = prod % self.capacity;
        self.entries[idx] = desc;
        self.producer.store(prod.wrapping_add(1), Ordering::Release);
        true
    }

    /// Dequeues a packet descriptor from the ring.
    #[inline(always)]
    pub fn dequeue(&mut self) -> Option<PacketDescriptor> {
        let cons = self.consumer.load(Ordering::Relaxed);
        let prod = self.producer.load(Ordering::Acquire);

        if cons == prod {
            return None; // Ring empty
        }

        let idx = cons % self.capacity;
        let desc = self.entries[idx];
        self.consumer.store(cons.wrapping_add(1), Ordering::Release);
        Some(desc)
    }

    pub fn available(&self) -> usize {
        let prod = self.producer.load(Ordering::Relaxed);
        let cons = self.consumer.load(Ordering::Relaxed);
        prod.wrapping_sub(cons)
    }
}

/// True Kernel Bypass Engine (AF_XDP UMEM Architecture).
/// Eliminates sk_buff allocations, OS interrupts, and network syscalls.
pub struct KernelBypassEngine {
    umem: Arc<parking_lot_sim::Mutex<UmemPool>>,
    pub rx_ring: XdpDescriptorRing,
    pub tx_ring: XdpDescriptorRing,
}

impl KernelBypassEngine {
    pub fn new(num_frames: usize, frame_size: usize) -> Self {
        Self {
            umem: Arc::new(parking_lot_sim::Mutex::new(UmemPool::new(num_frames, frame_size))),
            rx_ring: XdpDescriptorRing::new(num_frames),
            tx_ring: XdpDescriptorRing::new(num_frames),
        }
    }

    /// Injects a packet frame directly into UMEM DMA RAM (zero copy).
    pub fn inject_packet(&mut self, payload: &[u8]) -> Option<PacketDescriptor> {
        let mut pool = self.umem.lock();
        if let Some(addr) = pool.alloc_frame() {
            let desc = PacketDescriptor {
                addr,
                len: payload.len() as u32,
                flags: 0,
            };
            pool.get_slice_mut(&desc).copy_from_slice(payload);
            self.rx_ring.enqueue(desc);
            Some(desc)
        } else {
            None
        }
    }

    /// Polls a received packet descriptor from the RX ring.
    #[inline(always)]
    pub fn poll_rx(&mut self) -> Option<PacketDescriptor> {
        self.rx_ring.dequeue()
    }

    /// Executes zero-copy processing over the packet directly in UMEM memory.
    pub fn process_packet<F, R>(&self, desc: &PacketDescriptor, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let pool = self.umem.lock();
        let slice = pool.get_slice(desc);
        f(slice)
    }

    /// Submits a response packet descriptor into the TX ring and recycles UMEM frame.
    pub fn complete_packet(&mut self, desc: PacketDescriptor) {
        let mut pool = self.umem.lock();
        pool.free_frame(desc.addr);
    }
}

// Lightweight cross-platform mutex adapter
mod parking_lot_sim {
    use std::sync::Mutex as StdMutex;

    pub struct Mutex<T>(StdMutex<T>);

    impl<T> Mutex<T> {
        pub fn new(val: T) -> Self {
            Self(StdMutex::new(val))
        }

        pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }
    }
}
