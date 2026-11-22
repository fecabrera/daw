use super::*;
use daw_core::{BeatEdit, BeatResolution, BeatSubtrack, Chord, ChordKind, MidiNote, TrackKind};

pub(super) fn run(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    match path {
        "midi.chord.list" => Ok(json!(ChordKind::ALL.into_iter()
            .map(|kind| json!({"key": chord_key(kind), "name": kind.name(), "intervals": kind.intervals()}))
            .collect::<Vec<_>>())),
        "midi.pattern.add" | "beat.pattern.add" => {
            let track = track(&session.project, get(args, "track"))?;
            let at = *get(args, "at-frame");
            let id = if path == "beat.pattern.add" {
                session.project.add_beat_pattern(track, at)
            } else {
                session.project.add_midi_pattern(track, at)
            }.map_err(error)?;
            Ok(json!({"trackId": track, "clip": clip_row(&session.project, id)}))
        }
        "midi.pattern.merge" | "beat.pattern.merge" => {
            let ids = clip_ids(&session.project, args, "clip")?;
            let (project, id) = if path.starts_with("beat.") {
                session.project.merge_beat_clips(&ids)
            } else {
                session.project.merge_midi_clips(&ids)
            }.map_err(error)?;
            session.project = project;
            prepare(session)?;
            Ok(json!({"clip": clip_row(&session.project, id)}))
        }
        "midi.chord.add" => chord(args, session),
        "midi.options.set" => midi_options(args, session),
        path if path.starts_with("midi.pattern.") => midi_pattern(path, args, session),
        path if path.starts_with("beat.pattern.") => beat_pattern(path, args, session),
        _ => beat_row(path, args, session),
    }
}

fn chord_key(kind: ChordKind) -> String {
    kind.name().replace(' ', "-")
}

fn chord(args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let track_id = track(&session.project, get(args, "track"))?;
    let root = *get::<u8>(args, "root");
    let key = raw(args, "quality");
    let kind = ChordKind::ALL
        .into_iter()
        .find(|kind| chord_key(*kind) == key)
        .ok_or_else(|| error("Unknown chord quality. See midi.chord.list"))?;
    if u16::from(root) + u16::from(kind.intervals().iter().copied().max().unwrap_or(0)) > 127 {
        return Err(error("Chord notes exceed the MIDI range"));
    }
    let chord = Chord {
        root: root % 12,
        kind,
    };
    let (_, ids) = session
        .project
        .add_midi_chords(Some(track_id), *get(args, "at-frame"), &[chord])
        .map_err(error)?;
    let mut pattern = chord.pattern().map_err(error)?;
    for (note, interval) in pattern.notes.iter_mut().zip(kind.intervals()) {
        note.pitch = root + interval;
    }
    let clip_id = ids[0];
    prepare_edit(session, Edit::ReplaceMidiPattern { clip_id, pattern })?;
    Ok(json!({"trackId": track_id, "clip": clip_row(&session.project, clip_id)}))
}

fn midi_options(args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let track_id = track(&session.project, get(args, "track"))?;
    let track = track_row(&session.project, track_id);
    if track.kind != TrackKind::Midi {
        return Err(error("MIDI options require a MIDI track"));
    }
    let mut options = track.midi_options;
    let channel = opt::<String>(args, "channel");
    let monitor = opt::<String>(args, "monitor");
    changes(channel.is_some() || monitor.is_some());
    if let Some(value) = channel {
        options.input_channel = if value == "all" {
            None
        } else {
            Some(
                value
                    .parse::<u8>()
                    .ok()
                    .filter(|channel| (1..=16).contains(channel))
                    .ok_or_else(|| error("MIDI channel must be all or 1–16"))?
                    - 1,
            )
        };
    }
    if let Some(value) = monitor {
        options.monitor = match value.as_str() {
            "auto" => daw_core::MidiMonitor::Auto,
            "on" => daw_core::MidiMonitor::On,
            "off" => daw_core::MidiMonitor::Off,
            _ => usage("MIDI monitor must be auto, on or off"),
        };
    }
    edit(session, Edit::SetMidiOptions { track_id, options })?;
    Ok(json!({"trackId": track_id, "options": options}))
}

