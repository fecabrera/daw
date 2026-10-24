//! Control-thread estimates of owned storage. Excludes allocator and native overhead.
use crate::*;
use std::collections::HashMap;

pub fn vec_bytes<T>(values: &Vec<T>) -> u64 {
    (values.capacity() as u64).saturating_mul(std::mem::size_of::<T>() as u64)
}

/// Conservative bucket allowance: capacity is usable entries, not allocated buckets.
pub fn map_bytes<K, V>(values: &HashMap<K, V>) -> u64 {
    (values.capacity() as u64)
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<(K, V)>() as u64 + 1)
}

/// Allow for partly filled tree nodes, including a small root with spare slots.
/// This is a storage estimate, independent of the standard library's node layout.
pub fn tree_bytes<K, V>(len: usize) -> u64 {
    if len == 0 {
        return 0;
    }
    (len as u64)
        .div_ceil(4)
        .saturating_mul(16)
        .saturating_mul((std::mem::size_of::<(K, V)>() + std::mem::size_of::<usize>()) as u64)
}

fn json_bytes(value: &serde_json::Value) -> u64 {
    match value {
        serde_json::Value::String(s) => s.capacity() as u64,
        serde_json::Value::Array(v) => vec_bytes(v) + v.iter().map(json_bytes).sum::<u64>(),
        serde_json::Value::Object(m) => {
            tree_bytes::<String, serde_json::Value>(m.len())
                + m.iter()
                    .map(|(k, v)| k.capacity() as u64 + json_bytes(v))
                    .sum::<u64>()
        }
        _ => 0,
    }
}

impl EffectSettings {
    pub fn heap_bytes(&self) -> u64 {
        match self {
            Self::Eq3(s) => vec_bytes(&s.modulations),
            Self::Drive(s) => vec_bytes(&s.modulations),
            Self::DriveV2(s) => vec_bytes(&s.modulations),
            Self::AutoFilter(s) => vec_bytes(&s.modulations),
            Self::Filter(s) => vec_bytes(&s.modulations),
            Self::Compressor(s) => vec_bytes(&s.modulations),
            Self::Ott(s) => vec_bytes(&s.modulations),
            Self::Limiter(s) => vec_bytes(&s.modulations),
            Self::DirectionMixer(s) => vec_bytes(&s.modulations),
            Self::StereoSpread(s) => vec_bytes(&s.modulations),
            Self::Saturator(s) => vec_bytes(&s.modulations),
            Self::Bitcrusher(s) => vec_bytes(&s.modulations),
            Self::Missing { type_id, state } => type_id.capacity() as u64 + json_bytes(state),
        }
    }
}
pub fn effect_settings_bytes(effects: &Vec<EffectInstance>) -> u64 {
    vec_bytes(effects) + effects.iter().map(|e| e.unit.heap_bytes()).sum::<u64>()
}
fn provider_bytes(
    envelopes: &Vec<EnvelopeInstance>,
    lfos: &Vec<LfoInstance>,
    macros: &MacroModuleSettings,
) -> u64 {
    vec_bytes(envelopes)
        + envelopes
            .iter()
            .map(|e| vec_bytes(&e.modulations))
            .sum::<u64>()
        + vec_bytes(lfos)
        + lfos.iter().map(|l| vec_bytes(&l.modulations)).sum::<u64>()
        + vec_bytes(&macros.controls)
        + vec_bytes(&macros.modulations)
        + macros
            .controls
            .iter()
            .map(|m| m.name.as_ref().map_or(0, |n| n.capacity() as u64))
            .sum::<u64>()
}
impl InstrumentSettings {
    pub fn heap_bytes(&self) -> u64 {
        vec_bytes(&self.layers)
            + self
                .layers
                .iter()
                .map(|l| {
                    l.name.capacity() as u64
                        + vec_bytes(&l.component_order)
                        + effect_settings_bytes(&l.effects)
                        + vec_bytes(&l.oscillators)
                        + vec_bytes(&l.oscillator_modulations)
                        + vec_bytes(&l.amplitude.modulations)
                        + l.sampler.as_ref().map_or(0, |s| {
                            vec_bytes(&s.modulations)
                                + s.input.as_ref().map_or(0, |i| i.name.capacity() as u64)
                        })
                        + provider_bytes(&l.envelopes, &l.lfos, &l.macro_module)
                })
                .sum::<u64>()
            + vec_bytes(&self.main.component_order)
            + effect_settings_bytes(&self.main.effects)
            + provider_bytes(
                &self.main.envelopes,
                &self.main.lfos,
                &self.main.macro_module,
            )
    }
}
