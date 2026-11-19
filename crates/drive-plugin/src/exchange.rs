//! Shared control-to-audio recall and coherent audio-to-control publication.
use crate::{Parameter, PluginState};
const WORDS: usize = 7;

pub struct StateExchange(daw_plugin_support::RevisionExchange<WORDS, { WORDS + 1 }>);

impl Default for StateExchange {
    fn default() -> Self {
        Self(daw_plugin_support::RevisionExchange::new(words(
            PluginState::default(),
        )))
    }
}

impl StateExchange {
    pub fn recall(&self, state: PluginState) {
        self.0.recall(words(state));
    }

    pub fn consume(&self, applied: &mut u64) -> Option<PluginState> {
        self.0.consume(applied, state)
    }

    pub fn publish(&self, state: PluginState, applied: u64) {
        self.0.publish(words(state), applied);
    }

    pub fn current(&self) -> Option<PluginState> {
        self.0.current(state)
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
