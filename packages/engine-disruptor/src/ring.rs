use crate::cursor::SequenceCursor;
use engine_layout::RingEntry;

pub struct DisruptorRing<const N: usize> {
    buffer: Box<[RingEntry; N]>,
    cursor: SequenceCursor,
    mask: usize,
}

impl<const N: usize> DisruptorRing<N> {
    pub fn new() -> Self {
        assert!(N > 0 && (N & (N - 1)) == 0, "Ring capacity N must be a power of two");
        // Allocate zeroed box without blowing up stack
        let buffer = vec![RingEntry::new(); N].into_boxed_slice();
        let boxed_array = match buffer.try_into() {
            Ok(arr) => arr,
            Err(_) => unreachable!(),
        };

        Self {
            buffer: boxed_array,
            cursor: SequenceCursor::new(0),
            mask: N - 1,
        }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        N
    }

    #[inline(always)]
    pub fn cursor(&self) -> &SequenceCursor {
        &self.cursor
    }

    #[inline(always)]
    pub fn slot(&self, sequence: u64) -> &RingEntry {
        let idx = (sequence as usize) & self.mask;
        &self.buffer[idx]
    }

    #[inline(always)]
    pub fn slot_mut(&mut self, sequence: u64) -> &mut RingEntry {
        let idx = (sequence as usize) & self.mask;
        &mut self.buffer[idx]
    }

    #[inline(always)]
    pub fn publish(&mut self, entry: RingEntry) -> u64 {
        let seq = self.cursor.get();
        let idx = (seq as usize) & self.mask;
        self.buffer[idx] = entry;
        self.cursor.set(seq + 1);
        seq
    }
}