fn midi_pattern(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let clip = clip(&session.project, get(args, "clip"), None)?;
    let clip_id = clip.id;
    let pattern = session
        .project
        .assets
        .iter()
        .find(|asset| asset.id == clip.asset_id)
        .and_then(|asset| asset.midi_pattern.as_ref())
        .ok_or_else(|| error("Select a MIDI pattern clip"))?;
    let command = match path {
        "midi.pattern.show" => return Ok(json!({"clip": clip, "pattern": pattern})),
        "midi.pattern.note.list" => return Ok(json!(pattern.notes)),
        "midi.pattern.resize" => Edit::ResizeMidiPattern {
            clip_id,
            start_tick: *get(args, "start-tick"),
            end_tick: *get(args, "end-tick"),
        },
        "midi.pattern.note.set" => {
            let id = opt::<Id>(args, "id").copied();
            if id.is_some_and(|id| !pattern.notes.iter().any(|note| note.id == id)) {
                return Err(error("MIDI note ID not found"));
            }
            let note = MidiNote {
                id: id.unwrap_or_else(Id::new_v4),
                start_tick: *get(args, "start-tick"),
                length_ticks: *get(args, "length-ticks"),
                pitch: *get(args, "pitch"),
                velocity: *get(args, "velocity"),
            };
            let pattern = pattern.with_overwritten_note(note).map_err(error)?;
            Edit::ReplaceMidiPattern { clip_id, pattern }
        }
        "midi.pattern.note.remove" => {
            let ids = items::<Id>(args, "id").copied().collect::<Vec<_>>();
            if ids
                .iter()
                .any(|id| !pattern.notes.iter().any(|note| note.id == *id))
            {
                return Err(error("MIDI note ID not found"));
            }
            let mut pattern = pattern.clone();
            pattern.notes.retain(|note| !ids.contains(&note.id));
            Edit::ReplaceMidiPattern { clip_id, pattern }
        }
        _ => unreachable!("MIDI pattern route"),
    };
    prepare_edit(session, command)?;
    let clip = clip_row(&session.project, clip_id);
    let pattern = session
        .project
        .assets
        .iter()
        .find(|asset| asset.id == clip.asset_id)
        .and_then(|asset| asset.midi_pattern.as_ref());
    Ok(json!({"clip": clip, "pattern": pattern}))
}

fn beat_pattern(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let project = &session.project;
    let clip = clip(project, get(args, "clip"), None)?;
    let clip_id = clip.id;
    let source = project
        .assets
        .iter()
        .find(|asset| asset.id == clip.asset_id)
        .and_then(|asset| asset.beat_pattern.as_ref())
        .ok_or_else(|| error("Select a Beat pattern clip"))?;
    let beat = project
        .beat(clip_owner(project, clip_id).id)
        .filter(|beat| beat.id == source.instrument_id)
        .ok_or_else(|| error("Beat instrument not found"))?;
    let pattern = match path {
        "beat.pattern.show" => return Ok(json!({
            "clip": clip, "source": source,
            "subtracks": beat.subtracks.iter().map(|row| json!({"id": row.id, "name": row.name})).collect::<Vec<_>>(),
            "visibleStepIds": source.pattern.visible_steps()
                .filter(|step| beat.subtracks.iter().any(|row| row.id == step.subtrack_id))
                .map(|step| step.id).collect::<Vec<_>>(),
        })),
        "beat.pattern.grid.set" => {
            let resolution = match raw(args, "resolution") {
                "quarter" => BeatResolution::Quarter,
                "eighth" => BeatResolution::Eighth,
                "sixteenth" => BeatResolution::Sixteenth,
                _ => BeatResolution::ThirtySecond,
            };
            source.pattern.with_grid(u8::try_from(*get::<u32>(args, "bars")).map_err(error)?, resolution)
        }
        _ => {
            let id = beat_subtrack(&beat.subtracks, get(args, "subtrack"))?.id;
            source.pattern.with_step(id, *get(args, "at-tick"), opt::<u8>(args, "velocity").copied())
        }
    }.map_err(error)?;
    prepare_edit(
        session,
        Edit::ReplaceBeatPattern {
            clip_id,
            pattern: pattern.clone(),
        },
    )?;
    Ok(json!({"clip": clip_row(&session.project, clip_id), "pattern": pattern}))
}

