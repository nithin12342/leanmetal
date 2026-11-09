use crate::segment::SegmentLog;
use std::io;

pub struct WalBatcher {
    buffer: Vec<u8>,
    batch_threshold: usize,
}

impl WalBatcher {
    pub fn new(batch_threshold: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(batch_threshold * 2),
            batch_threshold,
        }
    }

    #[inline]
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    #[inline]
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn should_flush(&self) -> bool {
        self.buffer.len() >= self.batch_threshold
    }

    pub fn flush_to(&mut self, log: &mut SegmentLog) -> io::Result<Option<(u64, usize)>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        let (seg_id, offset) = log.append(&self.buffer)?;
        log.flush()?;
        self.buffer.clear();
        Ok(Some((seg_id, offset)))
    }
}
