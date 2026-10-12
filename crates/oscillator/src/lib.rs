//! FunDSP adapter. No backend types appear in the unit API or persisted settings.
use daw_unit_core::{
    OSCILLATOR_TYPE_ID, PortKind, ProcessingContext, SourceUnit, UnitDescriptor, Waveform,
};
use fundsp::prelude32::*;

pub struct Oscillator {
    node: Box<dyn AudioUnit>,
    frequency: f32,
    sample_rate_hz: u32,
    enabled: bool,
}
impl Oscillator {
    /// Construct and allocate on a control thread, before entering audio processing.
    pub fn new(waveform: Waveform, context: ProcessingContext) -> Self {
        let mut node: Box<dyn AudioUnit> = match waveform {
            Waveform::Sine => Box::new(sine()),
            Waveform::Saw => Box::new(saw()),
            Waveform::Square => Box::new(square()),
            Waveform::Triangle => Box::new(triangle()),
        };
        node.set_sample_rate(f64::from(context.sample_rate_hz));
        node.allocate();
        node.reset();
        Self {
            node,
            frequency: 440.0,
            sample_rate_hz: context.sample_rate_hz,
            enabled: true,
        }
    }
}
impl SourceUnit for Oscillator {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: OSCILLATOR_TYPE_ID,
            name: "Oscillator",
            is_source: true,
            inputs: &[PortKind::Modulation],
            outputs: &[PortKind::MonoAudio],
        }
    }
    fn reset(&mut self) {
        self.node.reset();
    }
    fn set_frequency(&mut self, hz: f32) {
        self.enabled = hz.is_finite() && hz > 0.0 && hz < self.sample_rate_hz as f32 * 0.5;
        self.frequency = if self.enabled { hz } else { 0.0 };
    }

    fn next_sample(&mut self) -> f32 {
        let mut output = [0.0];
        self.node.tick(&[self.frequency], &mut output);
        if self.enabled { output[0] } else { 0.0 }
    }
}
