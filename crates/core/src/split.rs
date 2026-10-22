//! Split clips at one timeline frame.
//! Audio uses the source-window split. MIDI rewrites only the clips being split.
use crate::{Clip, Edit, Error, Id, MidiNote, MidiPattern, Project, Result};
use std::collections::HashSet;

/// How a MIDI note that crosses the playhead is divided.
/// Stored in application settings, not in the project.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiNoteSplit {
    /// The part before the playhead stays on the first clip. The new clip does not continue it.
    PreserveFirst,
    /// The note becomes two notes. The original ID stays on the first clip.
    #[default]
    #[serde(other)]
    Split,
}

impl MidiNoteSplit {
    pub const ALL: [Self; 2] = [Self::Split, Self::PreserveFirst];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Split => "Split into two",
            Self::PreserveFirst => "Preserve first",
        }
    }
}

struct Occurrence {
    note: MidiNote,
    start: u64,
    end: u64,
}

impl Project {
    /// Split every listed clip that strictly contains `at`.
    /// The returned IDs are the new clips, in the same order as the clips that were split.
    /// A missing clip or an invalid result leaves the caller’s project unchanged.
    pub fn split_clips(
        &self,
        clip_ids: &[Id],
        at: u64,
        notes: MidiNoteSplit,
    ) -> Result<(Self, Vec<Id>)> {
        let mut targets = Vec::new();
        let mut seen = HashSet::new();
        for id in clip_ids {
            if !seen.insert(*id) {
                continue;
            }
            let clip = self
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == *id)
                .cloned()
                .ok_or_else(|| Error("Clip not found".into()))?;
            if clip.start_frame < at && at < clip.end() {
                targets.push(clip);
            }
        }
        if targets.is_empty() {
            return Ok((self.clone(), Vec::new()));
        }
        let mut next = self.clone();
        let mut created = Vec::with_capacity(targets.len());
        for clip in &targets {
            created.push(next.split_one(clip, at, notes)?);
        }
        next.validate()?;
        Ok((next, created))
    }

    fn split_one(&mut self, clip: &Clip, at: u64, notes: MidiNoteSplit) -> Result<Id> {
        let asset = self
            .assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .cloned()
            .ok_or_else(|| Error("Clip references a missing asset record".into()))?;
        if let Some(pattern) = asset.midi_pattern.clone() {
            if clip.repeat.is_some()
                && note_occurrences(self, clip, &pattern)
                    .iter()
                    .any(|occurrence| occurrence.start < at && occurrence.end > at)
            {
                // A loop stores one copy of each note. Unfold the two sides so a
                // split changes only the occurrence under the playhead.
                return self.unfold_midi_split(clip, &asset.name, &pattern, at, notes);
            }
            let new_id = self.split_geometry(clip, at)?;
            self.retarget_midi_split(clip, new_id, &asset.name, &pattern, at, notes)?;
            return Ok(new_id);
        }
        self.split_geometry(clip, at)
    }

    fn split_geometry(&mut self, clip: &Clip, at: u64) -> Result<Id> {
        let before: HashSet<Id> = self
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .map(|candidate| candidate.id)
            .collect();
        self.edit(Edit::Split {
            clip_id: clip.id,
            at,
        })?;
        self.tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|candidate| !before.contains(&candidate.id))
            .map(|candidate| candidate.id)
            .ok_or_else(|| Error("Split did not create a clip".into()))
    }

    fn retarget_midi_split(
        &mut self,
        clip: &Clip,
        new_id: Id,
        name: &str,
        pattern: &MidiPattern,
        at: u64,
        notes: MidiNoteSplit,
    ) -> Result<()> {
        let (left, right, crossed) = window_patterns(self, clip, pattern, at, notes);
        if !crossed {
            return Ok(());
        }
        left.validate()?;
        right.validate()?;
        if &left != pattern {
            self.assign_pattern(clip.id, name, left)?;
        }
        if &right != pattern {
            self.assign_pattern(new_id, name, right)?;
        }
        Ok(())
    }

    fn unfold_midi_split(
        &mut self,
        clip: &Clip,
        name: &str,
        pattern: &MidiPattern,
        at: u64,
        notes: MidiNoteSplit,
    ) -> Result<Id> {
        let occurrences = note_occurrences(self, clip, pattern);
        let new_id = Id::new_v4();
        let left_pattern =
            piece_pattern(self, &occurrences, clip.start_frame, at, at, notes, false)?;
        let right_pattern = piece_pattern(self, &occurrences, at, clip.end(), at, notes, true)?;
        let left_asset = self.midi_asset(name.to_owned(), left_pattern)?;
        let right_asset = self.midi_asset(name.to_owned(), right_pattern)?;
        let left_id = left_asset.id;
        let right_asset_id = right_asset.id;
        self.assets.push(left_asset);
        self.assets.push(right_asset);
        let track = self
            .tracks
            .iter_mut()
            .find(|track| track.clips.iter().any(|candidate| candidate.id == clip.id))
            .ok_or_else(|| Error("Clip not found".into()))?;
        let left = track
            .clips
            .iter_mut()
            .find(|candidate| candidate.id == clip.id)
            .ok_or_else(|| Error("Clip not found".into()))?;
        *left = linear_piece(clip, left_id, clip.start_frame, at, clip.id);
        track
            .clips
            .push(linear_piece(clip, right_asset_id, at, clip.end(), new_id));
        Ok(new_id)
    }

    fn assign_pattern(&mut self, clip_id: Id, name: &str, pattern: MidiPattern) -> Result<()> {
        let asset = self.midi_asset(name.to_owned(), pattern)?;
        let asset_id = asset.id;
        self.assets.push(asset);
        self.tracks
            .iter_mut()
            .flat_map(|track| &mut track.clips)
            .find(|clip| clip.id == clip_id)
            .ok_or_else(|| Error("Clip not found".into()))?
            .asset_id = asset_id;
        Ok(())
    }
}

