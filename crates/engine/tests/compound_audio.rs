use daw_core::{
    Asset, Clip, ClipChannels, ClipFade, ClipFades, ClipLoop, Id, Project, Source, SourceMetadata,
};
use daw_engine::RenderPlan;
use daw_media::AudioData;
use std::{collections::HashMap, sync::Arc};

fn fixture(rate: u32, mono: bool) -> (RenderPlan, Vec<Id>) {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Default::default()
    };
    let track = project.add_track().unwrap();
    let asset = Id::new_v4();
    let meta = SourceMetadata {
        sample_rate_hz: rate,
        channels: if mono { 1 } else { 2 },
        sample_format: "ieee_float".into(),
        bits_per_sample: 32,
    };
    project.assets.push(Asset {
        compound_audio: None,
        midi_pattern: None,
        beat_pattern: None,
        id: asset,
        name: "source".into(),
        source: Source {
            storage: None,
            kind: "external".into(),
            path_kind: "relative".into(),
            path: "source.wav".into(),
        },
        source_metadata: meta.clone(),
        decoded_frame_count: 10_000,
    });
    let mut ids = Vec::new();
    for i in 0..4 {
        let id = Id::new_v4();
        ids.push(id);
        project.tracks[0].clips.push(Clip {
            id,
            asset_id: asset,
            name: format!("clip {i}"),
            color: None,
            start_frame: 1000 + i * 1000,
            length_frames: 1000,
            source_offset_frame: 2000,
            gain_db: i as f32 - 5.0,
            transpose: 0,
            muted: false,
            channels: ClipChannels {
                swapped: i % 2 == 1,
                inverted: i == 2,
                ..Default::default()
            },
            repeat: (i == 1).then_some(ClipLoop {
                length_frames: 700,
                phase_frame: 120,
            }),
            stretch: None,
            fades: ClipFades::default(),
        });
    }
    let samples = Arc::new(
        (0..10_000)
            .map(|i| {
                [
                    (i as f32 * 0.017).sin() * 0.4,
                    (i as f32 * 0.021).cos() * 0.3,
                ]
            })
            .collect(),
    );
    let audio = AudioData {
        samples,
        peaks: Arc::new(vec![]),
        disk: None,
        metadata: meta,
        sample_rate_hz: rate,
    };
    let audio = HashMap::from([(asset, audio)]);
    project
        .tracks
        .iter_mut()
        .find(|t| t.id == track)
        .unwrap()
        .pan = 0.3;
    project.master.gain_db = -2.0;
    project.validate().unwrap();
    (
        RenderPlan {
            project,
            audio,
            stretched_audio: Default::default(),
            sampler_audio: Default::default(),
            beat_tracks: Default::default(),
        },
        ids,
    )
}

#[test]
fn merge_source_matches_original_internal_and_external_fades_and_live_effects() {
    for rate in daw_core::SAMPLE_RATES {
        for mono in [false, true] {
            for exterior in [false, true] {
                for missing in [false, true] {
                    let (mut before, ids) = fixture(rate, mono);
                    if exterior {
                        before.project.tracks[0].clips[1].fades.fade_in = ClipFade {
                            length_frames: 200,
                            crossfade: true,
                            ..Default::default()
                        };
                        before.project.tracks[0].clips[2].fades.fade_out = ClipFade {
                            length_frames: 200,
                            crossfade: true,
                            ..Default::default()
                        };
                    } else {
                        before.project.tracks[0].clips[1].fades.fade_out = ClipFade {
                            length_frames: 200,
                            crossfade: true,
                            ..Default::default()
                        };
                    }
                    if missing {
                        before.project.tracks[0].clips[0].muted = true;
                    }
                    before.project.validate().unwrap();
                    let mut after = before.clone();
                    (after.project, _) = before.project.merge_audio_clips(&ids[1..3]).unwrap();
                    let mut expected = vec![[0.0; 2]; 6500];
                    let mut actual = expected.clone();
                    before.render(0, &mut expected);
                    after.render(0, &mut actual);
                    assert!(
                        actual.iter().zip(&expected).all(|(a, b)| a == b),
                        "rate={rate}, mono={mono}, exterior={exterior}, missing={missing}"
                    );
                    // Track processing remains a live control after Merge.
                    after.project.tracks[0].gain_db = -12.0;
                    let mut changed = vec![[0.0; 2]; 6500];
                    after.render(0, &mut changed);
                    assert!(changed.iter().zip(&actual).any(|(a, b)| a != b));
                }
            }
        }
    }
}

