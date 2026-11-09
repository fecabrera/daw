//! Merge a contiguous selection without changing shared pattern assets.
use crate::split::{note_occurrences, piece_pattern};
use crate::{Asset, Clip, ClipLoop, Error, Id, MidiNoteSplit, Project, Result};
use std::collections::HashSet;

const MAX_MERGE_NOTES: usize = 4096;

impl Project {
    pub(crate) fn pattern_merge_targets(&self, ids: &[Id]) -> Result<(usize, Vec<&Clip>)> {
        let ids: HashSet<_> = ids.iter().copied().collect();
        if ids.is_empty() {
            return Err(Error("Select one or more MIDI or beat patterns".into()));
        }
        let (index, track) = self
            .tracks
            .iter()
            .enumerate()
            .find(|(_, track)| track.clips.iter().any(|clip| ids.contains(&clip.id)))
            .ok_or_else(|| Error("Clip not found".into()))?;
        let mut clips: Vec<_> = track
            .clips
            .iter()
            .filter(|clip| ids.contains(&clip.id))
            .collect();
        clips.sort_by_key(|clip| clip.start_frame);
        let origin = clips[0].start_frame;
        let musical_tick = |frame: u64| {
            ((frame - origin) as f64
                * f64::from(self.tempo_bpm)
                * f64::from(crate::MIDI_TICKS_PER_BEAT)
                / (f64::from(self.sample_rate_hz) * 60.0))
                .round()
        };
        if clips.len() != ids.len()
            || clips.windows(2).any(|pair| {
                let end = pair[0].end();
                let start = pair[1].start_frame;
                // Tempo/rate edits and ruler snapping can round the same tick to
                // different sample frames. Keep that small gap in the merged span.
                end > start || (end != start && musical_tick(end) != musical_tick(start))
            })
        {
            return Err(Error("Merge requires adjacent clips on one track".into()));
        }
        let first = clips[0];
        let midi = self
            .assets
            .iter()
            .any(|asset| asset.id == first.asset_id && asset.midi_pattern.is_some());
        if clips.iter().any(|clip| {
            !self.assets.iter().any(|asset| {
                asset.id == clip.asset_id
                    && if midi {
                        asset.midi_pattern.is_some()
                    } else {
                        asset.beat_pattern.is_some()
                    }
            }) || clip.gain_db != first.gain_db
                || clip.transpose != first.transpose
                || clip.muted != first.muted
                || clip.channels != first.channels
        }) {
            return Err(Error("Merge requires one pattern type with matching gain, transpose, enabled state and channels".into()));
        }
        Ok((index, clips))
    }

    pub fn can_merge_pattern_clips(&self, ids: &[Id]) -> bool {
        self.pattern_merge_targets(ids).is_ok_and(|(_, clips)| {
            if self
                .assets
                .iter()
                .any(|a| a.id == clips[0].asset_id && a.beat_pattern.is_some())
            {
                self.beat_merge_settings(&clips).is_ok()
            } else {
                true
            }
        })
    }

    pub fn can_merge_midi_clips(&self, ids: &[Id]) -> bool {
        self.pattern_merge_targets(ids).is_ok_and(|(_, clips)| {
            self.assets
                .iter()
                .any(|a| a.id == clips[0].asset_id && a.midi_pattern.is_some())
        })
    }

    /// Merge adjacent patterns or consolidate the visible range of one pattern.
    pub fn merge_pattern_clips(&self, ids: &[Id]) -> Result<(Self, Id)> {
        let (_, clips) = self.pattern_merge_targets(ids)?;
        if self
            .assets
            .iter()
            .any(|a| a.id == clips[0].asset_id && a.beat_pattern.is_some())
        {
            self.merge_beat_clips(ids)
        } else {
            self.merge_midi_clips(ids)
        }
    }

