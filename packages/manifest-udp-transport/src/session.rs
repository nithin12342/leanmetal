//! packages/manifest-udp-transport/src/session.rs
//! Contiguous 40-byte Session Slab managing up to 100,000 UDP connections in 4.0 MB flat RAM.
//!
//! Replaces OS TCP socket control blocks (TCBs). Maps WebSocket-like persistent streams
//! and HTTP clients without per-connection heap allocations.

use engine_layout::ConnectionSlot;

pub const DEFAULT_MAX_SESSIONS: usize = 100_000;

pub struct UdpSessionRegistry {
    slots: Box<[ConnectionSlot]>,
}

impl Default for UdpSessionRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_SESSIONS)
    }
}

impl UdpSessionRegistry {
    pub fn new(capacity: usize) -> Self {
        let vec_slots = vec![ConnectionSlot::default(); capacity];
        Self {
            slots: vec_slots.into_boxed_slice(),
        }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    #[inline(always)]
    pub fn register(&mut self, session_token: u64, slot: ConnectionSlot) -> bool {
        let idx = (session_token as usize) % self.slots.len();
        self.slots[idx] = slot;
        true
    }

    #[inline(always)]
    pub fn get(&self, session_token: u64) -> Option<&ConnectionSlot> {
        let idx = (session_token as usize) % self.slots.len();
        let slot = &self.slots[idx];
        if slot.session_token == session_token && slot.socket_fd != 0 {
            Some(slot)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn get_mut(&mut self, session_token: u64) -> Option<&mut ConnectionSlot> {
        let len = self.slots.len();
        let idx = (session_token as usize) % len;
        let slot = &mut self.slots[idx];
        if slot.session_token == session_token && slot.socket_fd != 0 {
            Some(slot)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn touch(&mut self, session_token: u64, now_ns: u64, seq: u64) -> bool {
        if let Some(slot) = self.get_mut(session_token) {
            slot.last_rx_ns = now_ns;
            if seq >= slot.expected_seq {
                slot.expected_seq = seq + 1;
            }
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn remove(&mut self, session_token: u64) -> bool {
        let idx = (session_token as usize) % self.slots.len();
        if self.slots[idx].session_token == session_token {
            self.slots[idx] = ConnectionSlot::default();
            true
        } else {
            false
        }
    }
}