#[test]
fn sparse_gaps_and_nested_merge_keep_samples_and_do_not_allocate_gap_pcm() {
    let (mut before, ids) = fixture(48_000, false);
    before.project.tracks[0].clips[2].start_frame = 10_000;
    before.project.tracks[0].clips[3].start_frame = 12_000;
    let mut after = before.clone();
    (after.project, _) = before.project.merge_audio_clips(&ids[..3]).unwrap();
    let mut expected = vec![[0.0; 2]; 14_000];
    let mut actual = expected.clone();
    daw_engine::Renderer::new(before.clone()).render_offline(&mut expected);
    daw_engine::Renderer::new(after.clone()).render_offline(&mut actual);
    assert!(actual.iter().zip(&expected).all(|(a, b)| a == b));
    let all = after.project.tracks[0]
        .clips
        .iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    (after.project, _) = after.project.merge_audio_clips(&all).unwrap();
    daw_engine::Renderer::new(after.clone()).render_offline(&mut actual);
    assert!(actual.iter().zip(&expected).all(|(a, b)| a == b));
    assert_eq!(after.audio.len(), 1);
}

#[test]
fn model_identity_schema_cycles_and_rate_changes() {
    let (plan, _) = fixture(48_000, false);
    let original = plan.project.clone();
    let ids: Vec<_> = original.tracks[0]
        .clips
        .iter()
        .take(2)
        .map(|c| c.id)
        .collect();
    let (merged, id) = original.merge_audio_clips(&ids).unwrap();
    assert_eq!(id, ids[0]);
    assert_eq!(&merged.assets[..original.assets.len()], &original.assets);
    let wrapper = merged.tracks[0].clips.iter().find(|c| c.id == id).unwrap();
    assert_eq!(wrapper.gain_db, 0.0);
    assert!(wrapper.fades.fade_in.inherit_source && wrapper.fades.fade_out.inherit_source);
    assert_eq!(merged.merge_audio_clips(&[id]).unwrap().0, merged);
    let encoded = serde_json::to_string(&merged).unwrap();
    let decoded: Project = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, merged);
    decoded.validate().unwrap();
    for rate in daw_core::SAMPLE_RATES {
        let retimed = merged.with_audio_settings(rate, 128).unwrap();
        assert_eq!(
            retimed.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .id,
            id
        );
        retimed.validate().unwrap();
    }
    let mut bad = merged.clone();
    let asset = bad.assets.last_mut().unwrap();
    asset.compound_audio.as_mut().unwrap().version += 1;
    assert!(bad.validate().is_err());
    let mut bad = merged.clone();
    let asset = bad.assets.last_mut().unwrap();
    let own = asset.id;
    asset.compound_audio.as_mut().unwrap().members[0].asset_id = own;
    assert!(bad.validate().is_err());
    assert!(original.merge_audio_clips(&[]).is_err());
    assert!(
        original
            .merge_audio_clips(&[ids[0], daw_core::Id::new_v4()])
            .is_err()
    );
    let all: Vec<_> = original.tracks[0].clips.iter().map(|c| c.id).collect();
    assert!(original.merge_audio_clips(&[all[0], all[2]]).is_err());
}

