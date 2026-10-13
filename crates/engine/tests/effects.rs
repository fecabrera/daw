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
            path_kind: "absolute".into(),
        },
        source_metadata: metadata.clone(),
        decoded_frame_count: 8192,
    });
    project.tracks[0].clips.push(Clip {
        id: Id::new_v4(),
        asset_id: id,
        name: "tone".into(),
        color: None,
        start_frame: 0,
        source_offset_frame: 0,
        length_frames: 8192,
        repeat: None,
        stretch: None,
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
        stretched_audio: HashMap::new(),
    }
}
fn add_eq(plan: &mut RenderPlan) {
    let mut eq = EffectInstance::equalizer();
    let EffectSettings::Eq3(settings) = &mut eq.unit else {
        unreachable!()
    };
    settings.bands[1].gain_db = 12.0;
    plan.project.tracks[0].effects.push(eq);
}
#[test]
fn serial_eq_playback_offline_and_random_access_match_at_all_rates_and_buffers() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for mono in [false, true] {
            let mut plan = plan(rate, mono);
            add_eq(&mut plan);
            add_eq(&mut plan);
            plan.project.tracks[0].gain_db = -6.0;
            plan.project.tracks[0].pan = 0.4;
            plan.project.master.gain_db = -3.0;
            let mut whole = vec![[0.0; 2]; 8192];
            Renderer::new(plan.clone()).render_offline(&mut whole);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, expected) in whole.iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    for ch in 0..2 {
                        assert!((actual[ch] - expected[ch]).abs() < 1e-6);
                    }
                }
            }
            for block in [32, 64, 128, 256, 512, 1024, 2048, 4096] {
                let mut output = vec![[0.0; 2]; 8192];
                let mut renderer = Renderer::new(plan.clone());
                for buffer in output.chunks_mut(block) {
                    renderer.render_offline(buffer);
                }
                assert_eq!(output, whole);
            }
            let mut range = [[0.0; 2]; 64];
            plan.render(1000, &mut range);
            assert_eq!(range.as_slice(), &whole[1000..1064]);
            assert_eq!(plan.sample_at(1000), whole[1000]);
            let mut dry = plan.clone();
            dry.project.tracks[0].effects.clear();
            assert!(
                whole[1000][0].abs() > dry.sample_at(1000)[0].abs() * 10.0,
                "both effects precede gain/pan"
            );
        }
    }
}
#[test]
fn prepared_updates_preserve_history_and_retarget_without_callback_allocations() {
    let mut plan = plan(48000, false);
    add_eq(&mut plan);
    let mut reference = Renderer::new(plan.clone());
    let mut current = Renderer::new(plan.clone());
    reference.play();
    current.play();
    for _ in 0..700 {
        reference.next_sample();
        current.next_sample();
    }
    plan.project.tracks[0].effects_compact = true;
    let EffectSettings::Eq3(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.collapsed = true;
    let mut next = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|value| value.set(true));
    next.inherit(&mut current);
    for _ in 0..1000 {
        let a = next.next_sample();
        let b = reference.next_sample();
        for ch in 0..2 {
            assert!((a[ch] - b[ch]).abs() < 1e-6);
        }
    }
    TRACK_ALLOCATIONS.with(|value| value.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let EffectSettings::Eq3(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.bands[0].enabled = false;
    settings.bands[1].gain_db = -12.0;
    settings.bands[2].q = 10.0;
    settings.enabled = false;
    let mut updated = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|value| value.set(true));
    updated.inherit(&mut next);
    for _ in 0..1000 {
        assert!(updated.next_sample().iter().all(|value| value.is_finite()));
    }
    updated.pause();
    updated.next_sample();
    updated.play();
    updated.next_sample();
    updated.seek(1000);
    updated.stop();
    TRACK_ALLOCATIONS.with(|value| value.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
#[test]
fn effects_process_muted_tracks_unknown_effects_bypass_and_seek_resets() {
    let mut plan = plan(48000, false);
    add_eq(&mut plan);
    let mut reference = Renderer::new(plan.clone());
    reference.play();
    plan.project.tracks[0].muted = true;
    let mut muted = Renderer::new(plan.clone());
    muted.play();
    for _ in 0..1000 {
        reference.next_sample();
        assert_eq!(muted.next_sample(), [0.0; 2]);
    }
    plan.project.tracks[0].muted = false;
    let mut next = Renderer::new(plan.clone());
    next.inherit(&mut muted);
    for frame in 0..500 {
        let a = next.next_sample();
        let b = reference.next_sample();
        if frame >= 240 {
            for ch in 0..2 {
                assert!((a[ch] - b[ch]).abs() < 1e-6);
            }
        }
    }
    next.seek(500);
    let mut reset = Renderer::new(plan.clone());
    reset.playhead = 500;
    let mut a = [[0.0; 2]; 64];
    let mut b = a;
    next.render_offline(&mut a);
    reset.render_offline(&mut b);
    assert_eq!(a, b);
    let mut unknown = plan.clone();
    unknown.project.tracks[0].effects = vec![EffectInstance {
        id: Id::new_v4(),
        unit: EffectSettings::Missing {
            type_id: "unavailable".into(),
            state: serde_json::json!({"value": 1}),
        },
    }];
    let mut dry = unknown.clone();
    dry.project.tracks[0].effects.clear();
    unknown.render(1000, &mut a);
    dry.render(1000, &mut b);
    assert_eq!(a, b);
}

#[test]
fn transport_loop_keeps_filter_history() {
    let mut plan = plan(48000, false);
    add_eq(&mut plan);
    let EffectSettings::Eq3(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.bands[0].frequency_hz = 20.0;
    settings.bands[0].q = 10.0;
    settings.bands[0].gain_db = 24.0;
    plan.project.transport.r#loop.enabled = true;
    plan.project.transport.r#loop.start_frame = 128;
    plan.project.transport.r#loop.end_frame = 1024;
    let mut live = Renderer::new(plan.clone());
    live.play();
    let mut reference = Renderer::new(plan);
    let mut since_wrap = 0;
    for _ in 0..5000 {
        if reference.playhead < 128 || reference.playhead >= 1024 {
            reference.playhead = 128;
            since_wrap = 0;
        }
        let mut expected = [[0.0; 2]];
        reference.render_offline(&mut expected);
        let actual = live.next_sample();
        if since_wrap >= 240 {
            for ch in 0..2 {
                assert!((actual[ch] - expected[0][ch]).abs() < 1e-6);
            }
        }
        since_wrap += 1;
    }
}

#[test]
fn all_eq_bands_off_silence_playback_export_and_live_updates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for mono in [false, true] {
            let mut plan = plan(rate, mono);
            add_eq(&mut plan);
            let mut playing = Renderer::new(plan.clone());
            playing.play();
            for _ in 0..1024 {
                playing.next_sample();
            }
            let EffectSettings::Eq3(settings) = &mut plan.project.tracks[0].effects[0].unit else {
                unreachable!()
            };
            for band in &mut settings.bands {
                band.enabled = false;
            }
            let mut updated = Renderer::new(plan.clone());
            updated.inherit(&mut playing);
            for _ in 0..rate / 200 {
                updated.next_sample();
            }
            for _ in 0..1024 {
                assert_eq!(updated.next_sample(), [0.0; 2]);
            }
            let mut live = Renderer::new(plan.clone());
            live.play();
            let mut offline = Renderer::new(plan.clone());
            for block in [32, 512, 4096] {
                let mut output = vec![[0.0; 2]; block];
                offline.render_offline(&mut output);
                assert!(output.iter().all(|sample| *sample == [0.0; 2]));
                for _ in 0..block {
                    assert_eq!(live.next_sample(), [0.0; 2]);
                }
            }
            assert_eq!(plan.sample_at(1000), [0.0; 2]);
        }
    }
}
