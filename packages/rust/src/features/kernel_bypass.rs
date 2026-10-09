//! packages/rust/src/features/kernel_bypass.rs
//! Zero-Overhead Linux AF_XDP Kernel Bypass & UMEM Frame Ring Abstraction.
//! Allows direct user-space packet inspection, bypassing the Linux kernel TCP/IP network stack.

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

    #[inline(always)]
    pub fn alloc_frame(&mut self) -> Option<u64> {
        self.free_frames.pop()
    }

    #[inline(always)]
    pub fn free_frame(&mut self, addr: u64) {
        if addr as usize + self.frame_size <= self.buffer.len() {
            self.free_frames.push(addr);
        }
    }

    #[inline(always)]
    pub fn get_slice(&self, desc: &PacketDescriptor) -> &[u8] {
        let start = desc.addr as usize;
        let end = start + desc.len as usize;
        &self.buffer[start..end]
    }
}

/// In-memory AF_XDP descriptor ring simulating hardware queue rings.
pub struct XdpDescriptorRing {
    descriptors: Vec<PacketDescriptor>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
}

impl XdpDescriptorRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            descriptors: vec![
                PacketDescriptor {
                    addr: 0,
                    len: 0,
                    flags: 0,
                };
                capacity
            ],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, desc: PacketDescriptor) -> bool {
        let h = self.head.load(Ordering::Relaxed);
        let t = self.tail.load(Ordering::Acquire);
        if h.wrapping_sub(t) >= self.capacity {
            return false;
        }
        let idx = h % self.capacity;
        self.descriptors[idx] = desc;
        self.head.store(h.wrapping_add(1), Ordering::Release);
        true
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<PacketDescriptor> {
        let t = self.tail.load(Ordering::Relaxed);
        let h = self.head.load(Ordering::Acquire);
        if t == h {
            return None;
        }
        let idx = t % self.capacity;
        let desc = self.descriptors[idx];
        self.tail.store(t.wrapping_add(1), Ordering::Release);
        Some(desc)
    }
}
