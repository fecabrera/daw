//! Track presentation over validated project metadata. No media or device access.
use daw_core::{Project, Track, TrackKind};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(super) enum AddType {
    Audio,
    Midi,
    Beat,
}

pub(super) fn parse_name(value: &str) -> Result<String, String> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        Err("Track name must contain text and no control characters".into())
    } else {
        Ok(value.to_owned())
    }
}

pub(super) fn add(
    folder: &std::path::Path,
    name: &str,
    track_type: AddType,
) -> daw_project::Result<()> {
    let mut session = daw_project::Session::open_document(folder)?.into_editor_session();
    let id = match track_type {
        AddType::Audio => session.project.add_track(),
        AddType::Midi => session.project.add_midi_track(),
        AddType::Beat => session.project.add_beat_track(),
    }
    .map_err(|error| daw_project::Error(error.to_string()))?;
    session
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == id)
        .unwrap()
        .name = name.to_owned();
    session
        .project
        .validate()
        .map_err(|error| daw_project::Error(error.to_string()))?;
    session.save(folder)?;
    for warning in &session.warnings {
        eprintln!("Warning: {warning}");
    }
    // The save has committed. A closed output pipe must not turn that into a
    // reported write failure or invite a duplicate retry.
    use std::io::Write as _;
    if let Err(error) = writeln!(std::io::stdout().lock(), "Added track: {name:?}\nID: {id}")
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        eprintln!("Warning: Track saved, but cannot write confirmation: {error}");
    }
    Ok(())
}

fn kind(track: &Track) -> &'static str {
    match track.kind {
        TrackKind::Audio => "Audio",
        TrackKind::Midi => "MIDI",
        TrackKind::Beat => "Beat",
        TrackKind::Group => "Group",
        TrackKind::Return => "Return",
    }
}

pub(super) fn describe(project: &Project, name: Option<&str>) -> daw_project::Result<String> {
    let mut text = String::new();
    match name {
        None => {
            writeln!(text, "Project: {:?}", project.name).unwrap();
            writeln!(text, "Tracks: {}", project.tracks.len()).unwrap();
            if project.tracks.is_empty() {
                writeln!(text, "No tracks.").unwrap();
                return Ok(text);
            }
            writeln!(
                text,
                "#\tID\tType\tName\tParent\tClips\tEffects\tMuted\tSolo"
            )
            .unwrap();
            for (index, track) in project.tracks.iter().enumerate() {
                let parent = track
                    .group_id
                    .map_or_else(|| "Master".into(), |id| id.to_string());
                // Debug-quoted names escape tabs, newlines and terminal control codes.
                writeln!(
                    text,
                    "{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}",
                    index + 1,
                    track.id,
                    kind(track),
                    track.name,
                    parent,
                    track.clips.len(),
                    track.effects.len(),
                    track.muted,
                    track.soloed
                )
                .unwrap();
            }
        }
        Some(name) => {
            let matches: Vec<_> = project
                .tracks
                .iter()
                .filter(|track| track.name == name)
                .collect();
            if matches.is_empty() {
                return Err(daw_project::Error(format!(
                    "Track not found: {name:?}. Run tracks without a name to list tracks."
                )));
            }
            for (index, track) in matches.into_iter().enumerate() {
                if index > 0 {
                    text.push('\n');
                }
                detail(&mut text, project, track);
            }
        }
    }
    Ok(text)
}

