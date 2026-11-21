//! Delay prepared shared publications to a native destination's input clock.
//! Audio-derived publications already carry their route's stage alignment.
use super::hosted_block::Controls;
use daw_core::HostedSettings;
use daw_unit_core::{ModulationSourceIndex, ModulationValue};
pub(super) struct ControlDelay {
    slots: Vec<usize>,
    delayed: Vec<usize>,
    current: Vec<ModulationValue>,
    history: Vec<ModulationValue>,
    frames: usize,
    write: usize,
    index: ModulationSourceIndex,
}
impl ControlDelay {
    pub fn new(settings: &HostedSettings, controls: &Controls, frames: u64) -> Self {
        let needed: std::collections::HashSet<_> = settings
            .modulations
            .iter()
            .filter_map(|r| r.effective_source())
            .collect();
        let audio: std::collections::HashSet<_> = settings
            .modulations
            .iter()
            .filter(|r| r.sidechain.is_some())
            .filter_map(|r| r.effective_source())
            .collect();
        let slots: Vec<_> = controls
            .at(0)
            .iter()
            .enumerate()
            .filter_map(|(i, v)| needed.contains(&v.source).then_some(i))
            .collect();
        let current: Vec<_> = slots.iter().map(|&i| controls.at(0)[i]).collect();
        let delayed: Vec<_> = current
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (!audio.contains(&v.source)).then_some(i))
            .collect();
        let index = ModulationSourceIndex::from_values(&current);
        let frames = if delayed.is_empty() {
            0
        } else {
            frames as usize
        };
        let mut history = Vec::with_capacity(frames * delayed.len());
        for _ in 0..frames {
            for &i in &delayed {
                let mut value = current[i];
                value.value = 0.0;
                value.weight = 0.0;
                value.release_age = None;
                history.push(value);
            }
        }
        Self {
            slots,
            delayed,
            current,
            history,
            frames,
            write: 0,
            index,
        }
    }
    pub fn sample(
        &mut self,
        values: &[ModulationValue],
    ) -> (&[ModulationValue], &ModulationSourceIndex) {
        for (out, &slot) in self.current.iter_mut().zip(&self.slots) {
            *out = values[slot];
        }
        if self.frames > 0 {
            for (column, &slot) in self.delayed.iter().enumerate() {
                let index = self.write * self.delayed.len() + column;
                std::mem::swap(&mut self.history[index], &mut self.current[slot]);
            }
            self.write = (self.write + 1) % self.frames;
        }
        (&self.current, &self.index)
    }
    pub fn compatible(&self, previous: &Self) -> bool {
        self.frames == previous.frames
            && self.delayed == previous.delayed
            && self
                .current
                .iter()
                .map(|v| v.source)
                .eq(previous.current.iter().map(|v| v.source))
    }
    pub fn inherit(&mut self, previous: &mut Self) {
        if self.compatible(previous) {
            std::mem::swap(&mut self.history, &mut previous.history);
            self.write = previous.write;
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        use daw_unit_core::memory::vec_bytes;
        vec_bytes(&self.slots)
            + vec_bytes(&self.delayed)
            + vec_bytes(&self.current)
            + vec_bytes(&self.history)
            + self.index.heap_bytes()
    }
}
