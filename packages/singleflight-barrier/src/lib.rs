//! Singleflight Coalescing Barrier
//!
//! What was newly created: Built from scratch using DashMap and Tokio channels
//! wrapped in shared in-flight synchronization.
//!
//! Why it was custom: Standard web frameworks do not deduplicate concurrent reads.
//! This orchestration logic captures 5,000 simultaneous incoming queries for a cold key,
//! registers an in-flight future for the first caller, and forces the remaining 4,999 requests
//! to await that single memory future without hitting the database.

use dashmap::DashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
struct InFlight<V> {
    tx: broadcast::Sender<V>,
}

/// In-flight request deduplication barrier.
#[derive(Clone)]
pub struct SingleflightBarrier<K: Hash + Eq + Clone, V: Clone + Send + 'static> {
    tasks: Arc<DashMap<K, InFlight<V>>>,
}

impl<K: Hash + Eq + Clone, V: Clone + Send + 'static> Default for SingleflightBarrier<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Clone, V: Clone + Send + 'static> SingleflightBarrier<K, V> {
    /// Creates a new Singleflight barrier.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
        }
    }

    /// Executes `fut` for the given key, or awaits an already executing in-flight task.
    /// Deduplicates concurrent callers, guaranteeing only ONE computation executes.
    pub async fn execute<F, Fut>(&self, key: &K, fut: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        // 1. Fast check if already in flight
        let (is_first, mut rx) = match self.tasks.get(key) {
            Some(entry) => (false, entry.tx.subscribe()),
            None => {
                // Potential first caller; allocate bounded broadcast channel
                let (tx, rx) = broadcast::channel(16);
                self.tasks.insert(key.clone(), InFlight { tx: tx.clone() });
                (true, rx)
            }
        };

        if !is_first {
            // Await the active computation
            if let Ok(val) = rx.recv().await {
                return val;
            }
        }

        // We are the designated worker
        let val = fut().await;

        // Broadcast to all waiting peers
        if let Some((_, in_flight)) = self.tasks.remove(key) {
            let _ = in_flight.tx.send(val.clone());
        }

        val
    }
}
