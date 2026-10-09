//! packages/rust/src/features/singleflight.rs
//! Zero-Overhead Stampede Coalescing Abstraction.
//! Synchronizes concurrent hot evaluations to prevent duplicate computation spikes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type SharedResult<T> = Arc<Mutex<Option<T>>>;

/// In-process request coalescer preventing thundering herd problems.
pub struct ZeroOverheadSingleflight<T: Clone> {
    in_flight: Mutex<HashMap<String, SharedResult<T>>>,
}

impl<T: Clone> ZeroOverheadSingleflight<T> {
    pub fn new() -> Self {
        Self {
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Executes `calc_fn` only once for concurrent requests sharing the same `key`.
    pub fn execute<F>(&self, key: &str, calc_fn: F) -> T
    where
        F: FnOnce() -> T,
    {
        let slot = {
            let mut map = self.in_flight.lock().unwrap();
            if let Some(existing) = map.get(key) {
                existing.clone()
            } else {
                let new_slot = Arc::new(Mutex::new(None));
                map.insert(key.to_string(), new_slot.clone());
                new_slot
            }
        };

        // If slot is empty, this thread computes the value
        let mut val_lock = slot.lock().unwrap();
        if let Some(ref val) = *val_lock {
            return val.clone();
        }

        let computed = calc_fn();
        *val_lock = Some(computed.clone());

        // Remove from map once finalized
        let mut map = self.in_flight.lock().unwrap();
        map.remove(key);

        computed
    }
}

impl<T: Clone> Default for ZeroOverheadSingleflight<T> {
    fn default() -> Self {
        Self::new()
    }
}
