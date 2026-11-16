//! Pure live-input routing and atomic recorded-pattern placement.
use crate::{ClipPlacement, Edit, Error, Id, MidiPattern, Project, Result, TrackKind};
use std::collections::HashSet;

/// Resolve in project order. Selection has no routing effect while any valid arm exists.
pub fn resolve_midi_targets(
    project: &Project,
    armed: &HashSet<Id>,
    selected: Option<Id>,
) -> Vec<Id> {
    let valid = |id| {
        project
            .tracks
            .iter()
            .any(|t| t.id == id && t.kind == TrackKind::Midi)
    };
    let targets: Vec<_> = project
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Midi && armed.contains(&t.id))
        .map(|t| t.id)
        .collect();
    if targets.is_empty() {
        selected.filter(|id| valid(*id)).into_iter().collect()
    } else {
        targets
    }
}

#[derive(Clone, Debug)]
pub struct RecordedMidiTrack {
    pub track_id: Id,
    pub pattern: MidiPattern,
}

impl Project {
    /// Build every asset and resolve every collision on one candidate. Never publish a subset.
    pub fn recorded_midi_take(
        &self,
        start_frame: u64,
        tracks: &[RecordedMidiTrack],
    ) -> Result<Self> {
        let mut next = self.clone();
        let mut placements = Vec::new();
        let mut seen = HashSet::new();
        for track in tracks {
            if !seen.insert(track.track_id)
                || !self
                    .tracks
                    .iter()
                    .any(|t| t.id == track.track_id && t.kind == TrackKind::Midi)
            {
                return Err(Error("Invalid or duplicate MIDI recording target".into()));
            }
            track.pattern.validate()?;
            if track.pattern.notes.is_empty() {
                continue;
            }
            let (asset, clip) =
                next.midi_clip("MIDI take".into(), track.pattern.clone(), start_frame)?;
            clip.start_frame
                .checked_add(clip.length_frames)
                .ok_or_else(|| Error("MIDI take exceeds timeline bounds".into()))?;
            next.assets.push(asset);
            placements.push(ClipPlacement {
                track_id: track.track_id,
                clip,
            });
        }
        next.edit(Edit::OverwriteClips(placements))?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_arms_override_selection_and_last_disarm_restores_fallback() {
        let mut p = Project::default();
        let a = p.add_midi_track().unwrap();
        let b = p.add_midi_track().unwrap();
        let c = p.add_midi_track().unwrap();
        assert_eq!(resolve_midi_targets(&p, &HashSet::new(), Some(c)), vec![c]);
        assert_eq!(
            resolve_midi_targets(&p, &HashSet::from([a, b]), Some(c)),
            vec![a, b]
        );
        assert!(resolve_midi_targets(&p, &HashSet::new(), None).is_empty());
        p.tracks[0].kind = TrackKind::Audio;
        assert_eq!(
            resolve_midi_targets(&p, &HashSet::from([a]), Some(c)),
            vec![c]
        );
    }
    #[test]
    fn take_rejects_whole_group_and_leaves_original_unchanged() {
        let mut p = Project::default();
        let a = p.add_midi_track().unwrap();
        let before = p.clone();
        let pattern = MidiPattern::default();
        assert!(
            p.recorded_midi_take(
                0,
                &[
                    RecordedMidiTrack {
                        track_id: a,
                        pattern: pattern.clone()
                    },
                    RecordedMidiTrack {
                        track_id: Id::new_v4(),
                        pattern
                    },
                ]
            )
            .is_err()
        );
        assert_eq!(p, before);
    }
}
