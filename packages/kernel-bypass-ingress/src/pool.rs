//! packages/kernel-bypass-ingress/src/pool.rs
//! Contiguous user-space DMA memory pool with zero runtime allocations.

pub const DEFAULT_FRAME_SIZE: usize = 2048;

/// Hardware descriptor referencing an exact frame offset in contiguous RAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct PacketDescriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u32,
}

/// Contiguous DMA RAM pool allocated once upfront.
pub struct DmaMemoryPool {
    buffer: Box<[u8]>,
    frame_size: usize,
    num_frames: usize,
    free_frames: Vec<u64>,
}

impl DmaMemoryPool {
    pub fn new(num_frames: usize, frame_size: usize) -> Self {
        let total_bytes = num_frames * frame_size;
        let mut free_frames = Vec::with_capacity(num_frames);
        for i in 0..num_frames {
            free_frames.push((i * frame_size) as u64);
        }

        Self {
            buffer: vec![0u8; total_bytes].into_boxed_slice(),
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
        if (addr as usize) + self.frame_size <= self.buffer.len() {
            self.free_frames.push(addr);
        }
    }

    #[inline(always)]
    pub fn get_slice(&self, desc: &PacketDescriptor) -> &[u8] {
        let start = desc.addr as usize;
        let end = start + desc.len as usize;
        &self.buffer[start..end]
    }

    #[inline(always)]
    pub fn get_slice_mut(&mut self, desc: &PacketDescriptor) -> &mut [u8] {
        let start = desc.addr as usize;
        let end = start + desc.len as usize;
        &mut self.buffer[start..end]
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.num_frames
    }
}
