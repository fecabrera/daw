use daw_core::{Asset, Clip, ClipEdge, Edit, Id, Project, Source, SourceMetadata};
#[test]
fn stretch_transactions_retain_scaled_group_fades_and_trim_neighbors() {
    let mut failures = vec![];
    for edge in [ClipEdge::Left, ClipEdge::Right] {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let default_track = project.add_track().unwrap();
        let asset = Id::new_v4();
        project.assets.push(Asset {
            id: asset,
            name: "Source".into(),
            source: Source {
                kind: "external".into(),
                path: "source.wav".into(),
                path_kind: "relative".into(),
                storage: None,
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: 48000,
                channels: 2,
                sample_format: "pcm_int".into(),
                bits_per_sample: 16,
            },
            decoded_frame_count: 960000,
            midi_pattern: None,
            rendered_audio: None,
            compound_audio: None,
            beat_pattern: None,
        });
        let make = |start, length| Clip {
            id: Id::new_v4(),
            asset_id: asset,
            name: "Clip".into(),
            start_frame: start,
            length_frames: length,
            source_offset_frame: 0,
            repeat: None,
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
            muted: false,
            color: None,
            channels: Default::default(),
            fades: Default::default(),
        };
        let mut first = make(96000, 48000);
        first.fades.fade_in.length_frames = 12000;
        first.fades.fade_out.length_frames = 6000;
        first.fades.fade_out.crossfade = true;
        first.fades.fade_in.midpoint = 0.7;
        first.fades.fade_in.midpoint_position = 0.4;
        let mut second = first.clone();
        second.id = Id::new_v4();
        second.start_frame = 288000;
        let neighbor = match edge {
            ClipEdge::Right => make(144000, 72000),
            ClipEdge::Left => make(192000, 96000),
        };
        let default = make(96000, 48000);
        for c in [first.clone(), second.clone(), neighbor.clone()] {
            project
                .edit(Edit::InsertClip {
                    track_id: track,
                    clip: c,
                })
                .unwrap();
        }
        project
            .edit(Edit::InsertClip {
                track_id: default_track,
                clip: default.clone(),
            })
            .unwrap();
        let candidates: Vec<_> = [first, second, default]
            .iter()
            .map(|clip| {
                clip.stretched_to(
                    match edge {
                        ClipEdge::Right => clip.start_frame,
                        ClipEdge::Left => clip.end() - 96000,
                    },
                    96000,
                )
                .unwrap()
            })
            .collect();
        let preview = project.stretched_clips(&candidates, edge).unwrap();
        project
            .edit(Edit::StretchClips {
                clips: candidates.clone(),
                edge,
            })
            .unwrap();
        assert_eq!(project, preview);
        for candidate in &candidates {
            let result = project
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| c.id == candidate.id)
                .unwrap();
            if result.fades != candidate.fades {
                failures.push(format!(
                    "{edge:?}: expected {:?}, actual {:?}",
                    candidate.fades, result.fades
                ));
            }
            assert_eq!(
                result.fades.fade_in.midpoint,
                candidate.fades.fade_in.midpoint
            );
            assert_eq!(
                result.fades.fade_in.midpoint_position,
                candidate.fades.fade_in.midpoint_position
            );
        }
        let survivor = project.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == neighbor.id)
            .unwrap();
        assert_eq!(
            survivor.length_frames,
            match edge {
                ClipEdge::Right => 24000,
                ClipEdge::Left => 48000,
            }
        );
        assert_eq!(survivor.fades, neighbor.fades);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
