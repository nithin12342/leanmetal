//! src/domain/ingress/connection_ring.rs
//! File ID: FILE-001
//! Responsibility: Manage persistent WebSockets and broadcast frames (<=7 words)
//! Must Never: Duplicate payload serialization across client connections.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Manages active WebSocket connections and multiplexes broadcast frames
/// using shared atomic byte buffers (Arc<[u8]>) with zero per-client duplication.
#[derive(Clone)]
pub struct ConnectionRing {
    tx: broadcast::Sender<Arc<[u8]>>,
    active_count: Arc<AtomicUsize>,
    capacity: usize,
}

impl ConnectionRing {
    /// Creates a new connection ring with specified buffer capacity (default: 10,000 frames).
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            active_count: Arc::new(AtomicUsize::new(0)),
            capacity,
        }
    }

    /// Registers a new active client stream and returns a subscription receiver.
    pub fn register_client(&self) -> (broadcast::Receiver<Arc<[u8]>>, ClientGuard) {
        self.active_count.fetch_add(1, Ordering::SeqCst);
        let rx = self.tx.subscribe();
        let guard = ClientGuard {
            counter: self.active_count.clone(),
        };
        (rx, guard)
    }

    /// Broadcasts an atomic byte slice to all connected WebSocket clients.
    /// Allocates memory only once.
    pub fn broadcast(&self, payload: Arc<[u8]>) -> usize {
        self.tx.send(payload).unwrap_or(0)
    }

    /// Returns the number of currently active client connections.
    pub fn active_clients(&self) -> usize {
        self.active_count.load(Ordering::Relaxed)
    }

    /// Returns ring capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Default for ConnectionRing {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// RAII guard ensuring active client count is decremented on disconnection.
pub struct ClientGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
