//! Stereo-linked transient shaping using fast and slow peak envelopes.
use daw_unit_core::{
    EffectUnit, LevelMonitor, ModulationContext, ModulationDestination,
    ModulationDestinationSettings, ModulationInputs, ModulationParameter, ModulationRamp,
    ModulationScope, ModulationSmoother, PortKind, ProcessingContext, SILENT_GAIN_DB,
    SaturatorClipMode, TRANSIENT_SHAPER_TYPE_ID, TransientShaperClipMode, TransientShaperSettings,
    UnitDescriptor,
};
use std::{
    array,
    sync::{Arc, atomic::AtomicU32},
};

fn gain(db: f64) -> f64 {
    if db <= f64::from(SILENT_GAIN_DB) {
        0.0
    } else {
        10_f64.powf(db / 20.0)
    }
}

fn level_db(amplitude: f64) -> f32 {
    if amplitude > 0.0 {
        (20.0 * amplitude.log10()) as f32
    } else {
        SILENT_GAIN_DB
    }
}

#[derive(Clone)]
pub struct TransientShaper {
    parameters: [ModulationRamp; 6],
    effective: [f32; 6],
    external: ModulationSmoother<6>,
    power: ModulationRamp,
    clip_mix: [ModulationRamp; 4],
    // Update coefficients for fast rise, fast fall, slow rise, and slow fall.
    smooth: [f64; 4],
    snappy: [f64; 4],
    fast_envelope: f64,
    slow_envelope: f64,
    fade_frames: u32,
    monitor: LevelMonitor<3>,
}

impl TransientShaper {
    fn ramp_values(settings: &TransientShaperSettings) -> [f32; 6] {
        let mut values = settings.values();
        values[4] = gain(f64::from(settings.level_db)) as f32;
        values
    }

    pub fn new(context: ProcessingContext, settings: &TransientShaperSettings) -> Self {
        let rate = f64::from(context.sample_rate_hz.max(1));
        let coefficients = |times: [f64; 4]| times.map(|ms| -(-1000.0 / (rate * ms)).exp_m1());
        Self {
            parameters: Self::ramp_values(settings).map(ModulationRamp::new),
            effective: settings.values(),
            external: ModulationSmoother::new(context),
            power: ModulationRamp::new(f32::from(settings.enabled)),
            clip_mix: TransientShaperClipMode::ALL
                .map(|mode| ModulationRamp::new(f32::from(mode == settings.clip_mode))),
            smooth: coefficients([0.8, 25.0, 24.0, 150.0]),
            snappy: coefficients([0.06, 5.0, 5.0, 35.0]),
            fast_envelope: 0.0,
            slow_envelope: 0.0,
            fade_frames: (context.sample_rate_hz / 200).max(1),
            monitor: LevelMonitor::new(context.sample_rate_hz, settings.enabled),
        }
    }

    pub fn retarget(&mut self, settings: &TransientShaperSettings) {
        self.monitor.set_enabled(settings.enabled);
        self.power
            .set(f32::from(settings.enabled), self.fade_frames);
        for (ramp, target) in self.parameters.iter_mut().zip(Self::ramp_values(settings)) {
            ramp.set(target, self.fade_frames);
        }
        for (ramp, mode) in self.clip_mix.iter_mut().zip(TransientShaperClipMode::ALL) {
            ramp.set(f32::from(settings.clip_mode == mode), self.fade_frames);
        }
    }

