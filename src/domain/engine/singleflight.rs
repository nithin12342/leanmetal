//! src/domain/engine/singleflight.rs
//! File ID: FILE-007
//! Responsibility: Coalesce concurrent duplicate in-flight reads into one (<= 7 words)
//! Must Never: Block asynchronous executor threads or leak memory under panics.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Internal channel tracking an in-flight computation
#[derive(Clone)]
struct InFlight<V> {
    tx: broadcast::Sender<V>,
}

/// Request coalescing coordinator preventing cache stampedes (REQ-013 / SPEC-013).
/// If thousands of requests arrive for the exact same key simultaneously,
/// only one lookup executes while others await the identical result in user-space RAM.
#[derive(Clone)]
pub struct Singleflight<K: Hash + Eq + Clone, V: Clone + Send + 'static> {
    tasks: Arc<Mutex<HashMap<K, InFlight<V>>>>,
}

impl<K: Hash + Eq + Clone, V: Clone + Send + 'static> Default for Singleflight<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Clone, V: Clone + Send + 'static> Singleflight<K, V> {
    /// Creates a new Singleflight coordinator.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Executes the future for the given key, or awaits an already in-flight execution.
    /// Guarantees that only ONE call to `fut` runs concurrently for the same key.
    pub async fn execute<F, Fut>(&self, key: &K, fut: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        let mut rx = {
            let mut map = self.tasks.lock().await;
            if let Some(in_flight) = map.get(key) {
                // Another request is already executing; subscribe to its result
                in_flight.tx.subscribe()
            } else {
                // We are the first caller; register in-flight broadcast channel
                let (tx, _) = broadcast::channel(16);
                map.insert(key.clone(), InFlight { tx });
                drop(map);

                // Run the actual computation
                let value = fut().await;

                // Broadcast result to all waiting callers and clean up map
                let mut map = self.tasks.lock().await;
                if let Some(in_flight) = map.remove(key) {
                    let _ = in_flight.tx.send(value.clone());
                }

                return value;
            }
        };

        // Awaiting callers receive the broadcast value without touching storage
        match rx.recv().await {
            Ok(val) => val,
            Err(_) => {
                // If the first caller dropped or failed, execute directly as fallback
                fut().await
            }
        }
    }

    /// Returns the number of currently in-flight coalesced keys.
    pub async fn in_flight_count(&self) -> usize {
        self.tasks.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_singleflight_coalesces_concurrent_calls() {
        let sf = Singleflight::new();
        let execution_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..100 {
            let sf_clone = sf.clone();
            let exec_clone = execution_count.clone();
            handles.push(tokio::spawn(async move {
                sf_clone
                    .execute(&"hot_key".to_string(), || async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        exec_clone.fetch_add(1, Ordering::SeqCst);
                        42
                    })
                    .await
            }));
        }

        for h in handles {
            let res = h.await.unwrap();
            assert_eq!(res, 42);
        }

        // Exactly 1 execution occurred despite 100 concurrent requests!
        assert_eq!(execution_count.load(Ordering::SeqCst), 1);
        assert_eq!(sf.in_flight_count().await, 0);
    }
}
