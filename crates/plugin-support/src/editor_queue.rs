use std::{collections::VecDeque, sync::Mutex};

/// Bounded command batches for editor and control threads.
pub struct EditorQueue<T> {
    events: Mutex<VecDeque<T>>,
    capacity: usize,
}

impl<T: Copy> EditorQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn push(&self, values: &[T]) -> bool {
        let Ok(mut events) = self.events.lock() else {
            return false;
        };
        if values.len() > self.capacity - events.len() {
            return false;
        }
        events.extend(values.iter().copied());
        true
    }

    pub fn pop(&self) -> Option<T> {
        self.events.lock().ok()?.pop_front()
    }

    pub fn remaining(&self) -> usize {
        self.events
            .lock()
            .map_or(0, |events| self.capacity - events.len())
    }

    pub fn retain(&self, keep: impl FnMut(&T) -> bool) {
        if let Ok(mut events) = self.events.lock() {
            events.retain(keep);
        }
    }
}