fn detail(text: &mut String, project: &Project, track: &Track) {
    writeln!(
        text,
        "Track: {:?}\nID: {}\nType: {}",
        track.name,
        track.id,
        kind(track)
    )
    .unwrap();
    if let Some(parent) = track
        .group_id
        .and_then(|id| project.tracks.iter().find(|t| t.id == id))
    {
        writeln!(text, "Parent: {:?} ({})", parent.name, parent.id).unwrap();
    } else {
        writeln!(text, "Parent: Master").unwrap();
    }
    writeln!(
        text,
        "Gain: {:.2} dB\nPan: {:.3} (-1 left, 0 center, 1 right)\nMuted: {}\nSolo: {}",
        track.gain_db, track.pan, track.muted, track.soloed
    )
    .unwrap();
    writeln!(
        text,
        "Track effects: {}\nClips: {}\nProject rate: {} Hz",
        track.effects.len(),
        track.clips.len(),
        project.sample_rate_hz
    )
    .unwrap();
    if let Some(beat) = &track.beat {
        writeln!(text, "Beat subtracks: {}", beat.subtracks.len()).unwrap();
        for row in &beat.subtracks {
            writeln!(
                text,
                "  {:?} ({}) · {} layers · pitch {} · {:.2} dB · pan {:.3}",
                row.name,
                row.id,
                row.instrument.layers.len(),
                row.trigger_pitch,
                row.instrument.main.gain_db,
                row.instrument.main.pan
            )
            .unwrap();
        }
    }
    if let Some(instrument) = &track.instrument {
        writeln!(text, "Instrument layers: {}", instrument.layers.len()).unwrap();
    }
    if track.kind == TrackKind::Group {
        let children: Vec<_> = project
            .tracks
            .iter()
            .filter(|t| t.group_id == Some(track.id))
            .collect();
        writeln!(text, "Direct children: {}", children.len()).unwrap();
        for child in children {
            writeln!(text, "  {:?} ({})", child.name, child.id).unwrap();
        }
    }
    for clip in &track.clips {
        writeln!(text, "  Clip: {:?}\n    ID: {}\n    Asset: {}\n    Start: {} frames ({:.6} s)\n    End (exclusive): {} frames ({:.6} s)\n    Length: {} frames ({:.6} s)\n    Muted: {}",
            clip.name, clip.id, clip.asset_id, clip.start_frame, project.seconds(clip.start_frame),
            clip.end(), project.seconds(clip.end()), clip.length_frames,
            project.seconds(clip.length_frames), clip.muted).unwrap();
        writeln!(text, "    Gain: {:.2} dB", clip.gain_db).unwrap();
        if project
            .assets
            .iter()
            .any(|asset| asset.id == clip.asset_id && asset.is_audio())
        {
            writeln!(
                text,
                "    Channels: L={} R={} Swap L/R={}\n    Polarity: {}",
                clip.channels.left,
                clip.channels.right,
                clip.channels.swapped,
                if clip.channels.inverted {
                    "Inverted"
                } else {
                    "Normal"
                }
            )
            .unwrap();
            fade_detail(text, project, clip, true);
            fade_detail(text, project, clip, false);
        }
    }
}

