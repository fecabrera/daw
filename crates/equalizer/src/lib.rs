//! Reusable stereo three-band EQ. FunDSP coefficients and filter state stay private.
use daw_dsp_support::filters::eq3::{Band, MidCut, MidShelf};
use daw_unit_core::{
    EQ_TYPE_ID, EffectUnit, EqBandSettings, EqSettings, PortKind, ProcessingContext, UnitDescriptor,
};

mod modulation;
#[cfg(test)]
mod modulation_tests;
mod monitor;

fn values(settings: EqBandSettings) -> [f32; 4] {
    [
        settings.frequency_hz,
        settings.q,
        settings.gain_db,
        if settings.enabled { 1.0 } else { 0.0 },
    ]
}

#[derive(Clone)]
pub struct Equalizer {
    bands: [Band; 3],
    mid_cut: MidCut,
    mid_shelf: MidShelf,
    mix: f32,
    target: f32,
    remaining: u32,
    fade_frames: u32,
    monitor: monitor::Monitor,
    spectrum: daw_spectrum::SpectrumAnalyzer,
    external: daw_unit_core::ModulationSmoother<7>,
    effective: [[f32; 4]; 3],
    // Conversion/filter configuration is unchanged while base ramps and offsets are steady.
    configuration: Option<([[f32; 4]; 3], [f32; 7])>,
}
impl Equalizer {
    /// Prepare on a control thread with valid settings and a supported processing rate.
    pub fn new(context: ProcessingContext, settings: &EqSettings) -> Self {
        let mix = if settings.enabled { 1.0 } else { 0.0 };
        Self {
            bands: std::array::from_fn(|index| {
                Band::new(index, context.sample_rate_hz, values(settings.bands[index]))
            }),
            mid_cut: MidCut::new(
                context.sample_rate_hz,
                values(settings.bands[0]),
                values(settings.bands[2]),
            ),
            mid_shelf: MidShelf::new(context.sample_rate_hz),
            mix,
            target: mix,
            remaining: 0,
            fade_frames: (context.sample_rate_hz / 200).max(1),
            monitor: monitor::Monitor::new(context.sample_rate_hz, settings.enabled),
            spectrum: daw_spectrum::SpectrumAnalyzer::new(context.sample_rate_hz, settings.enabled),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: settings.bands.map(values),
            configuration: None,
        }
    }
    /// Retarget without allocating or resetting filter history. Display-only changes are no-ops.
    pub fn retarget(&mut self, settings: &EqSettings) {
        self.spectrum.set_enabled(settings.enabled);
        for (band, settings) in self.bands.iter_mut().zip(settings.bands) {
            band.retarget(values(settings), self.fade_frames);
        }
        let target = if settings.enabled { 1.0 } else { 0.0 };
        if target != self.target {
            self.monitor.set_enabled(settings.enabled);
            self.target = target;
            self.remaining = self.fade_frames;
        }
    }
    pub fn set_spectrum_meter(&mut self, meter: std::sync::Arc<daw_unit_core::SpectrumMeter>) {
        self.spectrum.attach(meter);
    }
    /// Attach prepared stereo-peak monitors in Low/Mid/High order on the control thread.
    /// Values are linear amplitudes of the filtered unit output, with a 300 ms release.
    pub fn set_band_level_meter(
        &mut self,
        meter: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>,
    ) {
        self.monitor.attach(meter);
    }
}
impl EffectUnit for Equalizer {
    fn heap_bytes(&self) -> u64 {
        self.spectrum.heap_bytes()
    }
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: EQ_TYPE_ID,
            name: "3-band EQ",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
        self.mid_cut.reset();
        self.mid_shelf.reset();
        self.mix = self.target;
        self.remaining = 0;
        self.monitor.reset();
        self.spectrum.reset();
        self.external.reset();
        self.configuration = None;
        self.effective = std::array::from_fn(|i| self.bands[i].current());
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 7])
    }
}
impl Equalizer {
    fn configure_effective(&mut self) {
        for (band, effective) in self.bands.iter_mut().zip(self.effective) {
            band.configure(effective);
        }
        self.mid_cut.configure(
            self.effective[0][0],
            self.effective[2][0],
            self.effective[0][1],
            self.effective[2][1],
        );
        self.mid_shelf
            .configure(self.effective[0], self.effective[2], self.effective[1][2]);
    }
    fn process_effective_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        let mut wet = self.bands[0].sample(input, None);
        let cut = self.mid_cut.sample(wet);
        let normal = self.mid_shelf.sample(wet);
        wet = self.bands[1].sample(wet, Some((normal, cut)));
        wet = self.bands[2].sample(wet, None);
        // Finite cut slopes overlap. No enabled band means silence, including that leakage.
        // Use the existing band ramps so switching the last band does not add a click.
        let audible = 1.0
            - self
                .bands
                .iter()
                .map(|band| 1.0 - band.current()[3])
                .product::<f32>();
        for sample in &mut wet {
            *sample *= audible;
        }
        if self.remaining > 0 {
            self.mix += (self.target - self.mix) / self.remaining as f32;
            self.remaining -= 1;
        }
        let output = std::array::from_fn(|ch| input[ch] + (wet[ch] - input[ch]) * self.mix);
        self.monitor
            .sample(output, self.effective[0][0], self.effective[2][0]);
        self.spectrum.sample(output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read_meter(meter: &[std::sync::atomic::AtomicU32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| f32::from_bits(meter[i].load(std::sync::atomic::Ordering::Relaxed)))
    }

    #[test]
    fn band_levels_follow_output_frequency_gain_and_stereo_at_every_rate() {
        use std::sync::{Arc, atomic::AtomicU32};
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let context = ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            };
            for (band, hz) in [(0, 40.0), (1, 800.0), (2, 12000.0)] {
                let settings = EqSettings::default();
                let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
                let mut eq = Equalizer::new(context, &settings);
                let mut reference = Equalizer::new(context, &settings);
                eq.set_band_level_meter(meter.clone());
                for frame in 0..rate / 10 {
                    let sample = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin();
                    // The right channel is louder; monitoring must inspect both channels.
                    let input = [sample * 0.1, sample * 0.4];
                    assert_eq!(eq.process_sample(input), reference.process_sample(input));
                }
                let levels = read_meter(&meter);
                assert!(
                    (0.32..0.45).contains(&levels[band]),
                    "rate={rate} band={band}: {levels:?}"
                );
                for (other, level) in levels.into_iter().enumerate() {
                    if other != band {
                        assert!(level < levels[band] * 0.25, "{levels:?}");
                    }
                }
                let before = levels[band];
                let mut boosted = settings.clone();
                boosted.bands[band].gain_db = 12.0;
                eq.retarget(&boosted);
                for frame in 0..rate / 10 {
                    let sample =
                        (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.4;
                    eq.process_sample([sample, 0.0]);
                }
                assert!(read_meter(&meter)[band] > before * 3.0);
                // Bypass disconnects monitoring; all band cuts show silence after decay.
                boosted.enabled = false;
                eq.retarget(&boosted);
                eq.reset();
                for frame in 0..rate / 10 {
                    let sample =
                        (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.4;
                    eq.process_sample([sample; 2]);
                }
                assert_eq!(read_meter(&meter), [0.0; 3]);
                boosted.enabled = true;
                for band in &mut boosted.bands {
                    band.enabled = false;
                }
                eq.retarget(&boosted);
                eq.reset();
                for _ in 0..rate / 100 {
                    eq.process_sample([0.4; 2]);
                }
                assert!(read_meter(&meter).iter().all(|v| *v < 0.001));
                eq.reset();
                assert_eq!(read_meter(&meter), [0.0; 3]);
            }
        }
    }

    #[test]
    fn band_monitor_follows_ordered_frequency_boundaries_and_releases_on_silence() {
        use std::sync::{Arc, atomic::AtomicU32};
        let mut monitor = monitor::Monitor::new(48000, true);
        let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        monitor.attach(meter.clone());
        let run = |monitor: &mut monitor::Monitor, low, high| {
            for frame in 0..24000 {
                let sample = (frame as f32 * 800.0 * std::f32::consts::TAU / 48000.0).sin() * 0.4;
                monitor.sample([sample; 2], low, high);
            }
        };
        run(&mut monitor, 250.0, 2500.0);
        assert!(read_meter(&meter)[1] > 0.35);
        monitor.reset();
        run(&mut monitor, 2500.0, 250.0);
        assert!(read_meter(&meter)[1] > 0.35);
        monitor.reset();
        run(&mut monitor, 2000.0, 10000.0);
        let moved = read_meter(&meter);
        assert!(moved[0] > 0.35 && moved[1] < moved[0] * 0.2, "{moved:?}");
        monitor.reset();
        run(&mut monitor, 1000.0, 1000.0);
        assert_eq!(read_meter(&meter)[1], 0.0);
        let before = read_meter(&meter)[0];
        for _ in 0..48000 {
            monitor.sample([0.0; 2], 1000.0, 1000.0);
        }
        assert!(read_meter(&meter)[0] < before * 0.05);
        monitor.sample([f32::NAN, f32::INFINITY], 1000.0, 1000.0);
        assert!(read_meter(&meter).iter().all(|v| v.is_finite()));
    }

    fn response(settings: &EqSettings, rate: u32, hz: f32) -> f32 {
        let mut eq = Equalizer::new(
            ProcessingContext {
                sample_rate_hz: rate,
                max_block_frames: 512,
            },
            settings,
        );
        let mut input_power = 0.0_f64;
        let mut output_power = 0.0_f64;
        for frame in 0..rate {
            let input = (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.1;
            let output = eq.process_sample([input, 0.0]);
            assert!(output[0].is_finite());
            assert_eq!(output[1], 0.0, "stereo channels share no filter state");
            if frame > rate / 2 {
                input_power += f64::from(input).powi(2);
                output_power += f64::from(output[0]).powi(2);
            }
        }
        10.0 * (output_power / input_power).log10() as f32
    }
    #[test]
    fn shelves_have_the_requested_gain_at_all_rates() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for (index, hz) in [(0, 20.0), (1, 1000.0), (2, 15000.0)] {
                for gain in [-12.0, 12.0] {
                    let mut settings = EqSettings::default();
                    settings.bands[index].gain_db = gain;
                    if index == 0 {
                        settings.bands[index].frequency_hz = 1000.0;
                    }
                    if index == 2 {
                        settings.bands[index].frequency_hz = 1000.0;
                    }
                    if index == 1 {
                        settings.bands[0].frequency_hz = 100.0;
                        settings.bands[2].frequency_hz = 10000.0;
                    }
                    let actual = response(&settings, rate, hz);
                    assert!(
                        (actual - gain).abs() < 0.2,
                        "band={index} rate={rate} gain={gain} actual={actual}"
                    );
                }
            }
        }
    }
    #[test]
    fn middle_shelf_uses_outer_boundaries_and_ignores_legacy_mid_controls() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].gain_db = 12.0;
            let middle = response(&settings, rate, 800.0);
            assert!(middle > 10.0, "middle plateau is boosted: {middle}");
            for hz in [20.0, 18000.0] {
                assert!(response(&settings, rate, hz).abs() < 0.3);
            }
            settings.bands[1].frequency_hz = 18000.0;
            settings.bands[1].q = 10.0;
            assert_eq!(response(&settings, rate, 800.0), middle);
            settings.bands[0].frequency_hz = 3000.0;
            settings.bands[2].frequency_hz = 12000.0;
            assert!(response(&settings, rate, 800.0).abs() < 1.0);
            assert!(response(&settings, rate, 6000.0) > 10.0);
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            assert!(response(&settings, rate, 800.0).abs() < 0.001);
        }
    }
    #[test]
    fn band_off_cuts_the_range_and_retains_gain_frequency_and_q() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for (index, cut_hz, pass_hz) in [(0, 20.0, 10000.0), (2, 15000.0, 100.0)] {
                let mut settings = EqSettings::default();
                settings.bands[index].frequency_hz = 1000.0;
                settings.bands[index].gain_db = 12.0;
                settings.bands[index].enabled = false;
                let original = settings.clone();
                assert!(
                    response(&settings, rate, cut_hz) < -35.0,
                    "band={index}, rate={rate}"
                );
                assert!(response(&settings, rate, pass_hz).abs() < 0.2);
                assert_eq!(settings, original);
                settings.bands[index].frequency_hz = if index == 2 { 15000.0 } else { 20.0 };
                assert!(
                    response(&settings, rate, cut_hz) > -4.0,
                    "cut follows frequency"
                );
            }
        }
    }
    #[test]
    fn middle_cut_follows_shelf_boundaries_and_outer_q_controls_steepness() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].enabled = false;
            for hz in [400.0, 800.0, 1600.0] {
                assert!(
                    response(&settings, rate, hz) < -20.0,
                    "cuts the full middle range at {hz} Hz"
                );
            }
            for hz in [40.0, 10000.0] {
                assert!(
                    response(&settings, rate, hz).abs() < 0.5,
                    "retains outer range at {hz} Hz"
                );
            }
            let original = response(&settings, rate, 800.0);
            settings.bands[1].frequency_hz = 15000.0;
            settings.bands[1].gain_db = 24.0;
            settings.bands[1].q = 10.0;
            assert_eq!(
                response(&settings, rate, 800.0),
                original,
                "legacy Mid frequency/Q and disabled gain do not affect the cut"
            );
            settings.bands[0].frequency_hz = 2000.0;
            settings.bands[2].frequency_hz = 5000.0;
            assert!(
                response(&settings, rate, 800.0).abs() < 0.5,
                "moving boundaries restores the old middle range"
            );
            settings.bands[0].frequency_hz = 5000.0;
            settings.bands[2].frequency_hz = 2000.0;
            assert!(
                response(&settings, rate, 3200.0) < -20.0,
                "crossed controls still cut between their values"
            );
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            assert!(
                response(&settings, rate, 800.0).abs() < 0.001,
                "equal boundaries leave no middle range"
            );
        }
        let mut gentle = EqSettings::default();
        gentle.bands[1].enabled = false;
        gentle.bands[0].q = 0.1;
        gentle.bands[2].q = 0.1;
        let mut steep = gentle.clone();
        steep.bands[0].q = 10.0;
        steep.bands[2].q = 10.0;
        assert!(response(&steep, 48000, 400.0) < response(&gentle, 48000, 400.0) - 25.0);
    }

    #[test]
    fn neutral_bypass_reset_and_parameter_transitions_are_finite() {
        let mut settings = EqSettings::default();
        let mut eq = Equalizer::new(
            ProcessingContext {
                sample_rate_hz: 48000,
                max_block_frames: 32,
            },
            &settings,
        );
        for frame in 0..1000 {
            let input = [(frame as f32 * 0.1).sin(), (frame as f32 * 0.07).cos()];
            let output = eq.process_sample(input);
            for ch in 0..2 {
                assert!((output[ch] - input[ch]).abs() < 1e-6);
            }
        }
        settings.bands[0].gain_db = 24.0;
        settings.bands[1].enabled = false;
        eq.retarget(&settings);
        let first = eq.process_sample([0.1, -0.1]);
        assert!((first[0] - 0.1).abs() < 0.05, "5 ms ramp starts near dry");
        for _ in 0..240 {
            assert!(eq.process_sample([0.1, -0.1]).iter().all(|v| v.is_finite()));
        }
        settings.enabled = false;
        eq.retarget(&settings);
        for _ in 0..240 {
            eq.process_sample([0.1, -0.1]);
        }
        assert_eq!(eq.process_sample([0.1, -0.1]), [0.1, -0.1]);
        settings.enabled = true;
        eq.retarget(&settings);
        eq.reset();
        assert_eq!(eq.process_sample([0.0; 2]), [0.0; 2]);
        for index in 0..3 {
            settings.bands[index].frequency_hz = 20000.0;
            settings.bands[index].q = 10.0;
        }
        eq.retarget(&settings);
        for _ in 0..10000 {
            assert!(
                eq.process_sample([0.01, -0.01])
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }

    #[test]
    fn all_bands_off_are_silent_and_last_band_switches_ramp() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            for band in &mut settings.bands {
                band.enabled = false;
                band.gain_db = 24.0;
                band.q = 10.0;
            }
            // Equal and crossed boundaries must also remain silent.
            for boundaries in [(250.0, 2500.0), (1000.0, 1000.0), (5000.0, 200.0)] {
                settings.bands[0].frequency_hz = boundaries.0;
                settings.bands[2].frequency_hz = boundaries.1;
                let mut eq = Equalizer::new(
                    ProcessingContext {
                        sample_rate_hz: rate,
                        max_block_frames: 512,
                    },
                    &settings,
                );
                for frame in 0..rate / 10 {
                    let input = [(frame as f32 * 0.13).sin(), (frame as f32 * 0.07).cos()];
                    assert_eq!(eq.process_sample(input), [0.0; 2]);
                }
                settings.enabled = false;
                eq.retarget(&settings);
                for _ in 0..rate / 200 {
                    eq.process_sample([0.1, -0.1]);
                }
                assert_eq!(eq.process_sample([0.1, -0.1]), [0.1, -0.1]);
                settings.enabled = true;
            }

            // Equal boundaries and low frequencies isolate the last Low band's gain ramp.
            settings.bands[0].frequency_hz = 1000.0;
            settings.bands[2].frequency_hz = 1000.0;
            for band in &mut settings.bands {
                band.gain_db = 0.0;
                band.q = std::f32::consts::FRAC_1_SQRT_2;
            }
            settings.bands[0].enabled = true;
            let mut eq = Equalizer::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            for _ in 0..rate / 10 {
                eq.process_sample([0.1, -0.1]);
            }
            let before = eq.process_sample([0.1, -0.1]);
            assert!(before[0] > 0.09);
            settings.bands[0].enabled = false;
            eq.retarget(&settings);
            let first = eq.process_sample([0.1, -0.1]);
            assert!((first[0] - before[0]).abs() < 0.002);
            for _ in 1..rate / 200 {
                eq.process_sample([0.1, -0.1]);
            }
            assert_eq!(eq.process_sample([0.1, -0.1]), [0.0; 2]);
            settings.bands[0].enabled = true;
            eq.retarget(&settings);
            assert!(eq.process_sample([0.1, -0.1])[0].abs() < 0.002);
            for _ in 0..rate / 10 {
                eq.process_sample([0.1, -0.1]);
            }
            assert!(eq.process_sample([0.1, -0.1])[0] > 0.09);
            settings.bands[0].enabled = false;
            eq.retarget(&settings);
            eq.reset();
            assert_eq!(eq.process_sample([0.1, -0.1]), [0.0; 2]);
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;
    #[test]
    fn mid_cut_steepness_sweeps_and_crossed_boundaries_stay_stable() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut settings = EqSettings::default();
            settings.bands[1].enabled = false;
            settings.bands[0].q = 0.1;
            settings.bands[2].q = 0.1;
            let mut eq = Equalizer::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            for index in 0..40 {
                settings.bands[0].q = 0.1 * 100_f32.powf(index as f32 / 39.0);
                settings.bands[2].q = 10.0 / 100_f32.powf(index as f32 / 39.0);
                settings.bands[0].frequency_hz = 20.0 + index as f32 * 500.0;
                settings.bands[2].frequency_hz = 20000.0 - index as f32 * 500.0;
                eq.retarget(&settings);
                for frame in 0..rate / 100 {
                    let source = (frame as f32 * 0.1).sin() * 0.01;
                    assert!(
                        eq.process_sample([source, -source])
                            .iter()
                            .all(|value| value.is_finite() && value.abs() < 1.0)
                    );
                }
            }
        }
    }
}

mod multiband;
pub use multiband::MultibandEq;

pub use multiband::band_response;

mod response;
pub use response::response;
