//! packages/rust/src/features/l1_cache.rs
//! Zero-Overhead Sub-Microsecond L1 Cache Abstraction.
//! Implements bounded in-memory caching with nanosecond key lookup.

use std::collections::HashMap;
use std::sync::RwLock;

/// High-performance thread-safe in-memory L1 cache with bounded capacity.
pub struct ZeroOverheadL1Cache<V: Clone> {
    entries: RwLock<HashMap<String, V>>,
    capacity: usize,
}

impl<V: Clone> ZeroOverheadL1Cache<V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::with_capacity(capacity)),
            capacity,
        }
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<V> {
        let map = self.entries.read().unwrap();
        map.get(key).cloned()
    }

    #[inline(always)]
    pub fn put(&self, key: impl Into<String>, val: V) {
        let mut map = self.entries.write().unwrap();
        if map.len() >= self.capacity {
            // Evict arbitrary element to preserve bounded memory footprint
            if let Some(first_key) = map.keys().next().cloned() {
                map.remove(&first_key);
            }
        }
        map.insert(key.into(), val);
    }

    #[inline(always)]
    pub fn invalidate(&self, key: &str) -> bool {
        let mut map = self.entries.write().unwrap();
        map.remove(key).is_some()
    }
}