    pub fn set_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }

    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }

    pub fn effective_parameters(&self) -> [f32; 6] {
        self.effective
    }

    fn shape_db(&mut self, peak: f64, [attack, pump, sustain, speed]: [f64; 4]) -> f64 {
        let update = |envelope: f64, rise: usize| {
            let index = rise + usize::from(peak < envelope);
            let coefficient =
                self.smooth[index] + (self.snappy[index] - self.smooth[index]) * speed;
            envelope + (peak - envelope) * coefficient
        };
        self.fast_envelope = update(self.fast_envelope, 0);
        self.slow_envelope = update(self.slow_envelope, 2);

        let fast = self.fast_envelope;
        let slow = self.slow_envelope;
        // The floor keeps signals below -120 dBFS from driving gain changes.
        let reference = fast.max(slow).max(1e-6);
        let onset = ((fast - slow) / reference).max(0.0);
        let decay = ((slow - fast) / reference).max(0.0);
        // Preserve the attack region when changing sustain, and fade shaping out in silence.
        let body = (slow / reference) * (1.0 - onset);
        attack * onset - pump * decay + sustain * body
    }

    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 6]) -> [f32; 2] {
        let values = self.parameters.each_mut().map(ModulationRamp::next_value);
        let offsets = self.external.process(offsets);
        let target = |index: usize, min: f64, max: f64| {
            (f64::from(values[index]) + offsets[index]).clamp(min, max)
        };
        let attack = target(0, -12.0, 12.0);
        let pump = target(1, 0.0, 12.0);
        let sustain = target(2, -12.0, 12.0);
        let speed = target(3, 0.0, 1.0);
        // Apply the Level offset in dB while preserving exact silence at the minimum.
        let output_gain =
            (f64::from(values[4]) * gain(offsets[4].clamp(-96.0, 96.0))).min(gain(12.0));
        let mix = target(5, 0.0, 1.0);
        self.effective = [
            attack as f32,
            pump as f32,
            sustain as f32,
            speed as f32,
            level_db(output_gain),
            mix as f32,
        ];

        let dry = input.map(|v| if v.is_finite() { v } else { 0.0 });
        let peak = f64::from(dry[0].abs().max(dry[1].abs()));
        let shape_db = self.shape_db(peak, [attack, pump, sustain, speed]);
        let weights = self
            .clip_mix
            .each_mut()
            .map(|ramp| f64::from(ramp.next_value()));
        let wet_mix = f64::from(self.power.next_value()) * mix;
        if wet_mix == 0.0 || (shape_db == 0.0 && output_gain == 1.0 && weights[0] == 1.0) {
            self.monitor.sample_output(dry);
            return dry;
        }

        let shape_gain = gain(shape_db);
        let weight_sum = weights.iter().sum::<f64>();
        let modes = [
            SaturatorClipMode::NoClip,
            SaturatorClipMode::SoftClip,
            SaturatorClipMode::HardClip,
            SaturatorClipMode::TubeClip,
        ];
        let output = array::from_fn(|ch| {
            let sample = f64::from(dry[ch]);
            let shaped = sample * shape_gain;
            let clipped = weights
                .iter()
                .zip(modes)
                .filter(|(weight, _)| **weight != 0.0)
                .map(|(weight, mode)| weight * daw_dsp_support::waveshaping::clip(mode, shaped))
                .sum::<f64>()
                / weight_sum;
            let wet = clipped * output_gain;
            (sample * (1.0 - wet_mix) + wet * wet_mix)
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        });
        self.monitor.sample_output(output);
        output
    }
}

impl EffectUnit for TransientShaper {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: TRANSIENT_SHAPER_TYPE_ID,
            name: "Transient Shaper",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }

    fn reset(&mut self) {
        self.fast_envelope = 0.0;
        self.slow_envelope = 0.0;
        self.monitor.reset();
        self.external.reset();
        self.power.reset();
        for ramp in self.parameters.iter_mut().chain(&mut self.clip_mix) {
            ramp.reset();
        }
        self.effective = self.parameters.each_ref().map(ModulationRamp::target);
        self.effective[4] = level_db(f64::from(self.effective[4]));
    }

    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 6])
    }
}

impl ModulationDestination for TransientShaper {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        TransientShaperSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = TransientShaperSettings::MODULATION_SCOPE;

    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(ModulationParameter::TRANSIENT_SHAPER),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(sample_rate_hz: u32) -> ProcessingContext {
        ProcessingContext {
            sample_rate_hz,
            max_block_frames: 512,
        }
    }

