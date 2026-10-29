//! One shared stereo processor chain per layer, after the voice sum and before mixing.
use daw_core::{EffectInstance, Id};

#[derive(Clone)]
pub(super) struct LayerEffects {
    pub id: Id,
    pub peak: [f32; 2],
    pub meter_sample: [f32; 2],
    pub chain: super::effects::EffectChain,
    pub settings: Vec<EffectInstance>,
    pub delay: super::latency::Delay,
    gain: [f32; 2],
    target: [f32; 2],
    remaining: u32,
}
impl LayerEffects {
    pub fn heap_bytes(&self) -> u64 {
        self.chain.heap_bytes()
            + self.delay.heap_bytes()
            + daw_unit_core::memory::effect_settings_bytes(&self.settings)
    }

    pub fn new(
        id: Id,
        effects: &[EffectInstance],
        gain: [f32; 2],
        rate: u32,
        block: u32,
        max_latency: u64,
    ) -> Self {
        Self {
            id,
            peak: [0.0; 2],
            meter_sample: [0.0; 2],
            chain: super::effects::EffectChain::chain(Some(id), effects, rate, block),
            settings: effects.to_vec(),
            delay: super::latency::Delay::new(
                id,
                max_latency - super::latency::chain_frames(effects, rate),
            ),
            gain,
            target: gain,
            remaining: 0,
        }
    }
    pub fn retarget(&mut self, prepared: &mut Self, fade: u32, idle: bool) {
        if self.target != prepared.target {
            self.target = prepared.target;
            self.remaining = if idle {
                self.gain = self.target;
                0
            } else {
                fade
            };
        }
        std::mem::swap(&mut self.chain, &mut prepared.chain);
        std::mem::swap(&mut self.settings, &mut prepared.settings);
    }
    pub fn mix(&mut self, sample: [f32; 2]) -> [f32; 2] {
        if self.remaining > 0 {
            for ch in 0..2 {
                self.gain[ch] += (self.target[ch] - self.gain[ch]) / self.remaining as f32;
            }
            self.remaining -= 1;
        }
        std::array::from_fn(|ch| sample[ch] * self.gain[ch])
    }
    pub fn reset(&mut self) {
        self.peak = [0.0; 2];
        self.meter_sample = [0.0; 2];
        self.chain.reset();
        self.delay.reset();
        self.gain = self.target;
        self.remaining = 0;
    }
}

pub(super) fn parallel_latency(instrument: &daw_core::InstrumentSettings, rate: u32) -> u64 {
    instrument
        .layers
        .iter()
        .map(|l| super::latency::chain_frames(&l.effects, rate))
        .max()
        .unwrap_or(0)
}

pub(super) fn latency(instrument: &daw_core::InstrumentSettings, rate: u32) -> u64 {
    parallel_latency(instrument, rate)
        + super::latency::chain_frames(&instrument.main.effects, rate)
}

// Effect identity stays globally unique; all monitor entry points share the same chains.
pub(super) fn find(chains: &[LayerEffects], id: Id) -> Option<super::ModulationSnapshot> {
    chains.iter().find_map(|l| l.chain.modulation_values(id))
}
