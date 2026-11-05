#[path = "support/cache_project.rs"]
mod fixture;
use daw_core::*;
use daw_engine::{
    CacheProfile, PreparationMode, RenderPlan, Renderer, TransportCache, TransportState,
};
fn audio(rate: u32, n: usize) -> RenderPlan {
    let mut p = fixture::midi_project(rate, 0, 0);
    fixture::add_audio(&mut p, n, 0, false);
    for a in &mut p.project.assets {
        if a.source_metadata.sample_format == "float" {
            a.source_metadata.sample_format = "ieee_float".into();
        }
    }
    p
}
fn connect(p: &mut RenderPlan, i: usize, r: Id, tap: SendTap) {
    let mut s = TrackSend::new(r);
    s.gain_db = 0.0;
    s.tap = tap;
    p.project.tracks[i].sends.push(s);
}
fn render(p: RenderPlan) -> Vec<[f32; 2]> {
    let mut r = Renderer::new(p);
    let mut out = vec![[0.0; 2]; 8192];
    r.render_offline_at(0, &mut out);
    out
}
fn close(a: &[[f32; 2]], b: &[[f32; 2]]) {
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        for ch in 0..2 {
            assert!((a[ch] - b[ch]).abs() < 3e-6, "{i}: {a:?} != {b:?}");
        }
    }
}
#[test]
fn pre_post_and_mute_pan_are_consistent_at_every_rate() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut base = audio(rate, 1);
        base.project.tracks[0].pan = 0.4;
        base.project.tracks[0].gain_db = -6.0;
        let reference = render(base.clone());
        let mut p = base.clone();
        let r = p.project.add_return_track().unwrap();
        connect(&mut p, 0, r, SendTap::PostFader);
        p.project.tracks[0].muted = true;
        close(&reference, &render(p.clone()));
        p.project.tracks[0].sends[0].tap = SendTap::PreFader;
        p.project.tracks[0].gain_db = SILENT_GAIN_DB;
        base.project.tracks[0].gain_db = 0.0;
        close(&render(base), &render(p.clone()));
        p.project.tracks[0].sends[0].enabled = false;
        assert!(render(p).iter().all(|s| *s == [0.0; 2]));
    }
}
#[test]
fn return_only_solo_retains_muted_contributors_and_source_solo_suppresses_other_feeds() {
    let base = audio(48000, 2);
    let mut p = base.clone();
    let r = p.project.add_return_track().unwrap();
    connect(&mut p, 0, r, SendTap::PostFader);
    connect(&mut p, 1, r, SendTap::PostFader);
    p.project.tracks[2].soloed = true;
    close(&render(base.clone()), &render(p.clone()));
    p.project.tracks[0].muted = true;
    close(&render(base.clone()), &render(p.clone()));
    p.project.tracks[0].soloed = true;
    let mut one = base.clone();
    one.project.tracks[1].muted = true;
    let expected = render(one)
        .into_iter()
        .map(|s| s.map(|v| v * 2.0))
        .collect::<Vec<_>>();
    close(&expected, &render(p));
}
#[test]
fn independent_send_edges_align_with_dry_and_return_effect_latency() {
    for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
        let mut base = audio(rate, 1);
        base.project.tracks[0].effects =
            vec![EffectInstance::saturator(), EffectInstance::limiter()];
        let reference = render(base.clone());
        let mut p = base.clone();
        let r = p.project.add_return_track().unwrap();
        connect(&mut p, 0, r, SendTap::PostFader);
        p.project.tracks[0].muted = true;
        p.project.tracks[0].effects.clear();
        p.project.tracks[1].effects = base.project.tracks[0].effects.clone();
        close(&reference, &render(p));
        let mut p = base.clone();
        let mut second = p.project.tracks[0].clone();
        second.id = Id::new_v4();
        second.clips[0].id = Id::new_v4();
        second.effects.clear();
        second.muted = true;
        p.project.tracks.push(second);
        let r = p.project.add_return_track().unwrap();
        connect(&mut p, 0, r, SendTap::PostFader);
        connect(&mut p, 1, r, SendTap::PostFader);
        // The first source and its duplicate are phase aligned at Return ingress;
        // bypassed latency effects preserve the same latency and an exact reference.
        for e in &mut p.project.tracks[0].effects {
            match &mut e.unit {
                EffectSettings::Saturator(s) => s.enabled = false,
                EffectSettings::Limiter(s) => s.enabled = false,
                _ => unreachable!(),
            }
        }
        let mut reference = base;
        for e in &mut reference.project.tracks[0].effects {
            match &mut e.unit {
                EffectSettings::Saturator(s) => s.enabled = false,
                EffectSettings::Limiter(s) => s.enabled = false,
                _ => unreachable!(),
            }
        }
        let expected = render(reference)
            .into_iter()
            .map(|s| s.map(|v| v * 3.0))
            .collect::<Vec<_>>();
        close(&expected, &render(p));
    }
}
#[test]
fn send_graph_serial_parallel_loop_and_seek_reconstruction_match() {
    let mut p = fixture::midi_project(48000, 3, 1);
    let r = p.project.add_return_track().unwrap();
    for i in 0..3 {
        connect(&mut p, i, r, SendTap::PreFader);
    }
    p.project.tracks.last_mut().unwrap().effects = vec![
        EffectInstance::saturator(),
        EffectInstance::auto_filter(),
        EffectInstance::limiter(),
    ];
    p.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 1234,
        end_frame: 7123,
    };
    let mut serial =
        TransportCache::with_mode(p.clone(), CacheProfile::Responsive, PreparationMode::Serial);
    let mut parallel =
        TransportCache::with_mode(p, CacheProfile::Responsive, PreparationMode::Parallel);
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
}

