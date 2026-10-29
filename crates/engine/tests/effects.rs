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
    current.set_band_meter(
        plan.project.tracks[0].effects[0].id,
        Arc::new(std::array::from_fn(|_| {
            std::sync::atomic::AtomicU32::new(0)
        })),
    );
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

#[test]
fn drive_and_eq_chain_matches_playback_export_and_block_sizes_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for mono in [false, true] {
            let mut plan = plan(rate, mono);
            add_eq(&mut plan);
            let mut effect = EffectInstance::drive();
            let EffectSettings::Drive(settings) = &mut effect.unit else {
                unreachable!()
            };
            settings.color_shape = DriveColorShape::MidHumpBassCut;
            settings.color_frequency_hz = 1000.0;
            settings.color_strength = 0.8;
            settings.gain = 0.8;
            settings.volume_db = -6.0;
            plan.project.tracks[0].effects.push(effect);
            plan.project.tracks[0].gain_db = -6.0;
            plan.project.tracks[0].pan = -0.3;
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(plan.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    for channel in 0..2 {
                        assert!((actual[channel] - sample[channel]).abs() < 1e-6);
                    }
                }
            }
            for block in [32, 64, 128, 256, 512, 1024, 2048, 4096] {
                let mut output = vec![[0.0; 2]; 8192];
                let mut renderer = Renderer::new(plan.clone());
                for buffer in output.chunks_mut(block) {
                    renderer.render_offline(buffer);
                }
                assert_eq!(output, expected);
            }
            let mut range = [[0.0; 2]; 64];
            plan.render(1000, &mut range);
            assert_eq!(range.as_slice(), &expected[1000..1064]);
            let mut reversed = plan.clone();
            reversed.project.tracks[0].effects.reverse();
            let mut output = vec![[0.0; 2]; 8192];
            Renderer::new(reversed).render_offline(&mut output);
            assert!(
                output
                    .iter()
                    .zip(expected)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
            );
        }
    }
}

