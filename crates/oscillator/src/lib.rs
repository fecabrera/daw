//! FunDSP adapter. No backend types appear in the unit API or persisted settings.
use daw_dsp_support::oscillator::{OscillatorBank, Waveform as DspWaveform};
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationInputs, ModulationParameter,
    ModulationScope, ModulationSmoother, ModulationTargetId, OSCILLATOR_TYPE_ID,
    OscillatorSettings, PortKind, ProcessingContext, SourceUnit, UnitDescriptor, Waveform,
};

fn dsp_waveform(waveform: Waveform) -> DspWaveform {
    match waveform {
        Waveform::Sine => DspWaveform::Sine,
        Waveform::Saw => DspWaveform::Saw,
        Waveform::Square => DspWaveform::Square,
        Waveform::Triangle => DspWaveform::Triangle,
    }
}

#[derive(Clone)]
pub struct Oscillator {
    kernel: OscillatorBank,
    frequency: f32,
    target: Option<ModulationTargetId>,
    offsets: ModulationSmoother<4>,
    effective_offset: f32,
    output_pitch_ratio: f32,
    base_gain: f32,
    effective_gain: f32,
}
impl Oscillator {
    pub fn heap_bytes(&self) -> u64 {
        self.kernel.heap_bytes()
    }

    /// Construct and allocate on a control thread, before entering audio processing.
    pub fn new(waveform: Waveform, context: ProcessingContext) -> Self {
        Self {
            kernel: OscillatorBank::new(dsp_waveform(waveform), context.sample_rate_hz),
            frequency: 440.0,
            target: None,
            offsets: ModulationSmoother::new(context),
            effective_offset: 0.0,
            output_pitch_ratio: 1.0,
            base_gain: 1.0,
            effective_gain: 1.0,
        }
    }

    /// Prepare an identified instance. Hosts keep MIDI pitch and base transpose in the base frequency.
    pub fn from_settings(settings: &OscillatorSettings, context: ProcessingContext) -> Self {
        let mut source = Self::new(settings.waveform, context);
        source.target = Some(settings.id);
        source.kernel.prepare_voices(
            settings.unison.voices(),
            settings.cents,
            settings.detune_cents,
            settings.width,
        );
        source
    }

