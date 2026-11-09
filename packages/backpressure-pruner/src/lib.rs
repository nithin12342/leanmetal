//! Slow-Consumer Pruning & Backpressure Engine
//!
//! What was newly created: An active eviction loop inside the broadcast hub
//! using non-blocking channel checks (`try_send`).
//!
//! Why it was custom: Prevents stalled mobile connections from accumulating
//! unread TCP buffers and causing Out-Of-Memory (OOM) crashes. If a client's
//! bounded frame channel becomes full, the engine isolates that session ID
//! and drops the connection to safeguard server memory.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

pub const DEFAULT_MAX_BUFFER_BYTES: usize = 64 * 1024; // 64 KB limit
pub const DEFAULT_MAX_BUFFER_FRAMES: usize = 64;        // 64 frames limit

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushOutcome {
    Delivered,
    SlowConsumerPruned,
    ClientDisconnected,
}

/// Bounded client session actor with active backpressure tracking.
pub struct BoundedSession {
    pub session_id: String,
    tx: mpsc::Sender<Arc<[u8]>>,
    buffered_bytes: Arc<AtomicUsize>,
    is_pruned: Arc<AtomicBool>,
    max_bytes: usize,
}

impl BoundedSession {
    pub fn new(
        session_id: impl Into<String>,
        max_frames: usize,
        max_bytes: usize,
    ) -> (Self, mpsc::Receiver<Arc<[u8]>>, Arc<AtomicUsize>) {
        let (tx, rx) = mpsc::channel(max_frames);
        let buffered_bytes = Arc::new(AtomicUsize::new(0));
        let is_pruned = Arc::new(AtomicBool::new(false));

        let session = Self {
            session_id: session_id.into(),
            tx,
            buffered_bytes: buffered_bytes.clone(),
            is_pruned,
            max_bytes,
        };

        (session, rx, buffered_bytes)
    }

    /// Tries pushing a frame. Prunes the client session immediately if buffer depth exceeds threshold.
    pub fn try_push(&self, frame: Arc<[u8]>) -> PushOutcome {
        if self.is_pruned.load(Ordering::Relaxed) {
            return PushOutcome::SlowConsumerPruned;
        }

        let frame_len = frame.len();
        let current_bytes = self.buffered_bytes.load(Ordering::Relaxed);

        if current_bytes + frame_len > self.max_bytes {
            self.is_pruned.store(true, Ordering::SeqCst);
            return PushOutcome::SlowConsumerPruned;
        }

        match self.tx.try_send(frame) {
            Ok(_) => {
                self.buffered_bytes.fetch_add(frame_len, Ordering::Relaxed);
                PushOutcome::Delivered
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.is_pruned.store(true, Ordering::SeqCst);
                PushOutcome::SlowConsumerPruned
            }
            Err(mpsc::error::TrySendError::Closed(_)) => PushOutcome::ClientDisconnected,
        }
    }

    pub fn is_pruned(&self) -> bool {
        self.is_pruned.load(Ordering::Relaxed)
    }
}

/// Backpressure engine managing active sessions and pruning slow consumers.
#[derive(Clone)]
pub struct BackpressureEngine {
    sessions: Arc<RwLock<HashMap<String, Arc<BoundedSession>>>>,
}

impl BackpressureEngine {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, session: Arc<BoundedSession>) {
        let mut map = self.sessions.write().unwrap();
        map.insert(session.session_id.clone(), session);
    }

    pub fn broadcast_and_prune(&self, frame: Arc<[u8]>) -> (usize, usize) {
        let mut to_remove = Vec::new();
        let mut delivered = 0;

        let sessions = self.sessions.read().unwrap();
        for (id, session) in sessions.iter() {
            match session.try_push(frame.clone()) {
                PushOutcome::Delivered => delivered += 1,
                PushOutcome::SlowConsumerPruned | PushOutcome::ClientDisconnected => {
                    to_remove.push(id.clone());
                }
            }
        }
        drop(sessions);

        let pruned_count = to_remove.len();
        if !to_remove.is_empty() {
            let mut map = self.sessions.write().unwrap();
            for id in to_remove {
                map.remove(&id);
            }
        }

        (delivered, pruned_count)
    }
}

impl Default for BackpressureEngine {
    fn default() -> Self {
        Self::new()
    }
}