#[test]
fn drive_updates_preserve_history_and_retarget_reset_and_processing_allocate_nothing() {
    let mut plan = plan(48000, false);
    add_eq(&mut plan);
    plan.project.tracks[0].effects.push(EffectInstance::drive());
    let mut reference = Renderer::new(plan.clone());
    let mut current = Renderer::new(plan.clone());
    reference.play();
    current.play();
    for _ in 0..700 {
        reference.next_sample();
        current.next_sample();
    }
    let EffectSettings::Drive(settings) = &mut plan.project.tracks[0].effects[1].unit else {
        unreachable!()
    };
    settings.collapsed = true;
    plan.project.tracks[0].effects_compact = true;
    let mut next = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    next.inherit(&mut current);
    for _ in 0..500 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let EffectSettings::Drive(settings) = &mut plan.project.tracks[0].effects[1].unit else {
        unreachable!()
    };
    settings.gain = 1.0;
    settings.color_shape = DriveColorShape::MidScoop;
    settings.color_strength = 1.0;
    settings.color_frequency_hz = 250.0;
    settings.tone = 0.0;
    settings.enabled = false;
    let mut changed = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    changed.inherit(&mut next);
    for _ in 0..1000 {
        assert!(
            changed
                .next_sample()
                .iter()
                .all(|sample| sample.is_finite())
        );
    }
    changed.stop();
    changed.play();
    for _ in 0..1000 {
        changed.next_sample();
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let mut eq_only = plan.clone();
    eq_only.project.tracks[0].effects.pop();
    let mut bypass = vec![[0.0; 2]; 8192];
    let mut dry = vec![[0.0; 2]; 8192];
    Renderer::new(plan).render_offline(&mut bypass);
    Renderer::new(eq_only).render_offline(&mut dry);
    assert_eq!(bypass, dry);
}

#[test]
fn auto_filter_transport_processing_matches_playback_export_and_random_access_at_all_rates() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        for mono in [false, true] {
            let mut plan = plan(rate, mono);
            plan.project.tempo_bpm = 137.0;
            let mut effect = EffectInstance::auto_filter();
            let EffectSettings::AutoFilter(s) = &mut effect.unit else {
                unreachable!()
            };
            s.stereo_phase_degrees = -90.0;
            s.sync_phase_degrees = 45.0;
            s.strength_octaves = 3.0;
            s.note_rate = NoteRate(20);
            plan.project.tracks[0].effects = vec![effect, EffectInstance::drive()];
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(plan.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    for ch in 0..2 {
                        assert!((actual[ch] - sample[ch]).abs() < 1e-6);
                    }
                }
            }
            for block in [32, 64, 128, 256, 512, 1024, 2048, 4096] {
                let mut output = vec![[0.0; 2]; 8192];
                let mut renderer = Renderer::new(plan.clone());
                for buffer in output.chunks_mut(block) {
                    renderer.render_offline(buffer);
                }
                assert_eq!(output, expected);
            }
            let mut range = [[0.0; 2]; 64];
            plan.render(1000, &mut range);
            assert_eq!(range.as_slice(), &expected[1000..1064]);
            let mut reversed = plan.clone();
            reversed.project.tracks[0].effects.reverse();
            let mut output = vec![[0.0; 2]; 8192];
            Renderer::new(reversed).render_offline(&mut output);
            assert!(
                output
                    .iter()
                    .zip(expected)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 1e-4)
            );
        }
    }
}
#[test]
fn auto_filter_live_inheritance_preserves_state_and_processing_reset_and_retarget_do_not_allocate()
{
    let mut plan = plan(48000, false);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::auto_filter());
    let mut reference = Renderer::new(plan.clone());
    let mut current = Renderer::new(plan.clone());
    reference.play();
    current.play();
    for _ in 0..700 {
        assert_eq!(reference.next_sample(), current.next_sample());
    }
    let EffectSettings::AutoFilter(s) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.collapsed = true;
    let mut next = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    next.inherit(&mut current);
    for _ in 0..500 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let EffectSettings::AutoFilter(s) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.frequency_hz = 5000.0;
    s.rate_mode = LfoRateMode::Hz;
    s.rate_hz = 15.0;
    s.wave = LfoWave::Square;
    s.shape = FilterShape::HighPass;
    s.stereo_phase_degrees = 180.0;
    let mut changed = Renderer::new(plan);
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    changed.inherit(&mut next);
    for _ in 0..1000 {
        assert!(changed.next_sample().iter().all(|v| v.is_finite()));
    }
    changed.stop();
    changed.seek(6000);
    changed.play();
    for _ in 0..500 {
        changed.next_sample();
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn auto_filter_loop_wrap_locks_phase_and_keeps_filter_history() {
    let mut plan = plan(48000, false);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::auto_filter());
    let EffectSettings::AutoFilter(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.frequency_hz = 100.0;
    settings.q = 5.0;
    settings.rate_mode = LfoRateMode::Hz;
    settings.rate_hz = 20.0;
    settings.strength_octaves = 3.0;
    settings.stereo_phase_degrees = 90.0;
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

fn add_compressor(plan: &mut RenderPlan, detection: CompressorDetection) -> Id {
    let mut effect = EffectInstance::compressor();
    let EffectSettings::Compressor(settings) = &mut effect.unit else {
        unreachable!()
    };
    settings.threshold_db = -36.0;
    settings.ratio = 6.0;
    settings.attack_ms = 0.1;
    settings.release_ms = 30.0;
    settings.detection = detection;
    settings.level_db = 3.0;
    settings.dry_wet = 0.7;
    let id = effect.id;
    plan.project.tracks[0].effects.push(effect);
    id
}
#[test]
fn compressor_matches_playback_export_blocks_and_random_access_at_all_rates() {
    for rate in SAMPLE_RATES {
        for detection in [CompressorDetection::Peak, CompressorDetection::Rms] {
            let mut plan = plan(rate, false);
            add_compressor(&mut plan, detection);
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(plan.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    for channel in 0..2 {
                        assert!((actual[channel] - sample[channel]).abs() < 1e-6);
                    }
                }
            }
            for block in BUFFER_SIZES {
                let mut output = vec![[0.0; 2]; 8192];
                let mut renderer = Renderer::new(plan.clone());
                for chunk in output.chunks_mut(block as usize) {
                    renderer.render_offline(chunk);
                }
                assert_eq!(output, expected);
            }
            let mut range = [[0.0; 2]; 64];
            plan.render(1000, &mut range);
            assert_eq!(range.as_slice(), &expected[1000..1064]);
            let mut drive = plan.clone();
            drive.project.tracks[0]
                .effects
                .push(EffectInstance::drive());
            let mut first = vec![[0.0; 2]; 8192];
            Renderer::new(drive.clone()).render_offline(&mut first);
            drive.project.tracks[0].effects.reverse();
            let mut reversed = vec![[0.0; 2]; 8192];
            Renderer::new(drive).render_offline(&mut reversed);
            assert!(
                first
                    .iter()
                    .zip(reversed)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 0.001)
            );
        }
    }
}
#[test]
fn compressor_inherits_history_and_meter_without_callback_allocations() {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };
    let mut plan = plan(48000, false);
    let id = add_compressor(&mut plan, CompressorDetection::Rms);
    let meter = Arc::new(AtomicU32::new(0));
    let mut current = Renderer::new(plan.clone());
    current.set_effect_meter(id, meter.clone());
    let mut reference = Renderer::new(plan.clone());
    current.play();
    reference.play();
    for _ in 0..2000 {
        assert_eq!(current.next_sample(), reference.next_sample());
    }
    assert!(f32::from_bits(meter.load(Ordering::Relaxed)) > 3.0);
    plan.project.tracks[0].effects_compact = true;
    let EffectSettings::Compressor(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.collapsed = true;
    let mut next = Renderer::new(plan.clone());
    next.set_effect_meter(id, meter.clone());
    ALLOCATIONS.with(|value| value.set(0));
    TRACK_ALLOCATIONS.with(|value| value.set(true));
    next.inherit(&mut current);
    for _ in 0..1000 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    next.seek(1000);
    assert_eq!(meter.load(Ordering::Relaxed), 0);
    next.next_sample();
    next.stop();
    assert_eq!(meter.load(Ordering::Relaxed), 0);
    TRACK_ALLOCATIONS.with(|value| value.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    // Parameter retargeting retains live history and also remains callback-safe.
    let EffectSettings::Compressor(settings) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    settings.threshold_db = -48.0;
    settings.detection = CompressorDetection::Peak;
    let mut updated = Renderer::new(plan);
    updated.set_effect_meter(id, meter.clone());
    ALLOCATIONS.with(|value| value.set(0));
    TRACK_ALLOCATIONS.with(|value| value.set(true));
    updated.inherit(&mut next);
    updated.play();
    for _ in 0..1000 {
        assert!(
            updated
                .next_sample()
                .iter()
                .all(|sample| sample.is_finite())
        );
    }
    TRACK_ALLOCATIONS.with(|value| value.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(f32::from_bits(meter.load(Ordering::Relaxed)) > 3.0);
}

#[test]
fn ott_matches_live_export_blocks_and_preserves_history_without_allocations() {
    use std::sync::atomic::{AtomicU32, Ordering};
    for rate in SAMPLE_RATES {
        for detection in [CompressorDetection::Peak, CompressorDetection::Rms] {
            let mut plan = plan(rate, false);
            let mut effect = EffectInstance::ott();
            let EffectSettings::Ott(s) = &mut effect.unit else {
                unreachable!()
            };
            s.detection = detection;
            s.bands[1].depth = 0.6;
            s.dry_wet = 0.7;
            let id = effect.id;
            plan.project.tracks[0].effects.push(effect);
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(plan.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(plan.clone());
            live.play();
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    for channel in 0..2 {
                        assert!((actual[channel] - sample[channel]).abs() < 1e-6);
                    }
                }
            }
            for block in BUFFER_SIZES {
                let mut output = vec![[0.0; 2]; 8192];
                let mut renderer = Renderer::new(plan.clone());
                for chunk in output.chunks_mut(block as usize) {
                    renderer.render_offline(chunk);
                }
                assert_eq!(output, expected);
            }
            let mut range = [[0.0; 2]; 64];
            plan.render(1000, &mut range);
            assert_eq!(range.as_slice(), &expected[1000..1064]);
            let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
            let mut current = Renderer::new(plan.clone());
            current.set_ott_meter(id, meter.clone());
            current.play();
            let mut reference = Renderer::new(plan.clone());
            reference.play();
            for _ in 0..1000 {
                assert_eq!(current.next_sample(), reference.next_sample());
            }
            plan.project.tracks[0].effects_compact = true;
            let EffectSettings::Ott(s) = &mut plan.project.tracks[0].effects[0].unit else {
                unreachable!()
            };
            s.collapsed = true;
            let mut next = Renderer::new(plan.clone());
            next.set_ott_meter(id, meter.clone());
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            next.inherit(&mut current);
            for _ in 0..1000 {
                assert_eq!(next.next_sample(), reference.next_sample());
            }
            next.seek(1000);
            assert!(
                meter
                    .iter()
                    .all(|m| f32::from_bits(m.load(Ordering::Relaxed)) == 0.0)
            );
            next.next_sample();
            next.stop();
            assert!(
                meter
                    .iter()
                    .all(|m| f32::from_bits(m.load(Ordering::Relaxed)) == 0.0)
            );
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let EffectSettings::Ott(s) = &mut plan.project.tracks[0].effects[0].unit else {
                unreachable!()
            };
            s.low_frequency_hz = 500.0;
            s.high_frequency_hz = 5000.0;
            s.bands[0].enabled = false;
            let mut updated = Renderer::new(plan);
            updated.set_ott_meter(id, meter.clone());
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            updated.inherit(&mut next);
            updated.play();
            for _ in 0..1000 {
                assert!(updated.next_sample().iter().all(|x| x.is_finite()));
            }
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        }
    }
}

#[test]
fn master_effects_process_the_stereo_sum_before_gain_and_match_live_export_and_seek() {
    use daw_unit_core::{EffectUnit, ProcessingContext};
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut source = plan(rate, false);
        let mut second = source.project.tracks[0].clone();
        second.id = Id::new_v4();
        second.clips[0].id = Id::new_v4();
        second.gain_db = -3.0;
        second.pan = -0.4;
        source.project.tracks.push(second);
        source.project.tracks[0].pan = 0.25;
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 512,
        };
        let mut drive = EffectInstance::drive();
        let EffectSettings::Drive(drive_settings) = &mut drive.unit else {
            unreachable!()
        };
        drive_settings.gain = 0.6;
        let mut manual_drive = daw_drive::Drive::new(context, drive_settings);
        let mut comp = EffectInstance::compressor();
        let EffectSettings::Compressor(comp_settings) = &mut comp.unit else {
            unreachable!()
        };
        comp_settings.threshold_db = -36.0;
        comp_settings.attack_ms = 0.1;
        let mut manual_comp = daw_compressor::Compressor::new(context, comp_settings);
        let mut processed = source.clone();
        processed.project.master.effects = vec![drive, comp];
        processed.project.master.gain_db = -6.0;
        processed.project.validate().unwrap();
        let mut expected = vec![[0.0; 2]; 4096];
        source.render(0, &mut expected);
        for sample in &mut expected {
            *sample = manual_comp
                .process_sample(manual_drive.process_sample(*sample))
                .map(|v| v * linear_gain(-6.0));
        }
        let mut offline = Renderer::new(processed.clone());
        let mut actual = vec![[0.0; 2]; expected.len()];
        for block in actual.chunks_mut(37) {
            offline.render_offline(block);
        }
        let mut live = Renderer::new(processed.clone());
        live.play();
        let fade = (rate / 200) as usize;
        for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
            for ch in 0..2 {
                assert!((actual[ch] - expected[ch]).abs() < 2e-6);
            }
            let sample = live.next_sample();
            if index > fade {
                for ch in 0..2 {
                    assert!((sample[ch] - expected[ch]).abs() < 2e-6);
                }
            }
        }
        let mut random = [[0.0; 2]; 37];
        processed.render(1000, &mut random);
        assert_eq!(random.as_slice(), &actual[1000..1037]);
        assert_eq!(processed.sample_at(1500), actual[1500]);
        offline.seek(0);
        offline.render_offline(&mut random);
        assert_eq!(random.as_slice(), &actual[..37]);
    }
}