    /// Processed fine tuning plus modulation, in semitones; excludes MIDI/coarse pitch.
    pub fn pitch_offset(&self) -> f32 {
        self.effective_offset + self.kernel.cents() / 100.0
    }
    /// Common unit tuning, prepared once per note by OscillatorOutput.
    /// Keep source Pitch modulation independent of this ratio.
    pub fn set_output_pitch_ratio(&mut self, ratio: f32) {
        self.output_pitch_ratio = if ratio.is_finite() && ratio > 0.0 {
            ratio.clamp(1.0 / 8192.0, 8192.0)
        } else {
            1.0
        };
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
    /// Continuous changes retain every prepared oscillator phase and allocate nothing.
    pub fn compatible(&self, next: &Self) -> bool {
        self.kernel.compatible(&next.kernel)
    }

    pub fn retarget(&mut self, next: &Self) -> bool {
        self.kernel.retarget(&next.kernel)
    }

    pub fn effective_cents(&self) -> f32 {
        self.kernel.cents()
    }
    pub fn effective_width(&self) -> f32 {
        self.kernel.width()
    }
    pub fn next_stereo_sample(&mut self) -> [f32; 2] {
        self.kernel.sample(self.frequency, [0.0; 2])
    }
}
impl ModulationDestination for Oscillator {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = &ModulationParameter::OSCILLATOR;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        self.target
    }
    /// A generator has no input audio; the argument is unused. Set base frequency and gain first.
    fn process_with_modulation(
        &mut self,
        _input: [f32; 2],
        modulation: &ModulationInputs,
        _context: ModulationContext,
    ) -> [f32; 2] {
        let [offset, gain_offset, cents_offset, width_offset] = self.offsets.process([
            modulation.offset(ModulationParameter::Pitch),
            modulation.offset(ModulationParameter::OscillatorGain),
            modulation.offset(ModulationParameter::OscillatorCents),
            modulation.offset(ModulationParameter::OscillatorWidth),
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
        let frequency = if self.output_pitch_ratio == 1.0 {
            frequency
        } else {
            frequency * self.output_pitch_ratio
        };
        self.kernel
            .sample(frequency, [cents_offset as f32, width_offset as f32])
            .map(|v| v * self.effective_gain)
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
        self.kernel.reset();
        self.offsets.reset();
        self.effective_gain = self.base_gain;
        self.effective_offset = 0.0;
    }
    fn set_frequency(&mut self, hz: f32) {
        self.frequency = if hz.is_finite() && hz > 0.0 { hz } else { 0.0 };
    }

    fn next_sample(&mut self) -> f32 {
        let [left, right] = self.next_stereo_sample();
        if left == right {
            left
        } else {
            (left + right) * 0.5
        }
    }
}

/// The original SourceUnit interface remains a mono downmix adapter.
impl daw_unit_core::StereoSourceUnit for Oscillator {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            outputs: &[PortKind::StereoAudio],
            ..SourceUnit::descriptor(self)
        }
    }
    fn reset(&mut self) {
        SourceUnit::reset(self);
    }
    fn next_sample(&mut self) -> [f32; 2] {
        self.next_stereo_sample()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{
        Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
        ModulationValue, prepare_target_routes,
    };

    #[test]
    fn oscillator_cents_and_width_share_targeted_resolution_smoothing_bounds_and_cancellation() {
        use daw_unit_core::OscillatorUnison;
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for shared in [false, true] {
                let context = ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                };
                let settings = OscillatorSettings {
                    cents: 20.0,
                    width: 40.0,
                    unison: OscillatorUnison::Four,
                    ..Default::default()
                };
                let id = ModulationSourceId {
                    layer: settings.id,
                    unit: OscillatorSettings::default().id,
                };
                let value = ModulationValue {
                    source: id,
                    value: 1.0,
                    weight: 1.0,
                    range: ModulationRange::Unipolar,
                    behavior: ModulationBehavior::Continuous,
                    release_age: None,
                };
                let mut cents = Modulation::new(ModulationParameter::OscillatorCents);
                cents.target = Some(settings.id);
                cents.source = Some(id);
                cents.amount = 30.0;
                let mut width = Modulation::new(ModulationParameter::OscillatorWidth);
                width.target = Some(settings.id);
                width.source = Some(id);
                width.amount = -25.0;
                let mut source = Oscillator::from_settings(&settings, context);
                let mut routes = prepare_target_routes(&[cents, width], settings.id, rate);
                let local = if shared {
                    &[][..]
                } else {
                    std::slice::from_ref(&value)
                };
                let global = if shared {
                    std::slice::from_ref(&value)
                } else {
                    &[][..]
                };
                for _ in 0..rate / 20 {
                    let frame = daw_unit_core::process_modulation(
                        &mut source,
                        &mut routes,
                        [0.0; 2],
                        local,
                        global,
                        Default::default(),
                    );
                    assert!(frame.iter().all(|v| v.is_finite()));
                }
                assert!((source.effective_cents() - 50.0).abs() < 0.001);
                assert!((source.effective_width() - 15.0).abs() < 0.001);
                let mut minus_cents = cents;
                minus_cents.amount = -cents.amount;
                let mut minus_width = width;
                minus_width.amount = -width.amount;
                let mut cancelled = prepare_target_routes(
                    &[cents, width, minus_cents, minus_width],
                    settings.id,
                    rate,
                );
                source.reset();
                let mut reference = Oscillator::from_settings(&settings, context);
                for _ in 0..1024 {
                    assert_eq!(
                        daw_unit_core::process_modulation(
                            &mut source,
                            &mut cancelled,
                            [0.0; 2],
                            local,
                            global,
                            Default::default()
                        ),
                        reference.next_stereo_sample()
                    );
                }
                cents.amount = 100.0;
                width.amount = -100.0;
                let mut bounded =
                    prepare_target_routes(&[cents, cents, width, width], settings.id, rate);
                for _ in 0..rate / 20 {
                    daw_unit_core::process_modulation(
                        &mut source,
                        &mut bounded,
                        [0.0; 2],
                        local,
                        global,
                        Default::default(),
                    );
                }
                assert_eq!(source.effective_cents(), 100.0);
                assert_eq!(source.effective_width(), 0.0);
                source.reset();
                assert_eq!(source.effective_cents(), 20.0);
                assert_eq!(source.effective_width(), 40.0);
            }
        }
    }

    #[test]
    fn cents_and_unison_are_stereo_bounded_deterministic_and_phase_continuous() {
        use daw_unit_core::OscillatorUnison;
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            for waveform in [
                Waveform::Sine,
                Waveform::Saw,
                Waveform::Square,
                Waveform::Triangle,
            ] {
                for mode in OscillatorUnison::ALL {
                    let settings = OscillatorSettings {
                        waveform,
                        unison: mode,
                        cents: 100.0,
                        detune_cents: 100.0,
                        width: 100.0,
                        ..Default::default()
                    };
                    let mut source = Oscillator::from_settings(&settings, context);
                    let mut copy = source.clone();
                    let mut stereo = false;
                    for _ in 0..1024 {
                        let frame = source.next_stereo_sample();
                        assert_eq!(frame, copy.next_stereo_sample());
                        assert!(frame.iter().all(|v| v.is_finite() && v.abs() < 3.0));
                        stereo |= frame[0] != frame[1];
                    }
                    assert_eq!(stereo, mode != OscillatorUnison::Off);
                    source.reset();
                    copy.reset();
                    for _ in 0..128 {
                        assert_eq!(source.next_stereo_sample(), copy.next_stereo_sample());
                    }
                    let mut target = settings.clone();
                    target.width = 0.0;
                    target.cents = -100.0;
                    assert!(source.retarget(&Oscillator::from_settings(&target, context)));
                    for _ in 0..rate / 200 {
                        source.next_stereo_sample();
                    }
                    for _ in 0..128 {
                        let [l, r] = source.next_stereo_sample();
                        assert_eq!(l, r);
                    }
                    assert_eq!(source.pitch_offset(), -1.0);
                    target.unison = if mode == OscillatorUnison::Off {
                        OscillatorUnison::Two
                    } else {
                        OscillatorUnison::Off
                    };
                    assert!(!source.retarget(&Oscillator::from_settings(&target, context)));
                }
                // Fine tuning equals a base frequency change in the single-voice path.
                let settings = OscillatorSettings {
                    waveform,
                    cents: 100.0,
                    ..Default::default()
                };
                let mut tuned = Oscillator::from_settings(&settings, context);
                let mut reference = Oscillator::new(waveform, context);
                reference.set_frequency(440.0 * 2_f32.powf(100.0 / 1200.0));
                for _ in 0..1024 {
                    assert_eq!(tuned.next_stereo_sample(), [reference.next_sample(); 2]);
                }
            }
        }
    }

    fn process_modulation(
        unit: &mut Oscillator,
        routes: &mut [ModulationRoute],
        input: f32,
        local: &[ModulationValue],
        shared: &[ModulationValue],
        context: ModulationContext<'_>,
    ) -> f32 {
        daw_unit_core::process_modulation(unit, routes, [input; 2], local, shared, context)[0]
    }

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

mod level;
pub use level::OscillatorLevel;

/// Shared output Pitch and Level controls; retains the original Level API.
pub use level::OscillatorLevel as OscillatorOutput;
