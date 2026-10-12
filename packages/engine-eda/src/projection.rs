use arc_swap::ArcSwap;
use engine_layout::EventEnvelope;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct ReadModel {
    pub aggregate_versions: HashMap<u64, u64>,
    pub latest_event_id: u64,
}

pub struct ProjectionStore {
    state: ArcSwap<ReadModel>,
}

impl Default for ProjectionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectionStore {
    pub fn new() -> Self {
        Self {
            state: ArcSwap::from_pointee(ReadModel::default()),
        }
    }

    #[inline(always)]
    pub fn load_view(&self) -> Arc<ReadModel> {
        self.state.load_full()
    }

    pub fn apply_event(&self, event: &EventEnvelope) {
        let current = self.state.load();
        let mut next = (**current).clone();
        next.aggregate_versions.insert(event.aggregate_id, event.revision);
        next.latest_event_id = event.event_id;
        self.state.store(Arc::new(next));
    }
}