fn beat_subtrack<'a>(
    rows: &'a [BeatSubtrack],
    selector: &tracks::Selector,
) -> Result<&'a BeatSubtrack> {
    select(rows, selector, |row| (row.id, &row.name))
}

fn beat_row(path: &str, args: &ArgMatches, session: &mut Session) -> Result<Value> {
    let track_id = track(&session.project, get(args, "track"))?;
    let beat = session
        .project
        .beat(track_id)
        .ok_or_else(|| error("Select a Beat track"))?;
    let rows = &beat.subtracks;
    let command = match path {
        "beat.subtrack.list" => return Ok(json!(rows)),
        "beat.subtrack.add" => BeatEdit::Add {
            subtrack: Box::new(BeatSubtrack::new(raw(args, "name").into())),
            index: rows.len(),
        },
        "beat.subtrack.remove" | "beat.subtrack.move" => {
            let ids = resolve_ids(items(args, "subtrack").map(|s| Ok(beat_subtrack(rows, s)?.id)))?;
            if path == "beat.subtrack.move" {
                BeatEdit::Reorder {
                    ids,
                    index: *get(args, "index"),
                }
            } else {
                BeatEdit::Remove { ids }
            }
        }
        "beat.output.mix.set" => {
            let mut output = beat.output.clone();
            changes(
                update(args, "gain-db", &mut output.gain_db) | update(args, "pan", &mut output.pan),
            );
            BeatEdit::SetOutput(Box::new(output))
        }
        _ => {
            let row = beat_subtrack(rows, get(args, "subtrack"))?;
            let id = row.id;
            match path {
                "beat.subtrack.rename" => BeatEdit::Rename {
                    id,
                    name: raw(args, "name").into(),
                },
                "beat.subtrack.pitch.set" => BeatEdit::SetPitch {
                    id,
                    pitch: *get(args, "pitch"),
                },
                "beat.subtrack.stop.set" => {
                    let targets =
                        resolve_ids(items(args, "target").map(|s| Ok(beat_subtrack(rows, s)?.id)))?;
                    if targets.is_empty() != args.get_flag("clear") {
                        usage("Choose either --target or --clear");
                    }
                    BeatEdit::SetStops { id, targets }
                }
                "beat.subtrack.mix.set" => {
                    let mut gain_db = row.instrument.main.gain_db;
                    let mut pan = row.instrument.main.pan;
                    let mut muted = row.muted;
                    let mut soloed = row.soloed;
                    changes(
                        update(args, "gain-db", &mut gain_db)
                            | update(args, "pan", &mut pan)
                            | update(args, "muted", &mut muted)
                            | update(args, "soloed", &mut soloed),
                    );
                    BeatEdit::SetMix {
                        id,
                        gain_db,
                        pan,
                        muted,
                        soloed,
                    }
                }
                _ => unreachable!("Beat subtrack route"),
            }
        }
    };
    let command = Edit::Beat {
        track_id,
        edit: command,
    };
    if matches!(
        path,
        "beat.subtrack.add"
            | "beat.subtrack.remove"
            | "beat.subtrack.pitch.set"
            | "beat.subtrack.stop.set"
    ) {
        prepare_edit(session, command)?;
    } else {
        edit(session, command)?;
    }
    Ok(json!({"trackId": track_id, "instrument": session.project.beat(track_id)}))
}
