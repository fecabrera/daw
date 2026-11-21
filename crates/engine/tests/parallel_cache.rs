#[path = "support/cache_project.rs"]
mod fixture;
use daw_core::*;
use daw_engine::{CacheProfile, PreparationMode, Renderer, TransportCache, TransportState};

fn compare(mut serial: Renderer, mut parallel: Renderer, count: usize) {
    assert_eq!(serial.transport_state(), parallel.transport_state());
    for frame in 0..count {
        assert_eq!(
            serial.next_sample().map(f32::to_bits),
            parallel.next_sample().map(f32::to_bits),
            "sample {frame}"
        );
    }
}

#[test]
fn rendered_audio_replays_exactly_and_retains_serial_captured_returns() {
    let mut plan = fixture::midi_project(48_000, 2, 1);
    let rendered = fixture::add_audio(&mut plan, 1, 0, false);
    let companion = fixture::add_audio(&mut plan, 0, 0, false);
    for asset in &mut plan.project.assets {
        if [rendered, companion].contains(&asset.id) {
            asset.source_metadata.sample_format = "ieee_float".into();
        }
    }
    let output = &mut plan.project.tracks[0].instrument.as_mut().unwrap().main;
    output.effects = vec![EffectInstance::drive_v2(), EffectInstance::saturator()];
    plan.project.master.effects.push(EffectInstance::limiter());
    plan.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 1_235,
        end_frame: 24_567,
    };
    let provider = plan.project.tracks[2].id;
    let rendered_index = plan
        .project
        .assets
        .iter()
        .position(|a| a.id == rendered)
        .unwrap();
    let source = &mut plan.project.assets[rendered_index];
    source.source.kind = "rendered_audio".into();
    source.rendered_audio = Some(RenderedAudio {
        version: 1,
        pre_send_asset: companion,
        feeds: vec![RenderedFeed {
            provider,
            raw_asset: companion,
            post_asset: rendered,
        }],
        return_feeds: vec![],
    });
    plan.project.validate().unwrap();

    let state = TransportState {
        frame: 23_000,
        playing: true,
        playback_start: Some(1_235),
        ..Default::default()
    };
    for with_return in [false, true] {
        if with_return {
            let return_id = plan.project.add_return_track().unwrap();
            let source = plan.project.assets[rendered_index]
                .rendered_audio
                .as_mut()
                .unwrap();
            source.version = 3;
            source.return_feeds.push(RenderedReturnFeed {
                return_id,
                asset: companion,
            });
            plan.project.validate().unwrap();
        }
        let mut serial = TransportCache::with_mode(
            plan.clone(),
            CacheProfile::Balanced,
            PreparationMode::Serial,
        );
        let mut parallel = TransportCache::with_mode(
            plan.clone(),
            CacheProfile::Balanced,
            PreparationMode::Parallel,
        );
        for _ in 0..2 {
            compare(
                serial.prepare(state, || false).unwrap(),
                parallel.prepare(state, || false).unwrap(),
                4096,
            );
        }
        let stats = parallel.preparation_stats();
        assert_eq!(stats.workers, if with_return { 0 } else { 2 });
        assert_eq!(stats.parallel_blocks > 0, !with_return);
    }
}
#[test]
fn two_workers_preserve_exact_history_checkpoints_loops_bypass_and_ordered_master() {
    for rate in [44_100, 48_000, 192_000] {
        for looping in [false, true] {
            let mut plan = fixture::midi_project(rate, 4, 1);
            plan.project.buffer_size_frames = if looping { 32 } else { 4096 };
            plan.project.transport.r#loop = Loop {
                enabled: looping,
                start_frame: 1234,
                end_frame: 7123,
            };
            plan.project.tracks[0]
                .effects
                .push(EffectInstance::limiter());
            if let EffectSettings::Saturator(settings) = &mut plan.project.tracks[1].effects[0].unit
            {
                settings.enabled = false;
            }
            plan.project.tracks[2].instrument.as_mut().unwrap().layers[0]
                .effects
                .push(EffectInstance::saturator());
            plan.project.tracks[3]
                .instrument
                .as_mut()
                .unwrap()
                .main
                .effects
                .push(EffectInstance::auto_filter());
            plan.project.master.effects = vec![
                EffectInstance::saturator(),
                EffectInstance::auto_filter(),
                EffectInstance::limiter(),
            ];
            let mut serial = TransportCache::with_mode(
                plan.clone(),
                CacheProfile::Responsive,
                PreparationMode::Serial,
            );
            let mut parallel = TransportCache::with_mode(
                plan.clone(),
                CacheProfile::Responsive,
                PreparationMode::Parallel,
            );
            for frame in [11001, 4200, 17000] {
                let state = TransportState {
                    frame,
                    playing: true,
                    paused: false,
                    playback_start: Some(1234),
                };
                let reference = serial.prepare(state, || false).unwrap();
                let candidate = parallel.prepare(state, || false).unwrap();
                compare(reference, candidate, 768);
                let mut full = Renderer::new(plan.clone());
                full.prepare_transport(state);
                compare(full, parallel.prepare(state, || false).unwrap(), 128);
            }
            assert_eq!(parallel.preparation_stats().workers, 2);
            assert!(parallel.preparation_stats().parallel_blocks > 0);
            assert!(parallel.memory_usage().parallel_bytes > 0);
        }
    }
}

