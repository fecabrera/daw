#[path = "support/cache_project.rs"]
mod fixture;
use daw_core::*;
use daw_engine::{
    CacheProfile, PreparationMode, RenderPlan, Renderer, TransportCache, TransportState,
};
fn group(plan: &mut RenderPlan, ids: &[Id]) -> Id {
    for a in &mut plan.project.assets {
        if a.source_metadata.sample_format == "float" {
            a.source_metadata.sample_format = "ieee_float".into();
        }
    }
    let id = Id::new_v4();
    plan.project
        .edit(Edit::GroupTracks {
            track_ids: ids.to_vec(),
            group_id: id,
            name: "Group".into(),
        })
        .unwrap();
    id
}
fn render(plan: RenderPlan, start: u64, count: usize) -> Vec<[f32; 2]> {
    let mut renderer = Renderer::new(plan);
    let mut out = vec![[0.0; 2]; count];
    renderer.render_offline_at(start, &mut out);
    out
}
fn close(a: &[[f32; 2]], b: &[[f32; 2]]) {
    assert_eq!(a.len(), b.len());
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        for ch in 0..2 {
            assert!((a[ch] - b[ch]).abs() < 2e-6, "{i}: {a:?} != {b:?}");
        }
    }
}
#[test]
fn unity_and_nested_gain_pan_buses_preserve_audio_and_apply_only_once() {
    let mut plan = fixture::midi_project(48_000, 0, 0);
    fixture::add_audio(&mut plan, 2, 0, false);
    let ids: Vec<_> = plan.project.tracks.iter().map(|t| t.id).collect();
    let reference = render(plan.clone(), 2000, 2048);
    let inner = group(&mut plan, &ids);
    close(&reference, &render(plan.clone(), 2000, 2048));
    let outer = group(&mut plan, &[inner]);
    plan.project.tracks[0].gain_db = -6.0;
    plan.project.tracks[0].pan = 0.5;
    plan.project.tracks[1].gain_db = -3.0;
    let expected: Vec<_> = reference
        .iter()
        .map(|s| [s[0] * linear_gain(-9.0) * 0.5, s[1] * linear_gain(-9.0)])
        .collect();
    close(&expected, &render(plan.clone(), 2000, 2048));
    plan.project
        .edit(Edit::SetGroupCollapsed {
            track_id: outer,
            collapsed: true,
        })
        .unwrap();
    close(&expected, &render(plan.clone(), 2000, 2048));
    plan.project
        .edit(Edit::SetTrackMuted {
            track_id: outer,
            muted: true,
        })
        .unwrap();
    assert!(render(plan, 2000, 2048).iter().all(|s| *s == [0.0; 2]));
}
#[test]
fn bus_effects_process_the_sum_and_align_nested_latency() {
    for rate in [44_100, 48_000, 192_000] {
        let mut one = fixture::midi_project(rate, 0, 0);
        fixture::add_audio(&mut one, 1, 0, false);
        one.project.tracks[0].pan = 0.0;
        one.project.tracks[0].gain_db = 0.0;
        let mut grouped = one.clone();
        let mut child = grouped.project.tracks[0].clone();
        child.id = Id::new_v4();
        child.clips[0].id = Id::new_v4();
        grouped.project.tracks.push(child);
        let ids: Vec<_> = grouped.project.tracks.iter().map(|t| t.id).collect();
        // One doubled source is the reference for two identical post-fader sources.
        one.project.tracks[0].clips[0].gain_db = 20.0 * 2.0_f32.log10();
        one.project.tracks[0].effects = vec![
            EffectInstance::drive(),
            EffectInstance::saturator(),
            EffectInstance::limiter(),
        ];
        let bus = group(&mut grouped, &ids);
        grouped.project.tracks[0].effects = one.project.tracks[0].effects.clone();
        group(&mut grouped, &[bus]);
        assert_eq!(
            Renderer::new(one.clone()).latency_frames(),
            Renderer::new(grouped.clone()).latency_frames()
        );
        close(&render(one, 0, 8192), &render(grouped, 0, 8192));
    }
}
#[test]
fn leaf_and_group_delays_match_a_uniform_effect_path() {
    let mut plan = fixture::midi_project(48_000, 0, 0);
    fixture::add_audio(&mut plan, 2, 0, false);
    // All samples stay below the limiter ceiling; only its fixed delay is present.
    let reference = render(plan.clone(), 0, 8192);
    let ids: Vec<_> = plan.project.tracks.iter().map(|t| t.id).collect();
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::limiter());
    let inner = group(&mut plan, &[ids[0]]);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::limiter());
    group(&mut plan, &[inner, ids[1]]);
    plan.project.tracks[0]
        .effects
        .push(EffectInstance::limiter());
    close(&reference, &render(plan, 0, 8192));
}
#[test]
fn nested_group_cache_workers_and_serial_reconstruction_are_identical() {
    let mut plan = fixture::midi_project(48_000, 3, 1);
    let ids: Vec<_> = plan.project.tracks.iter().map(|t| t.id).collect();
    let inner = group(&mut plan, &ids[..2]);
    plan.project.tracks[0].effects = vec![EffectInstance::auto_filter(), EffectInstance::limiter()];
    let outer = group(&mut plan, &[inner, ids[2]]);
    plan.project.tracks[0].effects =
        vec![EffectInstance::saturator(), EffectInstance::auto_filter()];
    plan.project.master.effects.push(EffectInstance::limiter());
    plan.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 1234,
        end_frame: 7123,
    };
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
            ..Default::default()
        };
        let mut a = serial.prepare(state, || false).unwrap();
        let mut b = parallel.prepare(state, || false).unwrap();
        for _ in 0..512 {
            assert_eq!(a.next_sample(), b.next_sample());
        }
    }
    assert!(parallel.preparation_stats().parallel_blocks > 0);
    let old = Renderer::new(plan.clone());
    let mut collapsed = plan.clone();
    collapsed
        .project
        .edit(Edit::SetGroupCollapsed {
            track_id: outer,
            collapsed: true,
        })
        .unwrap();
    assert!(plan.same_processing_plan(&collapsed));
    assert!(Renderer::new(collapsed).can_inherit_without_preroll(&old));
    plan.project
        .edit(Edit::UngroupTrack { track_id: inner })
        .unwrap();
    assert!(!Renderer::new(plan).can_inherit_without_preroll(&old));
}

