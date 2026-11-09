//! packages/kernel-bypass-ingress/src/ring.rs
//! Circular lock-free DMA descriptor ring.

use crate::pool::PacketDescriptor;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const DEFAULT_RING_SIZE: usize = 4096;

pub struct DmaDescriptorRing {
    entries: Box<[PacketDescriptor]>,
    capacity: usize,
    producer: AtomicUsize,
    consumer: AtomicUsize,
    mask: usize,
}

impl DmaDescriptorRing {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0 && (capacity & (capacity - 1)) == 0, "Capacity must be power of two");
        Self {
            entries: vec![PacketDescriptor::default(); capacity].into_boxed_slice(),
            capacity,
            producer: AtomicUsize::new(0),
            consumer: AtomicUsize::new(0),
            mask: capacity - 1,
        }
    }

    #[inline(always)]
    pub fn enqueue(&mut self, desc: PacketDescriptor) -> bool {
        let prod = self.producer.load(Ordering::Relaxed);
        let cons = self.consumer.load(Ordering::Acquire);

        if prod.wrapping_sub(cons) >= self.capacity {
            return false;
        }

        let idx = prod & self.mask;
        self.entries[idx] = desc;
        self.producer.store(prod.wrapping_add(1), Ordering::Release);
        true
    }

    #[inline(always)]
    pub fn dequeue(&mut self) -> Option<PacketDescriptor> {
        let cons = self.consumer.load(Ordering::Relaxed);
        let prod = self.producer.load(Ordering::Acquire);

        if cons == prod {
            return None;
        }

        let idx = cons & self.mask;
        let desc = self.entries[idx];
        self.consumer.store(cons.wrapping_add(1), Ordering::Release);
        Some(desc)
    }

    #[inline(always)]
    pub fn available(&self) -> usize {
        let prod = self.producer.load(Ordering::Relaxed);
        let cons = self.consumer.load(Ordering::Relaxed);
        prod.wrapping_sub(cons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::*;

    #[test]
    fn test_dma_pool_and_ring() {
        let mut pool = DmaMemoryPool::new(16, 2048);
        let mut ring = DmaDescriptorRing::new(16);

        let addr = pool.alloc_frame().unwrap();
        let desc = PacketDescriptor {
            addr,
            len: 128,
            flags: 0,
        };

        pool.get_slice_mut(&desc)[0..4].copy_from_slice(b"PING");
        assert!(ring.enqueue(desc));

        let popped = ring.dequeue().unwrap();
        assert_eq!(&pool.get_slice(&popped)[0..4], b"PING");
        pool.free_frame(popped.addr);
    }
}
