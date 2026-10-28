//! Track presentation over validated project metadata. No media or device access.
use daw_core::{Project, Track, TrackKind};
use std::fmt::Write;

fn kind(track: &Track) -> &'static str {
    match track.kind {
        TrackKind::Audio => "Audio",
        TrackKind::Midi => "MIDI",
        TrackKind::Group => "Group",
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
    }
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
}
