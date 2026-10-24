//! Transport-locked stereo auto-filter. FunDSP supplies Simper SVF coefficients;
//! the shared integrator state exposes all four shapes without parallel filters.
use daw_filter::Filter;
use daw_unit_core::{
    AUTO_FILTER_TYPE_ID, AutoFilterSettings, EffectUnit, LfoRateMode, PortKind, ProcessingContext,
    UnitDescriptor,
};

#[derive(Clone, Copy)]
struct Ramp {
    value: f64,
    target: f64,
    remaining: u32,
}
impl Ramp {
    fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn next(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.value
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}
fn weights<const N: usize>(index: usize) -> [Ramp; N] {
    std::array::from_fn(|i| Ramp::new(f64::from(i == index)))
}
use daw_lfo::waveform as wave;

#[derive(Clone)]
pub struct AutoFilter {
    fade: u32,
    filter: Filter,
    depth: Ramp,
    lfo_mix: Ramp,
    sync_phase: Ramp,
    stereo_phase: Ramp,
    waves: [Ramp; 5],
    rate_mode: LfoRateMode,
    rate_hz: f32,
    note_beats: f64,
    modulation: [f64; 2],
    modulation_initialized: bool,
    modulation_alpha: f64,
    lfo_offsets: daw_unit_core::ModulationSmoother<3>,
    effective_lfo: [f32; 3],
    standalone_frame: u64,
    clock: daw_lfo::LfoClock,
}
impl AutoFilter {
    pub fn new(context: ProcessingContext, settings: &AutoFilterSettings) -> Self {
        let sample_rate = f64::from(context.sample_rate_hz);
        Self {
            fade: (context.sample_rate_hz / 200).max(1),
            filter: Filter::new(context, &filter_settings(settings)),
            depth: Ramp::new(settings.strength_octaves.into()),
            lfo_mix: Ramp::new(f64::from(settings.lfo_enabled)),
            sync_phase: Ramp::new(f64::from(settings.sync_phase_degrees) / 360.0),
            stereo_phase: Ramp::new(f64::from(settings.stereo_phase_degrees) / 360.0),
            waves: weights(settings.wave.index()),
            rate_mode: settings.rate_mode,
            rate_hz: settings.rate_hz,
            note_beats: settings.note_rate.beats(),
            modulation: [0.0; 2],
            modulation_initialized: false,
            modulation_alpha: 1.0 - (-1.0 / (0.001 * sample_rate)).exp(),
            lfo_offsets: daw_unit_core::ModulationSmoother::new(context),
            effective_lfo: [
                settings.strength_octaves,
                settings.sync_phase_degrees,
                settings.stereo_phase_degrees,
            ],
            standalone_frame: 0,
            clock: daw_lfo::LfoClock::new(context),
        }
    }
    /// Last processed cutoff L/R (Hz), resonance (Q), output level (dB), and wet fraction.
    /// These include parameter ramps, external smoothing, bounds, and the stereo LFO.
    pub fn effective_parameters(&self) -> [f32; 5] {
        self.filter.effective_parameters()
    }
    /// Existing filter readings followed by processed Strength (oct), Sync and Stereo phase (degrees).
    pub fn modulation_parameters(&self) -> [f32; 8] {
        let filter = self.effective_parameters();
        std::array::from_fn(|i| {
            if i < 5 {
                filter[i]
            } else {
                self.effective_lfo[i - 5]
            }
        })
    }
    /// Called on compatible state publication; histories and unrelated ramps remain intact.
    pub fn retarget(&mut self, settings: &AutoFilterSettings) {
        self.filter.retarget(&filter_settings(settings));
        self.depth
            .retarget(settings.strength_octaves.into(), self.fade);
        self.lfo_mix
            .retarget(f64::from(settings.lfo_enabled), self.fade);
        self.sync_phase
            .retarget(f64::from(settings.sync_phase_degrees) / 360.0, self.fade);
        self.stereo_phase
            .retarget(f64::from(settings.stereo_phase_degrees) / 360.0, self.fade);
        for (i, ramp) in self.waves.iter_mut().enumerate() {
            ramp.retarget(f64::from(i == settings.wave.index()), self.fade);
        }
        self.rate_mode = settings.rate_mode;
        self.rate_hz = settings.rate_hz;
        self.note_beats = settings.note_rate.beats();
    }
    fn phase(&mut self, frame: u64, tempo_bpm: f32) -> f64 {
        let rate = daw_lfo::rate_hz(self.rate_mode, self.rate_hz, self.note_beats, tempo_bpm);
        self.clock.phase_at(frame, 0, rate)
    }
    /// Explicit host position makes seeks, loop wraps and export phase deterministic.
    pub fn process_at(&mut self, input: [f32; 2], frame: u64, tempo_bpm: f32) -> [f32; 2] {
        self.process_modulated_at(input, frame, tempo_bpm, [0.0; 4])
    }
    /// External offsets: cutoff octaves, Q, output dB, wet fraction. The internal LFO remains independent.
    pub fn process_modulated_at(
        &mut self,
        input: [f32; 2],
        frame: u64,
        tempo_bpm: f32,
        offsets: [f32; 4],
    ) -> [f32; 2] {
        self.process_offsets_at(
            input,
            frame,
            tempo_bpm,
            [
                offsets[0], offsets[1], offsets[2], offsets[3], 0.0, 0.0, 0.0,
            ],
        )
    }
    fn process_offsets_at(
        &mut self,
        input: [f32; 2],
        frame: u64,
        tempo_bpm: f32,
        offsets: [f32; 7],
    ) -> [f32; 2] {
        let [strength, sync, stereo] = self
            .lfo_offsets
            .process([offsets[4], offsets[5], offsets[6]]);
        let strength = (self.depth.next() + strength).clamp(0.0, 6.0);
        let sync = (self.sync_phase.next() + sync / 360.0).clamp(-0.5, 0.5);
        let stereo = (self.stereo_phase.next() + stereo / 360.0).clamp(-0.5, 0.5);
        self.effective_lfo = [
            strength as f32,
            (sync * 360.0) as f32,
            (stereo * 360.0) as f32,
        ];
        let phase = self.phase(frame, tempo_bpm);
        let depth = strength * self.lfo_mix.next();
        let waves: [f64; 5] = std::array::from_fn(|i| self.waves[i].next());
        let mut cutoff_offsets = [0.0; 2];
        for (ch, cutoff_offset) in cutoff_offsets.iter_mut().enumerate() {
            let channel_phase = phase + sync + if ch == 1 { stereo } else { 0.0 };
            let modulation: f64 = waves
                .iter()
                .enumerate()
                .map(|(i, weight)| wave(i, channel_phase) * weight)
                .sum();
            if !self.modulation_initialized {
                self.modulation[ch] = modulation;
            } else {
                self.modulation[ch] += self.modulation_alpha * (modulation - self.modulation[ch]);
            }
            *cutoff_offset = depth * self.modulation[ch];
        }
        self.modulation_initialized = true;
        self.filter.process_with_cutoff_offsets(
            input,
            [offsets[0], offsets[1], offsets[2], offsets[3]],
            cutoff_offsets,
        )
    }
}
impl EffectUnit for AutoFilter {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: AUTO_FILTER_TYPE_ID,
            name: "Auto-filter",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for ramp in [
            &mut self.depth,
            &mut self.lfo_mix,
            &mut self.sync_phase,
            &mut self.stereo_phase,
        ] {
            ramp.reset();
        }
        for ramp in &mut self.waves {
            ramp.reset();
        }
        self.lfo_offsets.reset();
        self.effective_lfo = [
            self.depth.target as f32,
            (self.sync_phase.target * 360.0) as f32,
            (self.stereo_phase.target * 360.0) as f32,
        ];
        self.filter.reset();
        self.clock.reset();
        self.modulation_initialized = false;
        self.standalone_frame = 0;
    }
    /// Standalone hosts without a transport use 120 BPM and an advancing frame counter.
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        let output = self.process_at(input, self.standalone_frame, 120.0);
        self.standalone_frame = self.standalone_frame.saturating_add(1);
        output
    }
}