#[test]
fn cancellation_never_publishes_partial_track_state_and_complete_checkpoints_remain_reusable() {
    let plan = fixture::midi_project(48_000, 4, 1);
    let mut cache = TransportCache::with_mode(
        plan.clone(),
        CacheProfile::Responsive,
        PreparationMode::Parallel,
    );
    let state = TransportState {
        frame: 16000,
        playing: true,
        ..Default::default()
    };
    let mut calls = 0;
    assert!(
        cache
            .prepare(state, || {
                calls += 1;
                calls > 8
            })
            .is_none()
    );
    let resumed = cache.prepare(state, || false).unwrap();
    let mut reference = Renderer::new(plan);
    reference.prepare_transport(state);
    compare(reference, resumed, 1024);
    cache.release_optional_memory();
    assert_eq!(cache.memory_usage().parallel_bytes, 0);
    assert_eq!(cache.preparation_stats().workers, 0);
}

#[test]
fn light_and_small_workloads_fall_back_without_changing_samples_and_profile_switches_release_workers()
 {
    for tracks in [1, 2] {
        let plan = fixture::midi_project(48_000, tracks, 1);
        let mut light = TransportCache::with_profile(plan.clone(), CacheProfile::Light);
        let state = TransportState {
            frame: 4096,
            playing: true,
            ..Default::default()
        };
        let mut full = Renderer::new(plan.clone());
        full.prepare_transport(state);
        compare(full, light.prepare(state, || false).unwrap(), 512);
        assert_eq!(light.preparation_stats().workers, 0);
        let mut cache =
            TransportCache::with_mode(plan, CacheProfile::Balanced, PreparationMode::Auto);
        cache.prepare(state, || false).unwrap();
        if tracks == 1 {
            assert_eq!(cache.preparation_stats().workers, 0);
        }
        cache.set_profile(CacheProfile::Light);
        assert_eq!(cache.preparation_stats().workers, 0);
    }
}

#[test]
fn mixed_audio_and_sampler_tracks_preserve_source_positions_gaps_fades_and_stereo() {
    let mut plan = fixture::midi_project(48_000, 2, 1);
    let asset = fixture::add_audio(&mut plan, 2, 1, false);
    fixture::add_audio(&mut plan, 1, 0, true);
    let settings = SamplerSettings {
        root_note: 60,
        end_frame: 40000,
        trigger: SamplerTrigger::Gate,
        input: Some(SamplerInput {
            asset_id: asset,
            name: "sample".into(),
            source_offset_frame: 0,
            length_frames: 40000,
            repeat: None,
            stretch: None,
        }),
        ..Default::default()
    };
    let clip_asset = plan.project.tracks[1].clips[0].asset_id;
    for note in &mut plan
        .project
        .assets
        .iter_mut()
        .find(|a| a.id == clip_asset)
        .unwrap()
        .midi_pattern
        .as_mut()
        .unwrap()
        .notes
    {
        note.pitch = 60;
    }
    let layer = &mut plan.project.tracks[1].instrument.as_mut().unwrap().layers[0];
    layer.oscillators.clear();
    layer.sampler = Some(settings.clone());
    let prepared = daw_media::PreparedSampler::new_cancellable(
        &plan.audio[&asset],
        &settings,
        40000,
        plan.project.stretch_engine,
        &[60],
        None,
        &|| false,
    )
    .unwrap();
    plan.sampler_audio.insert(settings.id, prepared);
    plan.project.master.effects.push(EffectInstance::limiter());
    let state = TransportState {
        frame: 12345,
        playing: true,
        ..Default::default()
    };
    let mut cache = TransportCache::with_mode(
        plan.clone(),
        CacheProfile::Responsive,
        PreparationMode::Parallel,
    );
    let pooled = cache.prepare(state, || false).unwrap();
    let mut full = Renderer::new(plan);
    full.prepare_transport(state);
    compare(full, pooled, 4096);
    assert!(cache.preparation_stats().parallel_blocks > 0);
}
