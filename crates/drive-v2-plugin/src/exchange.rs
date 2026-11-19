//! Shared control-to-audio recall and coherent audio-to-control publication.
use crate::PluginState;
const WORDS: usize = crate::MAX_STATE_BYTES.div_ceil(8);

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
