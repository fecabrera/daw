use super::*;
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
const RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];
fn single(mode: ExciterMode) -> ExciterSettings {
    ExciterSettings {
        bands: vec![ExciterBand {
            mode,
            mix: 1.0,
            drive_db: 18.0,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn tone(frame: usize, bin: usize) -> f32 {
    (frame as f32 * std::f32::consts::TAU * bin as f32 / 4096.0).sin() * 0.2
}
fn harmonic(values: &[f32], bin: usize) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in values.iter().enumerate() {
        let a = i as f64 * std::f64::consts::TAU * bin as f64 / values.len() as f64;
        re += f64::from(*v) * a.cos();
        im += f64::from(*v) * a.sin();
    }
    2.0 * re.hypot(im) / values.len() as f64
}
#[test]
fn neutral_crossovers_are_flat_at_every_rate_and_bypass_has_exact_fixed_delay() {
    for rate in RATES {
        for count in [1, 4, 6] {
            let mut s = ExciterSettings::default();
            while s.bands.len() > count {
                s.remove_band(s.bands.last().unwrap().id);
            }
            while s.bands.len() < count {
                s.add_band();
            }
            for bin in [5, 43, 257, 853] {
                let mut unit = Exciter::new(context(rate), &s);
                let mut power = [0.0_f64; 2];
                for i in 0..8192 {
                    let x = tone(i, bin);
                    let y = unit.process_sample([x, 0.0]);
                    assert_eq!(y[1], 0.0);
                    if i >= 4096 {
                        power[0] += f64::from(x).powi(2);
                        power[1] += f64::from(y[0]).powi(2);
                    }
                }
                assert!(
                    (10.0 * (power[1] / power[0]).log10()).abs() < 0.02,
                    "rate={rate}, bands={count}, bin={bin}, {power:?}"
                );
            }
        }
        let mut s = single(ExciterMode::Hard);
        s.enabled = false;
        let mut unit = Exciter::new(context(rate), &s);
        let input: Vec<_> = (0..256).map(|i| [tone(i, 73), tone(i, 19)]).collect();
        assert_eq!(unit.latency_frames(), 32);
        // Finite bypass input must retain full f32 headroom.
        let mut extreme = Exciter::new(context(rate), &s);
        for i in 0..64 {
            assert_eq!(
                extreme.process_sample([f32::MAX, -f32::MAX]),
                if i < 32 {
                    [0.0; 2]
                } else {
                    [f32::MAX, -f32::MAX]
                }
            );
        }
        for (i, x) in input.iter().enumerate() {
            assert_eq!(
                unit.process_sample(*x),
                if i < 32 { [0.0; 2] } else { input[i - 32] }
            );
        }
    }
}
#[test]
fn modes_are_distinct_tube_adds_even_harmonics_and_dc_is_removed() {
    for rate in RATES {
        let rendered: Vec<Vec<f32>> = ExciterMode::ALL
            .map(|mode| {
                let mut u = Exciter::new(context(rate), &single(mode));
                let warmup = (rate as usize / 3).div_ceil(4096) * 4096;
                (0..warmup + 4096)
                    .map(|i| u.process_sample([tone(i, 43), 0.0])[0])
                    .skip(warmup)
                    .collect()
            })
            .into();
        for (i, a) in rendered.iter().enumerate() {
            for b in rendered.iter().take(i) {
                let error = a
                    .iter()
                    .zip(b)
                    .map(|(a, b)| f64::from(a - b).powi(2))
                    .sum::<f64>()
                    / a.len() as f64;
                assert!(
                    error.sqrt() > 0.001,
                    "mode pair {i}, rate={rate}, residual={error}"
                );
            }
        }
        let tube_even = harmonic(&rendered[1], 86);
        assert!(tube_even > harmonic(&rendered[0], 86) * 20.0 && tube_even > 0.001);
        assert!(rendered[1].iter().map(|v| f64::from(*v)).sum::<f64>().abs() / 4096.0 < 0.0002);
        assert!(harmonic(&rendered[3], 43 * 7) > harmonic(&rendered[0], 43 * 7));
    }
}
#[test]
fn solo_is_live_only_includes_dry_mix_and_all_bands_solo_reconstruct_the_full_signal() {
    let s = ExciterSettings {
        dry_wet: 0.0,
        ..Default::default()
    };
    let mut live = Exciter::new(context(48000), &s);
    let mut original = live.clone();
    let flags: Vec<_> = s
        .bands
        .iter()
        .map(|b| {
            let flag = Arc::new(AtomicBool::new(false));
            live.set_band_solo(b.id, flag.clone());
            flag
        })
        .collect();
    for (i, flag) in flags.iter().enumerate() {
        flag.store(i == 0, Ordering::Relaxed);
    }
    let mut samples = Vec::new();
    for i in 0..8192 {
        let x = [tone(i, 4) + tone(i, 640), 0.0];
        let y = live.process_sample(x);
        original.process_sample(x);
        if i >= 4096 {
            samples.push(y[0]);
        }
    }
    assert!(harmonic(&samples, 4) > 0.18);
    assert!(
        harmonic(&samples, 640) < 0.0001,
        "global dry must not leak unsoloed bands"
    );
    let mut snapshot = live.clone();
    for flag in &flags {
        flag.store(true, Ordering::Relaxed);
    }
    for i in 8192..12288 {
        let x = [tone(i, 4) + tone(i, 640), 0.0];
        let expected = original.process_sample(x);
        let actual = live.process_sample(x);
        let offline = snapshot.process_sample(x);
        if i > 8500 {
            assert!((actual[0] - expected[0]).abs() < 1e-6);
            assert!((offline[0] - expected[0]).abs() < 1e-6);
        }
    }
}
#[test]
fn silence_gain_mix_bypass_monitor_and_compatible_history_survive_updates() {
    let mut s = single(ExciterMode::Tube);
    let mut unit = Exciter::new(context(48000), &s);
    let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
    unit.set_output_peak_meter(meter.clone());
    for i in 0..2048 {
        unit.process_sample([tone(i, 101), -0.1]);
    }
    assert!(f32::from_bits(meter.current()[0].load(Ordering::Relaxed)) > 0.0);
    s.enabled = false;
    unit.retarget(&s);
    assert_eq!(meter.current()[0].load(Ordering::Relaxed), 0);
    for i in 0..500 {
        unit.process_sample([tone(i, 101), -0.1]);
    }
    assert_eq!(meter.current()[0].load(Ordering::Relaxed), 0);
    s.enabled = true;
    s.level_db = daw_unit_core::SILENT_GAIN_DB;
    unit.retarget(&s);
    for i in 0..500 {
        unit.process_sample([tone(i, 101), -0.1]);
    }
    assert_eq!(unit.process_sample([0.1, -0.2]), [0.0; 2]);
    s.level_db = 0.0;
    unit.retarget(&s);
    let mut next = Exciter::new(context(48000), &s);
    let mut expected = unit.clone();
    next.inherit(&mut unit);
    for i in 0..1024 {
        let x = [tone(i, 51), tone(i, 23)];
        assert_eq!(next.process_sample(x), expected.process_sample(x));
    }
    s.add_band();
    let mut topology = Exciter::new(context(48000), &s);
    topology.inherit(&mut next);
    for x in [[f32::NAN, f32::INFINITY], [f32::MAX, -f32::MAX], [0.0; 2]] {
        assert!(topology.process_sample(x).iter().all(|v| v.is_finite()));
    }
    topology.reset();
    for _ in 0..64 {
        assert_eq!(topology.process_sample([0.0; 2]), [0.0; 2]);
    }
}
#[test]
fn typed_target_routes_sum_smooth_and_keep_base_settings_unchanged() {
    use daw_unit_core::{Modulation, ModulationBehavior, ModulationRange, ModulationSourceId};
    let mut s = single(ExciterMode::Soft);
    s.bands[0].drive_db = 0.0;
    s.bands[0].mix = 0.0;
    let source = ModulationSourceId {
        layer: s.bands[0].id,
        unit: daw_unit_core::EffectInstance::exciter().id,
    };
    for amount in [12.0, 6.0] {
        let mut r = Modulation::new(P::SaturatorDrive);
        r.target = Some(s.bands[0].id);
        r.source = Some(source);
        r.amount = amount;
        s.modulations.push(r);
    }
    let mut r = Modulation::new(P::DryWet);
    r.target = Some(s.bands[0].id);
    r.source = Some(source);
    r.amount = 1.0;
    s.modulations.push(r);
    let shared = [ModulationValue {
        source,
        release_age: None,
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    }];
    let original = s.clone();
    let mut unit = Exciter::new(context(48000), &s);
    for i in 0..1024 {
        unit.process_at([tone(i, 43), 0.0], &shared, Default::default());
    }
    let p = unit.band_parameters(s.bands[0].id).unwrap();
    assert!((p[0] - 18.0).abs() < 0.01);
    assert!((p[2] - 1.0).abs() < 0.001);
    assert_eq!(s, original);
}
#[test]
fn rapid_boundary_retargets_preserve_order_with_one_shared_clock() {
    let mut s = ExciterSettings::default();
    let mut u = Exciter::new(context(48000), &s);
    for step in 0..64 {
        let a = if step % 2 == 0 {
            [300.0, 301.0, 302.0]
        } else {
            [9000.0, 9001.0, 15000.0]
        };
        for (b, f) in s.bands.iter_mut().zip(a) {
            b.upper_hz = f;
        }
        assert!(s.valid());
        u.retarget(&s);
        for i in 0..(step % 13 + 1) {
            assert!(u.process_sample([tone(i, 43), 0.0])[0].is_finite());
            assert!(u.frequencies.windows(2).all(|f| f[0][0] < f[1][0]));
        }
    }
}
#[test]
fn oversampled_excitation_reduces_folded_harmonics() {
    let mut s = single(ExciterMode::Hard);
    s.bands[0].drive_db = 30.0;
    let drive = gain(30.0);
    let mut unit = Exciter::new(context(48000), &s);
    let mut actual = Vec::new();
    let mut naive = Vec::new();
    for i in 0..8192 {
        let x = tone(i, 1531);
        let y = unit.process_sample([x, 0.0])[0];
        if i >= 4096 {
            actual.push(y);
            naive.push((x * drive).clamp(-1.2, 1.2) / drive);
        }
    }
    let alias = 3 * 1531 - 4096;
    assert!(harmonic(&actual, alias) < harmonic(&naive, alias) * 0.25);
}
#[test]
fn per_band_level_and_global_mix_level_have_independent_gain_order() {
    let mut s = ExciterSettings::default();
    for b in &mut s.bands {
        b.level_db = 6.0;
    }
    s.level_db = -6.0;
    let mut adjusted = Exciter::new(context(48000), &s);
    let mut neutral = Exciter::new(context(48000), &ExciterSettings::default());
    for i in 0..4096 {
        let x = [tone(i, 129), -tone(i, 71)];
        let a = adjusted.process_sample(x);
        let b = neutral.process_sample(x);
        assert!((a[0] - b[0]).abs() < 1e-6);
    }
    s.dry_wet = 0.0;
    let mut dry = Exciter::new(context(48000), &s);
    neutral.reset();
    for i in 0..4096 {
        let x = [tone(i, 129), 0.0];
        assert!(
            (dry.process_sample(x)[0] - neutral.process_sample(x)[0] * gain(-6.0)).abs() < 1e-6
        );
    }
}
