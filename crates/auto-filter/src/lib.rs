//! Transport-locked stereo auto-filter. FunDSP supplies Simper SVF coefficients;
//! the shared integrator state exposes all four shapes without parallel filters.
use daw_unit_core::{
    AUTO_FILTER_TYPE_ID, AutoFilterSettings, EffectUnit, LfoRateMode, PortKind, ProcessingContext,
    UnitDescriptor,
};
use fundsp::svf::SvfCoefs;

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
#[derive(Default)]
struct Filter {
    ic1: f64,
    ic2: f64,
    coefs: SvfCoefs<f64>,
    configured: Option<(f64, f64)>,
}
impl Filter {
    fn sample(&mut self, input: f64, frequency: f64, q: f64, rate: f64, weights: [f64; 4]) -> f64 {
        if self.configured != Some((frequency, q)) {
            self.coefs = SvfCoefs::lowpass(rate, frequency, q);
            self.configured = Some((frequency, q));
        }
        // Trapezoidal state-variable integration (Simper); see FunDSP's svf.rs.
        let c = &self.coefs;
        let v3 = input - self.ic2;
        let band = c.a1 * self.ic1 + c.a2 * v3;
        let low = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * band - self.ic1;
        self.ic2 = 2.0 * low - self.ic2;
        let normalized_band = band / q;
        let high = input - normalized_band - low;
        let notch = input - normalized_band;
        [low, high, normalized_band, notch]
            .into_iter()
            .zip(weights)
            .map(|(value, weight)| value * weight)
            .sum()
    }
}
fn weights<const N: usize>(index: usize) -> [Ramp; N] {
    std::array::from_fn(|i| Ramp::new(f64::from(i == index)))
}
fn wave(index: usize, phase: f64) -> f64 {
    let phase = phase.rem_euclid(1.0);
    match index {
        0 => (phase * std::f64::consts::TAU).sin(),
        1 => 1.0 - 4.0 * ((phase + 0.25).rem_euclid(1.0) - 0.5).abs(),
        2 => 2.0 * phase - 1.0,
        3 => 1.0 - 2.0 * phase,
        _ => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

pub struct AutoFilter {
    sample_rate: f64,
    fade: u32,
    filters: [Filter; 2],
    frequency_log: Ramp,
    q: Ramp,
    depth: Ramp,
    lfo_mix: Ramp,
    sync_phase: Ramp,
    stereo_phase: Ramp,
    level: Ramp,
    wet: Ramp,
    mix: Ramp,
    shapes: [Ramp; 4],
    waves: [Ramp; 5],
    rate_mode: LfoRateMode,
    rate_hz: f32,
    note_beats: f64,
    last_frame: Option<u64>,
    last_rate: f64,
    last_phase: f64,
    phase_correction: f64,
    phase_remaining: u32,
    modulation: [f64; 2],
    modulation_initialized: bool,
    modulation_alpha: f64,
    standalone_frame: u64,
}
impl AutoFilter {
    pub fn new(context: ProcessingContext, settings: &AutoFilterSettings) -> Self {
        let sample_rate = f64::from(context.sample_rate_hz);
        Self {
            sample_rate,
            fade: (context.sample_rate_hz / 200).max(1),
            filters: std::array::from_fn(|_| Filter::default()),
            frequency_log: Ramp::new(f64::from(settings.frequency_hz).log2()),
            q: Ramp::new(settings.q.into()),
            depth: Ramp::new(settings.strength_octaves.into()),
            lfo_mix: Ramp::new(f64::from(settings.lfo_enabled)),
            sync_phase: Ramp::new(f64::from(settings.sync_phase_degrees) / 360.0),
            stereo_phase: Ramp::new(f64::from(settings.stereo_phase_degrees) / 360.0),
            level: Ramp::new(10_f64.powf(f64::from(settings.level_db) / 20.0)),
            wet: Ramp::new(settings.dry_wet.into()),
            mix: Ramp::new(f64::from(settings.enabled)),
            shapes: weights(settings.shape.index()),
            waves: weights(settings.wave.index()),
            rate_mode: settings.rate_mode,
            rate_hz: settings.rate_hz,
            note_beats: settings.note_rate.beats(),
            last_frame: None,
            last_rate: 0.0,
            last_phase: 0.0,
            phase_correction: 0.0,
            phase_remaining: 0,
            modulation: [0.0; 2],
            modulation_initialized: false,
            modulation_alpha: 1.0 - (-1.0 / (0.001 * sample_rate)).exp(),
            standalone_frame: 0,
        }
    }
    /// Called on compatible state publication; histories and unrelated ramps remain intact.
    pub fn retarget(&mut self, settings: &AutoFilterSettings) {
        self.frequency_log
            .retarget(f64::from(settings.frequency_hz).log2(), self.fade);
        self.q.retarget(settings.q.into(), self.fade);
        self.depth
            .retarget(settings.strength_octaves.into(), self.fade);
        self.lfo_mix
            .retarget(f64::from(settings.lfo_enabled), self.fade);
        self.sync_phase
            .retarget(f64::from(settings.sync_phase_degrees) / 360.0, self.fade);
        self.stereo_phase
            .retarget(f64::from(settings.stereo_phase_degrees) / 360.0, self.fade);
        self.level
            .retarget(10_f64.powf(f64::from(settings.level_db) / 20.0), self.fade);
        self.wet.retarget(settings.dry_wet.into(), self.fade);
        self.mix.retarget(f64::from(settings.enabled), self.fade);
        for (i, ramp) in self.shapes.iter_mut().enumerate() {
            ramp.retarget(f64::from(i == settings.shape.index()), self.fade);
        }
        for (i, ramp) in self.waves.iter_mut().enumerate() {
            ramp.retarget(f64::from(i == settings.wave.index()), self.fade);
        }
        self.rate_mode = settings.rate_mode;
        self.rate_hz = settings.rate_hz;
        self.note_beats = settings.note_rate.beats();
    }
    fn phase(&mut self, frame: u64, tempo_bpm: f32) -> f64 {
        let rate = match self.rate_mode {
            LfoRateMode::Hz => f64::from(self.rate_hz),
            LfoRateMode::Note => f64::from(tempo_bpm) / (60.0 * self.note_beats),
        };
        let target = (frame as f64 * rate / self.sample_rate).rem_euclid(1.0);
        let continuous = self
            .last_frame
            .is_some_and(|previous| frame == previous.saturating_add(1) || frame == previous);
        if !continuous {
            self.phase_correction = 0.0;
            self.phase_remaining = 0;
        } else if rate != self.last_rate {
            // Align to the new transport grid over 5 ms, taking the shorter circular path.
            self.phase_correction = (self.last_phase + self.last_rate / self.sample_rate - target
                + 0.5)
                .rem_euclid(1.0)
                - 0.5;
            self.phase_remaining = self.fade;
        }
        let phase = (target + self.phase_correction).rem_euclid(1.0);
        if self.phase_remaining > 0 {
            self.phase_correction -= self.phase_correction / f64::from(self.phase_remaining);
            self.phase_remaining -= 1;
        }
        self.last_frame = Some(frame);
        self.last_rate = rate;
        self.last_phase = phase;
        phase
    }
    /// Explicit host position makes seeks, loop wraps and export phase deterministic.
    pub fn process_at(&mut self, input: [f32; 2], frame: u64, tempo_bpm: f32) -> [f32; 2] {
        let phase = self.phase(frame, tempo_bpm);
        let frequency = self.frequency_log.next();
        let q = self.q.next();
        let depth = self.depth.next() * self.lfo_mix.next();
        let sync = self.sync_phase.next();
        let stereo = self.stereo_phase.next();
        let shapes = std::array::from_fn(|i| self.shapes[i].next());
        let waves: [f64; 5] = std::array::from_fn(|i| self.waves[i].next());
        let level = self.level.next();
        let wet = self.wet.next();
        let mix = self.mix.next();
        let mut output = [0.0; 2];
        for ch in 0..2 {
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
            let cutoff = 2_f64
                .powf(frequency + depth * self.modulation[ch])
                .clamp(20.0, (self.sample_rate * 0.45).min(20_000.0));
            let dry = f64::from(input[ch]);
            let filtered = self.filters[ch].sample(dry, cutoff, q, self.sample_rate, shapes);
            let processed = (dry + (filtered - dry) * wet) * level;
            output[ch] = (dry + (processed - dry) * mix) as f32;
        }
        self.modulation_initialized = true;
        output
    }
}
impl EffectUnit for AutoFilter {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: AUTO_FILTER_TYPE_ID,
            name: "Auto-filter",
            is_source: false,
            inputs: &[PortKind::StereoAudio],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for ramp in [
            &mut self.frequency_log,
            &mut self.q,
            &mut self.depth,
            &mut self.lfo_mix,
            &mut self.sync_phase,
            &mut self.stereo_phase,
            &mut self.level,
            &mut self.wet,
            &mut self.mix,
        ] {
            ramp.reset();
        }
        for ramp in self.shapes.iter_mut().chain(&mut self.waves) {
            ramp.reset();
        }
        self.filters = std::array::from_fn(|_| Filter::default());
        self.last_frame = None;
        self.phase_correction = 0.0;
        self.phase_remaining = 0;
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
        assert_eq!(filter.phase_remaining, 0);
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
        let remaining = filter.frequency_log.remaining;
        settings.collapsed = true;
        filter.retarget(&settings);
        assert_eq!(filter.frequency_log.remaining, remaining);
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
        assert!((filter.filters[0].configured.unwrap().0 - 2000.0).abs() < 1e-9);
        assert!((filter.filters[1].configured.unwrap().0 - 500.0).abs() < 1e-9);
        settings.lfo_enabled = false;
        filter.retarget(&settings);
        for frame in 1..=240 {
            filter.process_at([0.5; 2], frame, 120.0);
        }
        assert!((filter.filters[0].configured.unwrap().0 - 1000.0).abs() < 1e-9);
        assert_eq!(filter.filters[0].configured, filter.filters[1].configured);
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
}
