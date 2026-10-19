//! FunDSP adapter. No backend types appear in the unit API or persisted settings.
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, ModulationSmoother, ModulationTargetId,
    OSCILLATOR_TYPE_ID, OscillatorSettings, PortKind, ProcessingContext, SourceUnit,
    UnitDescriptor, Waveform,
};
use fundsp::prelude32::*;

pub struct Oscillator {
    node: Box<dyn AudioUnit>,
    frequency: f32,
    sample_rate_hz: u32,
    target: Option<ModulationTargetId>,
    offsets: ModulationSmoother<2>,
    effective_offset: f32,
    base_gain: f32,
    effective_gain: f32,
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
            target: None,
            offsets: ModulationSmoother::new(context),
            effective_offset: 0.0,
            base_gain: 1.0,
            effective_gain: 1.0,
        }
    }
    /// Prepare an identified instance. Hosts keep MIDI pitch and base transpose in the base frequency.
    pub fn from_settings(settings: &OscillatorSettings, context: ProcessingContext) -> Self {
        let mut source = Self::new(settings.waveform, context);
        source.target = Some(settings.id);
        source
    }
    /// Applied, smoothed modulation offset in semitones; excludes MIDI pitch and base transpose.
    pub fn pitch_offset(&self) -> f32 {
        self.effective_offset
    }
    /// Supply the host-smoothed linear base gain, including source mute.
    /// Modulation cannot raise an exactly silent base gain.
    pub fn set_base_gain(&mut self, gain: f32) {
        self.base_gain = if gain.is_finite() { gain.max(0.0) } else { 0.0 };
    }
    /// Last processed linear gain; excludes velocity, note shaping and layer mixing.
    pub fn effective_gain(&self) -> f32 {
        self.effective_gain
    }
    fn sample_at(&mut self, frequency: f32) -> f32 {
        let enabled = frequency.is_finite()
            && frequency > 0.0
            && frequency < self.sample_rate_hz as f32 * 0.5;
        let mut output = [0.0];
        self.node
            .tick(&[if enabled { frequency } else { 0.0 }], &mut output);
        if enabled { output[0] } else { 0.0 }
    }
}
impl ModulationDestination for Oscillator {
    type Audio = f32;
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        <daw_unit_core::InstrumentLayerSettings as ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        self.target
    }
    /// A generator has no input audio; the argument is unused. Set base frequency and gain first.
    fn process_with_modulation(
        &mut self,
        _input: f32,
        modulation: &ModulationInputs,
        _context: ModulationContext,
    ) -> f32 {
        let [offset, gain_offset] = self.offsets.process([
            modulation.offset(ModulationParameter::Pitch),
            modulation.offset(ModulationParameter::OscillatorGain),
        ]);
        // Retain neutral arithmetic and bound the summed offset before dB conversion.
        self.effective_gain = if gain_offset == 0.0 {
            self.base_gain
        } else {
            (self.base_gain * 10_f32.powf(gain_offset.clamp(-96.0, 96.0) as f32 / 20.0))
                .min(10_f32.powf(12.0 / 20.0))
        };
        // Bound the summed offset before exponentiation. Invalid/high frequencies remain silent.
        self.effective_offset = offset.clamp(-96.0, 96.0) as f32;
        let frequency = if self.effective_offset == 0.0 {
            self.frequency
        } else {
            self.frequency * 2.0_f32.powf(self.effective_offset / 12.0)
        };
        self.sample_at(frequency) * self.effective_gain
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
        self.offsets.reset();
        self.effective_gain = self.base_gain;
        self.effective_offset = 0.0;
    }
    fn set_frequency(&mut self, hz: f32) {
        self.frequency = if hz.is_finite() && hz > 0.0 { hz } else { 0.0 };
    }

    fn next_sample(&mut self) -> f32 {
        self.sample_at(self.frequency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
        ModulationValue, prepare_target_routes, process_modulation,
    };

    #[test]
    fn gain_routes_preserve_neutral_audio_target_identity_silence_and_bounds_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = OscillatorSettings::default();
            let other = OscillatorSettings::default();
            let id = ModulationSourceId {
                layer: settings.id,
                unit: other.id,
            };
            let value = ModulationValue {
                source: id,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Contour,
                release_age: None,
            };
            let mut route = Modulation::new(ModulationParameter::OscillatorGain);
            route.source = Some(id);
            route.target = Some(settings.id);
            route.amount = 24.0;
            for shared in [false, true] {
                let mut unit = Oscillator::from_settings(&settings, context);
                let mut reference = Oscillator::new(Waveform::Sine, context);
                unit.set_base_gain(0.25);
                let mut wrong = prepare_target_routes(&[route], other.id, rate);
                for _ in 0..1000 {
                    assert_eq!(
                        process_modulation(
                            &mut unit,
                            &mut wrong,
                            0.0,
                            &[value],
                            &[],
                            Default::default()
                        ),
                        reference.next_sample() * 0.25
                    );
                }
                let mut routes = prepare_target_routes(&[route], settings.id, rate);
                for _ in 0..rate / 20 {
                    let (local, bus) = if shared {
                        (&[][..], &[value][..])
                    } else {
                        (&[value][..], &[][..])
                    };
                    let sample = process_modulation(
                        &mut unit,
                        &mut routes,
                        0.0,
                        local,
                        bus,
                        Default::default(),
                    );
                    assert!(sample.is_finite());
                    assert!(unit.effective_gain() <= 10_f32.powf(12.0 / 20.0));
                }
                assert!((20.0 * unit.effective_gain().log10() - 11.9588).abs() < 0.001);
                unit.set_base_gain(1.0);
                process_modulation(
                    &mut unit,
                    &mut routes,
                    0.0,
                    &[value],
                    &[],
                    Default::default(),
                );
                assert!((20.0 * unit.effective_gain().log10() - 12.0).abs() < 0.001);
                unit.set_base_gain(0.0);
                assert_eq!(
                    process_modulation(
                        &mut unit,
                        &mut routes,
                        0.0,
                        &[value],
                        &[],
                        Default::default()
                    ),
                    0.0
                );
                unit.set_base_gain(0.25);
                let mut opposite = route;
                opposite.amount = -24.0;
                let mut cancelled = prepare_target_routes(&[route, opposite], settings.id, rate);
                let before = unit.effective_gain();
                for _ in 0..rate / 20 {
                    process_modulation(
                        &mut unit,
                        &mut cancelled,
                        0.0,
                        &[value],
                        &[],
                        Default::default(),
                    );
                }
                assert!(before < unit.effective_gain()); // silent base recovers without resetting phase
                assert!((unit.effective_gain() - 0.25).abs() < 1e-6);
                unit.reset();
                assert_eq!(unit.effective_gain(), 0.25);
                assert_eq!(unit.pitch_offset(), 0.0);
            }
        }
    }

    #[test]
    fn identified_routes_preserve_neutral_audio_and_measure_fractional_semitones_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            let settings = OscillatorSettings::default();
            let other = OscillatorSettings::default();
            let source = ModulationSourceId {
                layer: settings.id,
                unit: other.id,
            };
            let value = ModulationValue {
                source,
                value: 1.0,
                weight: 1.0,
                range: ModulationRange::Unipolar,
                behavior: ModulationBehavior::Contour,
                release_age: None,
            };
            let mut route = Modulation::new(ModulationParameter::Pitch);
            route.source = Some(source);
            route.target = Some(settings.id);
            route.amount = 0.25;
            let mut routes = prepare_target_routes(&[route], settings.id, rate);
            let mut shifted = Oscillator::from_settings(&settings, context);
            let mut neutral = Oscillator::from_settings(&other, context);
            let mut reference = Oscillator::new(Waveform::Sine, context);
            let mut wrong_target = [ModulationRoute::new(route, rate)];
            let mut previous = 0.0;
            let mut crossings = 0;
            for frame in 0..rate {
                // Unrelated destinations reject the route even when a caller passes the whole list.
                assert_eq!(
                    process_modulation(
                        &mut neutral,
                        &mut wrong_target,
                        0.0,
                        &[value],
                        &[],
                        Default::default()
                    ),
                    reference.next_sample()
                );
                let sample = process_modulation(
                    &mut shifted,
                    &mut routes,
                    0.0,
                    &[value],
                    &[],
                    Default::default(),
                );
                assert!(sample.is_finite());
                if frame > rate / 10 && previous <= 0.0 && sample > 0.0 {
                    crossings += 1;
                }
                previous = sample;
            }
            let measured = crossings as f32 / 0.9;
            assert!(
                (measured - 440.0 * 2.0_f32.powf(0.25 / 12.0)).abs() < 1.5,
                "{rate}: {measured}"
            );
            assert!((shifted.pitch_offset() - 0.25).abs() < 1e-6);
            // Equal, opposite routes cancel without resetting source phase.
            route.amount = 12.0;
            let mut opposite = route;
            opposite.amount = -12.0;
            let mut cancelled = prepare_target_routes(&[route, opposite], settings.id, rate);
            shifted.reset();
            reference.reset();
            for _ in 0..1000 {
                assert_eq!(
                    process_modulation(
                        &mut shifted,
                        &mut cancelled,
                        0.0,
                        &[],
                        &[value],
                        Default::default()
                    ),
                    reference.next_sample()
                );
            }
            // Positive offsets at Nyquist are silent, finite, and return to audible range.
            shifted.set_frequency(rate as f32 / 2.0);
            for _ in 0..1000 {
                assert_eq!(
                    process_modulation(
                        &mut shifted,
                        &mut cancelled,
                        0.0,
                        &[],
                        &[value],
                        Default::default()
                    ),
                    0.0
                );
            }
            shifted.set_frequency(440.0);
            assert!((0..1000).any(|_| {
                process_modulation(
                    &mut shifted,
                    &mut cancelled,
                    0.0,
                    &[],
                    &[value],
                    Default::default(),
                )
                .abs()
                    > 0.1
            }));
        }
    }
}