    /// Return one unlooped pattern containing the visible notes of adjacent clips.
    /// Keep the earliest clip's identity, name, color and common playback settings.
    pub fn merge_midi_clips(&self, ids: &[Id]) -> Result<(Self, Id)> {
        let (track, clips) = self.pattern_merge_targets(ids)?;
        if !self.can_merge_midi_clips(ids) {
            return Err(Error("Merge requires MIDI clips".into()));
        }
        let first = clips[0];
        let end = clips.last().unwrap().end();
        let mut occurrences = Vec::new();
        for clip in &clips {
            let pattern = self
                .assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .and_then(|asset| asset.midi_pattern.as_ref())
                .unwrap();
            // A linear source window is one repetition with no phase shift.
            let mut window = (*clip).clone();
            window.repeat.get_or_insert(ClipLoop {
                length_frames: clip.length_frames,
                phase_frame: 0,
            });
            occurrences.extend(
                note_occurrences(self, &window, pattern, MAX_MERGE_NOTES - occurrences.len())
                    .map_err(|_| {
                        Error("Merge exceeds the 4,096 note limit; shorten the selection".into())
                    })?,
            );
        }
        let pattern = piece_pattern(
            self,
            &occurrences,
            first.start_frame,
            end,
            end,
            MidiNoteSplit::Split,
            false,
        )?;
        let asset = self.midi_asset(first.name.clone(), pattern)?;
        self.finish_pattern_merge(track, &clips, asset, false)
    }

    pub(crate) fn finish_pattern_merge(
        &self,
        track: usize,
        clips: &[&Clip],
        asset: Asset,
        keep_stretch: bool,
    ) -> Result<(Self, Id)> {
        let first = clips[0];
        let mut merged = first.clone();
        merged.asset_id = asset.id;
        merged.length_frames = clips.last().unwrap().end() - first.start_frame;
        merged.source_offset_frame = 0;
        merged.repeat = None;
        if !keep_stretch {
            merged.stretch = None;
        }
        let id = merged.id;
        let removed: HashSet<_> = clips.iter().map(|clip| clip.id).collect();
        let mut next = self.clone();
        next.assets.push(asset);
        next.tracks[track]
            .clips
            .retain(|clip| !removed.contains(&clip.id));
        next.tracks[track].clips.push(merged);
        next.tracks[track]
            .clips
            .sort_by_key(|clip| clip.start_frame);
        next.validate()?;
        Ok((next, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edit, MidiNote, MidiPattern};

    fn fixture() -> (Project, Vec<Id>) {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let mut ids = Vec::new();
        for index in 0..3 {
            let id = project
                .add_midi_pattern(track, project.midi_frame(index * 960))
                .unwrap();
            project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: id,
                    pattern: MidiPattern {
                        length_ticks: 960,
                        notes: vec![MidiNote {
                            id: Id::new_v4(),
                            start_tick: 240,
                            length_ticks: 480,
                            pitch: 60 + index as u8,
                            velocity: 90,
                        }],
                    },
                })
                .unwrap();
            let length = project.midi_frame(960);
            project.tracks[0]
                .clips
                .iter_mut()
                .find(|clip| clip.id == id)
                .unwrap()
                .length_frames = length;
            ids.push(id);
        }
        (project, ids)
    }

    #[test]
    fn merge_orders_three_clips_and_preserves_shared_assets_and_identity() {
        let (project, mut ids) = fixture();
        let original = project.clone();
        ids.reverse();
        let (merged, id) = project.merge_midi_clips(&ids).unwrap();
        assert_eq!(project, original);
        assert_eq!(merged.tracks[0].clips.len(), 1);
        assert_eq!(id, ids[2]);
        let clip = &merged.tracks[0].clips[0];
        assert_eq!(clip.length_frames, project.midi_frame(2880));
        assert_eq!(&merged.assets[..project.assets.len()], &project.assets);
        let notes = &merged
            .assets
            .last()
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap()
            .notes;
        assert_eq!(
            notes
                .iter()
                .map(|n| (n.start_tick, n.length_ticks, n.pitch))
                .collect::<Vec<_>>(),
            vec![(240, 480, 60), (1200, 480, 61), (2160, 480, 62)]
        );
        assert_eq!(
            serde_json::from_str::<Project>(&serde_json::to_string(&merged).unwrap()).unwrap(),
            merged
        );
    }

    #[test]
    fn merge_rejects_gaps_missing_clips_other_tracks_and_incompatible_settings() {
        let (project, ids) = fixture();
        assert!(project.can_merge_midi_clips(&ids[..1]));
        assert!(!project.can_merge_midi_clips(&[ids[0], ids[2]]));
        assert!(!project.can_merge_midi_clips(&[ids[0], Id::new_v4()]));
        let mut changed = project.clone();
        changed.tracks[0].clips[1].gain_db = -6.0;
        assert!(changed.merge_midi_clips(&ids).is_err());
        let other = changed.add_midi_track().unwrap();
        let other_clip = changed.add_midi_pattern(other, 0).unwrap();
        assert!(changed.merge_midi_clips(&[ids[0], other_clip]).is_err());
        assert_eq!(project.tracks[0].clips.len(), 3);
    }

