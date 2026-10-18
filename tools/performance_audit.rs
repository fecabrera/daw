//! Standalone synthetic audit harness. Run through tools/performance_audit.py.
//! This does not initialize a native window, graphics backend, or audio device.
use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use daw_media::{AudioData, AudioEncoder, WavCodec, WavEncoder};
use daw_project::Session;
use daw_unit_core::{
    EffectUnit, ModulationBehavior, ModulationContext, ModulationInputs, ModulationParameter as P,
    ModulationRange, ModulationRoute, ModulationValue, ProcessingContext,
};
use serde_json::{Value, json};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::HashMap,
    hint::black_box,
    path::Path,
    sync::Arc,
    time::Instant,
};

thread_local! {
    static COUNT: Cell<bool> = const { Cell::new(false) };
    static COUNTERS: Cell<[u64; 3]> = const { Cell::new([0; 3]) };
}
struct Allocator;
// SAFETY: Every call forwards the original pointer/layout to System. Counters are thread-local.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|v| {
            if v.get() {
                COUNTERS.with(|c| {
                    let mut n = c.get();
                    n[0] += 1;
                    n[2] += layout.size() as u64;
                    c.set(n);
                });
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        COUNT.with(|v| {
            if v.get() {
                COUNTERS.with(|c| {
                    let mut n = c.get();
                    n[1] += 1;
                    c.set(n);
                });
            }
        });
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|v| {
            if v.get() {
                COUNTERS.with(|c| {
                    let mut n = c.get();
                    n[0] += 1;
                    n[2] += size as u64;
                    c.set(n);
                });
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn measure<T>(suite: &str, name: &str, repetitions: usize, facts: Value, mut f: impl FnMut() -> T) {
    drop(black_box(f())); // One untimed warmup. Caller controls persistent state.
    let mut times = Vec::with_capacity(repetitions);
    let mut counts = Vec::with_capacity(repetitions);
    for _ in 0..repetitions {
        COUNTERS.with(|v| v.set([0; 3]));
        COUNT.with(|v| v.set(true));
        let start = Instant::now();
        let result = black_box(f());
        let elapsed = start.elapsed().as_secs_f64() * 1e6;
        COUNT.with(|v| v.set(false));
        let count = COUNTERS.with(Cell::get);
        times.push(elapsed);
        counts.push(count);
        drop(result);
    }
    times.sort_by(f64::total_cmp);
    let mut bytes: Vec<_> = counts.iter().map(|c| c[2]).collect();
    bytes.sort_unstable();
    let median = times[times.len() / 2];
    let p95 = times[(times.len() * 95).div_ceil(100).saturating_sub(1)];
    println!(
        "{}",
        json!({"suite":suite,"case":name,"samples":times.len(),"median_us":median,
        "p95_us":p95,"max_us":times[times.len()-1],"latencies_us":times,
        "max_alloc_calls":counts.iter().map(|c|c[0]).max().unwrap(),
        "max_dealloc_calls":counts.iter().map(|c|c[1]).max().unwrap(),
        "median_allocated_bytes":bytes[bytes.len()/2],"facts":facts})
    );
}
fn context(rate: u32) -> ProcessingContext {
    ProcessingContext {
        sample_rate_hz: rate,
        max_block_frames: 512,
    }
}
fn signal(rate: u32, frames: usize) -> Vec<[f32; 2]> {
    (0..frames)
        .map(|i| {
            let t = i as f32 / rate as f32;
            [
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.2,
                (t * 713.0 * std::f32::consts::TAU).sin() * 0.15,
            ]
        })
        .collect()
}
fn effect(kind: usize, enabled: bool) -> EffectInstance {
    let mut e = [
        EffectInstance::equalizer,
        EffectInstance::drive,
        EffectInstance::auto_filter,
        EffectInstance::compressor,
        EffectInstance::ott,
        EffectInstance::limiter,
        EffectInstance::direction_mixer,
        EffectInstance::stereo_spread,
        EffectInstance::bitcrusher,
    ][kind]();
    match &mut e.unit {
        EffectSettings::Eq3(s) => s.enabled = enabled,
        EffectSettings::Drive(s) => s.enabled = enabled,
        EffectSettings::AutoFilter(s) => s.enabled = enabled,
        EffectSettings::Compressor(s) => s.enabled = enabled,
        EffectSettings::Ott(s) => s.enabled = enabled,
        EffectSettings::Limiter(s) => s.enabled = enabled,
        EffectSettings::DirectionMixer(s) => s.enabled = enabled,
        EffectSettings::StereoSpread(s) => s.enabled = enabled,
        EffectSettings::Bitcrusher(s) => s.enabled = enabled,
        _ => unreachable!(),
    }
    e
}
fn dsp(runs: usize) {
    for rate in [48000, 192000] {
        let input = signal(rate, 4096);
        let n = 32768;
        for kind in 0..9 {
            for enabled in [true, false] {
                let e = effect(kind, enabled);
                let mut unit: Box<dyn EffectUnit> = match &e.unit {
                    EffectSettings::Eq3(s) => {
                        Box::new(daw_equalizer::Equalizer::new(context(rate), s))
                    }
                    EffectSettings::Drive(s) => Box::new(daw_drive::Drive::new(context(rate), s)),
                    EffectSettings::AutoFilter(s) => {
                        Box::new(daw_auto_filter::AutoFilter::new(context(rate), s))
                    }
                    EffectSettings::Compressor(s) => {
                        Box::new(daw_compressor::Compressor::new(context(rate), s))
                    }
                    EffectSettings::Ott(s) => Box::new(daw_ott::Ott::new(context(rate), s)),
                    EffectSettings::Limiter(s) => {
                        Box::new(daw_limiter::Limiter::new(context(rate), s))
                    }
                    EffectSettings::DirectionMixer(s) => {
                        Box::new(daw_direction_mixer::DirectionMixer::new(context(rate), s))
                    }
                    EffectSettings::StereoSpread(s) => {
                        Box::new(daw_stereo_spread::StereoSpread::new(context(rate), s))
                    }
                    EffectSettings::Bitcrusher(s) => {
                        Box::new(daw_bitcrusher::Bitcrusher::new(context(rate), s))
                    }
                    _ => unreachable!(),
                };
                let name = unit.descriptor().name;
                measure(
                    "dsp",
                    &format!("{name}/{rate}/enabled={enabled}"),
                    runs,
                    json!({"rate":rate,"frames":n,"defaults":true,"enabled":enabled}),
                    || {
                        let mut sum = 0.0;
                        for i in 0..n {
                            sum += unit.process_sample(black_box(input[i % 4096]))[0];
                        }
                        black_box(sum)
                    },
                );
            }
        }
        for moving in [false, true] {
            let s = StereoSpreadSettings {
                lower_intensity: 0.5,
                upper_intensity: 0.5,
                ..Default::default()
            };
            let mut unit = daw_stereo_spread::StereoSpread::new(context(rate), &s);
            measure(
                "dsp",
                &format!("Stereo Spread/{rate}/frequency_modulation={moving}"),
                runs,
                json!({"rate":rate,"frames":n,"moving_frequency":moving}),
                || {
                    let mut sum = 0.0;
                    for i in 0..n {
                        let x = if moving { input[i % 4096][0] } else { 0.0 };
                        sum += unit.process_modulated(input[i % 4096], [0.0, 0.0, x, -x])[0];
                    }
                    black_box(sum)
                },
            );
        }
    }
    for sources in [1, 32, 128] {
        for routes in [1, 16, 128] {
            let values: Vec<_> = (0..sources)
                .map(|i| ModulationValue {
                    source: ModulationSourceId {
                        layer: Id::from_u128(1000),
                        unit: Id::from_u128(i + 1),
                    },
                    value: 0.5,
                    weight: 1.0,
                    release_age: None,
                    range: ModulationRange::Unipolar,
                    behavior: ModulationBehavior::Continuous,
                })
                .collect();
            let mut connections: Vec<_> = (0..routes)
                .map(|_| {
                    ModulationRoute::new(
                        Modulation {
                            source: Some(values.last().unwrap().source),
                            amount: 1.0,
                            ..Modulation::new(P::Level)
                        },
                        48000,
                    )
                })
                .collect();
            let frames = 4096;
            measure(
                "dsp",
                &format!("route_lookup/sources={sources}/routes={routes}"),
                runs,
                json!({"frames":frames,"sources":sources,"routes":routes,"source_position":"last"}),
                || {
                    for _ in 0..frames {
                        black_box(ModulationInputs::resolve(
                            &mut connections,
                            &[P::Level],
                            &[],
                            &values,
                            ModulationContext::default(),
                        ));
                    }
                },
            );
        }
    }
}
fn audio_plan(rate: u32, tracks: usize, clips: usize) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Default::default()
    };
    let id = Id::new_v4();
    let samples = signal(rate, rate as usize);
    let metadata = SourceMetadata {
        sample_rate_hz: rate,
        channels: 2,
        sample_format: "ieee_float".into(),
        bits_per_sample: 32,
    };
    project.assets.push(Asset {
        id,
        name: "synthetic".into(),
        midi_pattern: None,
        source: Source {
            kind: "external".into(),
            path: "synthetic.wav".into(),
            path_kind: "relative".into(),
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: samples.len() as u64,
    });
    for _ in 0..tracks {
        project.add_track().unwrap();
        let track = project.tracks.last_mut().unwrap();
        for i in 0..clips {
            track.clips.push(Clip {
                id: Id::new_v4(),
                asset_id: id,
                name: "synthetic".into(),
                color: None,
                start_frame: i as u64 * u64::from(rate) * 60,
                source_offset_frame: 0,
                length_frames: u64::from(rate) * 60,
                repeat: Some(ClipLoop {
                    length_frames: u64::from(rate),
                    phase_frame: 0,
                }),
                stretch: None,
            });
        }
    }
    let peaks = samples
        .chunks(256)
        .map(|chunk| {
            let mut p = [[0.0_f32; 2]; 2];
            for x in chunk {
                for ch in 0..2 {
                    p[ch][0] = p[ch][0].min(x[ch]);
                    p[ch][1] = p[ch][1].max(x[ch]);
                }
            }
            p
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
                peaks: Arc::new(peaks),
            },
        )]),
        stretched_audio: HashMap::new(),
    }
}
fn midi_plan(rate: u32, active: usize, capacity: usize, fx: usize) -> RenderPlan {
    let mut p = Project {
        sample_rate_hz: rate,
        ..Default::default()
    };
    let track = p.add_midi_track().unwrap();
    p.tracks[0].instrument = Some(
        OscillatorSettings {
            waveform: Waveform::Saw,
            ..Default::default()
        }
        .into(),
    );
    let clip = p.add_midi_pattern(track, 0).unwrap();
    let mut notes: Vec<_> = (0..active)
        .map(|i| MidiNote {
            id: Id::new_v4(),
            start_tick: 0,
            length_ticks: 240000,
            pitch: if active <= 80 { 40 + i as u8 } else { i as u8 },
            velocity: 100,
        })
        .collect();
    if capacity > active {
        notes.extend((0..capacity).map(|i| MidiNote {
            id: Id::new_v4(),
            start_tick: 240960,
            length_ticks: 960,
            pitch: i as u8,
            velocity: 100,
        }));
    }
    p.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: MidiPattern {
            length_ticks: 245760,
            notes,
        },
    })
    .unwrap();
    p.tracks[0].clips[0].length_frames = p.midi_frame(245760);
    for kind in 0..fx {
        p.tracks[0]
            .effects
            .push(effect([0, 3, 6, 1, 2, 4, 5, 7, 8][kind], true));
    }
    RenderPlan {
        project: p,
        audio: HashMap::new(),
        stretched_audio: HashMap::new(),
    }
}
fn engine(runs: usize) {
    for notes in [256, 4096, 16384] {
        let mut p = midi_plan(48000, 1, 1, 0);
        let clip = p.project.tracks[0].clips[0].id;
        let pattern = MidiPattern {
            length_ticks: (notes as u32 + 1) * 960,
            notes: (0..notes)
                .map(|i| MidiNote {
                    id: Id::new_v4(),
                    start_tick: i as u32 * 960,
                    length_ticks: 480,
                    pitch: 40 + (i % 40) as u8,
                    velocity: 100,
                })
                .collect(),
        };
        p.project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern,
            })
            .unwrap();
        p.project.tracks[0].clips[0].length_frames = p.project.midi_frame((notes as u32 + 1) * 960);
        let at = p.project.midi_frame((notes as u32 - 1) * 960);
        let mut r = Renderer::new(p);
        r.play();
        measure(
            "engine",
            &format!("callback_seek_and_first_32/notes={notes}"),
            runs,
            json!({"rate":48000,"frames":32,"notes":notes,"events_replayed":2*notes-1,"deadline_us":32.0/48000.0*1e6}),
            || {
                r.seek(at);
                let mut sum = 0.0;
                for _ in 0..32 {
                    sum += r.next_sample()[0];
                }
                black_box(sum)
            },
        );
    }
    for rate in [48000, 192000] {
        for tracks in [1, 8] {
            let mut p = audio_plan(rate, tracks, 1);
            for t in &mut p.project.tracks {
                t.effects.push(effect(5, true));
            }
            let mut r = Renderer::new(p);
            r.play();
            let latency = r.latency_frames();
            measure(
                "engine",
                &format!("callback_seek_with_limiter/{rate}/tracks={tracks}"),
                runs,
                json!({"rate":rate,"tracks":tracks,"latency_frames":latency,"includes_preroll":true,"deadline_32_us":32.0/f64::from(rate)*1e6}),
                || r.seek(u64::from(rate)),
            );
        }
    }

    for (tracks, clips) in [
        (1, 1),
        (8, 1),
        (32, 1),
        (128, 1),
        (1, 64),
        (1, 1024),
        (8, 1024),
        (32, 256),
    ] {
        for looping in [false, true] {
            for last in [false, true] {
                if clips == 1 && last {
                    continue;
                }
                let mut p = audio_plan(48000, tracks, clips);
                let end = p.project.end();
                p.project.transport.r#loop = Loop {
                    enabled: looping,
                    start_frame: 0,
                    end_frame: end,
                };
                let mut renderer = Renderer::new(p);
                if last {
                    renderer.seek((clips as u64 - 1) * 48000 * 60);
                }
                renderer.play();
                let frames = 512;
                measure(
                    "engine",
                    &format!("audio/T={tracks}/C={clips}/loop={looping}/last={last}"),
                    runs.max(32),
                    json!({"rate":48000,"frames":frames,"tracks":tracks,"clips_per_track":clips,"loop":looping,"last_clip":last}),
                    || {
                        let mut sum = 0.0;
                        for _ in 0..frames {
                            sum += renderer.next_sample()[0];
                        }
                        black_box(sum)
                    },
                );
            }
        }
    }
    for looping in [false, true] {
        for last in [false, true] {
            let mut p = audio_plan(192000, 8, 1024);
            let end = p.project.end();
            p.project.transport.r#loop = Loop {
                enabled: looping,
                start_frame: 0,
                end_frame: end,
            };
            let mut r = Renderer::new(p);
            if last {
                r.seek(1023 * 192000 * 60);
            }
            r.play();
            measure(
                "engine",
                &format!("dense_192k/loop={looping}/last={last}"),
                runs.max(32),
                json!({"rate":192000,"frames":512,"tracks":8,"clips_per_track":1024,"loop":looping,"last_clip":last,"deadline_us":512.0/192000.0*1e6}),
                || {
                    let mut sum = 0.0;
                    for _ in 0..512 {
                        sum += r.next_sample()[0];
                    }
                    black_box(sum)
                },
            );
        }
    }
    for rate in [48_000, 192_000] {
        for tracks in [1, 8] {
            let mut plan = audio_plan(rate, tracks, 1);
            for track in &mut plan.project.tracks {
                track.effects.push(EffectInstance::limiter());
            }
            let old = Renderer::new(plan.clone());
            let mut prepared = Renderer::new(plan);
            prepared.prepare_transport(daw_engine::TransportState {
                frame: 12_000,
                playing: true,
                playback_start: Some(0),
                ..Default::default()
            });
            measure(
                "engine",
                &format!("prepared_transport_activation/rate={rate}/tracks={tracks}"),
                runs.max(64),
                json!({"rate":rate,"tracks":tracks,"stage":"activation_only","activations":1024,"minimum_buffer_deadline_us":32.0 / f64::from(rate) * 1e6}),
                || {
                    for _ in 0..1024 {
                        black_box(&mut prepared).activate_transport(black_box(&old));
                        black_box(prepared.transport_state());
                    }
                },
            );
        }
    }
    for rate in SAMPLE_RATES {
        for block in BUFFER_SIZES {
            let p = midi_plan(rate, 8, 8, 2);
            let mut renderer = Renderer::new(p);
            renderer.play();
            for _ in 0..2048 {
                black_box(renderer.next_sample());
            }
            measure(
                "engine",
                &format!("target/8_voices/EQ+Comp/{rate}/{block}"),
                runs.max(64),
                json!({"rate":rate,"frames":block,"voices":8,"effects":["EQ","Compressor"],"deadline_us":f64::from(block)/f64::from(rate)*1e6}),
                || {
                    let mut sum = 0.0;
                    for _ in 0..block {
                        sum += renderer.next_sample()[0];
                    }
                    black_box(sum)
                },
            );
        }
    }
    for (active, capacity, fx) in [
        (1, 1, 0),
        (8, 8, 0),
        (32, 32, 0),
        (128, 128, 0),
        (8, 128, 0),
        (8, 8, 9),
    ] {
        let p = midi_plan(48000, active, capacity, fx);
        let mut renderer = Renderer::new(p);
        renderer.play();
        measure(
            "engine",
            &format!("midi/active={active}/capacity={capacity}/fx={fx}"),
            runs.max(32),
            json!({"rate":48000,"frames":512,"active_voices":active,"prepared_polyphony":capacity,"effects":fx}),
            || {
                let mut sum = 0.0;
                for _ in 0..512 {
                    sum += renderer.next_sample()[0];
                }
                black_box(sum)
            },
        );
    }
    for tracks in [8, 32, 128] {
        let p = audio_plan(48000, tracks, 1);
        let mut renderer = Renderer::new(p);
        let mut out = [[0.0; 2]; 512];
        measure(
            "engine",
            &format!("offline_audio/tracks={tracks}"),
            runs.max(32),
            json!({"rate":48000,"frames":512,"tracks":tracks}),
            || renderer.render_offline(&mut out),
        );
    }
    let mut p = audio_plan(48000, 1, 1);
    p.project.tracks[0].effects.push(effect(0, true));
    for seconds in [0, 1, 10, 60] {
        measure(
            "engine",
            &format!("random_access/seconds={seconds}"),
            runs,
            json!({"rate":48000,"start_seconds":seconds,"frames":512,"effects":["EQ"]}),
            || {
                let mut out = [[0.0; 2]; 512];
                p.render(seconds * 48000, &mut out);
                black_box(out)
            },
        );
    }
}
fn asset_plan(assets: usize) -> RenderPlan {
    let mut p = audio_plan(48000, 8, 256);
    let original = p.project.assets[0].clone();
    let audio = p.audio[&original.id].clone();
    for _ in 1..assets {
        let mut a = original.clone();
        a.id = Id::new_v4();
        p.audio.insert(a.id, audio.clone());
        p.project.assets.push(a);
    }
    for (i, c) in p
        .project
        .tracks
        .iter_mut()
        .flat_map(|t| &mut t.clips)
        .enumerate()
    {
        c.asset_id = p.project.assets[i % assets].id;
    }
    p
}
fn model(runs: usize) {
    for assets in [1, 256, 2048] {
        let p = asset_plan(assets);
        measure(
            "model",
            &format!("asset_lookup_validate/assets={assets}/clips=2048"),
            runs,
            json!({"assets":assets,"clips":2048}),
            || p.project.validate().unwrap(),
        );
        let mut project = p.project.clone();
        let id = project.tracks[0].id;
        measure(
            "model",
            &format!("asset_lookup_edit/assets={assets}/clips=2048"),
            runs,
            json!({"assets":assets,"clips":2048}),
            || {
                project
                    .edit(Edit::SetTrackMuted {
                        track_id: id,
                        muted: true,
                    })
                    .unwrap()
            },
        );
    }
    for tracks in [8, 32, 128] {
        let mut p = audio_plan(48000, tracks, 1);
        for t in &mut p.project.tracks {
            for kind in 0..9 {
                t.effects.push(effect(kind, true));
            }
        }
        let keys: Vec<_> = p
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.effects)
            .map(|e| daw_engine::ModulationTarget::Effect(e.id))
            .collect();
        let cells: Vec<_> = keys
            .iter()
            .map(|_| std::array::from_fn::<_, 19, _>(|_| std::sync::atomic::AtomicU32::new(0)))
            .collect();
        let r = Renderer::new(p);
        measure(
            "model",
            &format!("callback_telemetry/handles={}", keys.len()),
            runs.max(64),
            json!({"handles":keys.len(),"slots_per_handle":19,"includes_relaxed_atomic_stores":true}),
            || {
                for (key, handle) in keys.iter().zip(&cells) {
                    let values = r.modulation_snapshot(*key).unwrap();
                    for (v, cell) in values.iter().zip(handle) {
                        cell.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    }
                }
            },
        );
    }

    for (tracks, clips) in [(8, 8), (32, 64), (128, 64)] {
        let p = audio_plan(48000, tracks, clips);
        measure(
            "model",
            &format!("clone/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips}),
            || p.clone(),
        );
        measure(
            "model",
            &format!("validate/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips}),
            || p.project.validate().unwrap(),
        );
        let mut edit = p.project.clone();
        let id = edit.tracks[0].id;
        let mut muted = false;
        measure(
            "model",
            &format!("edit_mute/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips}),
            || {
                muted = !muted;
                edit.edit(Edit::SetTrackMuted {
                    track_id: id,
                    muted,
                })
                .unwrap();
            },
        );
        measure(
            "model",
            &format!("prepare_renderer/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips,"includes_plan_clone":true}),
            || Renderer::new(p.clone()),
        );
    }
    for capacity in [8, 32, 128] {
        let p = midi_plan(48000, 8, capacity, 2);
        measure(
            "model",
            &format!("prepare_midi/capacity={capacity}"),
            runs,
            json!({"voices":capacity,"includes_plan_clone":true}),
            || Renderer::new(p.clone()),
        );
    }
    for routes in [0, 16, 128, 512, 2048] {
        let mut p = audio_plan(48000, 1, 1);
        let mut e = effect(8, true);
        let EffectSettings::Bitcrusher(s) = &mut e.unit else {
            unreachable!()
        };
        s.modulations = (0..routes).map(|_| Modulation::new(P::Level)).collect();
        p.project.tracks[0].effects.push(e);
        let mut next = p.clone();
        next.project.tracks[0].effects[0]
            .unit
            .modulations_mut()
            .reverse();
        let mut pairs: Vec<_> = (0..runs + 1)
            .map(|_| (Renderer::new(p.clone()), Renderer::new(next.clone())))
            .collect();
        measure(
            "model",
            &format!("callback_inherit/routes={routes}"),
            runs,
            json!({"routes":routes,"reordered":true,"deadline_32_at_192k_us":32.0/192000.0*1e6}),
            || {
                let (mut old, mut new) = pairs.pop().unwrap();
                new.inherit(&mut old);
                (old, new)
            },
        );
    }
}
fn ui(runs: usize) {
    for tracks in [8, 128] {
        let p = audio_plan(48000, tracks, 64);
        let mut app = daw_ui::DawUi::default();
        app.session = Session {
            project: p.project,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        daw_ui::fonts::configure(&ctx);
        daw_ui::icons::configure(&ctx);
        let mut time = 0.0;
        let mut frame = || {
            time += 1.0 / 60.0;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    time: Some(time),
                    focused: true,
                    ..Default::default()
                },
                |ui| app.show(ui),
            )
        };
        for _ in 0..8 {
            drop(frame());
        }
        measure(
            "ui",
            &format!("headless/Timeline_missing_audio/tracks={tracks}/clips=64"),
            runs.max(30),
            json!({"tracks":tracks,"clips_per_track":64,"audio_present":false,"screen":[1600,1000],"gpu_submission":false}),
            frame,
        );
    }

    for (tracks, clips) in [(8, 8), (32, 64), (128, 64)] {
        for view in [daw_ui::ViewMode::Timeline, daw_ui::ViewMode::Mixer] {
            let p = audio_plan(48000, tracks, clips);
            let mut app = daw_ui::DawUi::default();
            app.session = Session {
                project: p.project,
                audio: p.audio,
                stretched_audio: p.stretched_audio,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            daw_ui::fonts::configure(&ctx);
            daw_ui::icons::configure(&ctx);
            app.set_view_mode(view, &ctx);
            let mut time = 0.0;
            let mut frame = || {
                time += 1.0 / 60.0;
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1600.0, 1000.0),
                        )),
                        time: Some(time),
                        focused: true,
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                )
            };
            for _ in 0..8 {
                drop(frame());
            }
            measure(
                "ui",
                &format!("headless/{view:?}/tracks={tracks}/clips={clips}"),
                runs.max(30),
                json!({"tracks":tracks,"clips_per_track":clips,"screen":[1600,1000],"includes_layout_paint":true,"gpu_submission":false}),
                &mut frame,
            );
            measure(
                "ui",
                &format!("headless_with_tessellation/{view:?}/tracks={tracks}/clips={clips}"),
                runs.max(30),
                json!({"tracks":tracks,"clips_per_track":clips,"screen":[1600,1000],"gpu_submission":false,"includes_tessellation":true}),
                || {
                    let output = frame();
                    ctx.tessellate(output.shapes, output.pixels_per_point)
                },
            );
        }
    }
    for layers in [1, 8, 32] {
        let p = midi_plan(48000, 8, 8, 0);
        let mut app = daw_ui::DawUi::default();
        let ctx = egui::Context::default();
        daw_ui::fonts::configure(&ctx);
        daw_ui::icons::configure(&ctx);
        drop(ctx.run_ui(
            egui::RawInput {
                focused: true,
                ..Default::default()
            },
            |ui| app.show(ui),
        ));
        app.perform_track_action(daw_ui::TrackAction::AddMidi, &ctx);
        let selected = app.session.project.tracks[0].id;
        app.session = Session {
            project: p.project,
            ..Default::default()
        };
        app.session.project.tracks[0].id = selected;
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        for _ in 1..layers {
            let mut l = InstrumentLayerSettings::default();
            l.oscillators.push(OscillatorSettings::default());
            instrument.layers.push(l);
        }
        app.set_view_mode(daw_ui::ViewMode::Instrument, &ctx);
        let mut time = 0.0;
        let mut frame = || {
            time += 1.0 / 60.0;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    time: Some(time),
                    focused: true,
                    ..Default::default()
                },
                |ui| app.show(ui),
            )
        };
        for _ in 0..8 {
            drop(frame());
        }
        measure(
            "ui",
            &format!("headless/Instrument/layers={layers}"),
            runs.max(30),
            json!({"layers":layers,"screen":[1600,1000],"gpu_submission":false}),
            frame,
        );
    }
}
fn media(runs: usize, scratch: &Path) {
    let input = signal(48000, 48000 * 10);
    let wav = scratch.join("synthetic.wav");
    let mut encoder = WavEncoder::at_sample_rate(&wav, WavCodec::Float32, 48000).unwrap();
    encoder.write_frames(&input).unwrap();
    encoder.finish().unwrap();
    for rate in [48000, 96000, 192000] {
        measure(
            "media",
            &format!("decode/10s/48000_to_{rate}"),
            runs,
            json!({"input_frames":input.len(),"source_rate":48000,"output_rate":rate}),
            || daw_media::decode_wav_at_rate(&wav, rate).unwrap(),
        );
    }
    for seconds in [1, 10] {
        measure(
            "media",
            &format!("resample/{seconds}s/48000_to_96000"),
            runs,
            json!({"input_frames":seconds*48000,"source_rate":48000,"output_rate":96000}),
            || daw_media::resample(&input[..seconds * 48000], 48000, 96000).unwrap(),
        );
    }
    for seconds in [1, 4] {
        let audio = &input[..seconds * 48000];
        let interleaved: Vec<_> = audio.iter().flat_map(|s| s.iter().copied()).collect();
        for (n, d) in [(1, 1), (3, 2), (2, 1)] {
            let request =
                timestretch::Request::new(48000, 2, timestretch::Ratio::new(n, d).unwrap());
            measure(
                "media",
                &format!("integrated_stretch/{seconds}s/{n}:{d}"),
                runs,
                json!({"rate":48000,"input_frames":seconds*48000,"ratio":[n,d]}),
                || request.process(&interleaved).unwrap(),
            );
        }
    }
    let base = daw_media::decode_wav_at_rate(&wav, 48000).unwrap();
    for engine in [StretchEngine::RUBBER_BAND, StretchEngine::INTEGRATED] {
        let source = AudioData {
            samples: Arc::new(input[..48000 * 4].to_vec()),
            ..base.clone()
        };
        measure(
            "media",
            &format!("daw_stretch_adapter/4s/3:2/{}", engine.label()),
            runs,
            json!({"rate":48000,"input_frames":48000*4,"ratio":[3,2],"engine":engine.label()}),
            || {
                daw_media::stretch_audio_with_engine(
                    &source,
                    ClipStretch::new(2, 3).unwrap(),
                    engine,
                )
                .unwrap()
            },
        );
    }
    for rate in [48000, 192000] {
        let mut p = audio_plan(rate, 8, 1);
        for t in &mut p.project.tracks {
            t.clips[0].length_frames = u64::from(rate) * 4;
        }
        p.project.assets[0].source.path = wav.to_string_lossy().into_owned();
        p.project.assets[0].source.path_kind = "absolute".into();
        let session = Session {
            project: p.project,
            audio: p.audio,
            ..Default::default()
        };
        for (name, settings) in [
            ("pcm24", daw_media::ExportSettings::default()),
            (
                "float32",
                daw_media::ExportSettings {
                    wav_codec: WavCodec::Float32,
                    ..Default::default()
                },
            ),
            (
                "mp3",
                daw_media::ExportSettings {
                    format: daw_media::ExportFormat::Mp3,
                    ..Default::default()
                },
            ),
        ] {
            let output = scratch.join(format!(
                "export-{rate}-{name}.{}",
                settings.format.extension()
            ));
            measure(
                "media",
                &format!("export/8tracks/4s/{rate}/{name}"),
                runs,
                json!({"rate":rate,"tracks":8,"seconds":4,"format":name}),
                || {
                    session
                        .export_with_settings(&output, true, settings)
                        .unwrap()
                },
            );
        }
    }
    for (from, to) in [(48000, 44100), (192000, 48000), (48000, 192000)] {
        use rubato::Resampler;
        let mut converter = rubato::FftFixedIn::<f32>::new(from, to, 512, 2, 2).unwrap();
        let input = converter.input_buffer_allocate(true);
        let mut output = converter.output_buffer_allocate(true);
        measure(
            "media",
            &format!("callback_resampler/{from}_to_{to}"),
            runs.max(64),
            json!({"input_rate":from,"output_rate":to,"requested_project_block":512,"input_frames":converter.input_frames_next(),"output_frames_max":converter.output_frames_max()}),
            || {
                converter
                    .process_into_buffer(&input, &mut output, None)
                    .unwrap()
            },
        );
    }
    for (tracks, clips) in [(8, 8), (32, 64)] {
        let mut p = audio_plan(48000, tracks, clips);
        p.project.assets[0].source.path = wav.to_string_lossy().into_owned();
        p.project.assets[0].source.path_kind = "absolute".into();
        p.project.assets[0].decoded_frame_count = input.len() as u64;
        let mut session = Session {
            project: p.project,
            audio: p.audio,
            ..Default::default()
        };
        let folder = scratch.join(format!("project-{tracks}-{clips}"));
        measure(
            "media",
            &format!("save/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips,"includes_fsync":true}),
            || session.save(&folder).unwrap(),
        );
        measure(
            "media",
            &format!("open/tracks={tracks}/clips={clips}"),
            runs,
            json!({"tracks":tracks,"clips_per_track":clips,"source_seconds":10}),
            || Session::open(&folder).unwrap(),
        );
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let suite = args.get(1).map(String::as_str).unwrap_or("all");
    let runs = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(7);
    let path = std::path::PathBuf::from(
        args.get(3)
            .map(String::as_str)
            .unwrap_or("work/performance-audit"),
    );
    std::fs::create_dir_all(&path).unwrap();
    if suite == "all" || suite == "dsp" {
        dsp(runs);
    }
    if suite == "all" || suite == "engine" {
        engine(runs);
    }
    if suite == "all" || suite == "model" {
        model(runs);
    }
    if suite == "all" || suite == "ui" {
        ui(runs);
    }
    if suite == "all" || suite == "media" {
        media(runs, &path);
    }
}
