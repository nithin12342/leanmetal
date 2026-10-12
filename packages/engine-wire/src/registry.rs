use engine_layout::ConnectionSlot;

pub const MAX_SESSIONS: usize = 100_000;

pub struct SessionRegistry {
    slots: Box<[ConnectionSlot]>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new(MAX_SESSIONS)
    }
}

impl SessionRegistry {
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
    pub fn register(&mut self, session_id: u64, slot: ConnectionSlot) -> bool {
        let idx = (session_id as usize) % self.slots.len();
        self.slots[idx] = slot;
        true
    }

    #[inline(always)]
    pub fn get(&self, session_id: u64, token: u64) -> Option<&ConnectionSlot> {
        let idx = (session_id as usize) % self.slots.len();
        let slot = &self.slots[idx];
        if slot.session_token == token && slot.socket_fd != 0 {
            Some(slot)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn remove(&mut self, session_id: u64, token: u64) -> bool {
        let idx = (session_id as usize) % self.slots.len();
        if self.slots[idx].session_token == token {
            self.slots[idx] = ConnectionSlot::default();
            true
        } else {
            false
        }
    }
}
