use daw_core::*;
fn fixture() -> (Project, Id, Id) {
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument = Some(OscillatorSettings::default().into());
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
    assert_eq!(migrated.layers[0].oscillators, vec![legacy]);
    assert!(!migrated.layers[0].oscillator_collapsed && !migrated.layers[0].envelope_collapsed);
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
    settings.layers[0].oscillators.push(OscillatorSettings {
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
    settings.layers[0].oscillators[1].transpose = 49;
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
        restored.tracks[0].instrument.as_ref().unwrap().layers[0]
            .oscillators
            .len(),
        2
    );
    let empty = OscillatorInstrumentSettings::default();
    restored
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(empty),
        })
        .unwrap();
    restored.validate().unwrap();
}

#[test]
fn envelope_settings_round_trip_and_missing_envelopes_preserve_legacy_sound() {
    let mut instrument = OscillatorInstrumentSettings::default();
    assert_eq!(instrument.layers[0].envelope, EnvelopeSettings::default());
    for shape in CurveShape::ALL {
        instrument.layers[0].envelope.attack.shape = shape;
        instrument.layers[0].envelope.decay.shape = shape;
        instrument.layers[0].envelope.release.shape = shape;
        instrument.layers[0].envelope.attack.amount = 0.0;
        instrument.layers[0].envelope.decay.amount = 1.0;
        instrument.layers[0].envelope.release.time_ms = MAX_ENVELOPE_TIME_MS;
        let saved = serde_json::to_value(&instrument).unwrap();
        let restored: OscillatorInstrumentSettings = serde_json::from_value(saved).unwrap();
        assert_eq!(restored, instrument);
        assert!(restored.valid());
    }
    let mut saved = serde_json::json!({
        "version": 1,
        "oscillators": [OscillatorSettings::default()]
    });
    let restored: OscillatorInstrumentSettings = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored.layers[0].envelope, EnvelopeSettings::legacy());
    saved["envelope"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<OscillatorInstrumentSettings>(saved).is_err());
    let legacy: InstrumentSettings = serde_json::from_value(serde_json::json!({
        "version": 1,
        "oscillators": [OscillatorSettings::default()],
        "envelope": instrument.layers[0].envelope,
    }))
    .unwrap();
    assert!(legacy.layers[0].has_envelope);
    assert!(legacy.layers[0].oscillator_enabled && legacy.layers[0].envelope_enabled);
    assert_eq!(legacy.layers[0].envelope, instrument.layers[0].envelope);
    let restored: OscillatorInstrumentSettings =
        serde_json::from_value(serde_json::to_value(OscillatorSettings::default()).unwrap())
            .unwrap();
    assert_eq!(restored.layers[0].envelope, EnvelopeSettings::legacy());
}