#[test]
fn compound_loop_is_sparse_and_custom_outer_fades_cover_multiple_members() {
    let (mut plan, _) = fixture(48_000, false);
    let ids: Vec<_> = plan.project.tracks[0].clips.iter().map(|c| c.id).collect();
    let (project, id) = plan.project.merge_audio_clips(&ids).unwrap();
    plan.project = project;
    let mut dry_source =
        daw_engine::CompoundAudioSource::new(&plan, plan.project.tracks[0].clips[0].asset_id)
            .unwrap();
    let canonical: Vec<_> = (0..4000).map(|f| dry_source.sample(f).unwrap().0).collect();
    let clip = &mut plan.project.tracks[0].clips[0];
    clip.repeat = Some(daw_core::ClipLoop {
        length_frames: 4000,
        phase_frame: 700,
    });
    clip.length_frames = 12_000;
    plan.project.validate().unwrap();
    let mut source = daw_engine::CompoundAudioSource::new(&plan, clip_asset(&plan, id)).unwrap();
    assert_eq!(source.sample(1733).unwrap().0, canonical[1733]);
    let mut output = vec![[0.; 2]; 12000];
    daw_engine::Renderer::new(plan.clone()).render_offline(&mut output);
    assert!(output.iter().flatten().any(|v| v.abs() > 0.001));
    assert_eq!(plan.audio.len(), 1);
    // A custom fade can extend over more than the first member, without restarting at each member.
    let clip = &mut plan.project.tracks[0].clips[0];
    clip.repeat = None;
    clip.length_frames = 4000;
    clip.fades.fade_in = daw_core::ClipFade {
        length_frames: 3000,
        ..Default::default()
    };
    let track = plan
        .project
        .compile_audio_track_view(&plan.project.tracks[0], |_| false)
        .unwrap();
    assert!(
        track.envelopes[1]
            .iter()
            .any(|e| e.length_frames == 4000 && e.offset == 1000)
    );
}
fn clip_asset(plan: &RenderPlan, id: daw_core::Id) -> daw_core::Id {
    plan.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .asset_id
}

#[test]
fn recipe_depth_and_new_cut_envelopes_are_explicit() {
    let (plan, ids) = fixture(48000, false);
    let (mut project, id) = plan.project.merge_audio_clips(&ids).unwrap();
    let (split, created) = project
        .split_clips(&[id], 2500, daw_core::MidiNoteSplit::Split)
        .unwrap();
    let left = split.tracks[0].clips.iter().find(|c| c.id == id).unwrap();
    let right = split.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == created[0])
        .unwrap();
    assert!(left.fades.fade_in.inherit_source && !left.fades.fade_out.inherit_source);
    assert!(!right.fades.fade_in.inherit_source && right.fades.fade_out.inherit_source);
    for _ in 1..64 {
        project.tracks[0].clips[0].gain_db = 1.0;
        project = project.merge_audio_clips(&[id]).unwrap().0;
    }
    project.tracks[0].clips[0].gain_db = 1.0;
    let before = project.clone();
    assert!(
        project
            .merge_audio_clips(&[id])
            .unwrap_err()
            .to_string()
            .contains("64 levels")
    );
    assert_eq!(project, before);
}

#[test]
fn mixed_mono_stereo_members_keep_channel_tags_through_live_track_processing() {
    let (mut before, ids) = fixture(48000, false);
    let old = before.project.assets[0].id;
    let mono = Id::new_v4();
    let mut asset = before.project.assets[0].clone();
    asset.id = mono;
    asset.source_metadata.channels = 1;
    before.project.assets.push(asset);
    let mut audio = before.audio[&old].clone();
    audio.metadata.channels = 1;
    audio.samples = Arc::new(audio.samples.iter().map(|s| [s[0]; 2]).collect());
    before.audio.insert(mono, audio);
    before.project.tracks[0].clips[1].asset_id = mono;
    before.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::compressor());
    let mut after = before.clone();
    after.project = before.project.merge_audio_clips(&ids).unwrap().0;
    let mut expected = vec![[0.; 2]; 8000];
    let mut actual = expected.clone();
    daw_engine::Renderer::new(before).render_offline(&mut expected);
    daw_engine::Renderer::new(after).render_offline(&mut actual);
    assert!(expected.iter().zip(actual).all(|(a, b)| *a == b));
}

#[test]
fn shared_recipe_expansion_hits_metadata_admission_before_dense_allocation() {
    let (plan, ids) = fixture(48000, false);
    let (mut project, id) = plan.project.merge_audio_clips(&ids).unwrap();
    for _ in 0..20 {
        let mut copy = project.tracks[0].clips[0].clone();
        copy.id = Id::new_v4();
        copy.start_frame = copy.end();
        let copied = copy.id;
        project.tracks[0].clips.push(copy);
        project = project.merge_audio_clips(&[id, copied]).unwrap().0;
    }
    project.validate().unwrap();
    let original = project.clone();
    let error = project
        .compile_audio_track(&project.tracks[0], |_| false)
        .unwrap_err();
    assert!(error.to_string().contains("metadata admission"), "{error}");
    assert_eq!(project, original);
}
