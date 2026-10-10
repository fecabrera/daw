use crate::{Clip, ClipEdge, Edit, Error, Id, Project, Result};
use std::collections::HashSet;

impl Project {
    /// Find the original neighbor touching the requested waveform edge.
    pub fn trim_neighbor(&self, clip_id: Id, edge: ClipEdge) -> Option<&Clip> {
        let track = self
            .tracks
            .iter()
            .find(|track| track.clips.iter().any(|clip| clip.id == clip_id))?;
        let clip = track.clips.iter().find(|clip| clip.id == clip_id)?;
        track.clips.iter().find(|other| {
            other.id != clip_id
                && match edge {
                    ClipEdge::Left => other.end() == clip.start_frame,
                    ClipEdge::Right => other.start_frame == clip.end(),
                }
        })
    }

    /// Preview the same transaction used to commit a shared waveform boundary.
    pub fn trimmed_boundary(
        &self,
        clip_id: Id,
        selected: &[Id],
        edge: ClipEdge,
        delta: i128,
    ) -> Result<Self> {
        let mut next = self.clone();
        next.edit(Edit::TrimBoundary {
            clip_id,
            selected: selected.to_vec(),
            edge,
            delta,
        })?;
        Ok(next)
    }

    pub(crate) fn apply_trim_boundary(
        &mut self,
        clip_id: Id,
        selected: &[Id],
        edge: ClipEdge,
        delta: i128,
    ) -> Result<()> {
        let primary = self
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == clip_id)
            .cloned()
            .ok_or_else(|| Error("Clip not found".into()))?;
        let neighbor = self
            .trim_neighbor(clip_id, edge)
            .cloned()
            .ok_or_else(|| Error("No touching clip at this waveform edge".into()))?;
        let (left, right) = if edge == ClipEdge::Right {
            (primary, neighbor)
        } else {
            (neighbor, primary)
        };
        let source = |clip: &Clip| {
            self.assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .map(|asset| asset.decoded_frame_count)
                .ok_or_else(|| Error("Clip asset not found".into()))
        };
        let left_max = left
            .trimmed_by(ClipEdge::Right, i128::MAX, source(&left)?)
            .ok_or_else(|| Error("Invalid left source range".into()))?;
        let right_max = right
            .trimmed_by(ClipEdge::Left, i128::MIN, source(&right)?)
            .ok_or_else(|| Error("Invalid right source range".into()))?;
        let minimum = -i128::from(left.length_frames)
            .min(i128::from(right.start_frame - right_max.start_frame));
        let maximum = i128::from(right.length_frames).min(i128::from(left_max.end() - left.end()));
        let effective = delta.clamp(minimum, maximum);
        let mut ids = HashSet::new();
        let mut others = Vec::new();
        for id in selected {
            if !ids.insert(*id) {
                return Err(Error("Duplicate trimmed clip ID".into()));
            }
            if *id == left.id || *id == right.id {
                continue;
            }
            let clip = self
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == *id)
                .ok_or_else(|| Error("Clip not found".into()))?;
            others.push(clip.trimmed_by(edge, delta, source(clip)?).ok_or_else(|| {
                Error("Cannot trim selected clips: the amount would remove a resized clip.".into())
            })?);
        }
        // The pair keeps its outer endpoints, even if both members are selected.
        // Other selected clips retain the ordinary independent source clamps.
        let left_result = boundary_clip(
            &left,
            ClipEdge::Right,
            effective,
            source(&left)?,
            left.id != clip_id,
        )?;
        let right_result = boundary_clip(
            &right,
            ClipEdge::Left,
            effective,
            source(&right)?,
            right.id != clip_id,
        )?;
        for track in &mut self.tracks {
            track.clips.retain_mut(|clip| {
                let result = if clip.id == left.id {
                    &left_result
                } else if clip.id == right.id {
                    &right_result
                } else {
                    return true;
                };
                if let Some(next) = result {
                    *clip = next.clone();
                    true
                } else {
                    false
                }
            });
        }
        self.apply_resizes(&others, edge)
    }
}

