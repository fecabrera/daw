//! Time-range contents and removal for one or more tracks.
//! Pieces keep their source mapping through `Clip::retained_range`.
use crate::{ClipPlacement, Error, Id, Project, Result};
use std::collections::HashSet;

impl Project {
    /// Clips heard inside `[start, end)` on `tracks`.
    /// Each piece has a new ID and its original timeline position.
    /// A missing track or an invalid piece leaves the caller’s project unchanged.
    pub fn clip_range_slices(
        &self,
        tracks: &[Id],
        start: u64,
        end: u64,
    ) -> Result<Vec<ClipPlacement>> {
        let tracks = track_set(self, tracks, start, end)?;
        let mut slices = Vec::new();
        for track in &self.tracks {
            if !tracks.contains(&track.id) {
                continue;
            }
            for clip in &track.clips {
                let from = clip.start_frame.max(start);
                let to = clip.end().min(end);
                if from >= to {
                    continue;
                }
                let mut piece = clip.retained_range(from, to);
                piece.id = Id::new_v4();
                self.validate_clip(&piece)?;
                slices.push(ClipPlacement {
                    track_id: track.id,
                    clip: piece,
                });
            }
        }
        Ok(slices)
    }

    /// Remove `[start, end)` from `tracks`.
    /// A clip that extends past either edge keeps the outside piece and its source mapping.
    pub fn without_clip_range(&self, tracks: &[Id], start: u64, end: u64) -> Result<Self> {
        let tracks = track_set(self, tracks, start, end)?;
        let mut next = self.clone();
        for track in &mut next.tracks {
            if !tracks.contains(&track.id) {
                continue;
            }
            track.clips = track
                .clips
                .iter()
                .flat_map(|clip| clip.uncovered_ranges(start, end))
                .collect();
        }
        next.validate()?;
        Ok(next)
    }
}

fn track_set(project: &Project, tracks: &[Id], start: u64, end: u64) -> Result<HashSet<Id>> {
    if start >= end {
        return Err(Error("Clip range must be nonempty".into()));
    }
    if tracks.is_empty() {
        return Err(Error("Track not found".into()));
    }
    let mut set = HashSet::new();
    for id in tracks {
        if !project.tracks.iter().any(|track| track.id == *id) {
            return Err(Error("Track not found".into()));
        }
        set.insert(*id);
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Asset, Clip, ClipLoop, Edit, MidiNote, MidiPattern, Project, Source, SourceMetadata,
    };

    fn audio(length: u64) -> (Project, Id, Id) {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
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
            decoded_frame_count: 20_000,
        });
        let clip = Id::new_v4();
        project.tracks[0].clips.push(Clip {
            id: clip,
            asset_id: asset,
            name: "Take".into(),
            color: None,
            start_frame: 0,
            source_offset_frame: 100,
            length_frames: length,
            repeat: None,
            stretch: None,
        });
        project.validate().unwrap();
        (project, track, clip)
    }

    #[test]
    fn range_keeps_the_gap_and_the_source_mapping_outside_the_edges() {
        let (mut project, track, left_id) = audio(1_000);
        let right_id = Id::new_v4();
        let template = project.tracks[0].clips[0].clone();
        project.tracks[0].clips.push(Clip {
            id: right_id,
            name: "Later".into(),
            start_frame: 1_500,
            source_offset_frame: 0,
            length_frames: 1_000,
            ..template
        });
        project.validate().unwrap();
        let slices = project.clip_range_slices(&[track], 500, 2_000).unwrap();
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].clip.start_frame, 500);
        assert_eq!(slices[0].clip.length_frames, 500);
        assert_eq!(slices[0].clip.source_offset_frame, 600);
        assert_eq!(slices[0].clip.name, "Take");
        assert_ne!(slices[0].clip.id, left_id);
        assert_eq!(slices[1].clip.start_frame, 1_500);
        assert_eq!(slices[1].clip.length_frames, 500);
        assert_eq!(slices[1].clip.source_offset_frame, 0);
        let cleared = project.without_clip_range(&[track], 500, 2_000).unwrap();
        let clips = &cleared.tracks[0].clips;
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].id, left_id);
        assert_eq!(clips[0].length_frames, 500);
        assert_eq!(clips[0].source_offset_frame, 100);
        assert_eq!(clips[1].start_frame, 2_000);
        assert_eq!(clips[1].length_frames, 500);
        assert_eq!(clips[1].source_offset_frame, 500);
        assert_eq!(clips[1].id, right_id);
        assert_eq!(project.tracks[0].clips.len(), 2);
    }

    #[test]
    fn looped_range_unwraps_a_partial_cycle_and_rejects_an_empty_range() {
        let (mut project, track, clip) = audio(4_000);
        project.tracks[0].clips[0].repeat = Some(ClipLoop {
            length_frames: 1_000,
            phase_frame: 0,
        });
        project.tracks[0].clips[0].source_offset_frame = 0;
        project.validate().unwrap();
        let slices = project.clip_range_slices(&[track], 2_500, 3_000).unwrap();
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].clip.length_frames, 500);
        assert_eq!(slices[0].clip.source_offset_frame, 500);
        assert!(slices[0].clip.repeat.is_none());
        let before = format!("{project:?}");
        assert!(project.clip_range_slices(&[track], 10, 10).is_err());
        assert!(project.without_clip_range(&[Id::new_v4()], 0, 10).is_err());
        assert_eq!(format!("{project:?}"), before);
        assert_eq!(project.tracks[0].clips[0].id, clip);
    }

    #[test]
    fn midi_range_crops_the_source_window_and_keeps_other_pattern_users() {
        let mut project = Project {
            sample_rate_hz: 48_000,
            ..Project::default()
        };
        let track = project.add_midi_track().unwrap();
        let clip = project.add_midi_pattern(track, 0).unwrap();
        let beat = project.midi_frame(960);
        project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 960,
                    notes: vec![MidiNote {
                        id: Id::new_v4(),
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 60,
                        velocity: 100,
                    }],
                },
            })
            .unwrap();
        let asset = project.tracks[0].clips[0].asset_id;
        let other = project.add_midi_pattern(track, beat * 4).unwrap();
        let shared = project.tracks[0]
            .clips
            .iter_mut()
            .find(|candidate| candidate.id == other)
            .unwrap();
        shared.asset_id = asset;
        shared.length_frames = beat;
        project.validate().unwrap();
        let slices = project
            .clip_range_slices(&[track], beat / 4, beat / 4 * 3)
            .unwrap();
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].clip.asset_id, asset);
        assert_eq!(slices[0].clip.source_offset_frame, beat / 4);
        assert_eq!(slices[0].clip.length_frames, beat / 2);
        let cleared = project
            .without_clip_range(&[track], beat / 4, beat / 4 * 3)
            .unwrap();
        let kept = cleared.tracks[0]
            .clips
            .iter()
            .find(|candidate| candidate.id == other)
            .unwrap();
        assert_eq!(kept.asset_id, asset);
        assert_eq!(kept.start_frame, beat * 4);
        let pattern = cleared
            .assets
            .iter()
            .find(|candidate| candidate.id == asset)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap();
        assert_eq!(pattern.notes[0].length_ticks, 960);
    }
}
