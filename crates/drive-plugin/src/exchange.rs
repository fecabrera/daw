//! Shared control-to-audio recall and coherent audio-to-control publication.
use crate::{Parameter, PluginState};
use daw_plugin_support::Snapshot;

pub struct StateExchange {
    pending: Snapshot<7>,
    published: Snapshot<8>,
}
impl Default for StateExchange {
    fn default() -> Self {
        let words = words(PluginState::default());
        Self {
            pending: Snapshot::new(words),
            published: Snapshot::new(std::array::from_fn(|i| if i < 7 { words[i] } else { 0 })),
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
        self.published.publish(std::array::from_fn(
            |i| if i < 7 { words[i] } else { applied },
        ));
    }
    pub fn current(&self) -> Option<PluginState> {
        for _ in 0..8 {
            let Some((revision, pending)) = self.pending.read() else {
                continue;
            };
            let Some((_, published)) = self.published.read() else {
                continue;
            };
            return state(if revision != published[7] {
                pending
            } else {
                std::array::from_fn(|i| published[i])
            });
        }
        None
    }
}
fn words(state: PluginState) -> [u64; 7] {
    Parameter::ALL.map(|p| state.get(p).to_bits())
}
fn state(words: [u64; 7]) -> Option<PluginState> {
    let mut state = PluginState::default();
    for (parameter, word) in Parameter::ALL.into_iter().zip(words) {
        state.set(parameter, f64::from_bits(word)).ok()?;
    }
    Some(state)
}
