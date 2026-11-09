use core::sync::atomic::{AtomicU64, Ordering};

/// Monotonically increasing 64-bit sequence cursor aligned to 64 bytes
/// to eliminate false sharing across CPU cores.
#[repr(C, align(64))]
pub struct SequenceCursor {
    value: AtomicU64,
    _pad: [u8; 56],
}

impl Default for SequenceCursor {
    fn default() -> Self {
        Self::new(0)
    }
}

impl SequenceCursor {
    pub const fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
            _pad: [0u8; 56],
        }
    }

    #[inline(always)]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }

    #[inline(always)]
    pub fn set(&self, val: u64) {
        self.value.store(val, Ordering::Release);
    }

    #[inline(always)]
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::AcqRel)
    }
}