fn boundary_clip(
    clip: &Clip,
    edge: ClipEdge,
    delta: i128,
    source: u64,
    preserve_trim_mapping: bool,
) -> Result<Option<Clip>> {
    let inward = if edge == ClipEdge::Left {
        delta
    } else {
        -delta
    };
    if inward == i128::from(clip.length_frames) {
        return Ok(None);
    }
    if preserve_trim_mapping && inward > 0 {
        let (start, end) = match edge {
            ClipEdge::Left => (clip.start_frame + inward as u64, clip.end()),
            ClipEdge::Right => (clip.start_frame, clip.end() - inward as u64),
        };
        return Ok(Some(clip.retained_range(start, end)));
    }
    clip.trimmed_by(edge, delta, source)
        .map(Some)
        .ok_or_else(|| Error("Invalid shared waveform boundary".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Asset, ClipLoop, ClipStretch, SAMPLE_RATE, Source, SourceMetadata};

    fn fixture() -> (Project, [Id; 2]) {
        let mut project = Project::default();
        project.add_track().unwrap();
        let asset = Id::new_v4();
        project.assets.push(Asset {
            id: asset,
            name: "Source".into(),
            decoded_frame_count: 1000,
            source: Source {
                kind: "external".into(),
                path: "/missing.wav".into(),
                path_kind: "absolute".into(),
            },
            source_metadata: SourceMetadata {
                sample_rate_hz: SAMPLE_RATE,
                channels: 2,
                sample_format: "pcm_int".into(),
                bits_per_sample: 16,
            },
        });
        let ids = [Id::new_v4(), Id::new_v4()];
        for (id, start, offset) in [(ids[0], 100, 20), (ids[1], 150, 100)] {
            project.tracks[0].clips.push(Clip {
                id,
                asset_id: asset,
                name: format!("Clip {start}"),
                color: None,
                start_frame: start,
                source_offset_frame: offset,
                length_frames: 50,
                repeat: None,
                stretch: None,
            });
        }
        project.validate().unwrap();
        (project, ids)
    }

    #[test]
    fn either_waveform_edge_moves_the_same_boundary_and_preserves_outer_edges() {
        for delta in [-20, 20] {
            let (mut project, ids) = fixture();
            let original = project.tracks[0].clips.clone();
            let right = project
                .trimmed_boundary(ids[0], &ids, ClipEdge::Right, delta)
                .unwrap();
            let left = project
                .trimmed_boundary(ids[1], &ids, ClipEdge::Left, delta)
                .unwrap();
            assert_eq!(format!("{left:?}"), format!("{right:?}"));
            project
                .edit(Edit::TrimBoundary {
                    clip_id: ids[0],
                    selected: ids.to_vec(),
                    edge: ClipEdge::Right,
                    delta,
                })
                .unwrap();
            assert_eq!(format!("{project:?}"), format!("{right:?}"));
            let clips = &project.tracks[0].clips;
            assert_eq!(clips[0].start_frame, 100);
            assert_eq!(clips[1].end(), 200);
            assert_eq!(clips[0].end(), clips[1].start_frame);
            assert_eq!(clips[0].end(), (150_i128 + delta) as u64);
            assert_eq!(clips[1].source_offset_frame, (100_i128 + delta) as u64);
            for (old, new) in original.iter().zip(clips) {
                assert_eq!(old.id, new.id);
                for frame in old.start_frame.max(new.start_frame)..old.end().min(new.end()) {
                    assert_eq!(
                        old.source_frame(frame - old.start_frame),
                        new.source_frame(frame - new.start_frame)
                    );
                }
            }
        }
    }

    #[test]
    fn each_expanding_source_clamps_the_pair_in_stretched_coordinates() {
        let (mut project, ids) = fixture();
        project.tracks[0].clips[0].source_offset_frame = 1935;
        project.tracks[0].clips[0].stretch = ClipStretch::new(1, 2);
        project.tracks[0].clips[1].source_offset_frame = 10;
        for (delta, boundary) in [(i128::MAX, 165), (i128::MIN, 140)] {
            let next = project
                .trimmed_boundary(ids[0], &ids[..1], ClipEdge::Right, delta)
                .unwrap();
            assert_eq!(next.tracks[0].clips[0].end(), boundary);
            assert_eq!(next.tracks[0].clips[1].start_frame, boundary);
            assert_eq!(
                next.tracks[0].clips[0].stretch,
                project.tracks[0].clips[0].stretch
            );
        }
    }

    #[test]
    fn full_coverage_removes_either_member_without_empty_clips() {
        let (project, ids) = fixture();
        for (delta, survivor) in [(i128::MIN, ids[1]), (i128::MAX, ids[0])] {
            let next = project
                .trimmed_boundary(ids[0], &ids, ClipEdge::Right, delta)
                .unwrap();
            assert_eq!(next.tracks[0].clips.len(), 1);
            let clip = &next.tracks[0].clips[0];
            assert_eq!(clip.id, survivor);
            assert_eq!((clip.start_frame, clip.end()), (100, 200));
        }
    }

    #[test]
    fn neighbor_trimming_preserves_a_wrapped_full_copy_and_can_restore_source_bounds() {
        let (mut project, ids) = fixture();
        project.tracks[0].clips[1].repeat = Some(ClipLoop {
            length_frames: 30,
            phase_frame: 5,
        });
        let original = project.tracks[0].clips[1].clone();
        let next = project
            .trimmed_boundary(ids[0], &ids[..1], ClipEdge::Right, 20)
            .unwrap();
        let trimmed = &next.tracks[0].clips[1];
        assert_eq!(trimmed.repeat.unwrap().phase_frame, 25);
        for frame in 0..trimmed.length_frames {
            assert_eq!(
                trimmed.source_frame(frame),
                original.source_frame(frame + 20)
            );
        }
        // A still-wrapped loop cannot extend through its waveform edge.
        let clamped = project
            .trimmed_boundary(ids[0], &ids[..1], ClipEdge::Right, -10)
            .unwrap();
        assert_eq!(format!("{clamped:?}"), format!("{project:?}"));
        project.tracks[0].clips[1].repeat = Some(ClipLoop {
            length_frames: 50,
            phase_frame: 0,
        });
        let restored = project
            .trimmed_boundary(ids[0], &ids[..1], ClipEdge::Right, -20)
            .unwrap();
        assert_eq!(restored.tracks[0].clips[1].repeat, None);
        assert_eq!(restored.tracks[0].clips[1].source_offset_frame, 80);
    }

    #[test]
    fn other_selected_clips_trim_atomically_and_non_touching_edges_reject_linking() {
        let (mut project, ids) = fixture();
        let extra = Clip {
            id: Id::new_v4(),
            start_frame: 300,
            ..project.tracks[0].clips[0].clone()
        };
        project.tracks[0].clips.push(extra.clone());
        let before = format!("{project:?}");
        let next = project
            .trimmed_boundary(ids[0], &[ids[0], extra.id], ClipEdge::Right, -20)
            .unwrap();
        assert_eq!(next.tracks[0].clips[2].length_frames, 30);
        assert!(
            project
                .edit(Edit::TrimBoundary {
                    clip_id: ids[0],
                    selected: vec![ids[0], extra.id],
                    edge: ClipEdge::Right,
                    delta: -50
                })
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
        assert!(
            project
                .trimmed_boundary(extra.id, &[extra.id], ClipEdge::Left, 10)
                .is_err()
        );
        assert!(project.trim_neighbor(extra.id, ClipEdge::Left).is_none());
        assert!(
            project
                .trimmed_boundary(ids[0], &[ids[0], ids[0]], ClipEdge::Right, 10)
                .is_err()
        );
    }
}
