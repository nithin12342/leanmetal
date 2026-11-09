use crate::barrier::SequenceBarrier;
use crate::cursor::SequenceCursor;

pub struct StagePipeline {
    pub stage1_cursor: SequenceCursor, // Disk append
    pub stage2_cursor: SequenceCursor, // Bitset ledger
    pub stage3_cursor: SequenceCursor, // Business logic
}

impl Default for StagePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl StagePipeline {
    pub fn new() -> Self {
        Self {
            stage1_cursor: SequenceCursor::new(0),
            stage2_cursor: SequenceCursor::new(0),
            stage3_cursor: SequenceCursor::new(0),
        }
    }

    pub fn barrier_for_stage1<'a>(&'a self, producer: &'a SequenceCursor) -> SequenceBarrier<'a> {
        SequenceBarrier::new(producer)
    }

    pub fn barrier_for_stage2<'a>(&'a self) -> SequenceBarrier<'a> {
        SequenceBarrier::new(&self.stage1_cursor)
    }

    pub fn barrier_for_stage3<'a>(&'a self) -> SequenceBarrier<'a> {
        SequenceBarrier::new(&self.stage2_cursor)
    }
}
