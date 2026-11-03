use super::*;
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationRange, ModulationRoute, ModulationSourceId,
    ModulationValue, process_modulation,
};
fn noise_settings() -> NoiseSettings {
    NoiseSettings {
        id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
        ..Default::default()
    }
}
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
#[test]
fn noise_is_repeatable_independent_per_note_and_mono_or_uncorrelated_stereo_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut settings = NoiseSettings {
            level_db: 0.0,
            ..noise_settings()
        };
        let mut mono = Noise::new(&settings, context(rate));
        let mut reference = mono.clone();
        mono.trigger(17);
        reference.trigger(17);
        for _ in 0..1024 {
            let a = mono.next_sample();
            assert_eq!(a, reference.next_sample());
            assert_eq!(a[0], a[1]);
        }
        reference.trigger(18);
        assert_ne!(mono.next_sample(), reference.next_sample());
        settings.width = 1.0;
        let mut stereo = Noise::new(&settings, context(rate));
        let mut cross = 0.0_f64;
        let mut energy = [0.0_f64; 2];
        for _ in 0..rate / 4 {
            let a = stereo.next_sample().map(f64::from);
            cross += a[0] * a[1];
            energy[0] += a[0] * a[0];
            energy[1] += a[1] * a[1];
        }
        assert!((cross / (energy[0] * energy[1]).sqrt()).abs() < 0.05);
    }
}
#[test]
fn types_match_reference_rms_and_have_progressively_darker_spectra() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut differences = Vec::new();
        let settings = NoiseSettings {
            level_db: 0.0,
            ..noise_settings()
        };
        for kind in NoiseType::ALL {
            let mut noise = Noise::new(
                &NoiseSettings {
                    kind,
                    ..settings.clone()
                },
                context(rate),
            );
            for _ in 0..rate / 10 {
                noise.next_sample();
            }
            let mut energy = 0.0_f64;
            let mut delta = 0.0_f64;
            let mut last = 0.0;
            for _ in 0..rate / 2 {
                let value = f64::from(noise.next_sample()[0]);
                assert!(value.is_finite());
                energy += value * value;
                delta += (value - last).powi(2);
                last = value;
            }
            let rms = (energy / f64::from(rate / 2)).sqrt();
            assert!((0.43..0.74).contains(&rms), "{rate} {kind:?} rms={rms}");
            differences.push(delta / energy);
        }
        assert!(
            differences[0] > differences[1] * 2.0 && differences[1] > differences[2] * 2.0,
            "{rate}: {differences:?}"
        );
    }
}
#[test]
fn controls_types_power_and_shared_modulation_preserve_state_and_exact_silence() {
    let rate = 48000;
    let settings = noise_settings();
    let mut noise = Noise::new(&settings, context(rate));
    let mut reference = noise.clone();
    let id = ModulationSourceId {
        layer: settings.id,
        unit: settings.id,
    };
    let value = ModulationValue {
        source: id,
        value: 1.0,
        weight: 1.0,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
        release_age: None,
    };
    let make = |parameter, amount| {
        let mut route = Modulation::new(parameter);
        route.source = Some(id);
        route.amount = amount;
        ModulationRoute::new(route, rate)
    };
    let mut routes = [
        make(ModulationParameter::NoiseColor, 1.0),
        make(ModulationParameter::NoiseColor, -1.0),
        make(ModulationParameter::NoiseWidth, 1.0),
        make(ModulationParameter::NoiseWidth, -1.0),
    ];
    for _ in 0..1024 {
        assert_eq!(
            process_modulation(
                &mut noise,
                &mut routes,
                [0.0; 2],
                &[],
                &[value],
                Default::default()
            ),
            reference.next_sample()
        );
    }
    let next = Noise::new(
        &NoiseSettings {
            kind: NoiseType::Brown,
            color: 1.0,
            width: 1.0,
            ..settings.clone()
        },
        context(rate),
    );
    noise.retarget(&next);
    for _ in 0..1024 {
        assert!(noise.next_sample().iter().all(|v| v.is_finite()));
    }
    assert_eq!(noise.values()[0..2], [1.0, 1.0]);
    let off = Noise::new(
        &NoiseSettings {
            enabled: false,
            ..settings.clone()
        },
        context(rate),
    );
    noise.retarget(&off);
    for _ in 0..rate / 200 {
        noise.next_sample();
    }
    for _ in 0..100 {
        assert_eq!(noise.next_sample(), [0.0; 2]);
    }
    let mut silent = Noise::new(
        &NoiseSettings {
            level_db: daw_unit_core::SILENT_GAIN_DB,
            ..settings
        },
        context(rate),
    );
    let mut raised = [make(ModulationParameter::NoiseLevel, 24.0)];
    for _ in 0..512 {
        assert_eq!(
            process_modulation(
                &mut silent,
                &mut raised,
                [0.0; 2],
                &[value],
                &[],
                Default::default()
            ),
            [0.0; 2]
        );
    }
}

#[test]
fn invalid_direct_host_controls_are_bounded_without_nonfinite_audio() {
    for (color, width, level_db) in [(f32::NAN, f32::INFINITY, f32::NAN), (10.0, -10.0, 120.0)] {
        let mut noise = Noise::new(
            &NoiseSettings {
                color,
                width,
                level_db,
                ..noise_settings()
            },
            context(48000),
        );
        for _ in 0..1024 {
            assert!(noise.next_sample().iter().all(|v| v.is_finite()));
        }
        let [color, width, level] = noise.values();
        assert!((-1.0..=1.0).contains(&color) && (0.0..=1.0).contains(&width) && level <= 12.0);
    }
}
