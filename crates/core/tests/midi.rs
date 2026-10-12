use daw_core::*;
fn fixture() -> (Project, Id, Id) {
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    let clip = project.add_midi_pattern(track, 24_000).unwrap();
    let mut pattern = MidiPattern {
        length_ticks: 3840,
        ..Default::default()
    };
    pattern.notes.push(MidiNote {
        id: Id::new_v4(),
        start_tick: 240,
        length_ticks: 480,
        pitch: 60,
        velocity: 100,
    });
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    project.tracks[0].clips[0].length_frames = 96_000;
    (project, track, clip)
}

#[test]
fn oscillator_collections_migrate_legacy_state_and_validate_atomically() {
    let legacy = OscillatorSettings {
        waveform: Waveform::Square,
        transpose: -12,
        gain_db: -6.0,
        ..Default::default()
    };
    let mut saved = serde_json::to_value(&legacy).unwrap();
    saved.as_object_mut().unwrap().remove("muted");
    let migrated: OscillatorInstrumentSettings = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(migrated.oscillators, vec![legacy]);
    let older_collection: OscillatorInstrumentSettings =
        serde_json::from_value(serde_json::json!({
            "version": 1, "oscillators": [saved.clone()]
        }))
        .unwrap();
    assert_eq!(older_collection, migrated);
    let mut malformed = saved;
    malformed["oscillators"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<OscillatorInstrumentSettings>(malformed).is_err());
    let (mut project, track, _) = fixture();
    let mut settings = migrated;
    settings.oscillators.push(OscillatorSettings {
        transpose: 12,
        ..Default::default()
    });
    project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(settings.clone()),
        })
        .unwrap();
    let before = serde_json::to_value(&project).unwrap();
    settings.oscillators[1].transpose = 49;
    assert!(
        project
            .edit(Edit::SetInstrument {
                track_id: track,
                instrument: Some(settings)
            })
            .is_err()
    );
    assert_eq!(serde_json::to_value(&project).unwrap(), before);
    let mut restored: Project = serde_json::from_value(before).unwrap();
    restored.validate().unwrap();
    assert_eq!(
        restored.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .oscillators
            .len(),
        2
    );
    let empty = OscillatorInstrumentSettings {
        oscillators: vec![],
        ..Default::default()
    };
    restored
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(empty),
        })
        .unwrap();
    restored.validate().unwrap();
}
#[test]
fn copies_share_patterns_until_an_edit_and_clip_ranges_leave_content_unchanged() {
    let (mut project, track, clip) = fixture();
    let original = project.tracks[0].clips[0].clone();
    let source = project
        .assets
        .iter()
        .find(|a| a.id == original.asset_id)
        .unwrap()
        .clone();
    let copy = Clip {
        id: Id::new_v4(),
        start_frame: original.end(),
        ..original.clone()
    };
    project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: copy.clone(),
        })
        .unwrap();
    let mut pattern = source.midi_pattern.clone().unwrap();
    pattern.notes[0].pitch = 72;
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern,
        })
        .unwrap();
    assert_ne!(project.tracks[0].clips[0].asset_id, copy.asset_id);
    assert_eq!(
        project
            .assets
            .iter()
            .find(|a| a.id == copy.asset_id)
            .unwrap()
            .midi_pattern,
        source.midi_pattern
    );
    project
        .edit(Edit::Place {
            clip_id: copy.id,
            track_id: track,
            start: copy.start_frame,
            offset: 0,
            length: copy.length_frames * 2,
            repeat: Some(ClipLoop {
                length_frames: copy.length_frames,
                phase_frame: 0,
            }),
        })
        .unwrap();
    assert_eq!(
        project
            .assets
            .iter()
            .find(|a| a.id == copy.asset_id)
            .unwrap()
            .midi_pattern,
        source.midi_pattern
    );
}
#[test]
fn wrong_source_placement_instruments_and_invalid_pattern_edits_roll_back() {
    let (mut project, _, clip) = fixture();
    let audio = project.add_track().unwrap();
    let before = format!("{project:?}");
    for edit in [
        Edit::SetInstrument {
            track_id: audio,
            instrument: Some(Default::default()),
        },
        Edit::OverwriteClips(vec![ClipPlacement {
            track_id: audio,
            clip: project.tracks[0].clips[0].clone(),
        }]),
        Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 0,
                notes: vec![],
            },
        },
    ] {
        assert!(project.edit(edit).is_err());
        assert_eq!(format!("{project:?}"), before);
    }
}
#[test]
fn tempo_and_processing_rate_retime_midi_without_changing_note_ticks() {
    let (mut project, _, _) = fixture();
    let pattern = project.assets.last().unwrap().midi_pattern.clone();
    project.edit(Edit::SetTempo { bpm: 60.0 }).unwrap();
    assert_eq!(project.tracks[0].clips[0].start_frame, 48_000);
    assert_eq!(project.tracks[0].clips[0].length_frames, 192_000);
    assert_eq!(project.assets.last().unwrap().midi_pattern, pattern);
    for rate in SAMPLE_RATES {
        let converted = project.with_audio_settings(rate, 512).unwrap();
        assert_eq!(
            converted.seconds(converted.tracks[0].clips[0].length_frames),
            4.0
        );
        assert_eq!(converted.assets.last().unwrap().midi_pattern, pattern);
    }
}

