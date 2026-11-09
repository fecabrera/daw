use daw_core::*;

fn fixture(rate: u32, tempo: f32, stretch: Option<ClipStretch>) -> (Project, Id, Clip) {
    let mut p = Project {
        sample_rate_hz: rate,
        tempo_bpm: tempo,
        ..Project::default()
    };
    let track = p.add_midi_track().unwrap();
    let clip = p.add_midi_pattern(track, u64::from(rate) * 2).unwrap();
    p.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: MidiPattern {
            length_ticks: 3840,
            notes: [(120, 120, 60), (960, 960, 64), (3000, 600, 67)]
                .into_iter()
                .map(|(start_tick, length_ticks, pitch)| MidiNote {
                    id: Id::new_v4(),
                    start_tick,
                    length_ticks,
                    pitch,
                    velocity: 100,
                })
                .collect(),
        },
    })
    .unwrap();
    let available = p.midi_frame(3840);
    p.tracks[0].clips[0].stretch = stretch;
    p.tracks[0].clips[0].length_frames = stretch.map_or(available, |s| s.scale(available).unwrap());
    let original = p.tracks[0].clips[0].clone();
    (p, track, original)
}

fn trimmed(p: &Project, id: Id, edge: ClipEdge, delta: i128) -> Project {
    let mut candidate = p.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .clone();
    match edge {
        ClipEdge::Left => {
            candidate.start_frame = (i128::from(candidate.start_frame) + delta) as u64;
            candidate.length_frames = (i128::from(candidate.length_frames) - delta) as u64;
        }
        ClipEdge::Right => {
            candidate.length_frames = (i128::from(candidate.length_frames) + delta) as u64
        }
    }
    p.trimmed_clips(&[candidate], edge).unwrap()
}

#[test]
fn hidden_and_crossing_notes_survive_both_trims_reopen_and_expansion() {
    for rate in SAMPLE_RATES {
        for tempo in [120.0, 123.4] {
            for stretch in [None, ClipStretch::new(2, 3)] {
                let (mut p, track, original) = fixture(rate, tempo, stretch);
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
                let frame = |ticks| {
                    stretch.map_or(p.midi_frame(ticks), |s| {
                        s.scale(p.midi_frame(ticks)).unwrap()
                    })
                };
                let left = frame(1200);
                let kept_end = frame(1440);
                let right = i128::from(kept_end) - i128::from(original.length_frames);
                let cropped = trimmed(&p, original.id, ClipEdge::Right, right);
                let cropped = trimmed(&cropped, original.id, ClipEdge::Left, i128::from(left));
                assert_eq!(cropped.assets, p.assets);
                assert_eq!(cropped.tracks[0].clips[0].asset_id, original.asset_id);
                assert_eq!(cropped.tracks[0].clips[1], copy);
                let reopened: Project =
                    serde_json::from_str(&serde_json::to_string(&cropped).unwrap()).unwrap();
                let restored = trimmed(&reopened, original.id, ClipEdge::Left, -i128::from(left));
                let restored = trimmed(&restored, original.id, ClipEdge::Right, -right);
                assert_eq!(restored.assets, p.assets);
                assert_eq!(restored.tracks[0].clips[0], original);
                assert_eq!(restored.tracks[0].clips[1], copy);
                restored.validate().unwrap();
                let (merged, _) = reopened.merge_pattern_clips(&[original.id]).unwrap();
                let notes = &merged
                    .assets
                    .last()
                    .unwrap()
                    .midi_pattern
                    .as_ref()
                    .unwrap()
                    .notes;
                assert_eq!(notes.len(), 1);
                assert_eq!(notes[0].pitch, 64);
                assert_eq!(notes[0].start_tick, 0);
                assert_eq!(
                    notes[0].length_ticks,
                    if stretch.is_some() { 360 } else { 240 }
                );
            }
        }
    }
}

#[test]
fn loop_base_trims_retain_hidden_notes_and_restore_source_without_padding() {
    let (mut p, _, _) = fixture(48_000, 120.0, None);
    p.tracks[0].clips[0].repeat = Some(ClipLoop {
        length_frames: p.midi_frame(1920),
        phase_frame: p.midi_frame(240),
    });
    p.tracks[0].clips[0].length_frames = p.midi_frame(5760);
    let original = p.tracks[0].clips[0].clone();
    let cropped = trimmed(&p, original.id, ClipEdge::Left, 12_000);
    assert_eq!(cropped.assets, p.assets);
    assert_eq!(cropped.tracks[0].clips[0].source_offset_frame, 12_000);
    let restored = trimmed(&cropped, original.id, ClipEdge::Left, -12_000);
    assert_eq!(restored.tracks[0].clips[0], original);
    assert_eq!(restored.assets, p.assets);
    restored.validate().unwrap();
}

#[test]
fn hidden_ranges_can_be_restored_even_at_the_source_tick_limit() {
    let (mut p, _, original) = fixture(48_000, 120.0, None);
    p.edit(Edit::ReplaceMidiPattern {
        clip_id: original.id,
        pattern: MidiPattern {
            length_ticks: u32::MAX,
            notes: vec![MidiNote {
                id: Id::new_v4(),
                start_tick: u32::MAX - 200,
                length_ticks: 100,
                pitch: 60,
                velocity: 100,
            }],
        },
    })
    .unwrap();
    p.tracks[0].clips[0].length_frames = p.midi_frame(u32::MAX);
    let original = p.tracks[0].clips[0].clone();
    for edge in [ClipEdge::Left, ClipEdge::Right] {
        let delta = if edge == ClipEdge::Left {
            24_000
        } else {
            -24_000
        };
        let cropped = trimmed(&p, original.id, edge, delta);
        let restored = trimmed(&cropped, original.id, edge, -delta);
        assert_eq!(restored.tracks[0].clips[0], original);
        assert_eq!(restored.assets, p.assets);
    }
}