#[test]
fn master_effect_history_meters_and_transport_survive_prepared_updates_without_allocation() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let mut plan = plan(48000, false);
    let mut comp = EffectInstance::compressor();
    let EffectSettings::Compressor(settings) = &mut comp.unit else {
        unreachable!()
    };
    settings.threshold_db = -48.0;
    settings.attack_ms = 0.1;
    let id = comp.id;
    plan.project.master.effects = vec![
        EffectInstance::equalizer(),
        comp,
        EffectInstance::auto_filter(),
        EffectInstance::ott(),
    ];
    let meter = Arc::new(AtomicU32::new(0));
    let ott_meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
    let ott_id = plan.project.master.effects.last().unwrap().id;
    let mut old = Renderer::new(plan.clone());
    old.set_effect_meter(id, meter.clone());
    old.set_ott_meter(ott_id, ott_meter.clone());
    old.play();
    for _ in 0..2000 {
        old.next_sample();
    }
    assert!(f32::from_bits(meter.load(Ordering::Relaxed)) > 10.0);
    assert!(ott_meter.iter().any(|v| v.load(Ordering::Relaxed) != 0));
    let mut reference = Renderer::new(plan.clone());
    reference.play();
    for _ in 0..2000 {
        reference.next_sample();
    }
    plan.project.master.effects_compact = true;
    let mut next = Renderer::new(plan.clone());
    next.set_effect_meter(id, meter.clone());
    next.set_ott_meter(ott_id, ott_meter.clone());
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    next.inherit(&mut old);
    let mut error = 0.0_f32;
    for _ in 0..512 {
        let a = next.next_sample();
        let b = reference.next_sample();
        error = error.max((a[0] - b[0]).abs()).max((a[1] - b[1]).abs());
    }
    next.pause();
    for _ in 0..512 {
        next.next_sample();
    }
    next.stop();
    next.seek(0);
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(error < 1e-6, "history changed: {error}");
    assert_eq!(meter.load(Ordering::Relaxed), 0);
    assert!(ott_meter.iter().all(|v| v.load(Ordering::Relaxed) == 0));
}