fn fade_detail(text: &mut String, project: &Project, clip: &daw_core::Clip, left: bool) {
    let fade = if left {
        clip.fades.fade_in
    } else {
        clip.fades.fade_out
    };
    let label = if left { "Fade in" } else { "Fade out" };
    let limit = project.clip_fade_limit(clip.id, left, fade.crossfade);
    let mode = if fade.length_frames == 0 {
        "Automatic"
    } else if fade.crossfade {
        "Crossfade"
    } else {
        "Fade"
    };
    let effective = if fade.length_frames == 0 || (fade.crossfade && limit == 0) {
        (u64::from(project.sample_rate_hz) / 200)
            .max(1)
            .min(clip.length_frames / 2)
    } else {
        fade.length_frames.min(limit)
    };
    let position = if fade.crossfade {
        1.0 - fade.midpoint_position
    } else {
        fade.midpoint_position
    };
    writeln!(text, "    {label}: {mode}{}\n      Saved duration: {} frames ({:.3} ms)\n      Effective duration: {} frames ({:.3} ms)\n      Shape: {:+.2}%\n      Position: {:.2}%",
        if fade.crossfade && fade.length_frames > 0 && limit == 0 { " (inactive; automatic boundary fade)" } else { "" },
        fade.length_frames, project.seconds(fade.length_frames) * 1000.0,
        effective, project.seconds(effective) * 1000.0,
        (fade.midpoint - 0.5) * 200.0, position * 100.0).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        let mut project = Project::default();
        let group = project.add_track().unwrap();
        project.tracks[0].kind = TrackKind::Group;
        project.tracks[0].name = "Band".into();
        project.tracks[0].group_collapsed = true;
        project.add_track().unwrap();
        project.tracks[1].name = "Bäss synth".into();
        project.tracks[1].group_id = Some(group);
        let midi = project.add_midi_track().unwrap();
        project.tracks[2].name = "Bäss synth".into();
        project.tracks[2].group_id = Some(group);
        project.add_midi_pattern(midi, 48_000).unwrap();
        project.validate().unwrap();
        project
    }

    #[test]
    fn list_includes_collapsed_children_and_all_kinds_in_saved_order() {
        let project = project();
        let output = describe(&project, None).unwrap();
        let rows: Vec<_> = output.lines().skip(3).collect();
        assert_eq!(rows.len(), 3);
        for (index, track) in project.tracks.iter().enumerate() {
            assert!(rows[index].contains(&track.id.to_string()));
            assert!(rows[index].contains(kind(track)));
            assert!(rows[index].contains(&format!("{:?}", track.name)));
        }
        assert!(
            describe(&Project::default(), None)
                .unwrap()
                .ends_with("No tracks.\n")
        );
    }

    #[test]
    fn exact_name_reports_every_match_with_identity_mixer_and_clip_timing() {
        let mut project = project();
        project.tracks[1].gain_db = -6.0;
        project.tracks[1].pan = -0.25;
        project.tracks[1].muted = true;
        project.tracks[1].soloed = true;
        project.tracks[1]
            .effects
            .push(daw_core::EffectInstance::drive());
        let output = describe(&project, Some("Bäss synth")).unwrap();
        assert_eq!(output.matches("Track: ").count(), 2);
        assert!(
            output.find(&project.tracks[1].id.to_string()).unwrap()
                < output.find(&project.tracks[2].id.to_string()).unwrap()
        );
        for value in [
            "Gain: -6.00 dB",
            "Pan: -0.250",
            "Muted: true",
            "Solo: true",
            "Track effects: 1",
            "Instrument layers: ",
            "Parent: \"Band\"",
            "Start: 48000 frames (1.000000 s)",
        ] {
            assert!(output.contains(value), "missing {value}: {output}");
        }
        let clip = &project.tracks[2].clips[0];
        assert!(output.contains(&format!("End (exclusive): {} frames", clip.end())));
        assert!(output.contains(&format!("Length: {} frames", clip.length_frames)));
        assert!(
            describe(&project, Some("bäss synth"))
                .unwrap_err()
                .to_string()
                .contains("Track not found")
        );
        let group = describe(&project, Some("Band")).unwrap();
        assert!(group.contains("Direct children: 2"));
        for track in &project.tracks[1..] {
            assert!(group.contains(&track.id.to_string()));
        }
    }

    #[test]
    fn names_escape_terminal_controls() {
        let mut project = Project::default();
        project.add_track().unwrap();
        project.name = "Project\x1b[31m".into();
        project.tracks[0].name = "Bass\tlead\n\x1b[31m".into();
        for name in [None, Some(project.tracks[0].name.as_str())] {
            let output = describe(&project, name).unwrap();
            assert!(!output.contains('\x1b'));
            assert!(output.contains("Bass\\tlead\\n\\u{1b}[31m"));
        }
    }
    #[test]
    fn clip_details_report_gain_channels_polarity_and_saved_and_effective_fades() {
        let mut project = project();
        let mut asset = project.assets[0].clone();
        asset.id = daw_core::Id::new_v4();
        asset.midi_pattern = None;
        asset.decoded_frame_count = 96_000;
        let mut clip = project.tracks[2].clips[0].clone();
        clip.id = daw_core::Id::new_v4();
        clip.asset_id = asset.id;
        clip.start_frame = 0;
        clip.length_frames = 48_000;
        clip.gain_db = -3.5;
        clip.channels.left = false;
        clip.channels.swapped = true;
        clip.channels.inverted = true;
        clip.fades.fade_in = daw_core::ClipFade {
            length_frames: 4800,
            midpoint: 0.7,
            midpoint_position: 0.25,
            crossfade: false,
        };
        project.assets.push(asset);
        project.tracks[1].clips.push(clip.clone());
        let before = project.clone();
        let output = describe(&project, Some("Bäss synth")).unwrap();
        for value in [
            "Gain: -3.50 dB",
            "Channels: L=false R=true Swap L/R=true",
            "Polarity: Inverted",
            "Fade in: Fade",
            "Saved duration: 4800 frames (100.000 ms)",
            "Shape: +40.00%",
            "Position: 25.00%",
            "Fade out: Automatic",
            "Effective duration: 240 frames (5.000 ms)",
        ] {
            assert!(output.contains(value), "missing {value}: {output}");
        }
        assert_eq!(project, before);
        project.tracks[1].clips[0].fades.fade_out = daw_core::ClipFade {
            length_frames: 1000,
            crossfade: true,
            ..Default::default()
        };
        let output = describe(&project, Some("Bäss synth")).unwrap();
        assert!(output.contains("Fade out: Crossfade (inactive; automatic boundary fade)"));
        assert!(output.contains("Saved duration: 1000 frames"));
        let mut neighbor = clip.clone();
        neighbor.id = daw_core::Id::new_v4();
        neighbor.start_frame = clip.end();
        project.tracks[1].clips.push(neighbor);
        let active = describe(&project, Some("Bäss synth")).unwrap();
        assert!(active.contains("Fade out: Crossfade\n"));
        assert!(active.contains("Effective duration: 1000 frames (20.833 ms)"));
        let midi = describe(&project, Some("Bäss synth")).unwrap();
        let midi = midi.split("Type: MIDI").nth(1).unwrap();
        assert!(midi.contains("Gain: 0.00 dB"));
        assert!(!midi.contains("Channels:"));
    }
}
