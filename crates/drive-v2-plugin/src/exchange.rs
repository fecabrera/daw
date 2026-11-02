//! Shared control-to-audio recall and coherent audio-to-control publication.
use crate::PluginState;
const WORDS: usize = crate::MAX_STATE_BYTES.div_ceil(8);
use daw_plugin_support::Snapshot;

pub struct StateExchange {
    pending: Snapshot<{ WORDS }>,
    published: Snapshot<{ WORDS + 1 }>,
}
impl Default for StateExchange {
    fn default() -> Self {
        let words = words(PluginState::default());
        Self {
            pending: Snapshot::new(words),
            published: Snapshot::new(std::array::from_fn(
                |i| {
                    if i < WORDS { words[i] } else { 0 }
                },
            )),
        }
    }
}
impl StateExchange {
    /// One serialized control-thread writer. Only validated snapshots enter here.
    pub fn recall(&self, state: PluginState) {
        self.pending.publish(words(state));
    }
    /// One attempt on audio; a concurrent recall is deferred to the next boundary.
    pub fn consume(&self, applied: &mut u64) -> Option<PluginState> {
        let (revision, words) = self.pending.read()?;
        if revision == *applied {
            return None;
        }
        let state = state(words)?;
        *applied = revision;
        Some(state)
    }
    /// One serialized audio writer, or control while inactive.
    pub fn publish(&self, state: PluginState, applied: u64) {
        let words = words(state);
        self.published.publish(std::array::from_fn(|i| {
            if i < WORDS { words[i] } else { applied }
        }));
    }
    pub fn current(&self) -> Option<PluginState> {
        for _ in 0..8 {
            let Some((revision, pending)) = self.pending.read() else {
                continue;
            };
            let Some((_, published)) = self.published.read() else {
                continue;
            };
            return state(if revision != published[WORDS] {
                pending
            } else {
                std::array::from_fn(|i| published[i])
            });
        }
        None
    }
}
fn words(state: PluginState) -> [u64; WORDS] {
    let bytes = state.encode();
    std::array::from_fn(|i| {
        let mut word = [0; 8];
        for (j, v) in word.iter_mut().enumerate() {
            *v = bytes.get(i * 8 + j).copied().unwrap_or(0);
        }
        u64::from_le_bytes(word)
    })
}
fn state(words: [u64; WORDS]) -> Option<PluginState> {
    let bytes: [u8; crate::MAX_STATE_BYTES] =
        std::array::from_fn(|i| words[i / 8].to_le_bytes()[i % 8]);
    PluginState::decode(&bytes).ok()
}