    #[test]
    fn attack_pump_sustain_and_speed_act_in_different_parts_of_the_hit() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let shape = |attack_db, pump_db, sustain_db, speed| {
                let mut dsp = TransientShaper::new(
                    context(rate),
                    &TransientShaperSettings {
                        attack_db,
                        pump_db,
                        sustain_db,
                        speed,
                        ..Default::default()
                    },
                );
                let ms = |time| rate * time / 1000;
                let onset = dsp.process_sample([0.4, -0.2]);
                assert_eq!(onset[0], -2.0 * onset[1]);
                for _ in 0..ms(100) {
                    dsp.process_sample([0.4, -0.2]);
                }
                let steady = dsp.process_sample([0.4, -0.2])[0];
                for _ in 0..ms(20) {
                    dsp.process_sample([0.06, -0.03]);
                }
                let decay = dsp.process_sample([0.06, -0.03])[0];
                (onset[0], steady, decay)
            };
            let boost = shape(12.0, 0.0, 0.0, 0.5);
            let cut = shape(-12.0, 0.0, 0.0, 0.5);
            assert!(boost.0 > 0.9 && cut.0 < 0.19);
            assert!((boost.1 - 0.4).abs() < 0.02);
            assert!(shape(0.0, 12.0, 0.0, 0.5).2 < 0.041);
            assert!(shape(0.0, 0.0, 6.0, 0.5).1 > 0.77);
            let slow = shape(0.0, 12.0, 0.0, 0.0);
            let fast = shape(0.0, 12.0, 0.0, 1.0);
            assert!(fast.2 < slow.2);
        }
    }

    #[test]
    fn clip_modes_mix_silence_and_bypass_keep_their_signal_order() {
        use TransientShaperClipMode::*;
        let mut settings = TransientShaperSettings::default();
        let ctx = context(48_000);
        let input = [1.4, -1.4];
        let mut dsp = TransientShaper::new(ctx, &settings);
        assert_eq!(dsp.process_sample(input), input);
        assert_eq!(dsp.latency_frames(), 0);
        assert_eq!(dsp.tail_frames(), 0);

        settings.clip_mode = Hard;
        let mut dsp = TransientShaper::new(ctx, &settings);
        assert_eq!(dsp.process_sample(input), [1.0, -1.0]);
        settings.level_db = 6.0;
        settings.dry_wet = 0.5;
        assert!(
            (TransientShaper::new(ctx, &settings).process_sample(input)[0]
                - (1.4 + gain(6.0) as f32) * 0.5)
                .abs()
                < 1e-6
        );

        settings = TransientShaperSettings::default();
        settings.clip_mode = Soft;
        let soft = TransientShaper::new(ctx, &settings).process_sample(input);
        assert!(soft[0] < 1.0 && soft[0] > 0.5);
        assert_eq!(soft[0], -soft[1]);
        settings.clip_mode = Tube;
        let mut tube = TransientShaper::new(ctx, &settings);
        let saturated = tube.process_sample(input);
        assert_ne!(saturated[0], -saturated[1]);
        assert_eq!(tube.process_sample([0.0; 2]), [0.0; 2]);

        settings.enabled = false;
        tube.retarget(&settings);
        assert_ne!(tube.process_sample(input), input);
        for _ in 0..240 {
            tube.process_sample(input);
        }
        assert_eq!(tube.process_sample(input), input);
        assert_eq!(tube.process_sample([f32::NAN, f32::INFINITY]), [0.0; 2]);
    }

    #[test]
    fn continuous_modulation_is_bounded_and_reset_clears_detector_history() {
        let settings = TransientShaperSettings {
            pump_db: 4.0,
            sustain_db: 4.0,
            level_db: SILENT_GAIN_DB,
            ..Default::default()
        };
        let ctx = context(192_000);
        let mut dsp = TransientShaper::new(ctx, &settings);
        for _ in 0..4096 {
            assert_eq!(
                dsp.process_modulated([f32::MAX, -f32::MAX], [100.0; 6]),
                [0.0; 2]
            );
        }
        assert_eq!(
            dsp.effective_parameters(),
            [12.0, 12.0, 12.0, 1.0, SILENT_GAIN_DB, 1.0]
        );
        let settings = TransientShaperSettings {
            level_db: 0.0,
            ..settings
        };
        dsp.retarget(&settings);
        dsp.reset();
        assert_eq!(dsp.effective_parameters(), settings.values());
        assert_eq!(
            dsp.process_sample([0.4, -0.1]),
            TransientShaper::new(ctx, &settings).process_sample([0.4, -0.1]),
        );
    }
}
