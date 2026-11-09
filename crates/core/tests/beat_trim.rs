use daw_core::*;

fn fixture(rate: u32, stretch: Option<ClipStretch>) -> (Project, Id, Clip) {
    let mut p = Project {
        sample_rate_hz: rate,
        tempo_bpm: 123.4,
        ..Project::default()
    };
    let track = p.add_beat_track().unwrap();
    let mut rows = Vec::new();
    for name in ["Kick", "Hat"] {
        let row = BeatSubtrack::new(name.into());
        rows.push(row.id);
        p.edit(Edit::Beat {
            track_id: track,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: rows.len() - 1,
            },
        })
        .unwrap();
    }
    let clip = p.add_beat_pattern(track, u64::from(rate) * 2).unwrap();
    let pattern = BeatPattern::default()
        .with_step(rows[0], 0, Some(71))
        .unwrap()
        .with_step(rows[1], 960, Some(91))
        .unwrap()
        .with_step(rows[0], 3360, Some(127))
        .unwrap();
    p.edit(Edit::ReplaceBeatPattern {
        clip_id: clip,
        pattern,
    })
    .unwrap();
    let available = p.midi_frame(3840);
    p.tracks[0].clips[0].stretch = stretch;
    p.tracks[0].clips[0].length_frames = stretch.map_or(available, |s| s.scale(available).unwrap());
    let original = p.tracks[0].clips[0].clone();
    (p, track, original)
}

fn trim(p: &Project, candidate: Clip, edge: ClipEdge) -> Project {
    p.trimmed_clips(&[candidate], edge).unwrap()
}

#[test]
fn beat_hidden_steps_survive_both_trims_reopen_and_expansion_at_all_rates() {
    for rate in SAMPLE_RATES {
        for stretch in [None, ClipStretch::new(2, 3)] {
            let (mut p, track, original) = fixture(rate, stretch);
            let copy = Clip {
                id: Id::new_v4(),
                start_frame: original.end() + u64::from(rate),
                ..original.clone()
            };
            p.edit(Edit::InsertClip {
                track_id: track,
                clip: copy.clone(),
            })
            .unwrap();
            let map = |ticks| {
                stretch.map_or(p.midi_frame(ticks), |s| {
                    s.scale(p.midi_frame(ticks)).unwrap()
                })
            };
            let mut candidate = original.clone();
            candidate.length_frames = map(1920);
            let right = trim(&p, candidate, ClipEdge::Right);
            let mut candidate = right.tracks[0].clips[0].clone();
            candidate.start_frame += map(960);
            candidate.length_frames -= map(960);
            let cropped = trim(&right, candidate, ClipEdge::Left);
            assert_eq!(cropped.assets, p.assets);
            assert_eq!(cropped.tracks[0].clips[0].asset_id, original.asset_id);
            assert_eq!(cropped.tracks[0].clips[1], copy);
            assert_eq!(cropped.beat(track), p.beat(track));
            let reopened: Project =
                serde_json::from_str(&serde_json::to_string(&cropped).unwrap()).unwrap();
            let mut candidate = original.clone();
            candidate.length_frames = reopened.tracks[0].clips[0].end() - original.start_frame;
            let restored_left = trim(&reopened, candidate, ClipEdge::Left);
            let restored = trim(&restored_left, original.clone(), ClipEdge::Right);
            assert_eq!(restored, p);
            let pattern = &restored
                .assets
                .iter()
                .find(|a| a.id == original.asset_id)
                .unwrap()
                .beat_pattern
                .as_ref()
                .unwrap()
                .pattern;
            assert_eq!(
                pattern.steps.iter().map(|s| s.velocity).collect::<Vec<_>>(),
                vec![71, 91, 127]
            );
            restored.validate().unwrap();
        }
    }
}

#[test]
fn beat_trim_preserves_steps_hidden_by_source_length_and_subdivision() {
    let (mut p, _, original) = fixture(48_000, None);
    let mut pattern = p
        .assets
        .iter()
        .find(|a| a.id == original.asset_id)
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap()
        .pattern
        .clone();
    let row = pattern.steps[0].subtrack_id;
    pattern = pattern
        .with_grid(2, BeatResolution::Sixteenth)
        .unwrap()
        .with_step(row, 240, Some(63))
        .unwrap()
        .with_step(row, 4800, Some(110))
        .unwrap();
    let stored = pattern.steps.clone();
    pattern = pattern.with_grid(1, BeatResolution::Eighth).unwrap();
    p.edit(Edit::ReplaceBeatPattern {
        clip_id: original.id,
        pattern,
    })
    .unwrap();
    let original = p.tracks[0].clips[0].clone();
    let mut candidate = original.clone();
    candidate.length_frames = p.midi_frame(1920);
    let cropped = trim(&p, candidate, ClipEdge::Right);
    assert_eq!(cropped.assets, p.assets);
    let reopened: Project =
        serde_json::from_str(&serde_json::to_string(&cropped).unwrap()).unwrap();
    let mut candidate = original.clone();
    candidate.length_frames = p.midi_frame(7680);
    let mut expanded = trim(&reopened, candidate, ClipEdge::Right);
    let source = expanded
        .assets
        .last()
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap();
    assert_eq!(source.pattern.steps, stored);
    assert_eq!(source.pattern.bars, 2);
    assert!(source.pattern.visible_steps().any(|s| s.start_tick == 4800));
    assert!(!source.pattern.visible_steps().any(|s| s.start_tick == 240));
    let finer = source
        .pattern
        .with_grid(2, BeatResolution::Sixteenth)
        .unwrap();
    expanded
        .edit(Edit::ReplaceBeatPattern {
            clip_id: original.id,
            pattern: finer,
        })
        .unwrap();
    let source = expanded
        .assets
        .last()
        .unwrap()
        .beat_pattern
        .as_ref()
        .unwrap();
    assert_eq!(source.pattern.steps, stored);
    assert_eq!(source.pattern.visible_steps().count(), stored.len());
    assert_eq!(&expanded.assets[..p.assets.len()], &p.assets);
    expanded.validate().unwrap();
}