fn bypass_limiter() -> EffectInstance {
    let mut effect = EffectInstance::limiter();
    let EffectSettings::Limiter(s) = &mut effect.unit else {
        unreachable!()
    };
    s.enabled = false;
    effect
}
#[test]
fn limiter_latency_compensation_preserves_tracks_master_export_seek_and_loop_alignment() {
    for rate in SAMPLE_RATES {
        let mut dry = plan(rate, false);
        let mut second = dry.project.tracks[0].clone();
        second.id = Id::new_v4();
        second.clips[0].id = Id::new_v4();
        second.pan = -0.3;
        dry.project.tracks.push(second);
        let mut wet = dry.clone();
        wet.project.tracks[0].effects = vec![bypass_limiter(), bypass_limiter()];
        wet.project.tracks[1].effects = vec![bypass_limiter()];
        wet.project.master.effects = vec![bypass_limiter()];
        let mut a = Renderer::new(dry.clone());
        let mut b = Renderer::new(wet.clone());
        assert!(b.latency_frames() > 0);
        let mut expected = vec![[0.0; 2]; 8192];
        let mut actual = expected.clone();
        a.render_offline(&mut expected);
        b.render_offline(&mut actual);
        assert_eq!(actual, expected, "offline {rate}");
        wet.project.transport.playhead_frame = 1234;
        let mut direct = [[0.0; 2]; 7];
        wet.render(500, &mut direct);
        assert_eq!(&direct, &expected[500..507]);
        b.seek(3400);
        b.render_offline(&mut actual[..64]);
        assert_eq!(&actual[..64], &expected[3400..3464]);
        for looping in [false, true] {
            for p in [&mut dry.project, &mut wet.project] {
                p.transport.playhead_frame = 0;
                p.transport.r#loop.enabled = looping;
                p.transport.r#loop.start_frame = 1000;
                p.transport.r#loop.end_frame = 1500;
            }
            let mut a = Renderer::new(dry.clone());
            let mut b = Renderer::new(wet.clone());
            a.play();
            b.play();
            a.ramp = 1.0;
            b.ramp = 1.0;
            for i in 0..2200 {
                let expected = a.next_sample();
                let actual = b.next_sample();
                assert_eq!(
                    actual, expected,
                    "live rate={rate} loop={looping} frame={i}"
                );
                assert_eq!(a.playhead, b.playhead);
            }
        }
    }
}
#[test]
fn limiter_live_export_retarget_meter_and_reset_remain_allocation_free() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let mut plan = plan(48_000, false);
    let mut effect = EffectInstance::limiter();
    let id = effect.id;
    let EffectSettings::Limiter(s) = &mut effect.unit else {
        unreachable!()
    };
    s.input_db = 18.0;
    s.ceiling_db = -3.0;
    s.routing = LimiterRouting::MidSide;
    s.mode = LimiterMode::TruePeak;
    plan.project.tracks[0].effects = vec![effect];
    let meter = Arc::new(AtomicU32::new(0));
    let mut a = Renderer::new(plan.clone());
    let mut expected = vec![[0.0; 2]; 2048];
    a.render_offline(&mut expected);
    let mut b = Renderer::new(plan.clone());
    b.set_effect_meter(id, meter.clone());
    b.play();
    b.ramp = 1.0;
    for sample in &expected {
        assert_eq!(*sample, b.next_sample());
    }
    assert!(f32::from_bits(meter.load(Ordering::Relaxed)) > 0.0);
    let mut reference = Renderer::new(plan.clone());
    reference.play();
    reference.ramp = 1.0;
    for _ in 0..2048 {
        reference.next_sample();
    }
    plan.project.tracks[0].effects_compact = true;
    let mut next = Renderer::new(plan.clone());
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    next.inherit(&mut b);
    for _ in 0..500 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    next.pause();
    for _ in 0..300 {
        next.next_sample();
    }
    next.play();
    next.seek(400);
    next.stop();
    next.play();
    for _ in 0..600 {
        next.next_sample();
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    // Changing the latency topology prepares new delays; the callback primes without allocating.
    plan.project.master.effects.push(bypass_limiter());
    let mut changed = Renderer::new(plan);
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    changed.inherit(&mut next);
    changed.next_sample();
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn eq_band_meters_preserve_audio_history_and_do_not_allocate_on_track_or_master() {
    use std::sync::atomic::{AtomicU32, Ordering};
    for master in [false, true] {
        let mut plan = plan(48000, false);
        add_eq(&mut plan);
        if master {
            plan.project.master.effects = std::mem::take(&mut plan.project.tracks[0].effects);
        }
        let effects = if master {
            &plan.project.master.effects
        } else {
            &plan.project.tracks[0].effects
        };
        let id = effects[0].id;
        let meter = Arc::new(std::array::from_fn(|_| AtomicU32::new(0)));
        let mut current = Renderer::new(plan.clone());
        current.set_band_meter(id, meter.clone());
        let mut reference = Renderer::new(plan.clone());
        current.play();
        reference.play();
        for _ in 0..2000 {
            assert_eq!(current.next_sample(), reference.next_sample());
        }
        assert!(f32::from_bits(meter[1].load(Ordering::Relaxed)) > 0.1);
        let effects = if master {
            &mut plan.project.master.effects
        } else {
            &mut plan.project.tracks[0].effects
        };
        let EffectSettings::Eq3(settings) = &mut effects[0].unit else {
            unreachable!()
        };
        settings.collapsed = true;
        let mut next = Renderer::new(plan.clone());
        next.set_band_meter(id, meter.clone());
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|value| value.set(true));
        next.inherit(&mut current);
        for _ in 0..1000 {
            assert_eq!(next.next_sample(), reference.next_sample());
        }
        next.pause();
        // Pause completes the existing 5 ms output transition before freezing DSP.
        for _ in 0..300 {
            next.next_sample();
        }
        let snapshot = meter.each_ref().map(|cell| cell.load(Ordering::Relaxed));
        for _ in 0..100 {
            next.next_sample();
        }
        assert_eq!(
            snapshot,
            meter.each_ref().map(|cell| cell.load(Ordering::Relaxed))
        );
        next.play();
        next.seek(1000);
        assert!(meter.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
        next.next_sample();
        next.stop();
        assert!(meter.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
        TRACK_ALLOCATIONS.with(|value| value.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}

#[test]
fn direction_mixer_ms_decode_matches_predecoded_audio_on_tracks_and_master() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let baseline = plan(rate, false);
            let mut encoded = baseline.clone();
            let mut effect = EffectInstance::direction_mixer();
            let EffectSettings::DirectionMixer(settings) = &mut effect.unit else {
                unreachable!()
            };
            settings.input = DirectionMixerInput::MidSide;
            if master {
                encoded.project.master.effects.push(effect);
            } else {
                encoded.project.tracks[0].effects.push(effect);
            }
            let mut decoded = baseline.clone();
            for audio in decoded.audio.values_mut() {
                audio.samples = Arc::new(
                    audio
                        .samples
                        .iter()
                        .map(|[mid, side]| [mid + side, mid - side])
                        .collect(),
                );
            }
            let mut actual = vec![[0.0; 2]; 8192];
            let mut expected = actual.clone();
            Renderer::new(encoded).render_offline(&mut actual);
            Renderer::new(decoded).render_offline(&mut expected);
            for (a, b) in actual.iter().zip(&expected) {
                for ch in 0..2 {
                    assert!(
                        (a[ch] - b[ch]).abs() < 1e-7,
                        "rate={rate} master={master}: {a:?} {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn direction_mixer_lr_spread_changes_stereo_width_on_tracks_master_and_live_updates() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let mut baseline = plan(rate, false);
            for audio in baseline.audio.values_mut() {
                // Different tones ensure real stereo content, rather than two copies of mono.
                audio.samples = Arc::new(
                    (0..8192)
                        .map(|frame| {
                            [440.0, 731.0].map(|hz| {
                                (frame as f32 * hz * std::f32::consts::TAU / rate as f32).sin()
                                    * 0.1
                            })
                        })
                        .collect(),
                );
            }
            let mut dry = vec![[0.0; 2]; 8192];
            Renderer::new(baseline.clone()).render_offline(&mut dry);
            assert!(dry.iter().any(|[left, right]| (left - right).abs() > 0.1));
            let effect = EffectInstance::direction_mixer();
            let chain = if master {
                &mut baseline.project.master.effects
            } else {
                &mut baseline.project.tracks[0].effects
            };
            chain.push(effect);
            for (spread, mid_gain, side_gain) in [
                (0.0, 1.0, 0.0),
                (0.5, 1.0, 0.5),
                (1.0, 1.0, 1.0),
                (1.25, 0.818, 1.0),
                (1.5, 0.65, 1.0),
                (1.75, 0.482, 1.0),
                (2.0, 0.3, 1.0),
            ] {
                let mut changed = baseline.clone();
                let chain = if master {
                    &mut changed.project.master.effects
                } else {
                    &mut changed.project.tracks[0].effects
                };
                let EffectSettings::DirectionMixer(settings) = &mut chain[0].unit else {
                    unreachable!()
                };
                settings.spread = spread;
                let mut actual = vec![[0.0; 2]; 8192];
                Renderer::new(changed.clone()).render_offline(&mut actual);
                let mut old = Renderer::new(baseline.clone());
                old.play();
                for _ in 0..1000 {
                    old.next_sample();
                }
                let mut live = Renderer::new(changed);
                live.inherit(&mut old);
                for (frame, sample) in actual.iter().enumerate() {
                    let [left, right] = dry[frame];
                    let mid = (left + right) * 0.5 * mid_gain;
                    let side = (left - right) * 0.5 * side_gain;
                    let expected = [mid + side, mid - side];
                    for ch in 0..2 {
                        assert!(
                            (sample[ch] - expected[ch]).abs() < 1e-7,
                            "rate={rate} master={master} spread={spread}: {sample:?} != {expected:?}"
                        );
                    }
                    if frame >= 1000 {
                        let playing = live.next_sample();
                        if frame >= 1000 + rate as usize / 200 {
                            for ch in 0..2 {
                                assert!((playing[ch] - sample[ch]).abs() < 1e-7);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn stereo_spread_live_offline_master_and_prepared_order_updates_share_processing() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let baseline = plan(rate, true);
            let mut p = baseline.clone();
            let mut effect = EffectInstance::stereo_spread();
            let EffectSettings::StereoSpread(s) = &mut effect.unit else {
                unreachable!()
            };
            s.lower_intensity = 0.5;
            s.upper_intensity = 0.5;
            let id = effect.id;
            if master {
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0].effects.push(effect);
            }
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut dry = vec![[0.0; 2]; 8192];
            Renderer::new(baseline).render_offline(&mut dry);
            assert!(expected.iter().any(|[l, r]| (l - r).abs() > 0.001));
            let mut live = Renderer::new(p.clone());
            live.play();
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                assert!(((sample[0] + sample[1]) * 0.5 - dry[frame][0]).abs() < 1e-6);
                if frame >= rate as usize / 200 {
                    for ch in 0..2 {
                        assert!((actual[ch] - sample[ch]).abs() < 1e-6);
                    }
                }
            }
            let mut output = vec![[0.0; 2]; 8192];
            let mut blocks = Renderer::new(p.clone());
            for buffer in output.chunks_mut(128) {
                blocks.render_offline(buffer);
            }
            assert_eq!(output, expected);
            let mut random = [[0.0; 2]; 64];
            p.render(1000, &mut random);
            assert_eq!(random.as_slice(), &expected[1000..1064]);
            live.seek(1000);
            for _ in 0..512 {
                live.next_sample();
            }
            let chain = if master {
                &mut p.project.master.effects
            } else {
                &mut p.project.tracks[0].effects
            };
            let EffectSettings::StereoSpread(s) = &mut chain[0].unit else {
                unreachable!()
            };
            s.order = 12;
            s.lower_frequency_hz = 1000.0;
            let mut replacement = Renderer::new(p.clone());
            ALLOCATIONS.with(|c| c.set(0));
            TRACK_ALLOCATIONS.with(|c| c.set(true));
            replacement.inherit(&mut live);
            for _ in 0..rate / 50 {
                assert!(replacement.next_sample().iter().all(|v| v.is_finite()));
            }
            let snapshot = replacement
                .modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                .unwrap();
            assert!((snapshot[2] - 1000.0).abs() < 0.02);
            replacement.seek(1000);
            replacement.stop();
            TRACK_ALLOCATIONS.with(|c| c.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        }
    }
}

#[test]
fn bitcrusher_track_master_live_offline_blocks_and_compatible_updates_share_clock() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            for mono in [false, true] {
                let mut p = plan(rate, mono);
                let mut effect = EffectInstance::bitcrusher();
                let EffectSettings::Bitcrusher(s) = &mut effect.unit else {
                    unreachable!()
                };
                s.drive_db = 6.0;
                s.resolution_bits = 5;
                s.downsampling = 7;
                s.level_db = -3.0;
                s.dry_wet = 0.6;
                let id = effect.id;
                if master {
                    p.project.master.effects.push(effect);
                } else {
                    p.project.tracks[0].effects.push(effect);
                }
                let mut expected = vec![[0.0; 2]; 8192];
                Renderer::new(p.clone()).render_offline(&mut expected);
                let mut live = Renderer::new(p.clone());
                live.play();
                for (frame, sample) in expected.iter().enumerate() {
                    let actual = live.next_sample();
                    if frame >= rate as usize / 200 {
                        for ch in 0..2 {
                            assert!((actual[ch] - sample[ch]).abs() < 1e-6);
                        }
                    }
                }
                let mut output = vec![[0.0; 2]; 8192];
                let mut blocks = Renderer::new(p.clone());
                for buffer in output.chunks_mut(127) {
                    blocks.render_offline(buffer);
                }
                assert_eq!(output, expected);
                let mut random = [[0.0; 2]; 64];
                p.render(1000, &mut random);
                assert_eq!(random.as_slice(), &expected[1000..1064]);
                // Replacing the renderer for a presentation edit retains the capture phase.
                let chain = if master {
                    &mut p.project.master.effects
                } else {
                    &mut p.project.tracks[0].effects
                };
                let EffectSettings::Bitcrusher(s) = &mut chain[0].unit else {
                    unreachable!()
                };
                s.modulation_open = true;
                let mut old = Renderer::new(p.clone());
                let mut reference = Renderer::new(p.clone());
                old.play();
                reference.play();
                for _ in 0..1001 {
                    old.next_sample();
                    reference.next_sample();
                }
                let mut replacement = Renderer::new(p.clone());
                ALLOCATIONS.with(|c| c.set(0));
                TRACK_ALLOCATIONS.with(|c| c.set(true));
                replacement.inherit(&mut old);
                for _ in 0..101 {
                    assert_eq!(replacement.next_sample(), reference.next_sample());
                }
                TRACK_ALLOCATIONS.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                let chain = if master {
                    &mut p.project.master.effects
                } else {
                    &mut p.project.tracks[0].effects
                };
                let EffectSettings::Bitcrusher(s) = &mut chain[0].unit else {
                    unreachable!()
                };
                s.resolution_bits = 8;
                s.downsampling = 13;
                s.drive_db = 12.0;
                let mut changed = Renderer::new(p.clone());
                ALLOCATIONS.with(|c| c.set(0));
                TRACK_ALLOCATIONS.with(|c| c.set(true));
                changed.inherit(&mut replacement);
                for _ in 0..rate / 50 {
                    assert!(changed.next_sample().iter().all(|v| v.is_finite()));
                }
                let snapshot = changed
                    .modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                    .unwrap();
                assert_eq!(snapshot[..3], [12.0, 8.0, 13.0]);
                assert!(snapshot[5..].iter().all(|v| v.is_nan()));
                changed.seek(1000);
                changed.stop();
                TRACK_ALLOCATIONS.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            }
        }
    }
}

#[test]
fn persistent_offline_ranges_preserve_history_and_reset_for_backward_requests() {
    let mut plan = plan(48_000, false);
    add_eq(&mut plan);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::limiter());
    let mut expected = vec![[0.0; 2]; 8192];
    Renderer::new(plan.clone()).render_offline(&mut expected);
    let mut renderer = Renderer::new(plan);
    for (start, count) in [
        (0, 32),
        (32, 512),
        (2000, 41),
        (2041, 512),
        (100, 64),
        (5000, 33),
    ] {
        let mut actual = vec![[0.0; 2]; count];
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|c| c.set(true));
        renderer.render_offline_at(start, &mut actual);
        TRACK_ALLOCATIONS.with(|c| c.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(actual, expected[start as usize..start as usize + count]);
    }
}

#[test]
fn worker_prepared_transport_activation_does_not_allocate_or_prime_latency() {
    for rate in daw_core::SAMPLE_RATES {
        let mut plan = plan(rate, false);
        for _ in 0..8 {
            plan.project.tracks[0]
                .effects
                .push(EffectInstance::limiter());
        }
        let mut old = Renderer::new(plan.clone());
        old.play();
        for _ in 0..32 {
            old.next_sample();
        }
        let mut prepared = Renderer::new(plan);
        prepared.prepare_transport(daw_engine::TransportState {
            frame: 4000,
            playing: true,
            paused: false,
            playback_start: Some(0),
        });
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|c| c.set(true));
        prepared.activate_transport(&old);
        let state = prepared.transport_state();
        TRACK_ALLOCATIONS.with(|c| c.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(state.frame, 4000);
        assert!(state.playing);
        assert_eq!(state.playback_start, Some(0));
        assert!(prepared.latency_frames() > 0);
    }
}

#[test]
fn delayed_effect_clock_matches_track_to_master_boundary_in_live_export_and_seek() {
    for rate in SAMPLE_RATES {
        let mut serial = plan(rate, false);
        serial.project.tracks[0].effects = vec![EffectInstance::saturator(), bypass_limiter()];
        let mut filter = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(s) = &mut filter.unit else {
            unreachable!()
        };
        s.frequency_hz = 200.0;
        s.q = 3.0;
        s.rate_mode = LfoRateMode::Hz;
        s.rate_hz = 20.0;
        s.strength_octaves = 4.0;
        s.stereo_phase_degrees = 90.0;
        let mut split = serial.clone();
        serial.project.tracks[0].effects.push(filter.clone());
        split.project.master.effects.push(filter);
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(split.clone()).render_offline(&mut expected);
        let mut actual = expected.clone();
        let mut renderer = Renderer::new(serial.clone());
        for block in actual.chunks_mut(127) {
            renderer.render_offline(block);
        }
        assert_eq!(actual, expected, "offline rate={rate}");
        assert!(expected.iter().any(|s| s[0].abs() > 1e-4));
        let mut sought = [[0.0; 2]; 127];
        Renderer::new(serial.clone()).render_offline_at(2048, &mut sought);
        assert_eq!(sought.as_slice(), &expected[2048..2175]);
        for looping in [false, true] {
            for p in [&mut serial, &mut split] {
                p.project.transport.r#loop = Loop {
                    enabled: looping,
                    start_frame: 1000,
                    end_frame: 1700,
                };
            }
            let mut a = Renderer::new(serial.clone());
            let mut b = Renderer::new(split.clone());
            a.play();
            b.play();
            for tick in 0..4096 {
                if tick == 2000 {
                    a.seek(1001);
                    b.seek(1001);
                }
                assert_eq!(
                    a.next_sample(),
                    b.next_sample(),
                    "rate={rate} loop={looping} tick={tick}"
                );
                assert_eq!(a.playhead, b.playhead);
            }
        }
    }
}

#[test]
fn saturator_track_master_live_offline_blocks_and_compatible_updates_share_clock() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            for mono in [false, true] {
                let mut p = plan(rate, mono);
                let mut effect = EffectInstance::saturator();
                let EffectSettings::Saturator(s) = &mut effect.unit else {
                    unreachable!()
                };
                s.drive_db = 6.0;
                s.shape = SaturatorShape::SoftSine;
                s.clip_mode = SaturatorClipMode::SoftClip;
                s.level_db = -3.0;
                s.dry_wet = 0.6;
                let id = effect.id;
                if master {
                    p.project.master.effects.push(effect);
                } else {
                    p.project.tracks[0].effects.push(effect);
                }
                let mut expected = vec![[0.0; 2]; 8192];
                Renderer::new(p.clone()).render_offline(&mut expected);
                let mut live = Renderer::new(p.clone());
                live.play();
                for (frame, sample) in expected.iter().enumerate() {
                    let actual = live.next_sample();
                    if frame >= rate as usize / 200 {
                        for ch in 0..2 {
                            assert!((actual[ch] - sample[ch]).abs() < 1e-6);
                        }
                    }
                }
                let mut output = vec![[0.0; 2]; 8192];
                let mut blocks = Renderer::new(p.clone());
                for buffer in output.chunks_mut(127) {
                    blocks.render_offline(buffer);
                }
                assert_eq!(output, expected);
                let mut random = [[0.0; 2]; 64];
                p.render(1000, &mut random);
                assert_eq!(random.as_slice(), &expected[1000..1064]);
                // Replacing the renderer for a presentation edit retains the capture phase.
                let chain = if master {
                    &mut p.project.master.effects
                } else {
                    &mut p.project.tracks[0].effects
                };
                let EffectSettings::Saturator(s) = &mut chain[0].unit else {
                    unreachable!()
                };
                s.modulation_open = true;
                let mut old = Renderer::new(p.clone());
                let mut reference = Renderer::new(p.clone());
                old.play();
                reference.play();
                for _ in 0..1001 {
                    old.next_sample();
                    reference.next_sample();
                }
                let mut replacement = Renderer::new(p.clone());
                ALLOCATIONS.with(|c| c.set(0));
                TRACK_ALLOCATIONS.with(|c| c.set(true));
                replacement.inherit(&mut old);
                for _ in 0..101 {
                    assert_eq!(replacement.next_sample(), reference.next_sample());
                }
                TRACK_ALLOCATIONS.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                let chain = if master {
                    &mut p.project.master.effects
                } else {
                    &mut p.project.tracks[0].effects
                };
                let EffectSettings::Saturator(s) = &mut chain[0].unit else {
                    unreachable!()
                };
                s.shape = SaturatorShape::BassShaper;
                s.clip_mode = SaturatorClipMode::HardClip;
                s.drive_db = 12.0;
                let mut changed = Renderer::new(p.clone());
                ALLOCATIONS.with(|c| c.set(0));
                TRACK_ALLOCATIONS.with(|c| c.set(true));
                changed.inherit(&mut replacement);
                for _ in 0..rate / 50 {
                    assert!(changed.next_sample().iter().all(|v| v.is_finite()));
                }
                let snapshot = changed
                    .modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                    .unwrap();
                assert!(
                    snapshot[..3]
                        .iter()
                        .zip([12.0, -3.0, 0.6])
                        .all(|(a, b)| (*a - b).abs() < 1e-5)
                );
                assert!(snapshot[3..].iter().all(|v| v.is_nan()));
                changed.seek(1000);
                changed.stop();
                TRACK_ALLOCATIONS.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            }
        }
    }
}