fn linear_piece(clip: &Clip, asset_id: Id, start: u64, end: u64, id: Id) -> Clip {
    Clip {
        muted: clip.muted,
        id,
        asset_id,
        name: clip.name.clone(),
        color: clip.color,
        start_frame: start,
        source_offset_frame: 0,
        length_frames: end - start,
        repeat: None,
        stretch: None,
        gain_db: 0.0,
        transpose: 0,
    }
}

fn raw_source(clip: &Clip, stretched: u64) -> u64 {
    clip.stretch
        .map_or(stretched, |stretch| stretch.original_frame(stretched))
}

fn ticks_for_frames(project: &Project, frames: u64) -> u32 {
    let ticks =
        frames as f64 * f64::from(project.tempo_bpm) * f64::from(crate::MIDI_TICKS_PER_BEAT)
            / (f64::from(project.sample_rate_hz) * 60.0);
    if !ticks.is_finite() || ticks <= 0.0 {
        0
    } else {
        u32::try_from(ticks.round() as u64).unwrap_or(u32::MAX)
    }
}

fn audible(project: &Project, note: &MidiNote) -> bool {
    let end = note.start_tick.saturating_add(note.length_ticks);
    project.midi_frame(end) > project.midi_frame(note.start_tick)
}

fn window_patterns(
    project: &Project,
    clip: &Clip,
    pattern: &MidiPattern,
    at: u64,
    mode: MidiNoteSplit,
) -> (MidiPattern, MidiPattern, bool) {
    let boundary = raw_source(
        clip,
        clip.source_offset_frame
            .saturating_add(at.saturating_sub(clip.start_frame)),
    );
    let mut left_notes = Vec::new();
    let mut right_notes = Vec::new();
    let mut crossed = false;
    for note in &pattern.notes {
        let end_tick = note.start_tick.saturating_add(note.length_ticks);
        let start_frame = project.midi_frame(note.start_tick);
        let end_frame = project.midi_frame(end_tick);
        if !(start_frame < boundary && boundary < end_frame) {
            if end_frame <= boundary {
                left_notes.push(note.clone());
            } else {
                right_notes.push(note.clone());
            }
            continue;
        }
        crossed = true;
        let mut split_tick = ticks_for_frames(project, boundary);
        if split_tick <= note.start_tick {
            split_tick = note.start_tick.saturating_add(1);
        }
        if split_tick >= end_tick {
            split_tick = end_tick.saturating_sub(1);
        }
        let can_divide =
            note.length_ticks >= 2 && split_tick > note.start_tick && split_tick < end_tick;
        if !can_divide {
            if start_frame < boundary {
                left_notes.push(note.clone());
            } else {
                right_notes.push(note.clone());
            }
            continue;
        }
        let left = MidiNote {
            length_ticks: split_tick - note.start_tick,
            ..note.clone()
        };
        if audible(project, &left) {
            left_notes.push(left);
        }
        if mode == MidiNoteSplit::Split {
            let right = MidiNote {
                id: Id::new_v4(),
                start_tick: split_tick,
                length_ticks: end_tick - split_tick,
                ..note.clone()
            };
            if audible(project, &right) {
                right_notes.push(right);
            }
        }
    }
    (
        MidiPattern {
            length_ticks: pattern.length_ticks,
            notes: left_notes,
        },
        MidiPattern {
            length_ticks: pattern.length_ticks,
            notes: right_notes,
        },
        crossed,
    )
}