#[test]
fn cold_audio_bus_seeks_restore_exact_filter_and_dynamics_history() {
    let mut plan = fixture::midi_project(48_000, 0, 0);
    fixture::add_audio(&mut plan, 2, 0, false);
    let ids: Vec<_> = plan.project.tracks.iter().map(|t| t.id).collect();
    group(&mut plan, &ids);
    plan.project.tracks[0].effects = vec![
        EffectInstance::auto_filter(),
        EffectInstance::compressor(),
        EffectInstance::limiter(),
    ];
    let reference = render(plan.clone(), 11001, 512);
    for mode in [PreparationMode::Serial, PreparationMode::Parallel] {
        let mut cache = TransportCache::with_mode(plan.clone(), CacheProfile::Responsive, mode);
        let mut prepared = cache
            .prepare(
                TransportState {
                    frame: 11001,
                    ..Default::default()
                },
                || false,
            )
            .unwrap();
        let mut output = vec![[0.0; 2]; 512];
        prepared.render_offline(&mut output);
        assert_eq!(output, reference);
    }
}

#[test]
fn group_filter_clock_matches_a_serial_chain_after_child_latency() {
    for rate in [44_100, 48_000, 192_000] {
        let mut plan = fixture::midi_project(rate, 0, 0);
        fixture::add_audio(&mut plan, 1, 0, false);
        plan.project.tracks[0].gain_db = 0.0;
        plan.project.tracks[0].pan = 0.0;
        let child = plan.project.tracks[0].id;
        let limiter = EffectInstance::limiter();
        let mut filter = EffectInstance::auto_filter();
        if let EffectSettings::AutoFilter(s) = &mut filter.unit {
            s.strength_octaves = 4.0;
        }
        plan.project.tracks[0].effects = vec![limiter, filter.clone()];
        let reference = render(plan.clone(), 0, 8192);
        plan.project.tracks[0].effects.pop();
        group(&mut plan, &[child]);
        plan.project.tracks[0].effects = vec![filter];
        close(&reference, &render(plan, 0, 8192));
    }
}