#[test]
fn worker_snapshots_preserve_every_effect_history_and_own_independent_state() {
    for effect in [
        EffectInstance::equalizer(),
        EffectInstance::drive(),
        EffectInstance::auto_filter(),
        EffectInstance::compressor(),
        EffectInstance::ott(),
        EffectInstance::limiter(),
        EffectInstance::direction_mixer(),
        EffectInstance::stereo_spread(),
        EffectInstance::saturator(),
        EffectInstance::bitcrusher(),
    ] {
        let mut p = plan(48_000, false);
        p.project.tracks[0].effects.push(effect);
        let mut renderer = Renderer::new(p);
        renderer.play();
        for _ in 0..4096 {
            renderer.next_sample();
        }
        let mut snapshot = renderer.clone();
        let mut independent = snapshot.clone();
        for _ in 0..2048 {
            assert_eq!(renderer.next_sample(), snapshot.next_sample());
        }
        // Mutating the original cannot change a checkpoint's processor buffers.
        renderer.prepare_transport(Default::default());
        for _ in 0..2048 {
            independent.next_sample();
        }
        for _ in 0..512 {
            assert_eq!(independent.next_sample(), snapshot.next_sample());
        }
    }
}

#[test]
fn group_buses_meter_and_retarget_without_callback_allocation() {
    let mut plan = plan(48_000, false);
    let track = plan.project.tracks[0].id;
    let bus = Id::new_v4();
    plan.project
        .edit(Edit::GroupTracks {
            track_ids: vec![track],
            group_id: bus,
            name: "Group".into(),
        })
        .unwrap();
    plan.project.tracks[0].effects = vec![EffectInstance::compressor(), EffectInstance::limiter()];
    let mut current = Renderer::new(plan.clone());
    current.play();
    for _ in 0..1000 {
        current.next_sample();
    }
    plan.project.tracks[0].gain_db = -6.0;
    plan.project.tracks[0].group_collapsed = true;
    let mut next = Renderer::new(plan);
    assert!(next.can_inherit_without_preroll(&current));
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    next.inherit(&mut current);
    next.reset_track_peaks();
    for _ in 0..1000 {
        next.next_sample();
    }
    let mut peaks = next.track_peaks();
    let bus_peak = peaks.next().unwrap();
    let child_peak = peaks.next().unwrap();
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(bus_peak[0] > 0.0 && bus_peak[0] < child_peak[0]);
}