fn note_occurrences(project: &Project, clip: &Clip, pattern: &MidiPattern) -> Vec<Occurrence> {
    let base = clip
        .repeat
        .map_or(clip.length_frames, |repeat| repeat.length_frames);
    if base == 0 {
        return Vec::new();
    }
    let count = clip
        .repeat
        .map_or(1, |_| (clip.length_frames / base).saturating_add(2));
    let phase = clip.repeat.map_or(0, |repeat| repeat.phase_frame);
    let mut occurrences = Vec::new();
    for repetition in 0..count {
        for note in &pattern.notes {
            let original_start = project.midi_frame(note.start_tick);
            let original_end =
                project.midi_frame(note.start_tick.saturating_add(note.length_ticks));
            let mut start = clip.stretch.map_or(original_start, |stretch| {
                stretch.scale(original_start).unwrap_or(0)
            });
            let mut end = clip.stretch.map_or(original_end, |stretch| {
                stretch.scale(original_end).unwrap_or(0)
            });
            if let Some(repeat) = clip.repeat {
                start = start.max(clip.source_offset_frame);
                end = end.min(
                    clip.source_offset_frame
                        .saturating_add(repeat.length_frames),
                );
                if end <= start {
                    continue;
                }
            }
            let local =
                i128::from(start) - i128::from(clip.source_offset_frame) - i128::from(phase)
                    + i128::from(repetition) * i128::from(base);
            let finish = local + i128::from(end.saturating_sub(start));
            if finish <= 0 || local >= i128::from(clip.length_frames) {
                continue;
            }
            let visible_start = local.max(0) as u64;
            let visible_end = u64::try_from(finish.min(i128::from(clip.length_frames)))
                .unwrap_or(clip.length_frames);
            if visible_end <= visible_start {
                continue;
            }
            let Some(project_end) = clip.start_frame.checked_add(visible_end) else {
                continue;
            };
            occurrences.push(Occurrence {
                note: note.clone(),
                start: clip.start_frame.saturating_add(visible_start),
                end: project_end,
            });
        }
    }
    occurrences
}