impl daw_unit_core::ModulationDestination for AutoFilter {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [daw_unit_core::ModulationParameter] = <daw_unit_core::AutoFilterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: daw_unit_core::ModulationScope = <daw_unit_core::AutoFilterSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &daw_unit_core::ModulationInputs,
        context: daw_unit_core::ModulationContext,
    ) -> Self::Audio {
        self.process_offsets_at(
            input,
            context.frame,
            context.tempo_bpm,
            modulation.offsets(daw_unit_core::ModulationParameter::AUTO_FILTER),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{FilterShape, LfoWave, NoteRate, SILENT_GAIN_DB};
    fn unit(rate: u32, settings: &AutoFilterSettings) -> AutoFilter {
        AutoFilter::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            },
            settings,
        )
    }
    fn amplitude(shape: FilterShape, frequency: f64, rate: u32) -> f64 {
        let mut filter = unit(
            rate,
            &AutoFilterSettings {
                shape,
                lfo_enabled: false,
                ..Default::default()
            },
        );
        let mut power = 0.0;
        for frame in 0..rate / 4 {
            let signal = (f64::from(frame) * frequency * std::f64::consts::TAU / f64::from(rate))
                .sin() as f32;
            let output = filter.process_at([signal; 2], u64::from(frame), 120.0);
            if frame > rate / 8 {
                power += f64::from(output[0]).powi(2);
            }
        }
        (power / f64::from(rate / 8) * 2.0).sqrt()
    }
    #[test]
    fn shapes_have_expected_static_responses_at_all_project_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            assert!(amplitude(FilterShape::LowPass, 100.0, rate) > 0.98);
            assert!(amplitude(FilterShape::LowPass, 8000.0, rate) < 0.03);
            assert!(amplitude(FilterShape::HighPass, 100.0, rate) < 0.03);
            assert!(amplitude(FilterShape::HighPass, 8000.0, rate) > 0.98);
            assert!(amplitude(FilterShape::BandPass, 1000.0, rate) > 0.98);
            assert!(amplitude(FilterShape::BandPass, 100.0, rate) < 0.2);
            assert!(amplitude(FilterShape::Notch, 1000.0, rate) < 0.001);
            assert!(amplitude(FilterShape::Notch, 100.0, rate) > 0.98);
        }
    }
    #[test]
    fn signed_phase_note_timing_tempo_and_transport_jumps_match_the_grid() {
        let mut filter = unit(48000, &AutoFilterSettings::default());
        assert_eq!(filter.phase(0, 120.0), 0.0);
        assert_eq!(filter.phase(6000, 120.0), 0.25);
        assert_eq!(filter.phase(12000, 60.0), 0.25);
        assert_eq!(filter.phase(24000, 120.0), 0.0);
        assert_eq!(filter.phase(6000, 120.0), 0.25);
        let mut settings = AutoFilterSettings {
            sync_phase_degrees: -90.0,
            stereo_phase_degrees: 180.0,
            ..Default::default()
        };
        let mut filter = unit(48000, &settings);
        filter.process_at([1.0; 2], 0, 120.0);
        assert!((filter.modulation[0] + 1.0).abs() < 1e-12);
        assert!((filter.modulation[1] - 1.0).abs() < 1e-12);
        settings.stereo_phase_degrees = -90.0;
        let mut filter = unit(48000, &settings);
        filter.process_at([0.0; 2], 6000, 120.0);
        assert!(filter.modulation[0].abs() < 1e-12);
        assert!((filter.modulation[1] + 1.0).abs() < 1e-12);
        for index in 0..NoteRate::COUNT {
            let note = NoteRate(index);
            let settings = AutoFilterSettings {
                note_rate: note,
                ..Default::default()
            };
            let mut filter = unit(192000, &settings);
            let frame = (note.beats() * 0.5 * 192000.0 * 0.25).round() as u64;
            assert!((filter.phase(frame, 120.0) - 0.25).abs() < 0.0001);
        }
    }
    #[test]
    fn hz_mode_ignores_tempo_and_rate_changes_realign_smoothly() {
        let mut settings = AutoFilterSettings {
            rate_mode: LfoRateMode::Hz,
            rate_hz: 1.0,
            ..Default::default()
        };
        let mut filter = unit(48000, &settings);
        assert_eq!(filter.phase(12000, 120.0), 0.25);
        assert_eq!(filter.phase(12000, 60.0), 0.25);
        settings.rate_hz = 3.0;
        filter.retarget(&settings);
        let first = filter.phase(12001, 60.0);
        assert!((first - 0.25).abs() < 0.001);
        for frame in 12002..12242 {
            filter.phase(frame, 60.0);
        }
        assert!((filter.phase(12242, 60.0) - (12242.0 * 3.0 / 48000.0)).abs() < 1e-12);
    }
    #[test]
    fn zero_depth_and_lfo_off_are_the_same_static_filter() {
        for shape in FilterShape::ALL {
            let mut a = unit(
                48000,
                &AutoFilterSettings {
                    shape: *shape,
                    lfo_enabled: false,
                    ..Default::default()
                },
            );
            let mut b = unit(
                48000,
                &AutoFilterSettings {
                    shape: *shape,
                    strength_octaves: 0.0,
                    stereo_phase_degrees: 120.0,
                    ..Default::default()
                },
            );
            for frame in 0..2000 {
                let input = [(frame as f32 * 0.13).sin(); 2];
                assert_eq!(
                    a.process_at(input, frame, 120.0),
                    b.process_at(input, frame, 120.0)
                );
            }
        }
    }
    #[test]
    fn bypass_dry_mix_level_and_stereo_histories_are_independent() {
        let input = [0.3, -0.2];
        for settings in [
            AutoFilterSettings {
                enabled: false,
                ..Default::default()
            },
            AutoFilterSettings {
                dry_wet: 0.0,
                ..Default::default()
            },
        ] {
            let mut filter = unit(48000, &settings);
            for frame in 0..1000 {
                assert_eq!(filter.process_at(input, frame, 120.0), input);
            }
        }
        let mut filter = unit(
            48000,
            &AutoFilterSettings {
                level_db: SILENT_GAIN_DB,
                ..Default::default()
            },
        );
        assert_eq!(filter.process_at(input, 0, 120.0), [0.0; 2]);
        let mut filter = unit(
            48000,
            &AutoFilterSettings {
                dry_wet: 0.0,
                level_db: -6.0,
                ..Default::default()
            },
        );
        let output = filter.process_at(input, 0, 120.0);
        assert!((output[0] - input[0] * 10_f32.powf(-6.0 / 20.0)).abs() < 1e-7);
        let mut filter = unit(48000, &AutoFilterSettings::default());
        for frame in 0..500 {
            assert_eq!(filter.process_at([1.0, 0.0], frame, 120.0)[1], 0.0);
        }
    }
    #[test]
    fn reset_is_deterministic_and_unrelated_edits_do_not_restart_ramps() {
        let mut settings = AutoFilterSettings::default();
        let mut filter = unit(48000, &settings);
        let expected: Vec<_> = (0..1000)
            .map(|f| filter.process_at([0.2; 2], f, 120.0))
            .collect();
        filter.reset();
        assert_eq!(
            expected,
            (0..1000)
                .map(|f| filter.process_at([0.2; 2], f, 120.0))
                .collect::<Vec<_>>()
        );
        settings.frequency_hz = 2000.0;
        filter.retarget(&settings);
        for frame in 1000..1100 {
            filter.process_at([0.2; 2], frame, 120.0);
        }
        let mut before_presentation = filter.clone();
        settings.collapsed = true;
        settings.modulation_open = true;
        filter.retarget(&settings);
        let mut after_presentation = filter.clone();
        for frame in 1100..1200 {
            assert_eq!(
                before_presentation.process_at([0.2; 2], frame, 120.0),
                after_presentation.process_at([0.2; 2], frame, 120.0)
            );
        }
        settings.enabled = false;
        filter.retarget(&settings);
        for frame in 1100..1400 {
            filter.process_at([0.2; 2], frame, 120.0);
        }
        assert_eq!(filter.process_at([0.2; 2], 1400, 120.0), [0.2; 2]);
    }
    #[test]
    fn extreme_controls_and_all_waves_remain_finite_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for wave in LfoWave::ALL {
                let settings = AutoFilterSettings {
                    wave: *wave,
                    q: 10.0,
                    frequency_hz: 20000.0,
                    strength_octaves: 6.0,
                    rate_mode: LfoRateMode::Hz,
                    rate_hz: 20.0,
                    stereo_phase_degrees: -180.0,
                    level_db: 12.0,
                    ..Default::default()
                };
                let mut filter = unit(rate, &settings);
                for frame in 0..rate / 10 {
                    let input = [(frame as f32 * 0.4).sin() * 8.0; 2];
                    assert!(
                        filter
                            .process_at(input, u64::from(frame), 120.0)
                            .iter()
                            .all(|value| value.is_finite())
                    );
                }
            }
        }
    }
    #[test]
    fn external_offsets_match_static_controls_and_neutralize_nonfinite_inputs() {
        let settings = AutoFilterSettings {
            lfo_enabled: false,
            frequency_hz: 200.0,
            q: 0.7,
            level_db: -6.0,
            dry_wet: 0.6,
            ..Default::default()
        };
        for (offsets, frequency_hz, q, level_db, dry_wet) in [
            ([2.0, 1.0, 3.0, -0.2], 800.0, 1.7, -3.0, 0.4),
            ([6.0, 10.0, 24.0, 1.0], 12_800.0, 10.0, 12.0, 1.0),
            ([-6.0, -10.0, -24.0, -1.0], 20.0, 0.1, -30.0, 0.0),
        ] {
            let mut modulated = unit(48_000, &settings);
            let mut reference = unit(
                48_000,
                &AutoFilterSettings {
                    frequency_hz,
                    q,
                    level_db,
                    dry_wet,
                    ..settings.clone()
                },
            );
            for frame in 0..24_000 {
                let input = [
                    (frame as f32 * 0.031).sin() * 0.25,
                    (frame as f32 * 0.047).cos() * 0.25,
                ];
                let actual = modulated.process_modulated_at(input, frame, 120.0, offsets);
                let expected = reference.process_at(input, frame, 120.0);
                if frame > 12_000 {
                    assert!((0..2).all(|ch| (actual[ch] - expected[ch]).abs() < 1e-5));
                }
            }
        }
        let mut modulated = unit(48_000, &settings);
        let mut reference = unit(48_000, &settings);
        for frame in 0..512 {
            assert_eq!(
                modulated.process_modulated_at(
                    [0.25, -0.5],
                    frame,
                    120.0,
                    [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN]
                ),
                reference.process_at([0.25, -0.5], frame, 120.0)
            );
        }
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;
    #[test]
    fn octave_depth_signed_stereo_phase_and_lfo_disable_use_the_expected_cutoffs() {
        let context = ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        };
        let mut settings = AutoFilterSettings {
            sync_phase_degrees: 90.0,
            stereo_phase_degrees: 180.0,
            ..Default::default()
        };
        let mut filter = AutoFilter::new(context, &settings);
        filter.process_at([0.5; 2], 0, 120.0);
        assert!((f64::from(filter.effective_parameters()[0]) - 2000.0).abs() < 1e-9);
        assert!((f64::from(filter.effective_parameters()[1]) - 500.0).abs() < 1e-9);
        settings.lfo_enabled = false;
        filter.retarget(&settings);
        for frame in 1..=240 {
            filter.process_at([0.5; 2], frame, 120.0);
        }
        assert!((f64::from(filter.effective_parameters()[0]) - 1000.0).abs() < 1e-9);
        assert_eq!(
            filter.effective_parameters()[0],
            filter.effective_parameters()[1]
        );
    }
    #[test]
    fn waves_have_bounded_known_values_and_resonance_changes_the_filter_response() {
        assert!(wave(0, 0.0).abs() < 1e-12);
        assert_eq!(wave(1, 0.25), 1.0);
        assert_eq!(wave(2, 0.0), -1.0);
        assert_eq!(wave(3, 0.0), 1.0);
        assert_eq!(wave(4, 0.0), 1.0);
        assert_eq!(wave(4, 0.5), -1.0);
        for index in 0..5 {
            for step in 0..1000 {
                assert!((-1.0..=1.0).contains(&wave(index, f64::from(step) / 1000.0)));
            }
        }
        let context = ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        };
        let mut low = AutoFilter::new(
            context,
            &AutoFilterSettings {
                lfo_enabled: false,
                q: 0.1,
                ..Default::default()
            },
        );
        let mut high = AutoFilter::new(
            context,
            &AutoFilterSettings {
                lfo_enabled: false,
                q: 10.0,
                ..Default::default()
            },
        );
        let mut power = [0.0; 2];
        for frame in 0..12000 {
            let input = [(frame as f32 * std::f32::consts::TAU / 48.0).sin(); 2];
            let a = low.process_at(input, frame, 120.0);
            let b = high.process_at(input, frame, 120.0);
            if frame > 6000 {
                power[0] += a[0] * a[0];
                power[1] += b[0] * b[0];
            }
        }
        assert!(power[1] > power[0] * 100.0);
    }
    #[test]
    fn effective_parameters_follow_smoothed_offsets_stereo_lfo_and_dsp_bounds() {
        let settings = AutoFilterSettings {
            lfo_enabled: false,
            frequency_hz: 1000.0,
            q: 1.0,
            level_db: 0.0,
            dry_wet: 0.5,
            ..Default::default()
        };
        let mut filter = AutoFilter::new(
            ProcessingContext {
                sample_rate_hz: 48000,
                max_block_frames: 512,
            },
            &settings,
        );
        filter.process_modulated_at([0.0; 2], 0, 120.0, [2.0, 1.0, -6.0, 0.25]);
        let initial = filter.effective_parameters();
        assert!((1000.0..1100.0).contains(&initial[0]));
        for frame in 1..2000 {
            filter.process_modulated_at([0.0; 2], frame, 120.0, [2.0, 1.0, -6.0, 0.25]);
        }
        let effective = filter.effective_parameters();
        for (actual, expected) in effective.into_iter().zip([4000.0, 4000.0, 2.0, -6.0, 0.75]) {
            assert!((actual - expected).abs() < 1e-4, "{effective:?}");
        }
        filter.reset();
        assert_eq!(
            filter.effective_parameters(),
            [1000.0, 1000.0, 1.0, 0.0, 0.5]
        );
        let stereo = AutoFilterSettings {
            lfo_enabled: true,
            strength_octaves: 6.0,
            stereo_phase_degrees: 180.0,
            ..settings
        };
        let mut filter = AutoFilter::new(
            ProcessingContext {
                sample_rate_hz: 44100,
                max_block_frames: 512,
            },
            &stereo,
        );
        for frame in 0..1000 {
            filter.process_at([0.0; 2], frame, 120.0);
        }
        let values = filter.effective_parameters();
        assert_ne!(values[0], values[1]);
        for value in &values[..2] {
            assert!((20.0..=19845.0).contains(value));
        }
    }
}