#[test]
fn filter_live_inheritance_preserves_state_and_processing_reset_and_retarget_do_not_allocate() {
    let mut plan = plan(48000, false);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::filter());
    let mut reference = Renderer::new(plan.clone());
    let mut current = Renderer::new(plan.clone());
    reference.play();
    current.play();
    for _ in 0..700 {
        assert_eq!(reference.next_sample(), current.next_sample());
    }
    let EffectSettings::Filter(s) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.collapsed = true;
    let mut next = Renderer::new(plan.clone());
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    next.inherit(&mut current);
    for _ in 0..500 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let EffectSettings::Filter(s) = &mut plan.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.frequency_hz = 5000.0;
    s.shape = FilterShape::HighPass;
    let mut changed = Renderer::new(plan);
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    changed.inherit(&mut next);
    for _ in 0..1000 {
        assert!(changed.next_sample().iter().all(|v| v.is_finite()));
    }
    changed.stop();
    changed.seek(6000);
    changed.play();
    for _ in 0..500 {
        changed.next_sample();
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn drive_v2_live_export_seek_block_processing_and_retarget_share_the_portable_path() {
    for rate in SAMPLE_RATES {
        for master in [false, true] {
            let mut p = plan(rate, false);
            let mut effect = EffectInstance::drive_v2();
            let key = daw_engine::ModulationTarget::Effect(effect.id);
            let EffectSettings::DriveV2(s) = &mut effect.unit else {
                panic!()
            };
            s.input_gain_db = 6.0;
            s.color_enabled = true;
            s.gain = 0.6;
            s.tone = 0.7;
            s.level_db = -6.0;
            s.dry_wet = 0.7;
            s.clip = DriveV2Clip::Hard;
            if master {
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0].effects.push(effect);
            }
            p.project.validate().unwrap();
            let mut expected = vec![[0.0; 2]; 4096];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut blocks = vec![[0.0; 2]; 4096];
            let mut r = Renderer::new(p.clone());
            for block in blocks.chunks_mut(37) {
                r.render_offline(block);
            }
            assert_eq!(blocks, expected);
            let mut random = [[0.0; 2]; 128];
            p.render(2000, &mut random);
            assert_eq!(&random[..], &expected[2000..2128]);
            let mut live = Renderer::new(p.clone());
            live.play();
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            for (frame, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame > rate as usize / 200 {
                    assert_eq!(*sample, actual);
                }
            }
            let values = live.modulation_snapshot(key).unwrap();
            assert!(values[..7].iter().all(|v| v.is_finite()));
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let effects = if master {
                &mut p.project.master.effects
            } else {
                &mut p.project.tracks[0].effects
            };
            let EffectSettings::DriveV2(s) = &mut effects[0].unit else {
                panic!()
            };
            s.color_enabled = false;
            s.clip = DriveV2Clip::Soft;
            s.dry_wet = 0.0;
            let mut next = Renderer::new(p);
            ALLOCATIONS.with(|n| n.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            next.inherit(&mut live);
            for _ in 0..512 {
                assert!(next.next_sample().iter().all(|v| v.is_finite()));
            }
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        }
    }
}

#[test]
fn multiband_eq_live_export_seek_and_structural_updates_preserve_history_without_allocations() {
    for rate in [44_100, 48_000, 96_000, 192_000] {
        let mut plan = plan(rate, false);
        let mut effect = EffectInstance::multiband_equalizer();
        let EffectSettings::MultibandEq(s) = &mut effect.unit else {
            unreachable!()
        };
        s.bands[0].gain_db = 9.0;
        s.bands.push(MultibandEqBand {
            mode: EqBandMode::LowShelf,
            frequency_hz: 100.0,
            gain_db: -6.0,
            ..Default::default()
        });
        s.output_gain_db = -3.0;
        let band = s.bands[0].id;
        let effect_id = effect.id;
        plan.project.tracks[0].effects.push(effect);
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(plan.clone()).render_offline(&mut expected);
        let mut current = Renderer::new(plan.clone());
        let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
        current.set_target_band_peak_meter(
            daw_engine::ModulationTarget::EffectBand {
                effect: effect_id,
                band,
            },
            meter.clone(),
        );
        current.play();
        for (frame, want) in expected.iter().enumerate() {
            let actual = current.next_sample();
            if frame >= rate as usize / 200 {
                for ch in 0..2 {
                    assert!((actual[ch] - want[ch]).abs() < 1e-6);
                }
            }
        }
        assert!(meter.readings()[0] > 0.0);
        let snapshot = current
            .modulation_snapshot(daw_engine::ModulationTarget::EffectBand {
                effect: effect_id,
                band,
            })
            .unwrap();
        assert_eq!(
            &snapshot[..3],
            &[9.0, 1000.0, std::f32::consts::FRAC_1_SQRT_2]
        );
        for block in [32, 512, 4096] {
            let mut output = vec![[0.0; 2]; 4096];
            let mut renderer = Renderer::new(plan.clone());
            for buffer in output.chunks_mut(block) {
                renderer.render_offline(buffer);
            }
            assert_eq!(output, expected);
        }
        let mut range = [[0.0; 2]; 64];
        plan.render(1000, &mut range);
        assert_eq!(range.as_slice(), &expected[1000..1064]);
        let mut reference = Renderer::new(plan.clone());
        reference.play();
        for _ in 0..4096 {
            reference.next_sample();
        }
        let EffectSettings::MultibandEq(s) = &mut plan.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        s.collapsed = true;
        s.bands.push(Default::default());
        s.bands.rotate_right(1);
        // Added neutral bands and visual edits must retain the active filters' exact histories.
        let mut next = Renderer::new(plan.clone());
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|v| v.set(true));
        next.inherit(&mut current);
        for _ in 0..512 {
            let a = next.next_sample();
            let b = reference.next_sample();
            for ch in 0..2 {
                assert!((a[ch] - b[ch]).abs() < 1e-6);
            }
        }
        TRACK_ALLOCATIONS.with(|v| v.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        let EffectSettings::MultibandEq(s) = &mut plan.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        s.remove_band(band);
        s.bands[0].mode = EqBandMode::Notch;
        s.bands[0].q = 10.0;
        s.output_gain_db = 12.0;
        let mut updated = Renderer::new(plan);
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|v| v.set(true));
        updated.inherit(&mut next);
        for _ in 0..512 {
            assert!(updated.next_sample().iter().all(|v| v.is_finite()));
        }
        TRACK_ALLOCATIONS.with(|v| v.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}

#[test]
fn chorus_track_master_export_seek_and_compatible_edits_share_history_without_allocating() {
    for rate in SAMPLE_RATES {
        for (master, note) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut p = plan(rate, false);
            let mut effect = EffectInstance::chorus();
            if note {
                p.project.tempo_bpm = 153.0;
                let EffectSettings::Chorus(s) = &mut effect.unit else {
                    panic!()
                };
                s.rate_mode = daw_core::LfoRateMode::Note;
                s.note_rate = daw_core::NoteRate(14); // Quarter-note triplet.
            }
            let id = effect.id;
            if master {
                p.project.master.effects.push(EffectInstance::saturator());
                p.project.master.effects.push(effect);
            } else {
                p.project.tracks[0]
                    .effects
                    .push(EffectInstance::saturator());
                p.project.tracks[0].effects.push(effect);
            }
            let mut expected = vec![[0.0; 2]; 8192];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p.clone());
            live.play();
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            for (frame, expected) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if frame > rate as usize / 200 {
                    assert_eq!(&actual, expected);
                }
            }
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let mut prepared = Renderer::new(p.clone());
            let mut state = live.transport_state();
            state.frame = 4096;
            prepared.prepare_transport(state);
            for (frame, expected_sample) in expected.iter().enumerate().skip(4096) {
                let actual = prepared.next_sample();
                if frame > 4096 + rate as usize / 200 {
                    assert_eq!(actual, *expected_sample);
                }
            }
            let mut current = Renderer::new(p.clone());
            let mut reference = Renderer::new(p.clone());
            current.play();
            reference.play();
            for _ in 0..3000 {
                assert_eq!(current.next_sample(), reference.next_sample());
            }
            let chain = if master {
                &mut p.project.master.effects
            } else {
                &mut p.project.tracks[0].effects
            };
            let EffectSettings::Chorus(settings) =
                &mut chain.iter_mut().find(|e| e.id == id).unwrap().unit
            else {
                panic!()
            };
            settings.collapsed = true;
            let mut next = Renderer::new(p);
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            next.inherit(&mut current);
            for _ in 0..1024 {
                assert_eq!(next.next_sample(), reference.next_sample());
            }
            let snapshot = next
                .modulation_snapshot(daw_engine::ModulationTarget::Effect(id))
                .unwrap();
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            assert!((snapshot[0] - 15.0).abs() < 1e-6);
        }
    }
}
