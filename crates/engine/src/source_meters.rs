//! Source-unit output taps, summed across voices before Amplitude and downstream buses.
use daw_core::{Id, InstrumentLayerSettings};
use daw_unit_core::LevelMonitor;

/// Scoped Out-strip identity: Oscillator uses its layer ID; Granulizer uses its source ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceOutputTarget {
    pub track: Id,
    pub layer: Id,
    pub unit: Id,
}
impl SourceOutputTarget {
    pub fn for_layer(track: Id, layer: &InstrumentLayerSettings) -> Option<(Self, bool)> {
        let (unit, enabled) = if let Some(s) = &layer.sampler {
            // Regular Sampler has a Sample strip, so it needs no Out-strip publication.
            s.granulizer.as_ref()?;
            (s.id, s.enabled)
        } else if !layer.oscillators.is_empty() {
            (layer.id, layer.oscillator_enabled)
        } else {
            return None;
        };
        Some((
            Self {
                track,
                layer: layer.id,
                unit,
            },
            enabled,
        ))
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
