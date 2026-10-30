use super::*;
use daw_engine::SourceOutputTarget;
use daw_unit_core::BandMeter;
use std::sync::Arc;

fn source_plan(rate: u32, granular: bool) -> RenderPlan {
    let mut p = if granular {
        sampler_plan(rate, SamplerTrigger::OneShot)
    } else {
        plan(Waveform::Sine, rate)
    };
    if granular {
        let source = p.project.tracks[0].instrument.as_mut().unwrap().layers[0]
            .sampler
            .as_mut()
            .unwrap();
        source.granulizer = Some(Default::default());
        source.version = 2;
    }
    let clip = p.project.tracks[0].clips[0].id;
    let pitch = if granular { 20 } else { 69 };
    p.project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![
                    MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 1920,
                        pitch,
                        velocity: 127,
                    },
                    MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 1920,
                        pitch: pitch + 4,
                        velocity: 100,
                    },
                ],
            },
        })
        .unwrap();
    p
}
fn target(p: &RenderPlan) -> SourceOutputTarget {
    SourceOutputTarget::for_layer(
        p.project.tracks[0].id,
        &p.project.tracks[0].instrument.as_ref().unwrap().layers[0],
    )
    .unwrap()
    .0
}
fn set_power(p: &mut RenderPlan, enabled: bool) {
    let l = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0];
    if let Some(s) = &mut l.sampler {
        s.enabled = enabled;
    } else {
        l.oscillator_enabled = enabled;
    }
}
#[test]
fn source_out_meters_are_passive_polyphonic_and_before_layer_controls_at_every_rate() {
    for rate in SAMPLE_RATES {
        for granular in [false, true] {
            let p = source_plan(rate, granular);
            let key = target(&p);
            let meter = Arc::new(BandMeter::<3>::default());
            let mut monitored = Renderer::new(p.clone());
            let mut reference = Renderer::new(p.clone());
            monitored.set_source_output_peak_meter(key, meter.clone());
            monitored.play();
            reference.play();
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|a| a.set(true));
            for _ in 0..4096 {
                assert_eq!(monitored.next_sample(), reference.next_sample());
            }
            TRACK_ALLOCATIONS.with(|a| a.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let peak = meter.boost_peaks()[0].peak();
            assert!(peak > 0.05, "rate={rate} granular={granular}, peak={peak}");
            assert!(meter.readings()[0] > 0.01);
            let mut single = p.clone();
            for asset in &mut single.project.assets {
                if let Some(pattern) = &mut asset.midi_pattern {
                    pattern.notes.truncate(1);
                }
            }
            let one = Arc::new(BandMeter::<3>::default());
            let mut single = Renderer::new(single);
            single.set_source_output_peak_meter(key, one.clone());
            single.play();
            for _ in 0..4096 {
                single.next_sample();
            }
            assert!(
                peak > one.boost_peaks()[0].peak() * 1.4,
                "measure the voice sum, not an individual voice"
            );
            for amplitude in [false, true] {
                let mut isolated = p.clone();
                let l = &mut isolated.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0];
                if amplitude {
                    l.amplitude.present = true;
                    l.amplitude.level_db = daw_core::SILENT_GAIN_DB;
                } else {
                    l.gain_db = daw_core::SILENT_GAIN_DB;
                }
                let mut muted = Renderer::new(isolated);
                let other = Arc::new(BandMeter::<3>::default());
                muted.set_source_output_peak_meter(key, other.clone());
                muted.play();
                for _ in 0..4096 {
                    assert_eq!(muted.next_sample(), [0.0; 2]);
                }
                assert_eq!(
                    other.boost_peaks()[0].peak(),
                    peak,
                    "source tap must precede Amplitude and the layer fader"
                );
            }
        }
    }
}
#[test]
fn source_out_holds_reset_snapshots_detach_and_compatible_power_edits_allocate_nothing() {
    for granular in [false, true] {
        let mut p = source_plan(48000, granular);
        let key = target(&p);
        let mut meter = Arc::new(BandMeter::<3>::default());
        let mut live = Renderer::new(p.clone());
        live.set_source_output_peak_meter(key, meter.clone());
        live.play();
        for _ in 0..2048 {
            live.next_sample();
        }
        assert!(meter.boost_peaks()[0].peak() > 0.0);
        meter.boost_peaks()[0].reset_peak();
        let reading = meter.readings();
        let mut snapshot = live.clone();
        for _ in 0..512 {
            snapshot.next_sample();
        }
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(meter.readings(), reading);
        live.next_sample();
        assert!(meter.boost_peaks()[0].peak() > 0.0);
        set_power(&mut p, false);
        let retired = meter.clone();
        let retired_reading = retired.readings();
        meter = Arc::new(BandMeter::<3>::default());
        let mut off = Renderer::new(p.clone());
        off.set_source_output_peak_meter(key, meter.clone());
        ALLOCATIONS.with(|n| n.set(0));
        TRACK_ALLOCATIONS.with(|a| a.set(true));
        off.inherit(&mut live);
        for _ in 0..512 {
            off.next_sample();
        }
        TRACK_ALLOCATIONS.with(|a| a.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(meter.readings(), [0.0; 3]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(
            retired.readings(),
            retired_reading,
            "retired bindings must stop publishing"
        );
        let disabled = meter.clone();
        meter = Arc::new(BandMeter::<3>::default());
        set_power(&mut p, true);
        let mut on = Renderer::new(p.clone());
        on.set_source_output_peak_meter(key, meter.clone());
        on.inherit(&mut off);
        for _ in 0..512 {
            on.next_sample();
        }
        assert!(meter.readings()[0] > 0.0);
        assert!(meter.boost_peaks()[0].peak() > 0.0);
        assert_eq!(disabled.readings(), [0.0; 3]);
        let layer = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0];
        if let Some(s) = &mut layer.sampler {
            s.gain_db = SILENT_GAIN_DB;
        } else {
            layer.oscillator_level_db = SILENT_GAIN_DB;
        }
        let mut silent = Renderer::new(p);
        silent.set_source_output_peak_meter(key, meter.clone());
        silent.inherit(&mut on);
        for _ in 0..512 {
            silent.next_sample();
        }

        assert!(
            meter.readings()[0] > 0.0,
            "display release remains after Out reaches silence"
        );
        meter.boost_peaks()[0].reset_peak();
        for _ in 0..512 {
            silent.next_sample();
        }
        assert_eq!(
            meter.boost_peaks()[0].peak(),
            0.0,
            "display decay must not restore the held peak"
        );
    }
}
#[test]
fn source_out_gain_is_metered_and_wrong_scoped_bindings_are_ignored() {
    for granular in [false, true] {
        let mut p = source_plan(48000, granular);
        let key = target(&p);
        let run = |p: RenderPlan, key| {
            let meter = Arc::new(BandMeter::<3>::default());
            let mut r = Renderer::new(p);
            r.set_source_output_peak_meter(key, meter.clone());
            r.play();
            for _ in 0..4096 {
                r.next_sample();
            }
            meter.boost_peaks()[0].peak()
        };
        let base = run(p.clone(), key);
        let l = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0];
        if let Some(s) = &mut l.sampler {
            s.gain_db = -6.0;
        } else {
            l.oscillator_level_db = -6.0;
        }
        assert!((run(p.clone(), key) / base - linear_gain(-6.0)).abs() < 1e-5);
        for bad in [
            SourceOutputTarget {
                track: Id::new_v4(),
                ..key
            },
            SourceOutputTarget {
                layer: Id::new_v4(),
                ..key
            },
            SourceOutputTarget {
                unit: Id::new_v4(),
                ..key
            },
        ] {
            assert_eq!(run(p.clone(), bad), 0.0);
        }
    }
}