/// DSP parameters only: routing is prepared by the host and passed as typed offsets.
fn filter_settings(settings: &AutoFilterSettings) -> daw_unit_core::FilterSettings {
    daw_unit_core::FilterSettings {
        enabled: settings.enabled,
        shape: settings.shape,
        frequency_hz: settings.frequency_hz,
        q: settings.q,
        level_db: settings.level_db,
        dry_wet: settings.dry_wet,
        ..Default::default()
    }
}

#[cfg(test)]
mod static_filter_tests {
    use super::*;
    use daw_unit_core::{FilterSettings, FilterShape};
    #[test]
    fn static_filter_matches_auto_filter_without_its_lfo_at_every_rate_and_shape() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            for &shape in FilterShape::ALL {
                let context = ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                };
                let mut settings = FilterSettings {
                    shape,
                    ..Default::default()
                };
                let mut static_filter = Filter::new(context, &settings);
                let mut auto_settings = AutoFilterSettings {
                    shape,
                    lfo_enabled: false,
                    ..Default::default()
                };
                let mut auto = AutoFilter::new(context, &auto_settings);
                for frame in 0..2048 {
                    if frame == 500 {
                        settings.frequency_hz = 7000.0;
                        settings.q = 5.0;
                        settings.dry_wet = 0.3;
                        auto_settings.frequency_hz = settings.frequency_hz;
                        auto_settings.q = settings.q;
                        auto_settings.dry_wet = settings.dry_wet;
                        static_filter.retarget(&settings);
                        auto.retarget(&auto_settings);
                    }
                    let signal = [
                        (frame as f32 * 0.03).sin() * 0.2,
                        (frame as f32 * 0.02).cos() * 0.3,
                    ];
                    let offsets = [0.3, -0.1, 1.0, -0.2];
                    assert_eq!(
                        static_filter.process_modulated(signal, offsets),
                        auto.process_modulated_at(signal, frame, 137.0, offsets)
                    );
                    assert_eq!(
                        static_filter.effective_parameters(),
                        auto.effective_parameters()
                    );
                }
                static_filter.reset();
                auto.reset();
                assert_eq!(
                    static_filter.process_sample([0.2; 2]),
                    auto.process_sample([0.2; 2])
                );
            }
        }
    }
}

#[cfg(test)]
mod lfo_modulation_tests;
