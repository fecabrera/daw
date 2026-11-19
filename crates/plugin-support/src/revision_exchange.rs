//! Bounded plugin recall and publication over fixed-word snapshots.
use crate::Snapshot;

pub struct RevisionExchange<const WORDS: usize, const PUBLISHED: usize> {
    pending: Snapshot<WORDS>,
    published: Snapshot<PUBLISHED>,
}
impl<const WORDS: usize, const PUBLISHED: usize> RevisionExchange<WORDS, PUBLISHED> {
    pub fn new(words: [u64; WORDS]) -> Self {
        assert_eq!(PUBLISHED, WORDS + 1);
        Self {
            pending: Snapshot::new(words),
            published: Snapshot::new(std::array::from_fn(
                |i| if i < WORDS { words[i] } else { 0 },
            )),
        }
    }

    /// One serialized control-thread writer. Only validated snapshots enter here.
    pub fn recall(&self, words: [u64; WORDS]) {
        self.pending.publish(words);
    }
    /// One attempt on audio; a concurrent recall is deferred to the next boundary.
    pub fn consume<T>(
        &self,
        applied: &mut u64,
        decode: impl FnOnce([u64; WORDS]) -> Option<T>,
    ) -> Option<T> {
        let (revision, words) = self.pending.read()?;
        if revision == *applied {
            return None;
        }
        let state = decode(words)?;
        *applied = revision;
        Some(state)
    }
    /// One serialized audio writer, or control while inactive.
    pub fn publish(&self, words: [u64; WORDS], applied: u64) {
        self.published.publish(std::array::from_fn(|i| {
            if i < WORDS { words[i] } else { applied }
        }));
    }
    pub fn current<T>(&self, decode: impl FnOnce([u64; WORDS]) -> Option<T>) -> Option<T> {
        for _ in 0..8 {
            let Some((revision, pending)) = self.pending.read() else {
                continue;
            };
            let Some((_, published)) = self.published.read() else {
                continue;
            };
            return decode(if revision != published[WORDS] {
                pending
            } else {
                std::array::from_fn(|i| published[i])
            });
        }
        None
    }
}
