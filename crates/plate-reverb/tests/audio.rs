use daw_plate_reverb::PlateReverb;
use daw_unit_core::{EffectUnit, PlateReverbSettings as Settings, ProcessingContext};
fn unit(rate: u32, s: &Settings) -> PlateReverb {
    PlateReverb::new(
        ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 4096,
        },
        s,
    )
}
#[test]
fn finite_stereo_wet_tail_and_repeatable_reset_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let s = Settings {
            dry_wet: 1.0,
            ..Default::default()
        };
        let mut u = unit(rate, &s);
        let mut reference = Vec::new();
        for i in 0..8192 {
            let y = u.process_sample(if i == 0 { [0.1, 0.0] } else { [0.0; 2] });
            assert!(y.iter().all(|x| x.is_finite()));
            reference.push(y);
        }
        assert!(reference[1024..].iter().any(|y| y[0] != 0.0 || y[1] != 0.0));
        u.reset();
        for (i, expected) in reference.into_iter().enumerate() {
            assert_eq!(
                u.process_sample(if i == 0 { [0.1, 0.0] } else { [0.0; 2] }),
                expected
            );
        }
    }
}
#[test]
fn exact_bypass_silence_and_live_retarget_keeps_tail() {
    let mut s = Settings::default();
    let mut u = unit(48000, &s);
    for _ in 0..2048 {
        u.process_sample([0.1, -0.05]);
    }
    s.level_db = daw_unit_core::SILENT_GAIN_DB;
    u.retarget(&s);
    for _ in 0..512 {
        u.process_sample([0.0; 2]);
    }
    assert_eq!(u.process_sample([0.0; 2]), [0.0; 2]);
    s.enabled = false;
    u.retarget(&s);
    for _ in 0..512 {
        u.process_sample([0.2, -0.4]);
    }
    assert_eq!(u.process_sample([0.2, -0.4]), [0.2, -0.4]);
}
#[test]
fn gated_quality_is_independent() {
    let s = Settings {
        gated: true,
        dry_wet: 1.0,
        ..Default::default()
    };
    let mut a = unit(48000, &s);
    let mut b = unit(48000, &Settings { hq: true, ..s });
    for i in 0..8192 {
        let input = if i == 0 { [0.1, 0.0] } else { [0.0; 2] };
        assert_eq!(a.process_sample(input), b.process_sample(input));
    }
}

#[test]
fn longer_decay_retains_more_late_energy() {
    let mut short = unit(
        48000,
        &Settings {
            decay_seconds: 0.3,
            dry_wet: 1.0,
            ..Default::default()
        },
    );
    let mut long = unit(
        48000,
        &Settings {
            decay_seconds: 15.0,
            dry_wet: 1.0,
            ..Default::default()
        },
    );
    let mut energy = [0.0_f64; 2];
    for i in 0..192000 {
        let x = if i == 0 { [0.1, 0.0] } else { [0.0; 2] };
        let a = short.process_sample(x);
        let b = long.process_sample(x);
        if i > 96000 {
            energy[0] += a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>();
            energy[1] += b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>();
        }
    }
    assert!(
        energy[1] > energy[0] * 4.0 && energy[1] > 1e-20,
        "late energies: {energy:?}"
    );
}
