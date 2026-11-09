//! src/domain/ingress/backpressure.rs
//! File ID: FILE-010
//! Responsibility: Manage WebSocket backpressure and prune slow consumers (<= 7 words)
//! Must Never: Allow unbounded client socket buffering or leak server memory on slow networks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

pub const MAX_CLIENT_BUFFER_BYTES: usize = 64 * 1024; // 64 KB hard limit
pub const MAX_CLIENT_BUFFER_FRAMES: usize = 64;       // 64 frames hard limit

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    Delivered,
    SlowConsumerPruned,
    ClientDisconnected,
}

/// Actor representing a connected client session with backpressure tracking (REQ-017 / SPEC-017).
pub struct ClientSession {
    pub client_id: String,
    tx: mpsc::Sender<Arc<[u8]>>,
    buffered_bytes: Arc<AtomicUsize>,
    is_pruned: Arc<AtomicBool>,
}

impl ClientSession {
    /// Creates a new bounded client session actor.
    pub fn new(client_id: impl Into<String>) -> (Self, mpsc::Receiver<Arc<[u8]>>, Arc<AtomicUsize>) {
        let (tx, rx) = mpsc::channel(MAX_CLIENT_BUFFER_FRAMES);
        let buffered_bytes = Arc::new(AtomicUsize::new(0));
        let is_pruned = Arc::new(AtomicBool::new(false));

        let session = Self {
            client_id: client_id.into(),
            tx,
            buffered_bytes: buffered_bytes.clone(),
            is_pruned,
        };

        (session, rx, buffered_bytes)
    }

    /// Pushes a shared zero-copy frame pointer to the client with strict backpressure.
    /// If client queue exceeds 64KB or 64 frames, the client is auto-pruned to prevent host OOM.
    pub fn try_push(&self, frame: Arc<[u8]>) -> PushResult {
        if self.is_pruned.load(Ordering::Relaxed) {
            return PushResult::SlowConsumerPruned;
        }

        let frame_len = frame.len();
        let current_bytes = self.buffered_bytes.load(Ordering::Relaxed);

        // 1. Check byte depth backpressure (64 KB ceiling)
        if current_bytes + frame_len > MAX_CLIENT_BUFFER_BYTES {
            self.is_pruned.store(true, Ordering::SeqCst);
            return PushResult::SlowConsumerPruned;
        }

        // 2. Try non-blocking send into bounded queue
        match self.tx.try_send(frame) {
            Ok(()) => {
                self.buffered_bytes.fetch_add(frame_len, Ordering::Relaxed);
                PushResult::Delivered
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Queue full: Slow consumer cannot keep up with broadcast throughput
                self.is_pruned.store(true, Ordering::SeqCst);
                PushResult::SlowConsumerPruned
            }
            Err(mpsc::error::TrySendError::Closed(_)) => PushResult::ClientDisconnected,
        }
    }

    pub fn is_pruned(&self) -> bool {
        self.is_pruned.load(Ordering::Relaxed)
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffered_bytes.load(Ordering::Relaxed)
    }
}

/// Zero-copy broadcast hub managing thousands of client connections with backpressure (REQ-016 / REQ-017).
#[derive(Clone, Default)]
pub struct BackpressureHub {
    clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

impl BackpressureHub {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new client connection.
    pub fn register(&self, client_id: &str) -> (mpsc::Receiver<Arc<[u8]>>, Arc<AtomicUsize>) {
        let (session, rx, tracker) = ClientSession::new(client_id);
        let mut map = self.clients.write().unwrap();
        map.insert(client_id.to_string(), Arc::new(session));
        (rx, tracker)
    }

    /// Broadcasts an Arc<[u8]> frame to all active connections without heap duplication.
    /// Returns (delivered_count, pruned_slow_consumers).
    pub fn broadcast(&self, frame: Arc<[u8]>) -> (usize, usize) {
        let clients: Vec<Arc<ClientSession>> = {
            let map = self.clients.read().unwrap();
            map.values().cloned().collect()
        };

        let mut delivered = 0;
        let mut pruned = 0;
        let mut to_remove = Vec::new();

        for client in clients {
            match client.try_push(frame.clone()) {
                PushResult::Delivered => delivered += 1,
                PushResult::SlowConsumerPruned => {
                    pruned += 1;
                    to_remove.push(client.client_id.clone());
                }
                PushResult::ClientDisconnected => {
                    to_remove.push(client.client_id.clone());
                }
            }
        }

        // Clean up pruned and disconnected clients
        if !to_remove.is_empty() {
            let mut map = self.clients.write().unwrap();
            for id in to_remove {
                map.remove(&id);
            }
        }

        (delivered, pruned)
    }

    pub fn active_count(&self) -> usize {
        self.clients.read().unwrap().len()
    }
}
