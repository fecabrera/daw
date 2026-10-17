use super::*;
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn unit(rate: u32, settings: &OttSettings) -> Ott {
    Ott::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        },
        settings,
    )
}
fn neutral() -> OttSettings {
    let mut s = OttSettings::default();
    for band in &mut s.bands {
        band.depth = 0.0;
    }
    s
}
fn sine_energy(rate: u32, hz: f32, settings: &OttSettings, input_gain: f32) -> (f64, [f32; 3]) {
    let mut ott = unit(rate, settings);
    let mut energy = 0.0;
    for i in 0..rate {
        let x = input_gain * (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin();
        let out = ott.process_sample([x, x * -0.25]);
        assert!((out[1] + out[0] * 0.25).abs() < 1e-6);
        if i >= rate / 2 {
            energy += f64::from(out[0]).powi(2);
        }
    }
    (energy / f64::from(rate / 2), ott.gain_change_db())
}
#[test]
fn crossovers_reconstruct_flat_magnitude_and_dry_wet_is_phase_aligned_at_all_rates() {
    for rate in RATES {
        for hz in [40.0, 250.0, 800.0, 2500.0, 10_000.0] {
            let reference = sine_energy(rate, hz, &neutral(), 0.1).0;
            assert!(
                (reference / 0.005 - 1.0).abs() < 0.005,
                "{rate} {hz}: {reference}"
            );
            for wet in [0.0, 0.5] {
                let s = OttSettings {
                    dry_wet: wet,
                    ..neutral()
                };
                let mixed = sine_energy(rate, hz, &s, 0.1).0;
                assert!((mixed / reference - 1.0).abs() < 1e-6);
            }
        }
    }
}
#[test]
fn quiet_signals_are_boosted_and_loud_signals_reduced_in_each_band_and_mode() {
    for detection in [CompressorDetection::Peak, CompressorDetection::Rms] {
        for (index, hz) in [60.0, 800.0, 10_000.0].into_iter().enumerate() {
            let s = OttSettings {
                detection,
                ..Default::default()
            };
            let (quiet, change) = sine_energy(48000, hz, &s, 0.001);
            assert!(quiet > 0.000002);
            assert!(change[index] > 6.0);
            let (loud, change) = sine_energy(48000, hz, &s, 0.8);
            assert!(loud < 0.15, "{index}: {loud}");
            assert!(change[index] < -3.0);
        }
    }
}
#[test]
fn band_bypass_retains_audio_and_gains_and_all_bands_bypassed_are_neutral() {
    let mut s = OttSettings::default();
    for band in &mut s.bands {
        band.enabled = false;
    }
    for hz in [60.0, 800.0, 10_000.0] {
        let (bypassed, change) = sine_energy(48000, hz, &s, 0.1);
        let neutral = sine_energy(48000, hz, &neutral(), 0.1).0;
        assert_eq!(bypassed, neutral);
        assert_eq!(change, [0.0; 3]);
    }
    s.bands[0].input_db = 6.0;
    s.bands[0].output_db = 6.0;
    assert!(sine_energy(48000, 40.0, &s, 0.1).0 > 0.06);
}
#[test]
fn silence_bypass_level_and_reset_are_exact_and_meters_are_signed() {
    let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
    let mut s = OttSettings::default();
    let mut ott = unit(48000, &s);
    ott.set_gain_change_meter(meter.clone());
    for _ in 0..1000 {
        assert_eq!(ott.process_sample([0.0; 2]), [0.0; 2]);
    }
    for _ in 0..2000 {
        ott.process_sample([0.001, -0.00025]);
    }
    assert!(f32::from_bits(meter[0].load(Ordering::Relaxed)) > 0.0);
    s.enabled = false;
    ott.retarget(&s);
    for _ in 0..240 {
        ott.process_sample([0.25, -0.125]);
    }
    assert_eq!(ott.process_sample([0.25, -0.125]), [0.25, -0.125]);
    assert_eq!(ott.gain_change_db(), [0.0; 3]);
    s.enabled = true;
    s.level_db = daw_unit_core::SILENT_GAIN_DB;
    ott.retarget(&s);
    ott.reset();
    assert!(
        meter
            .iter()
            .all(|m| f32::from_bits(m.load(Ordering::Relaxed)) == 0.0)
    );
    for _ in 0..1000 {
        assert_eq!(ott.process_sample([0.3, -0.1]), [0.0; 2]);
    }
}
#[test]
fn attack_release_and_depth_control_signed_gain_independently_of_mix() {
    let settings = OttBandSettings::default();
    for rate in RATES {
        for sample in [1.0_f32, 0.001] {
            let mut band = Band::new(rate, settings);
            let target = f64::from(gain_curve(20.0 * sample.log10()));
            let frames = rate / 100;
            for _ in 0..frames {
                band.sample([sample, -sample * 0.25], 0.0, [0.0; 5]);
            }
            let expected = target * (1.0 - (-f64::from(frames) / (f64::from(rate) * 0.01)).exp());
            assert!((f64::from(band.change_db()) - expected).abs() < 0.001);
            let before = band.change_db();
            for _ in 0..rate / 10 {
                band.sample([0.03, -0.0075], 0.0, [0.0; 5]);
            }
            assert!(
                (f64::from(band.change_db()) - f64::from(before) * (-1.0_f64).exp()).abs() < 0.001
            );
        }
    }
    let mut band = Band::new(48000, settings);
    for _ in 0..5000 {
        band.sample([0.001; 2], 0.0, [0.0; 5]);
    }
    let full = band.change_db();
    let mut half = settings;
    half.depth = 0.5;
    band.retarget(half, 240);
    for _ in 0..240 {
        band.sample([0.001; 2], 0.0, [0.0; 5]);
    }
    assert!((band.change_db() - full * 0.5).abs() < 0.001);
}
#[test]
fn curves_have_continuous_soft_knees_and_bounded_boost() {
    for boundary in [-39.0, -33.0, -21.0, -15.0] {
        assert!((gain_curve(boundary + 0.0001) - gain_curve(boundary - 0.0001)).abs() < 0.001);
    }
    assert_eq!(gain_curve(-120.0), 24.0);
    assert_eq!(gain_curve(120.0), -60.0);
    assert_eq!(gain_curve(-27.0), 0.0);
}
#[test]
fn retarget_keeps_history_and_unchanged_settings_do_not_restart_ramps() {
    let s = OttSettings::default();
    let mut ott = unit(48000, &s);
    for _ in 0..1000 {
        ott.process_sample([0.001; 2]);
    }
    let before = ott.gain_change_db();
    ott.retarget(&s);
    assert_eq!(ott.gain_change_db(), before);
    let mut next = s;
    next.low_frequency_hz = 500.0;
    next.high_frequency_hz = 5000.0;
    next.bands[0].depth = 0.0;
    ott.retarget(&next);
    for _ in 0..240 {
        ott.retarget(&next);
        assert!(ott.process_sample([0.001; 2]).iter().all(|v| v.is_finite()));
    }
    assert_eq!(ott.parameters[0].value, next.low_frequency_hz.ln());
    assert_eq!(ott.gain_change_db()[0], 0.0);
}
#[test]
fn extreme_controls_and_nonfinite_input_remain_finite_at_every_rate() {
    for rate in RATES {
        let mut s = OttSettings {
            low_frequency_hz: 1999.0,
            high_frequency_hz: 2000.0,
            ..Default::default()
        };
        for band in &mut s.bands {
            band.input_db = 24.0;
            band.output_db = 24.0;
            band.attack_ms = 0.1;
            band.release_ms = 10.0;
        }
        let mut ott = unit(rate, &s);
        for i in 0..2000 {
            let input = [f32::MAX, f32::NAN, f32::INFINITY, 0.0, -f32::MAX][i % 5];
            assert!(ott.process_sample([input; 2]).iter().all(|v| v.is_finite()));
        }
        s.low_frequency_hz = 20.0;
        s.high_frequency_hz = 20_000.0;
        ott.retarget(&s);
        for _ in 0..1000 {
            assert!(ott.process_sample([0.0; 2]).iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn consecutive_crossover_edits_keep_boundaries_ordered_through_the_transition() {
    let mut s = OttSettings {
        low_frequency_hz: 1700.0,
        high_frequency_hz: 1701.0,
        ..Default::default()
    };
    let mut ott = unit(48000, &s);
    s.low_frequency_hz = 1900.0;
    s.high_frequency_hz = 2000.0;
    ott.retarget(&s);
    for _ in 0..120 {
        ott.process_sample([0.001; 2]);
    }
    s.high_frequency_hz = 1901.0;
    ott.retarget(&s);
    for _ in 0..240 {
        ott.retarget(&s);
        ott.process_sample([0.001; 2]);
        assert!(ott.splits[0].frequency < ott.splits[1].frequency);
    }
    assert_eq!(ott.parameters[0].value, s.low_frequency_hz.ln());
    assert_eq!(ott.parameters[1].value, s.high_frequency_hz.ln());
}