#[test]
fn repeated_tempo_and_rate_changes_keep_rounded_source_ends_valid() {
    for looping in [false, true] {
        for stretch in [None, ClipStretch::new(2, 3)] {
            let (mut project, _, _) = fixture();
            let clip = &mut project.tracks[0].clips[0];
            clip.stretch = stretch;
            clip.length_frames = clip.source_length(96_000).unwrap();
            if looping {
                clip.repeat = Some(ClipLoop {
                    length_frames: clip.length_frames,
                    phase_frame: 1,
                });
                clip.length_frames *= 2;
            }
            project.validate().unwrap();
            let notes = project.assets.last().unwrap().midi_pattern.clone();
            for bpm in [127.0, 135.0, 93.7, 120.0] {
                project.edit(Edit::SetTempo { bpm }).unwrap();
                for rate in SAMPLE_RATES {
                    project = project.with_audio_settings(rate, 512).unwrap();
                    project.validate().unwrap();
                    assert_eq!(project.assets.last().unwrap().midi_pattern, notes);
                }
            }
        }
    }
}

#[test]
fn source_resize_crops_notes_preserves_project_timing_and_isolates_copies() {
    let (mut project, track, clip) = fixture();
    let source = project.tracks[0].clips[0].clone();
    let copy = Clip {
        id: Id::new_v4(),
        start_frame: source.end(),
        ..source.clone()
    };
    project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: copy.clone(),
        })
        .unwrap();
    let old_notes = project.assets.last().unwrap().midi_pattern.clone();
    let before = format!("{project:?}");
    let preview = project.resized_midi_pattern(clip, 480, 2880).unwrap();
    assert_eq!(format!("{project:?}"), before);
    let next = &preview.tracks[0].clips[0];
    assert_eq!(next.start_frame, source.start_frame + 12_000);
    assert_eq!(next.length_frames, 60_000);
    let pattern = preview
        .assets
        .iter()
        .find(|a| a.id == next.asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap();
    assert_eq!(pattern.length_ticks, 2400);
    assert_eq!(
        (pattern.notes[0].start_tick, pattern.notes[0].length_ticks),
        (0, 240)
    );
    assert_eq!(preview.tracks[0].clips[1].asset_id, copy.asset_id);
    assert_eq!(
        preview
            .assets
            .iter()
            .find(|a| a.id == copy.asset_id)
            .unwrap()
            .midi_pattern,
        old_notes
    );
    project
        .edit(Edit::ResizeMidiPattern {
            clip_id: clip,
            start_tick: 480,
            end_tick: 2880,
        })
        .unwrap();
    project
        .edit(Edit::ResizeMidiPattern {
            clip_id: clip,
            start_tick: -480,
            end_tick: 2400,
        })
        .unwrap();
    let current = &project.tracks[0].clips[0];
    let pattern = project
        .assets
        .iter()
        .find(|a| a.id == current.asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap();
    assert_eq!(pattern.notes[0].start_tick, 480);
    assert_eq!(current.start_frame, source.start_frame);
    let before = format!("{project:?}");
    for (start_tick, end_tick) in [
        (0, 0),
        (0, -1),
        (i64::MIN, i64::MAX),
        (0, i64::from(u32::MAX) + 1),
    ] {
        assert!(
            project
                .edit(Edit::ResizeMidiPattern {
                    clip_id: clip,
                    start_tick,
                    end_tick
                })
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
    }
    project
        .edit(Edit::ResizeMidiPattern {
            clip_id: clip,
            start_tick: 0,
            end_tick: 7200,
        })
        .unwrap();
    assert_eq!(project.tracks[0].clips[1].start_frame, 204_000);
    assert_eq!(project.tracks[0].clips[1].length_frames, 12_000);
    assert_eq!(project.tracks[0].clips[1].asset_id, copy.asset_id);
    assert_eq!(
        project
            .assets
            .iter()
            .find(|a| a.id == copy.asset_id)
            .unwrap()
            .midi_pattern,
        old_notes
    );
    project
        .edit(Edit::ResizeMidiPattern {
            clip_id: clip,
            start_tick: 0,
            end_tick: 8000,
        })
        .unwrap();
    assert_eq!(project.tracks[0].clips.len(), 1);
    project.validate().unwrap();
}

#[test]
fn drawn_notes_replace_same_pitch_ranges_and_keep_other_pitches() {
    let existing = MidiNote {
        id: Id::new_v4(),
        start_tick: 960,
        length_ticks: 1920,
        pitch: 60,
        velocity: 80,
    };
    let chord = MidiNote {
        id: Id::new_v4(),
        pitch: 64,
        ..existing.clone()
    };
    let original = MidiPattern {
        length_ticks: 3840,
        notes: vec![existing.clone(), chord.clone()],
    };
    for (start, length, expected) in [
        (0, 1440, vec![(1440, 1440)]),
        (1440, 960, vec![(960, 480), (2400, 480)]),
        (2400, 960, vec![(960, 1440)]),
        (960, 1920, vec![]),
    ] {
        let note = MidiNote {
            id: Id::new_v4(),
            start_tick: start,
            length_ticks: length,
            pitch: 60,
            velocity: 100,
        };
        let next = original.with_overwritten_note(note.clone()).unwrap();
        assert!(next.notes.contains(&chord));
        let remnants: Vec<_> = next
            .notes
            .iter()
            .filter(|n| n.pitch == 60 && n.id != note.id)
            .collect();
        assert_eq!(
            remnants
                .iter()
                .map(|n| (n.start_tick, n.length_ticks))
                .collect::<Vec<_>>(),
            expected
        );
        if let Some(first) = remnants.first() {
            assert_eq!(first.id, existing.id);
        }
        if remnants.len() == 2 {
            assert_ne!(remnants[1].id, existing.id);
        }
        assert_eq!(original.notes, vec![existing.clone(), chord.clone()]);
        next.validate().unwrap();
    }
    assert!(
        original
            .with_overwritten_note(MidiNote {
                start_tick: 3840,
                ..existing
            })
            .is_err()
    );
}

#[test]
fn left_source_extension_preserves_note_time_trims_neighbors_and_rejects_negative_start() {
    let (mut project, track, clip) = fixture();
    // Start later so the source has room to extend in both directions.
    project.tracks[0].clips[0].start_frame = 48_000;
    let source = project.tracks[0].clips[0].clone();
    let neighbor = Clip {
        id: Id::new_v4(),
        start_frame: 0,
        length_frames: 48_000,
        ..source.clone()
    };
    project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: neighbor.clone(),
        })
        .unwrap();
    let old_note_frame = source.start_frame + project.midi_frame(240);
    let before = format!("{project:?}");
    let preview = project.resized_midi_pattern(clip, -960, 3840).unwrap();
    assert_eq!(format!("{project:?}"), before);
    let extended = preview.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == clip)
        .unwrap();
    assert_eq!(
        (extended.start_frame, extended.length_frames, extended.end()),
        (24_000, 120_000, source.end())
    );
    let pattern = preview
        .assets
        .iter()
        .find(|a| a.id == extended.asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap();
    assert_eq!(pattern.notes[0].start_tick, 1200);
    assert_eq!(
        extended.start_frame + preview.midi_frame(pattern.notes[0].start_tick),
        old_note_frame
    );
    assert_eq!(
        preview.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == neighbor.id)
            .unwrap()
            .length_frames,
        24_000
    );
    project
        .edit(Edit::ResizeMidiPattern {
            clip_id: clip,
            start_tick: -1920,
            end_tick: 3840,
        })
        .unwrap();
    assert_eq!(project.tracks[0].clips.len(), 1);
    assert_eq!(project.tracks[0].clips[0].start_frame, 0);
    let before = format!("{project:?}");
    assert!(
        project
            .edit(Edit::ResizeMidiPattern {
                clip_id: clip,
                start_tick: -1,
                end_tick: 5760
            })
            .is_err()
    );
    assert_eq!(format!("{project:?}"), before);
}

