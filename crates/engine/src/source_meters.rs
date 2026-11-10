//! Instrument audio-unit output taps, summed across voices before downstream buses.
use daw_core::{Id, InstrumentLayerSettings};
use daw_unit_core::LevelMonitor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceOutputStage {
    Source,
    Amplitude,
}

/// Scoped audio-unit identity. The stage separates source and Amplitude taps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceOutputTarget {
    pub track: Id,
    pub layer: Id,
    pub unit: Id,
    pub stage: SourceOutputStage,
}
impl SourceOutputTarget {
    pub fn source(track: Id, layer: Id, unit: Id) -> Self {
        Self {
            track,
            layer,
            unit,
            stage: SourceOutputStage::Source,
        }
    }
    pub fn amplitude(track: Id, layer: Id) -> Self {
        Self {
            track,
            layer,
            unit: layer,
            stage: SourceOutputStage::Amplitude,
        }
    }
    pub fn for_layer(track: Id, layer: &InstrumentLayerSettings) -> Option<(Self, bool)> {
        let (unit, enabled) = if let Some(n) = &layer.noise {
            (n.id, n.enabled)
        } else if let Some(s) = &layer.sampler {
            (s.id, s.enabled)
        } else if !layer.oscillators.is_empty() {
            (layer.id, layer.oscillator_enabled)
        } else {
            return None;
        };
        Some((Self::source(track, layer.id, unit), enabled))
    }
}

#[derive(Clone)]
pub(super) struct SourceOutput {
    pub unit: Id,
    pub level: LevelMonitor<3>,
    pub sample: [f32; 2],
}
impl SourceOutput {
    pub fn new(unit: Id, enabled: bool, rate: u32) -> Self {
        Self {
            unit,
            level: LevelMonitor::new(rate, enabled),
            sample: [0.0; 2],
        }
    }
}