#[test]
fn group_sends_ignore_group_mute_but_reflect_child_mute_and_bypass_ancestor_faders() {
    let mut p = audio(48000, 1);
    let source = p.project.tracks[0].id;
    let reference = render(p.clone());
    let group = Id::new_v4();
    p.project
        .edit(Edit::GroupTracks {
            track_ids: vec![source],
            group_id: group,
            name: "Group".into(),
        })
        .unwrap();
    let ret = p.project.add_return_track().unwrap();
    connect(&mut p, 0, ret, SendTap::PostFader);
    connect(&mut p, 1, ret, SendTap::PostFader);
    p.project.tracks[0].muted = true;
    let doubled = reference
        .iter()
        .map(|s| s.map(|v| v * 2.0))
        .collect::<Vec<_>>();
    close(&doubled, &render(p.clone()));
    p.project.tracks[1].muted = true;
    close(&reference, &render(p.clone()));
    p.project.tracks[0].gain_db = SILENT_GAIN_DB;
    close(&reference, &render(p));
}

#[test]
fn send_free_and_empty_return_projects_keep_exact_samples_and_live_offline_parity() {
    for mono in [false, true] {
        let mut p = fixture::midi_project(48000, 0, 0);
        fixture::add_audio(&mut p, 1, 0, mono);
        for a in &mut p.project.assets {
            a.source_metadata.sample_format = "ieee_float".into();
        }
        let expected = (0..8192).map(|i| p.sample_at(i)).collect::<Vec<_>>();
        close(&expected, &render(p.clone()));
        p.project.add_return_track().unwrap();
        close(&expected, &render(p.clone()));
        let r = p.project.tracks[1].id;
        connect(&mut p, 0, r, SendTap::PostFader);
        p.project.tracks[1].soloed = true;
        let offline = render(p.clone());
        let mut live = Renderer::new(p);
        live.play();
        let realtime = (0..8192).map(|_| live.next_sample()).collect::<Vec<_>>();
        // Playback's existing output-start ramp completes before this comparison.
        close(&offline[480..], &realtime[480..]);
    }
}

#[test]
fn impulses_align_across_dry_two_returns_and_nested_group_paths_at_all_rates() {
    for rate in SAMPLE_RATES {
        let mut p = audio(rate, 1);
        p.project.tracks[0].gain_db = 0.0;
        let audio = p.audio.values_mut().next().unwrap();
        let samples = std::sync::Arc::make_mut(&mut audio.samples);
        samples.fill([0.0; 2]);
        samples[1000] = [0.2, 0.1];
        let reference = render(p.clone());
        let source = p.project.tracks[0].id;
        let g = Id::new_v4();
        p.project
            .edit(Edit::GroupTracks {
                track_ids: vec![source],
                group_id: g,
                name: "Group".into(),
            })
            .unwrap();
        let a = p.project.add_return_track().unwrap();
        let b = p.project.add_return_track().unwrap();
        connect(&mut p, 1, a, SendTap::PreFader);
        connect(&mut p, 0, b, SendTap::PostFader);
        p.project.tracks[0].effects.push(EffectInstance::limiter());
        p.project.tracks[2]
            .effects
            .push(EffectInstance::saturator());
        p.project.tracks[3]
            .effects
            .extend([EffectInstance::saturator(), EffectInstance::limiter()]);
        for t in &mut p.project.tracks {
            for e in &mut t.effects {
                match &mut e.unit {
                    EffectSettings::Limiter(s) => s.enabled = false,
                    EffectSettings::Saturator(s) => s.enabled = false,
                    _ => unreachable!(),
                }
            }
        }
        let expected = reference
            .iter()
            .map(|s| s.map(|v| v * 3.0))
            .collect::<Vec<_>>();
        close(&expected, &render(p));
    }
}

#[test]
fn disabling_a_feed_preserves_the_returns_existing_tail() {
    let mut p = audio(48000, 1);
    let r = p.project.add_return_track().unwrap();
    connect(&mut p, 0, r, SendTap::PostFader);
    p.project.tracks[0].muted = true;
    p.project.tracks[1]
        .effects
        .push(EffectInstance::ambient_reverb());
    let mut current = Renderer::new(p.clone());
    current.play();
    for _ in 0..24000 {
        current.next_sample();
    }
    p.project.tracks[0].sends[0].enabled = false;
    let mut next = Renderer::new(p);
    assert!(next.can_inherit_without_preroll(&current));
    next.inherit(&mut current);
    let mut energy = 0.0;
    for i in 0..10000 {
        let s = next.next_sample();
        if i > 1000 {
            energy += s[0] * s[0] + s[1] * s[1];
        }
    }
    assert!(
        energy > 1e-6,
        "existing Return history must remain audible after the feed ramp ends"
    );
}
