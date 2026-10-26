//! Output monitoring must follow final stereo audio without changing DSP or ownership.
use daw_unit_core::{EffectUnit, ProcessingContext};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

fn readings(meter: &[AtomicU32; 3]) -> [f32; 3] {
    meter
        .each_ref()
        .map(|v| f32::from_bits(v.load(Ordering::Relaxed)))
}
fn verify<T: EffectUnit + Clone>(
    mut unit: T,
    rate: u32,
    attach: impl Fn(&mut T, Arc<[AtomicU32; 3]>),
    power: impl Fn(&mut T, bool),
) {
    let mut reference = unit.clone();
    let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
    attach(&mut unit, meter.clone());
    let release = (-1.0 / (0.3 * rate as f32)).exp();
    let mut peak = 0.0_f32;
    for frame in 0..1024 {
        let sample = if frame < 512 { [0.05, -0.4] } else { [0.0; 2] };
        let expected = reference.process_sample(sample);
        assert_eq!(
            unit.process_sample(sample),
            expected,
            "monitoring must not change audio"
        );
        peak = expected[0].abs().max(expected[1].abs()).max(peak * release);
        let values = readings(&meter);
        assert!(
            (values[0] - peak).abs() < 1e-6,
            "meter must follow final stereo output with the shared release"
        );
        assert_eq!(&values[1..], &[0.0; 2]);
    }
    assert!(readings(&meter)[0] > 0.0);
    assert_eq!(Arc::strong_count(&meter), 2);
    let previous = readings(&meter);
    let mut snapshot = unit.clone();
    snapshot.reset();
    for _ in 0..32 {
        snapshot.process_sample([1.0; 2]);
    }
    assert_eq!(
        readings(&meter),
        previous,
        "DSP snapshots must detach publication handles"
    );
    assert_eq!(Arc::strong_count(&meter), 2);
    power(&mut unit, false);
    assert_eq!(readings(&meter), [0.0; 3]);
    for _ in 0..512 {
        unit.process_sample([0.2, -0.6]);
    }
    assert_eq!(
        readings(&meter),
        [0.0; 3],
        "bypass must stop publication immediately"
    );
    assert_eq!(
        Arc::strong_count(&meter),
        2,
        "bypass must retain the handle until retirement"
    );
    power(&mut unit, true);
    for _ in 0..512 {
        unit.process_sample([0.2, -0.6]);
    }
    assert!(readings(&meter)[0] > 0.0);
    unit.reset();
    assert_eq!(readings(&meter), [0.0; 3]);
}

#[test]
fn output_meters_follow_final_audio_and_share_bypass_reset_and_snapshot_lifecycle() {
    use daw_unit_core::{
        BitcrusherSettings, ChorusSettings, DriveSettings, DriveV2Settings, SaturatorSettings,
    };
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let settings = DriveSettings {
            volume_db: 6.0,
            ..Default::default()
        };
        verify(
            daw_drive::Drive::new(context, &settings),
            rate,
            daw_drive::Drive::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = DriveV2Settings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_drive::DriveV2::new(context, &settings),
            rate,
            daw_drive::DriveV2::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = BitcrusherSettings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_bitcrusher::Bitcrusher::new(context, &settings),
            rate,
            daw_bitcrusher::Bitcrusher::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = SaturatorSettings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_saturator::Saturator::new(context, &settings),
            rate,
            daw_saturator::Saturator::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = ChorusSettings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_chorus::Chorus::new(context, &settings),
            rate,
            daw_chorus::Chorus::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = daw_unit_core::FilterSettings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_filter::Filter::new(context, &settings),
            rate,
            daw_filter::Filter::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = daw_unit_core::AutoFilterSettings {
            level_db: 6.0,
            dry_wet: 0.37,
            ..Default::default()
        };
        verify(
            daw_auto_filter::AutoFilter::new(context, &settings),
            rate,
            daw_auto_filter::AutoFilter::set_meter,
            |u, on| {
                let mut s = settings.clone();
                s.enabled = on;
                u.retarget(&s);
            },
        );
        let settings = daw_unit_core::MultibandEqSettings {
            output_gain_db: 6.0,
            ..Default::default()
        };
        let mut eq = daw_equalizer::MultibandEq::new(context, &settings);
        let mut reference = eq.clone();
        let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
        eq.set_output_peak_meter(meter.clone());
        let mut held = 0.0_f32;
        for _ in 0..1024 {
            let expected = reference.process_sample([0.1, -0.4]);
            assert_eq!(eq.process_sample([0.1, -0.4]), expected);
            held = held.max(expected[0].abs().max(expected[1].abs()));
            assert_eq!(meter.boost_peaks()[0].peak(), held);
        }
        assert!(
            held > 0.4,
            "Multiband EQ must meter after final output gain"
        );
    }
}