    #[test]
    fn merge_unfolds_phased_stretched_loops_and_clips_source_windows() {
        let (mut project, ids) = fixture();
        let beat = project.midi_frame(960);
        project.tracks[0].clips[0].repeat = Some(ClipLoop {
            length_frames: beat / 2,
            phase_frame: beat / 4,
        });
        project.tracks[0].clips[0].stretch = Some(crate::ClipStretch::new(2, 1).unwrap());
        project.tracks[0].clips[1].source_offset_frame = beat / 2;
        let (merged, _) = project.merge_midi_clips(&ids).unwrap();
        let notes = &merged
            .assets
            .last()
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap()
            .notes;
        assert_eq!(
            notes
                .iter()
                .map(|n| (n.start_tick, n.length_ticks))
                .collect::<Vec<_>>(),
            vec![(0, 120), (360, 240), (840, 120), (960, 240), (2160, 480)]
        );
        let note_ids: HashSet<_> = notes.iter().map(|n| n.id).collect();
        assert_eq!(notes.len(), note_ids.len());
        assert!(merged.tracks[0].clips[0].repeat.is_none());
        assert!(merged.tracks[0].clips[0].stretch.is_none());
    }

    #[test]
    fn merge_rejects_excessive_loop_expansion() {
        let (mut project, ids) = fixture();
        project.tracks[0].clips[0].repeat = Some(ClipLoop {
            length_frames: 1,
            phase_frame: 0,
        });
        let asset = project
            .assets
            .iter_mut()
            .find(|a| a.id == project.tracks[0].clips[0].asset_id)
            .unwrap();
        asset.midi_pattern.as_mut().unwrap().notes[0].start_tick = 0;
        let original = project.clone();
        assert!(
            project
                .merge_midi_clips(&ids)
                .unwrap_err()
                .to_string()
                .contains("4,096")
        );
        assert_eq!(project, original);
    }
    #[test]
    fn merge_midi_accepts_rounded_edges_but_rejects_a_full_tick_gap() {
        let (mut p, ids) = fixture();
        let boundary = p.tracks[0].clips[0].end();
        p.tracks[0].clips[0].length_frames -= 1;
        p.tracks[0].clips[1].start_frame = boundary + 1;
        p.tracks[0].clips[1].length_frames -= 1;
        let end = p.tracks[0].clips[1].end();
        assert!(p.can_merge_midi_clips(&ids[..2]));
        let (merged, _) = p.merge_midi_clips(&ids[..2]).unwrap();
        assert_eq!(merged.tracks[0].clips[0].length_frames, end);
        p.tracks[0].clips[1].start_frame = p.tracks[0].clips[0].end() + p.midi_frame(1);
        assert!(!p.can_merge_midi_clips(&ids[..2]));
        p.tracks[0].clips[1].start_frame = p.tracks[0].clips[0].end() - 1;
        assert!(!p.can_merge_midi_clips(&ids[..2]));
    }
    #[test]
    fn merge_consolidates_one_phased_midi_loop_with_stretch() {
        for scale in [1, 2] {
            let (mut p, ids) = fixture();
            p.tracks[0].clips.retain(|c| c.id == ids[0]);
            let offset = p.midi_frame(120) * scale;
            let base = p.midi_frame(720) * scale;
            let clip = &mut p.tracks[0].clips[0];
            clip.source_offset_frame = offset;
            clip.length_frames = base * 3;
            clip.repeat = Some(ClipLoop {
                length_frames: base,
                phase_frame: offset,
            });
            clip.stretch = (scale == 2).then(|| crate::ClipStretch::new(1, 2).unwrap());
            clip.gain_db = -6.0;
            clip.transpose = 12;
            clip.muted = true;
            p.validate().unwrap();
            let original = p.clone();
            assert!(p.can_merge_pattern_clips(&ids[..1]));
            let (next, id) = p.merge_pattern_clips(&ids[..1]).unwrap();
            assert_eq!(p, original);
            assert_eq!(id, ids[0]);
            assert_eq!(&next.assets[..p.assets.len()], &p.assets);
            let clip = &next.tracks[0].clips[0];
            assert_eq!(clip.length_frames, base * 3);
            assert_eq!(clip.source_offset_frame, 0);
            assert!(clip.repeat.is_none() && clip.stretch.is_none());
            assert_eq!((clip.gain_db, clip.transpose, clip.muted), (-6.0, 12, true));
            let pattern = next.assets.last().unwrap().midi_pattern.as_ref().unwrap();
            assert_eq!(
                pattern
                    .notes
                    .iter()
                    .map(|n| (n.start_tick, n.length_ticks))
                    .collect::<Vec<_>>(),
                vec![
                    (0, 480 * scale as u32),
                    (720 * scale as u32, 480 * scale as u32),
                    (1440 * scale as u32, 480 * scale as u32)
                ]
            );
            assert_eq!(
                pattern
                    .notes
                    .iter()
                    .map(|n| n.id)
                    .collect::<HashSet<_>>()
                    .len(),
                3
            );
            assert_eq!(
                serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap()).unwrap(),
                next
            );
        }
    }

    #[test]
    fn merge_consolidates_single_midi_left_right_and_both_trims() {
        for (offset_ticks, length_ticks, note_start) in
            [(0, 480, 240), (480, 480, 0), (480, 240, 0)]
        {
            let (mut p, ids) = fixture();
            let offset = p.midi_frame(offset_ticks);
            let length = p.midi_frame(length_ticks);
            p.tracks[0].clips[0].source_offset_frame = offset;
            p.tracks[0].clips[0].length_frames = length;
            let original = p.clone();
            assert!(p.can_merge_midi_clips(&ids[..1]));
            let (next, id) = p.merge_midi_clips(&ids[..1]).unwrap();
            assert_eq!(p, original);
            assert_eq!(next.tracks[0].clips.len(), 3);
            assert_eq!(id, ids[0]);
            assert_eq!(next.tracks[0].clips[0].source_offset_frame, 0);
            assert_eq!(next.tracks[0].clips[0].length_frames, length);
            let pattern = next.assets.last().unwrap().midi_pattern.as_ref().unwrap();
            assert_eq!(
                pattern
                    .notes
                    .iter()
                    .map(|n| (n.start_tick, n.length_ticks))
                    .collect::<Vec<_>>(),
                vec![(note_start, 240)]
            );
            assert_eq!(&next.assets[..p.assets.len()], &p.assets);
        }
        let (p, ids) = fixture();
        assert!(p.can_merge_midi_clips(&ids[..1]));
        assert!(!p.can_merge_pattern_clips(&[]));
    }
    #[test]
    fn merge_consolidates_midi_patterns_after_source_edge_trim_and_reopen() {
        for (start, end) in [(0, 480), (480, 960), (480, 720)] {
            let (p, ids) = fixture();
            let trimmed = p.resized_midi_pattern(ids[0], start, end).unwrap();
            let reopened: Project =
                serde_json::from_str(&serde_json::to_string(&trimmed).unwrap()).unwrap();
            let clip = &reopened.tracks[0].clips[0];
            let asset = reopened
                .assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .unwrap();
            assert_eq!(clip.source_offset_frame, p.midi_frame(start as u32));
            assert_eq!(reopened.assets, p.assets);
            assert!(reopened.can_merge_pattern_clips(&ids[..1]));
            let (next, _) = reopened.merge_pattern_clips(&ids[..1]).unwrap();
            let consolidated = &next.tracks[0].clips[0];
            assert_eq!(
                (consolidated.start_frame, consolidated.length_frames),
                (clip.start_frame, clip.length_frames)
            );
            assert_ne!(consolidated.asset_id, clip.asset_id);
            assert_eq!(
                asset.midi_pattern,
                p.assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .unwrap()
                    .midi_pattern
            );
            let notes = &next
                .assets
                .last()
                .unwrap()
                .midi_pattern
                .as_ref()
                .unwrap()
                .notes;
            assert_eq!(
                notes
                    .iter()
                    .map(|n| (n.start_tick, n.length_ticks))
                    .collect::<Vec<_>>(),
                vec![(if start == 0 { 240 } else { 0 }, 240)]
            );
            assert_eq!(&next.assets[..reopened.assets.len()], &reopened.assets);
        }
    }
}
