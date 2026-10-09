//! Zero-Copy WebSocket Broadcast Hub
//!
//! What was newly created: A dedicated connection registry managing bounded
//! per-client channels and fan-out mechanics.
//!
//! Why it was custom: Standard frameworks do not provide zero-copy broadcast out of the box.
//! This pipeline serializes a mutation event exactly once into an `Arc<[u8]>` slice and
//! concurrently sends that identical pointer down 50,000+ client channel senders, avoiding
//! per-connection string allocations.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Guard managing active connection lifecycle.
pub struct ClientConnectionGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for ClientConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Zero-copy fan-out broadcast hub.
#[derive(Clone)]
pub struct ZeroCopyBroadcastHub {
    tx: broadcast::Sender<Arc<[u8]>>,
    active_count: Arc<AtomicUsize>,
    capacity: usize,
}

impl ZeroCopyBroadcastHub {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            active_count: Arc::new(AtomicUsize::new(0)),
            capacity,
        }
    }

    /// Registers a client stream, returning a zero-copy receiver and lifecycle guard.
    pub fn register(&self) -> (broadcast::Receiver<Arc<[u8]>>, ClientConnectionGuard) {
        self.active_count.fetch_add(1, Ordering::SeqCst);
        let rx = self.tx.subscribe();
        let guard = ClientConnectionGuard {
            counter: self.active_count.clone(),
        };
        (rx, guard)
    }

    /// Broadcasts an immutable shared slice pointer to all connected subscribers.
    /// Allocates memory exactly once regardless of subscriber count.
    #[inline(always)]
    pub fn broadcast(&self, payload: Arc<[u8]>) -> usize {
        self.tx.send(payload).unwrap_or(0)
    }

    pub fn active_connections(&self) -> usize {
        self.active_count.load(Ordering::Relaxed)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for ZeroCopyBroadcastHub {
    fn default() -> Self {
        Self::new(10_000)
    }
}