#[test]
fn stretched_trimmed_source_resize_preserves_retained_note_project_frames() {
    let (mut project, _, clip) = fixture();
    project.tracks[0].clips[0].start_frame = 48_000;
    project.tracks[0].clips[0].stretch = ClipStretch::new(2, 3);
    project.tracks[0].clips[0].source_offset_frame = 6_000;
    project.tracks[0].clips[0].length_frames = 138_000;
    project.validate().unwrap();
    let original = project.tracks[0].clips[0].clone();
    let note_frame = original.start_frame - original.source_offset_frame
        + original
            .stretch
            .unwrap()
            .scale(project.midi_frame(240))
            .unwrap();
    let extended = project.resized_midi_pattern(clip, -480, 3840).unwrap();
    let current = &extended.tracks[0].clips[0];
    let pattern = extended
        .assets
        .iter()
        .find(|a| a.id == current.asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap();
    assert_eq!(current.start_frame, 30_000);
    assert_eq!(current.end(), original.end());
    assert_eq!(current.source_offset_frame, original.source_offset_frame);
    assert_eq!(
        current.start_frame - current.source_offset_frame
            + current
                .stretch
                .unwrap()
                .scale(extended.midi_frame(pattern.notes[0].start_tick))
                .unwrap(),
        note_frame
    );
}

#[test]
fn source_edges_resize_trimmed_ranges_and_keep_looped_arrangement_end() {
    let (mut project, _, clip) = fixture();
    project.tracks[0].clips[0].source_offset_frame = 6_000;
    project.tracks[0].clips[0].length_frames = 48_000;
    let extended = project.resized_midi_pattern(clip, -480, 3840).unwrap();
    assert_eq!(
        (
            extended.tracks[0].clips[0].start_frame,
            extended.tracks[0].clips[0].length_frames
        ),
        (12_000, 60_000)
    );
    assert_eq!(
        extended.tracks[0].clips[0].end(),
        project.tracks[0].clips[0].end()
    );
    let extended = project.resized_midi_pattern(clip, 0, 4320).unwrap();
    assert_eq!(extended.tracks[0].clips[0].length_frames, 60_000);
    project.tracks[0].clips[0].repeat = Some(ClipLoop {
        length_frames: 48_000,
        phase_frame: 6_000,
    });
    project.tracks[0].clips[0].length_frames = 120_000;
    project.validate().unwrap();
    let extended = project.resized_midi_pattern(clip, -480, 3840).unwrap();
    let current = &extended.tracks[0].clips[0];
    assert_eq!(
        (current.start_frame, current.length_frames, current.end()),
        (12_000, 132_000, 144_000)
    );
    assert_eq!(current.repeat.unwrap().length_frames, 60_000);
    assert_eq!(current.repeat.unwrap().phase_frame, 6_000);
}