#[test]
fn new_instruments_start_empty_and_layer_edits_validate_atomically() {
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    let mut instrument = project.tracks[0].instrument.clone().unwrap();
    assert_eq!(instrument.version, INSTRUMENT_STATE_VERSION);
    assert_eq!(instrument.layers.len(), 1);
    assert!(instrument.layers[0].oscillators.is_empty());
    assert!(!instrument.layers[0].has_envelope);
    assert!(instrument.layers[0].oscillator_enabled && instrument.layers[0].envelope_enabled);
    assert!(!instrument.layers[0].oscillator_collapsed && !instrument.layers[0].envelope_collapsed);
    assert!(!instrument.layers[0].muted && !instrument.layers[0].soloed);
    assert_eq!(instrument.layers[0].gain_db, 0.0);
    assert_eq!(instrument.layers[0].pan, 0.0);
    instrument.layers.push(InstrumentLayerSettings::default());
    instrument.layers[1]
        .oscillators
        .push(OscillatorSettings::default());
    instrument.layers[1].has_envelope = true;
    project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument.clone()),
        })
        .unwrap();
    instrument.layers[0].muted = true;
    instrument.layers[1].soloed = true;
    instrument.layers[1].gain_db = -6.0;
    instrument.layers[1].pan = -0.7;
    instrument.layers[0].oscillator_enabled = false;
    instrument.layers[1].envelope_enabled = false;
    instrument.layers[0].oscillator_collapsed = true;
    instrument.layers[1].envelope_collapsed = true;
    project
        .edit(Edit::SetInstrument {
            track_id: track,
            instrument: Some(instrument.clone()),
        })
        .unwrap();
    let before = serde_json::to_value(&project).unwrap();
    let restored: Project = serde_json::from_value(before.clone()).unwrap();
    assert_eq!(restored.tracks[0].instrument.as_ref(), Some(&instrument));
    let mut unnamed = before.clone();
    for layer in unnamed["tracks"][0]["instrument"]["layers"]
        .as_array_mut()
        .unwrap()
    {
        layer.as_object_mut().unwrap().remove("name");
        for field in [
            "muted",
            "soloed",
            "gain_db",
            "pan",
            "oscillator_enabled",
            "envelope_enabled",
            "oscillator_collapsed",
            "envelope_collapsed",
        ] {
            layer.as_object_mut().unwrap().remove(field);
        }
    }
    let older: Project = serde_json::from_value(unnamed).unwrap();
    let older_layers = &older.tracks[0].instrument.as_ref().unwrap().layers;
    assert!(
        older_layers
            .iter()
            .all(|layer| layer.oscillator_enabled && layer.envelope_enabled)
    );
    assert!(
        older_layers
            .iter()
            .all(|layer| !layer.oscillator_collapsed && !layer.envelope_collapsed)
    );
    assert_eq!(older_layers[0].name, "Layer 1");
    assert_eq!(older_layers[1].name, "Layer 2");
    assert_eq!(older_layers[1].id, instrument.layers[1].id);
    assert!(
        older_layers
            .iter()
            .all(|layer| !layer.muted && !layer.soloed && layer.gain_db == 0.0 && layer.pan == 0.0)
    );
    assert_eq!(
        older_layers[1].oscillators,
        instrument.layers[1].oscillators
    );
    for field in ["oscillator_collapsed", "envelope_collapsed"] {
        let mut malformed = before.clone();
        malformed["tracks"][0]["instrument"]["layers"][0][field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<Project>(malformed).is_err());
    }
    let mut malformed_name = before.clone();
    malformed_name["tracks"][0]["instrument"]["layers"][0]["name"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Project>(malformed_name).is_err());
    let mut invalid = vec![];
    let mut value = instrument.clone();
    value.layers[0].name = "   ".into();
    invalid.push(value);
    let mut value = instrument.clone();
    value.layers.clear();
    invalid.push(value);
    let mut value = instrument.clone();
    value.layers[1].id = value.layers[0].id;
    invalid.push(value);
    let mut value = instrument.clone();
    value.layers[0].id = Id::nil();
    invalid.push(value);
    let mut value = instrument.clone();
    value.version = 1;
    invalid.push(value);
    for gain in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1000.0] {
        let mut value = instrument.clone();
        value.layers[0].gain_db = gain;
        invalid.push(value);
    }
    for pan in [f32::NAN, f32::INFINITY, -1.01, 1.01] {
        let mut value = instrument.clone();
        value.layers[0].pan = pan;
        invalid.push(value);
    }
    let mut value = instrument;
    value.layers[1].oscillators[0].transpose = 49;
    invalid.push(value);
    for value in invalid {
        assert!(
            project
                .edit(Edit::SetInstrument {
                    track_id: track,
                    instrument: Some(value),
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), before);
    }
    let mut malformed = before;
    malformed["tracks"][0]["instrument"]["layers"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Project>(malformed).is_err());
}

#[test]
fn invalid_envelope_edits_reject_the_entire_transaction() {
    let (mut project, track, _) = fixture();
    let before = serde_json::to_value(&project).unwrap();
    let mut invalid = Vec::new();
    for value in [-0.1, MAX_ENVELOPE_TIME_MS + 0.1, f32::NAN, f32::INFINITY] {
        for stage in 0..3 {
            let mut envelope = EnvelopeSettings::default();
            match stage {
                0 => envelope.attack.time_ms = value,
                1 => envelope.decay.time_ms = value,
                _ => envelope.release.time_ms = value,
            }
            invalid.push(envelope);
        }
    }
    for value in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
        invalid.push(EnvelopeSettings {
            sustain: value,
            ..Default::default()
        });
        for stage in 0..3 {
            let mut envelope = EnvelopeSettings::default();
            match stage {
                0 => envelope.attack.amount = value,
                1 => envelope.decay.amount = value,
                _ => envelope.release.amount = value,
            }
            invalid.push(envelope);
        }
    }
    invalid.push(EnvelopeSettings {
        version: 2,
        ..Default::default()
    });
    for envelope in invalid {
        let mut instrument = project.tracks[0].instrument.clone().unwrap();
        instrument.layers[0].envelope = envelope;
        assert!(
            project
                .edit(Edit::Batch(vec![
                    Edit::SetInstrument {
                        track_id: track,
                        instrument: Some(instrument)
                    },
                    Edit::SetTrackMuted {
                        track_id: track,
                        muted: true
                    },
                ]))
                .is_err()
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), before);
    }
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

