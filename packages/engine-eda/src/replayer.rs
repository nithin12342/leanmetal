use crate::projection::ProjectionStore;
use engine_layout::EventEnvelope;
use engine_log::MmapStore;
use std::io;
use std::path::Path;

pub struct SnapshotReplayer;

impl SnapshotReplayer {
    pub fn replay_segment<P: AsRef<Path>>(path: P, store: &ProjectionStore) -> io::Result<usize> {
        let mmap_store = MmapStore::open(path)?;
        let slice = mmap_store.as_slice();
        let envelope_size = core::mem::size_of::<EventEnvelope>(); // 64 bytes

        let count = slice.len() / envelope_size;
        let mut replayed = 0;

        for i in 0..count {
            let offset = i * envelope_size;
            let envelope_ptr = unsafe {
                slice.as_ptr().add(offset) as *const EventEnvelope
            };
            let envelope = unsafe { core::ptr::read_unaligned(envelope_ptr) };
            if envelope.aggregate_id != 0 {
                store.apply_event(&envelope);
                replayed += 1;
            }
        }

        Ok(replayed)
    }
}
