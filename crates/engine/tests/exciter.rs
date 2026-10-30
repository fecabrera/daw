use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
thread_local! { static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
struct CountingAllocator;
// SAFETY: This allocator forwards unchanged layouts and pointers to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        TRACK_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
use daw_media::AudioData;
use std::sync::Arc;
fn plan(rate: u32, mono: bool) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Project::default()
    };
    project.add_track().unwrap();
    let id = Id::new_v4();
    let metadata = SourceMetadata {
        sample_rate_hz: rate,
        channels: if mono { 1 } else { 2 },
        sample_format: "ieee_float".into(),
        bits_per_sample: 32,
    };
    project.assets.push(Asset {
        id,
        name: "tone".into(),
        midi_pattern: None,
        source: Source {
            kind: "external".into(),
            path: "/tone.wav".into(),
            storage: None,
            path_kind: "absolute".into(),
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: 8192,
    });
    project.tracks[0].clips.push(Clip {
        muted: false,
        id: Id::new_v4(),
        asset_id: id,
        name: "tone".into(),
        color: None,
        start_frame: 0,
        source_offset_frame: 0,
        length_frames: 8192,
        repeat: None,
        stretch: None,
        gain_db: 0.0,
        transpose: 0,
    });
    let samples = (0..8192)
        .map(|frame| {
            let sample = (frame as f32 * 1000.0 * std::f32::consts::TAU / rate as f32).sin() * 0.1;
            [sample, if mono { sample } else { -sample * 0.5 }]
        })
        .collect();
    RenderPlan {
        project,
        audio: HashMap::from([(
            id,
            AudioData {
                sample_rate_hz: rate,
                metadata,
                samples: Arc::new(samples),
                peaks: Arc::new(vec![]),
            },
        )]),
        sampler_audio: Default::default(),
        stretched_audio: HashMap::new(),
    }
}
fn configured() -> EffectInstance {
    let mut e = EffectInstance::exciter();
    let EffectSettings::Exciter(s) = &mut e.unit else {
        unreachable!()
    };
    for (i, b) in s.bands.iter_mut().enumerate() {
        b.mix = 0.4;
        b.drive_db = 18.0;
        b.mode = ExciterMode::ALL[i % 4];
    }
    e
}
#[test]
fn live_offline_blocks_seek_and_latency_match_on_tracks_and_master_at_every_rate() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for master in [false, true] {
            let mut p = plan(rate, false);
            let effect = configured();
            let id = effect.id;
            if master {
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0].effects.push(effect);
            }
            p.project.validate().unwrap();
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p.clone());
            assert_eq!(live.latency_frames(), 32);
            live.play();
            for (i, want) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if i > rate as usize / 200 {
                    assert!((actual[0] - want[0]).abs() < 1e-6);
                }
            }
            assert!(expected.iter().any(|v| v[0].abs() > 0.01));
            assert!(
                live.modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                    .is_some()
            );
            if rate == 48000 {
                for block in [32, 512, 4096] {
                    let mut renderer = Renderer::new(p.clone());
                    let mut out = vec![[0.0; 2]; 8192];
                    for (i, chunk) in out.chunks_mut(block).enumerate() {
                        renderer.render_offline_at((i * block) as u64, chunk);
                    }
                    assert_eq!(out, expected);
                }
                let mut renderer = Renderer::new(p);
                let mut seek = vec![[0.0; 2]; 1024];
                renderer.render_offline_at(4096, &mut seek);
                assert_eq!(seek, expected[4096..5120]);
            }
        }
    }
}
#[test]
fn live_solo_never_changes_a_new_offline_renderer_and_callback_updates_do_not_allocate_or_retire() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut p = plan(48000, false);
    let effect = configured();
    let id = effect.id;
    let band = effect.unit.band_ids().next().unwrap();
    p.project.tracks[0].effects.push(effect);
    let encoded = serde_json::to_string(&p.project).unwrap();
    let mut expected = vec![[0.0; 2]; 8192];
    Renderer::new(p.clone()).render_offline(&mut expected);
    let mut live = Renderer::new(p.clone());
    let solo = Arc::new(AtomicBool::new(true));
    live.bind_exciter_solo(id, band, solo.clone());
    live.play();
    let mut heard = vec![[0.0; 2]; 2048];
    for v in &mut heard {
        *v = live.next_sample();
    }
    assert!(
        heard[512..]
            .iter()
            .zip(&expected[512..2048])
            .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
    );
    let mut exported = vec![[0.0; 2]; 8192];
    Renderer::new(p.clone()).render_offline(&mut exported);
    assert_eq!(exported, expected);
    assert_eq!(serde_json::to_string(&p.project).unwrap(), encoded);
    let EffectSettings::Exciter(s) = &mut p.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.bands[1].drive_db = 24.0;
    s.bands[1].mode = ExciterMode::Tube;
    s.level_db = -3.0;
    let mut next = Renderer::new(p);
    next.bind_exciter_solo(id, band, solo.clone());
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut live);
    solo.store(false, Ordering::Relaxed);
    for _ in 0..1024 {
        assert!(next.next_sample().iter().all(|v| v.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(
        next.modulation_snapshot(daw_engine::ModulationTarget::EffectBand { effect: id, band })
            .is_some()
    );
}

fn midi_plan(rate: u32) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Project::default()
    };
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument = Some(OscillatorSettings::default().into());
    project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].waveform =
        Waveform::Sine;
    // Existing oscillator tests isolate source/mixer behavior using the legacy gate.
    project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
        EnvelopeSettings::legacy();
    let clip = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![
                    MidiNote {
                        id: Id::from_u128(1),
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 69,
                        velocity: 127,
                    },
                    MidiNote {
                        id: Id::from_u128(2),
                        start_tick: 480,
                        length_ticks: 1440,
                        pitch: 72,
                        velocity: 100,
                    },
                ],
            },
        })
        .unwrap();
    project.tracks[0].clips[0].length_frames = project.midi_frame(3840);
    RenderPlan {
        project,
        audio: HashMap::new(),
        sampler_audio: Default::default(),
        stretched_audio: HashMap::new(),
    }
}
#[test]
fn every_instrument_bus_receives_the_same_live_only_solo_and_output_meter_binding() {
    use std::sync::atomic::AtomicBool;
    for location in 0..4 {
        let mut p = midi_plan(48000);
        let e = configured();
        let id = e.id;
        let band = e.unit.band_ids().next().unwrap();
        let t = &mut p.project.tracks[0];
        match location {
            0 => t.instrument.as_mut().unwrap().layers[0].effects.push(e),
            1 => t.instrument.as_mut().unwrap().main.effects.push(e),
            2 => t.effects.push(e),
            _ => p.project.master.effects.push(e),
        }
        p.project.validate().unwrap();
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(p.clone()).render_offline(&mut expected);
        let mut soloed = Renderer::new(p.clone());
        soloed.bind_exciter_solo(id, band, Arc::new(AtomicBool::new(true)));
        let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
        soloed.set_band_peak_meter(id, meter.clone());
        let mut heard = vec![[0.0; 2]; 4096];
        soloed.render_offline(&mut heard);
        let energy = |v: &[[f32; 2]]| {
            v[2048..]
                .iter()
                .map(|s| f64::from(s[0]).powi(2))
                .sum::<f64>()
        };
        assert!(energy(&expected) > 0.01);
        assert!(
            energy(&heard) < energy(&expected) * 0.01,
            "location {location}"
        );
        assert!(
            f32::from_bits(meter.current()[0].load(std::sync::atomic::Ordering::Relaxed)) > 0.0
        );
        let mut exported = vec![[0.0; 2]; 4096];
        Renderer::new(p).render_offline(&mut exported);
        assert_eq!(exported, expected);
    }
}
#[test]
fn active_targeted_routes_and_changed_route_amounts_stay_allocation_free() {
    let mut p = midi_plan(48000);
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.macro_module.add();
    instrument.main.macro_module.controls[0].settings.value = 0.5;
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.macro_module.controls[0].id,
    };
    let mut effect = configured();
    let EffectSettings::Exciter(s) = &mut effect.unit else {
        unreachable!()
    };
    let mut r = Modulation::new(ModulationParameter::SaturatorDrive);
    r.target = Some(s.bands[0].id);
    r.source = Some(source);
    r.amount = 12.0;
    s.modulations.push(r);
    instrument.layers[0].effects.push(effect);
    p.project.validate().unwrap();
    let mut old = Renderer::new(p.clone());
    old.play();
    for _ in 0..2048 {
        old.next_sample();
    }
    let EffectSettings::Exciter(s) =
        &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0].effects[0].unit
    else {
        unreachable!()
    };
    s.modulations[0].amount = -12.0;
    let mut next = Renderer::new(p);
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut old);
    for _ in 0..1024 {
        next.next_sample();
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