#[test]
fn shared_body_trim_pads_both_edges_preserves_notes_and_isolates_copies() {
    let (mut project, track, _) = fixture();
    let original = project.tracks[0].clips[0].clone();
    let copy = Clip {
        id: Id::new_v4(),
        start_frame: 240_000,
        ..original.clone()
    };
    project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: copy.clone(),
        })
        .unwrap();
    let before = serde_json::to_value(&project).unwrap();
    for edge in [ClipEdge::Left, ClipEdge::Right] {
        let mut candidate = original.clone();
        candidate.length_frames += 24_000;
        if edge == ClipEdge::Left {
            candidate.start_frame -= 24_000;
        }
        let next = project.trimmed_clips(&[candidate], edge).unwrap();
        let edited = &next.tracks[0].clips[0];
        let source = next
            .assets
            .iter()
            .find(|a| a.id == edited.asset_id)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap();
        assert_eq!(source.length_ticks, 4800);
        let source_note = &source.notes[0];
        assert_eq!(
            source_note.id,
            project
                .assets
                .last()
                .unwrap()
                .midi_pattern
                .as_ref()
                .unwrap()
                .notes[0]
                .id
        );
        assert_eq!(
            edited.start_frame + next.midi_frame(source_note.start_tick),
            original.start_frame + project.midi_frame(240)
        );
        assert_ne!(edited.asset_id, original.asset_id);
        assert_eq!(
            serde_json::to_value(&next.tracks[0].clips[1]).unwrap(),
            serde_json::to_value(&copy).unwrap()
        );
        next.validate().unwrap();
    }
    assert_eq!(serde_json::to_value(&project).unwrap(), before);
    let no_op = project
        .trimmed_clips(std::slice::from_ref(&original), ClipEdge::Right)
        .unwrap();
    assert_eq!(serde_json::to_value(no_op).unwrap(), before);
    assert!(
        project
            .trimmed_clips(&[original.clone(), original], ClipEdge::Right)
            .is_err()
    );
}

#[test]
fn shared_body_trim_retains_timing_at_fractional_tempos_rates_and_stretch() {
    for rate in SAMPLE_RATES {
        for bpm in [93.7, 127.0] {
            for stretch in [None, ClipStretch::new(2, 3)] {
                let (mut project, _, _) = fixture();
                project.edit(Edit::SetTempo { bpm }).unwrap();
                project = project.with_audio_settings(rate, 512).unwrap();
                let clip = &mut project.tracks[0].clips[0];
                clip.stretch = stretch;
                clip.start_frame = u64::from(rate) * 2;
                clip.length_frames = clip
                    .source_length(project.assets.last().unwrap().decoded_frame_count)
                    .unwrap();
                let original = project.tracks[0].clips[0].clone();
                for edge in [ClipEdge::Left, ClipEdge::Right] {
                    let mut candidate = original.clone();
                    let extension = stretch.map_or(project.midi_frame(317), |s| {
                        s.scale(project.midi_frame(317)).unwrap()
                    });
                    candidate.length_frames += extension;
                    if edge == ClipEdge::Left {
                        candidate.start_frame -= extension;
                    }
                    let next = project
                        .trimmed_clips(&[candidate], edge)
                        .unwrap_or_else(|e| panic!("{rate}/{bpm}/{stretch:?}/{edge:?}: {e}"));
                    let result = &next.tracks[0].clips[0];
                    let note = &next
                        .assets
                        .last()
                        .unwrap()
                        .midi_pattern
                        .as_ref()
                        .unwrap()
                        .notes[0];
                    let mapped = |p: &Project, tick| {
                        stretch.map_or(p.midi_frame(tick), |s| s.scale(p.midi_frame(tick)).unwrap())
                    };
                    assert!(
                        (result.start_frame + mapped(&next, note.start_tick))
                            .abs_diff(original.start_frame + mapped(&project, 240))
                            <= 2
                    );
                    if edge == ClipEdge::Left {
                        assert!(result.end().abs_diff(original.end()) <= 2);
                    } else {
                        assert_eq!(result.start_frame, original.start_frame);
                    }
                    next.validate().unwrap();
                }
            }
        }
    }
}

#[test]
fn shared_body_trim_resolves_selected_source_neighbors_once_in_direction_order() {
    for edge in [ClipEdge::Left, ClipEdge::Right] {
        let (mut project, track, _) = fixture();
        let original = project.tracks[0].clips[0].clone();
        let second = Clip {
            id: Id::new_v4(),
            start_frame: original.end(),
            ..original.clone()
        };
        project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: second.clone(),
            })
            .unwrap();
        let mut candidates = vec![original.clone(), second.clone()];
        for c in &mut candidates {
            c.length_frames += 24_000;
            if edge == ClipEdge::Left {
                c.start_frame -= 24_000;
            }
        }
        // Payload order must not decide which selected clip owns the overlap.
        candidates.reverse();
        let next = project.trimmed_clips(&candidates, edge).unwrap();
        let clips = &next.tracks[0].clips;
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].end(), clips[1].start_frame);
        if edge == ClipEdge::Right {
            assert_eq!(clips[0].end(), 144_000);
        } else {
            assert_eq!(clips[1].start_frame, 96_000);
        }
        for clip in clips {
            assert_ne!(clip.asset_id, original.asset_id);
            assert_eq!(
                next.assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .unwrap()
                    .midi_pattern
                    .as_ref()
                    .unwrap()
                    .length_ticks,
                4800
            );
        }
        assert_ne!(clips[0].asset_id, clips[1].asset_id);
        next.validate().unwrap();
        assert_eq!(project.tracks[0].clips[0].asset_id, original.asset_id);
    }
}
