use crate::cursor::SequenceCursor;
use core::hint::spin_loop;

pub struct SequenceBarrier<'a> {
    dependent_cursor: &'a SequenceCursor,
}

impl<'a> SequenceBarrier<'a> {
    pub fn new(dependent_cursor: &'a SequenceCursor) -> Self {
        Self { dependent_cursor }
    }

    /// Wait until dependent_cursor >= required_seq using progressive spin-yield.
    pub fn wait_for(&self, required_seq: u64) -> u64 {
        let mut spins = 0usize;
        loop {
            let available = self.dependent_cursor.get();
            if available >= required_seq {
                return available;
            }

            if spins < 100 {
                spin_loop();
            } else if spins < 500 {
                std::thread::yield_now();
            } else {
                std::thread::sleep(std::time::Duration::from_micros(10));
            }
            spins = spins.wrapping_add(1);
        }
    }
}
