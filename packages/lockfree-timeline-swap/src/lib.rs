//! Lock-Free Mutable Timeline Replacement
//!
//! What was newly created: A custom state-update routine in create_post_handler.
//!
//! Why it was custom: Rather than protecting the global feed with a tokio::sync::RwLock or Mutex
//! (which would force readers to wait behind write locks), this mechanism implements copy-on-write
//! state replacement: the worker clones the existing feed snapshot, inserts the new record,
//! bounds the size to 50 items, and replaces the pointer using `ArcSwap::store()`. Readers execute
//! uninterrupted with zero lock contention.

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Generic timeline item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineItem<T: Clone> {
    pub id: u64,
    pub timestamp: u64,
    pub data: T,
}

/// Lock-free bounded timeline container utilizing atomic pointer swap.
pub struct LockFreeTimeline<T: Clone + Send + Sync + 'static> {
    snapshot: ArcSwap<Vec<TimelineItem<T>>>,
    max_items: usize,
}

impl<T: Clone + Send + Sync + 'static> LockFreeTimeline<T> {
    /// Creates a new lock-free timeline with bounded capacity (e.g. 50 items).
    pub fn new(max_items: usize) -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(Vec::with_capacity(max_items)),
            max_items,
        }
    }

    /// Reads the current snapshot without acquiring read locks or stalling writers.
    #[inline(always)]
    pub fn get_snapshot(&self) -> Arc<Vec<TimelineItem<T>>> {
        self.snapshot.load_full()
    }

    /// Appends a new item via Copy-On-Write pointer replacement.
    /// Readers never block while mutation takes place.
    pub fn push(&self, item: TimelineItem<T>) {
        let current = self.snapshot.load();
        let mut new_vec = Vec::with_capacity(self.max_items);

        // Prepend new item (reverse chronological)
        new_vec.push(item);

        // Copy existing items up to max capacity - 1
        for old in current.iter().take(self.max_items - 1) {
            new_vec.push(old.clone());
        }

        // Atomically replace global snapshot pointer
        self.snapshot.store(Arc::new(new_vec));
    }
}