fn piece_pattern(
    project: &Project,
    occurrences: &[Occurrence],
    piece_start: u64,
    piece_end: u64,
    at: u64,
    mode: MidiNoteSplit,
    right_side: bool,
) -> Result<MidiPattern> {
    let mut notes = Vec::new();
    let mut used = HashSet::new();
    for occurrence in occurrences {
        let crosses = occurrence.start < at && occurrence.end > at;
        if crosses && mode == MidiNoteSplit::PreserveFirst && right_side {
            continue;
        }
        let start = occurrence.start.max(piece_start);
        let end = occurrence.end.min(piece_end);
        if end <= start {
            continue;
        }
        let start_tick = ticks_for_frames(project, start - piece_start);
        let mut end_tick = ticks_for_frames(project, end - piece_start);
        if end_tick <= start_tick {
            end_tick = start_tick.saturating_add(1);
        }
        let reuse = (!crosses || !right_side) && used.insert(occurrence.note.id);
        let note = MidiNote {
            id: if reuse {
                occurrence.note.id
            } else {
                Id::new_v4()
            },
            start_tick,
            length_ticks: end_tick - start_tick,
            pitch: occurrence.note.pitch,
            velocity: occurrence.note.velocity,
        };
        if audible(project, &note) {
            notes.push(note);
        }
    }
    let frames = piece_end.saturating_sub(piece_start);
    let mut length_ticks = ticks_for_frames(project, frames).max(1);
    if let Some(end) = notes
        .iter()
        .map(|note| note.start_tick.saturating_add(note.length_ticks))
        .max()
    {
        length_ticks = length_ticks.max(end);
    }
    while project.midi_frame(length_ticks) < frames {
        length_ticks = length_ticks
            .checked_add(1)
            .ok_or_else(|| Error("MIDI pattern exceeds supported timing bounds".into()))?;
    }
    let pattern = MidiPattern {
        length_ticks,
        notes,
    };
    pattern.validate()?;
    Ok(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Asset, ClipLoop, MidiNoteSplit, Source, SourceMetadata};

    fn audio_project() -> (Project, Id) {
        let mut project = Project::default();
        project.add_track().unwrap();
        let asset = Id::new_v4();
        project.assets.push(Asset {
            midi_pattern: None,
            id: asset,
            name: "source.wav".into(),
            source: Source {
                kind: "external".into(),
                path: "source.wav".into(),
                path_kind: "relative".into(),
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: 48_000,
                channels: 2,
                bits_per_sample: 16,
                sample_format: "pcm_int".into(),
            },
            decoded_frame_count: 96_000,
        });
        let clip = Id::new_v4();
        project.tracks[0].clips.push(Clip {
            muted: false,
            id: clip,
            asset_id: asset,
            name: "Take".into(),
            color: None,
            start_frame: 1_000,
            source_offset_frame: 100,
            length_frames: 5_000,
            repeat: Some(ClipLoop {
                length_frames: 1_000,
                phase_frame: 100,
            }),
            stretch: None,
            gain_db: 0.0,
            transpose: 0,
        });
        (project, clip)
    }

    #[test]
    fn audio_split_keeps_the_source_and_advances_loop_phase() {
        let (project, clip) = audio_project();
        let asset = project.tracks[0].clips[0].asset_id;
        let (split, created) = project
            .split_clips(&[clip], 1_250, MidiNoteSplit::Split)
            .unwrap();
        assert_eq!(created.len(), 1);
        let left = split.tracks[0].clips.iter().find(|c| c.id == clip).unwrap();
        let right = split.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == created[0])
            .unwrap();
        assert_eq!(left.length_frames, 250);
        assert_eq!(left.source_offset_frame, 100);
        assert_eq!(left.asset_id, asset);
        assert_eq!(right.start_frame, 1_250);
        assert_eq!(right.length_frames, 4_750);
        assert_eq!(right.source_offset_frame, 100);
        assert_eq!(right.asset_id, asset);
        assert_eq!(right.name, "Take");
        assert_eq!(
            right.repeat,
            Some(ClipLoop {
                length_frames: 1_000,
                phase_frame: 350,
            })
        );
    }

    #[test]
    fn split_ignores_clips_outside_the_frame_and_rejects_a_missing_clip() {
        let (mut project, clip) = audio_project();
        let outside = Id::new_v4();
        let template = project.tracks[0].clips[0].clone();
        project.tracks[0].clips.push(Clip {
            id: outside,
            start_frame: 20_000,
            length_frames: 1_000,
            source_offset_frame: 0,
            ..template
        });
        let before = format!("{project:?}");
        let (split, created) = project
            .split_clips(&[clip, outside], 1_250, MidiNoteSplit::PreserveFirst)
            .unwrap();
        assert_eq!(created.len(), 1);
        assert!(
            split.tracks[0]
                .clips
                .iter()
                .any(|c| c.id == outside && c.length_frames == 1_000)
        );
        assert!(
            project
                .split_clips(&[clip, Id::new_v4()], 1_250, MidiNoteSplit::Split)
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
    }

    fn midi_project(rate: u32) -> (Project, Id, Id, Id) {
        let mut project = Project {
            sample_rate_hz: rate,
            ..Project::default()
        };
        let track = project.add_midi_track().unwrap();
        let clip = project.add_midi_pattern(track, 0).unwrap();
        let note = Id::new_v4();
        let before = Id::new_v4();
        let after = Id::new_v4();
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![
                        MidiNote {
                            id: before,
                            start_tick: 0,
                            length_ticks: 480,
                            pitch: 62,
                            velocity: 80,
                        },
                        MidiNote {
                            id: note,
                            start_tick: 480,
                            length_ticks: 960,
                            pitch: 60,
                            velocity: 100,
                        },
                        MidiNote {
                            id: after,
                            start_tick: 1920,
                            length_ticks: 480,
                            pitch: 64,
                            velocity: 90,
                        },
                    ],
                },
            })
            .unwrap();
        let length = project.midi_frame(3840);
        project.tracks[0].clips[0].length_frames = length;
        let _ = track;
        (project, clip, note, before)
    }

    #[test]
    fn midi_split_divides_only_the_crossing_note_and_preserves_other_pattern_users() {
        for rate in crate::SAMPLE_RATES {
            for mode in MidiNoteSplit::ALL {
                let (mut project, clip, note, before) = midi_project(rate);
                let shared = project.tracks[0].clips[0].asset_id;
                let other = project
                    .add_midi_pattern(project.tracks[0].id, project.midi_frame(3840))
                    .unwrap();
                project.tracks[0]
                    .clips
                    .iter_mut()
                    .find(|clip| clip.id == other)
                    .unwrap()
                    .asset_id = shared;
                let at = project.midi_frame(960);
                let original = project
                    .assets
                    .iter()
                    .find(|asset| asset.id == shared)
                    .unwrap()
                    .midi_pattern
                    .clone()
                    .unwrap();
                let (split, created) = project.split_clips(&[clip], at, mode).unwrap();
                let left = split.tracks[0].clips.iter().find(|c| c.id == clip).unwrap();
                let right = split.tracks[0]
                    .clips
                    .iter()
                    .find(|c| c.id == created[0])
                    .unwrap();
                assert_eq!(
                    (left.end(), right.start_frame, right.end()),
                    (at, at, project.midi_frame(3840))
                );
                assert_ne!(left.asset_id, shared);
                assert_ne!(right.asset_id, shared);
                let left_notes = split
                    .assets
                    .iter()
                    .find(|asset| asset.id == left.asset_id)
                    .unwrap()
                    .midi_pattern
                    .as_ref()
                    .unwrap();
                let right_notes = split
                    .assets
                    .iter()
                    .find(|asset| asset.id == right.asset_id)
                    .unwrap()
                    .midi_pattern
                    .as_ref()
                    .unwrap();
                assert!(
                    left_notes
                        .notes
                        .iter()
                        .any(|n| n.id == before && n.length_ticks == 480)
                );
                assert!(
                    left_notes
                        .notes
                        .iter()
                        .any(|n| n.id == note && n.length_ticks == 480)
                );
                assert!(!right_notes.notes.iter().any(|n| n.id == note));
                assert!(
                    right_notes
                        .notes
                        .iter()
                        .any(|n| n.pitch == 64 && n.start_tick == 1920)
                );
                if mode == MidiNoteSplit::Split {
                    assert!(right_notes.notes.iter().any(|n| {
                        n.pitch == 60
                            && n.velocity == 100
                            && n.start_tick == 960
                            && n.length_ticks == 480
                    }));
                } else {
                    assert!(!right_notes.notes.iter().any(|n| n.pitch == 60));
                }
                let untouched = split
                    .assets
                    .iter()
                    .find(|asset| asset.id == shared)
                    .unwrap()
                    .midi_pattern
                    .as_ref()
                    .unwrap();
                assert_eq!(untouched, &original);
                assert_eq!(
                    split.tracks[0]
                        .clips
                        .iter()
                        .find(|clip| clip.id == other)
                        .unwrap()
                        .asset_id,
                    shared
                );
            }
        }
    }

    #[test]
    fn looped_midi_split_keeps_repetitions_that_do_not_cross_the_playhead() {
        let (mut project, clip, note, _) = midi_project(48_000);
        let beat = project.midi_frame(960);
        {
            let current = &mut project.tracks[0].clips[0];
            current.length_frames = beat;
            current.source_offset_frame = 0;
            current.repeat = None;
        }
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 960,
                    notes: vec![MidiNote {
                        id: note,
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 60,
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        {
            let current = &mut project.tracks[0].clips[0];
            current.length_frames = beat * 4;
            current.repeat = Some(ClipLoop {
                length_frames: beat,
                phase_frame: 0,
            });
            current.source_offset_frame = 0;
        }
        let at = beat + beat / 2;
        let (split, created) = project
            .split_clips(&[clip], at, MidiNoteSplit::Split)
            .unwrap();
        let left = pattern_notes(&split, clip);
        let right = pattern_notes(&split, created[0]);
        assert_eq!(left.len(), 2);
        assert_eq!(left[0], (note, 0, 960));
        assert_eq!((left[1].1, left[1].2), (960, 480));
        assert_ne!(left[1].0, note);
        assert_eq!(right[0].1, 0);
        assert_eq!(right[0].2, 480);
        assert_ne!(right[0].0, note);
        assert!(
            right
                .iter()
                .any(|(_, start, length)| *start == 480 && *length == 960)
        );
        let (preserved, _) = project
            .split_clips(&[clip], at, MidiNoteSplit::PreserveFirst)
            .unwrap();
        let right = pattern_notes(
            &preserved,
            preserved.tracks[0]
                .clips
                .iter()
                .find(|c| c.start_frame == at)
                .unwrap()
                .id,
        );
        assert!(right.iter().all(|(_, start, _)| *start >= 480));
        assert!(
            right
                .iter()
                .any(|(_, start, length)| *start == 480 && *length == 960)
        );
    }

    fn pattern_notes(project: &Project, clip: Id) -> Vec<(Id, u32, u32)> {
        let asset_id = project
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|candidate| candidate.id == clip)
            .unwrap()
            .asset_id;
        let mut notes: Vec<_> = project
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap()
            .notes
            .iter()
            .map(|note| (note.id, note.start_tick, note.length_ticks))
            .collect();
        notes.sort_by_key(|note| (note.1, note.2));
        notes
    }
}
