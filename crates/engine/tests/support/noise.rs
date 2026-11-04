use super::*;
use daw_engine::{ModulationTarget, SourceOutputTarget};
use std::sync::Arc;
#[test]
fn noise_live_export_seek_modulation_and_output_meter_are_passive_and_allocation_free() {
    for rate in SAMPLE_RATES {
        let mut p = plan(Waveform::Sine, rate);
        let track = p.project.tracks[0].id;
        let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
        instrument.main.macro_module.add();
        instrument.main.macro_module.controls[0].settings.value = 1.0;
        let source = ModulationSourceId {
            layer: instrument.main.id,
            unit: instrument.main.macro_module.controls[0].id,
        };
        let layer = &mut instrument.layers[0];
        layer.clear_sources();
        layer.noise = Some(NoiseSettings {
            width: 1.0,
            ..Default::default()
        });
        for (parameter, amount) in [
            (ModulationParameter::NoiseColor, 0.5),
            (ModulationParameter::NoiseWidth, -0.25),
            (ModulationParameter::NoiseLevel, -6.0),
        ] {
            let mut route = Modulation::new(parameter);
            route.source = Some(source);
            route.amount = amount;
            layer.noise.as_mut().unwrap().modulations.push(route);
        }
        let key = SourceOutputTarget::for_layer(track, layer).unwrap().0;
        p.project.validate().unwrap();
        let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
        let mut live = Renderer::new(p.clone());
        live.set_source_output_peak_meter(key, meter.clone());
        live.play();
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(p.clone()).render_offline(&mut expected);
        ALLOCATIONS.with(|n| n.set(0));
        TRACK_ALLOCATIONS.with(|n| n.set(true));
        for (frame, value) in expected.iter().enumerate() {
            let actual = live.next_sample();
            if frame >= (rate / 200) as usize {
                assert_eq!(actual, *value);
            }
        }
        TRACK_ALLOCATIONS.with(|n| n.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert!(expected.iter().any(|s| s[0] != s[1]));
        assert!(meter.boost_peaks()[0].peak() > 0.01);
        let snapshot = live
            .modulation_snapshot(ModulationTarget::Noise {
                track: key.track,
                layer: key.layer,
            })
            .unwrap();
        for (index, expected) in [(0, 0.5), (2, 0.75), (4, -18.0)] {
            assert!((snapshot[index] - expected).abs() < 0.001, "{snapshot:?}");
        }
        let mut range = vec![[0.0; 2]; 256];
        p.render(2048, &mut range);
        assert_eq!(range, expected[2048..2304]);
        let mut isolated = p.clone();
        let l = &mut isolated.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        l.amplitude.present = true;
        l.amplitude.level_db = SILENT_GAIN_DB;
        let other = Arc::new(daw_unit_core::BandMeter::<3>::default());
        let mut muted = Renderer::new(isolated);
        muted.set_source_output_peak_meter(key, other.clone());
        muted.play();
        for _ in 0..4096 {
            assert_eq!(muted.next_sample(), [0.0; 2]);
        }
        assert_eq!(other.boost_peaks()[0].peak(), meter.boost_peaks()[0].peak());
        let mut next = p.clone();
        next.project.tracks[0].instrument.as_mut().unwrap().layers[0]
            .noise
            .as_mut()
            .unwrap()
            .color = 0.5;
        let mut next = Renderer::new(next); // Prepare before tracking callback operations.
        ALLOCATIONS.with(|n| n.set(0));
        TRACK_ALLOCATIONS.with(|n| n.set(true));
        next.inherit(&mut live);
        for _ in 0..512 {
            assert!(next.next_sample().iter().all(|v| v.is_finite()));
        }
        TRACK_ALLOCATIONS.with(|n| n.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}

#[test]
fn noise_prepared_seek_and_cache_match_uninterrupted_history() {
    for rate in SAMPLE_RATES {
        for color in [0.0, 0.5, 1.0] {
            let mut p = plan(Waveform::Sine, rate);
            let layer = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0];
            layer.clear_sources();
            layer.noise = Some(NoiseSettings {
                color,
                width: 1.0,
                ..Default::default()
            });
            p.project.validate().unwrap();
            let seek = 4096;
            let count = rate as usize / 200 + 512;
            let mut expected = vec![[0.0; 2]; seek + count];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let state = daw_engine::TransportState {
                frame: seek as u64,
                playing: true,
                ..Default::default()
            };
            let mut direct = Renderer::new(p.clone());
            direct.prepare_transport(state);
            let mut cache = daw_engine::TransportCache::new(p);
            let mut cached = cache.prepare(state, || false).unwrap();
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|n| n.set(true));
            for offset in 0..count {
                let a = direct.next_sample();
                let b = cached.next_sample();
                if offset > rate as usize / 200 {
                    assert_eq!(a, expected[seek + offset], "rate={rate} color={color}");
                    assert_eq!(b, a, "cache rate={rate} color={color}");
                }
            }
            TRACK_ALLOCATIONS.with(|n| n.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        }
    }
}
