use crate::{Clip, ClipEdge, Error, Id, Project, Result};
use std::collections::HashSet;

impl Clip {
    /// Keep a nonempty subrange without changing the remaining samples' mapping.
    fn retained_range(&self, start: u64, end: u64) -> Self {
        let advance = start - self.start_frame;
        let mut clip = self.clone();
        clip.start_frame = start;
        clip.length_frames = end - start;
        if let Some(repeat) = clip.repeat {
            let repeat = repeat.shifted(i128::from(advance));
            // A full copy with nonzero phase still wraps: do not reset its phase.
            if clip.length_frames <= repeat.length_frames - repeat.phase_frame {
                clip.source_offset_frame += repeat.phase_frame;
                clip.repeat = None;
            } else {
                clip.repeat = Some(repeat);
            }
        } else {
            clip.source_offset_frame += advance;
        }
        clip
    }
}

impl Project {
    /// Calculate the complete resize result for preview and release.
    pub fn resized_clips(&self, clips: &[Clip], edge: ClipEdge) -> Result<Self> {
        let mut next = self.clone();
        next.apply_resizes(clips, edge)?;
        next.validate()?;
        Ok(next)
    }

    pub(crate) fn apply_resizes(&mut self, clips: &[Clip], edge: ClipEdge) -> Result<()> {
        let mut ids = HashSet::new();
        let mut order = Vec::new();
        for candidate in clips {
            if !ids.insert(candidate.id) {
                return Err(Error("Duplicate resized clip ID".into()));
            }
            let original = self
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .find(|clip| clip.id == candidate.id)
                .ok_or_else(|| Error("Clip not found".into()))?;
            let mut clip = original.clone();
            clip.start_frame = candidate.start_frame;
            clip.source_offset_frame = candidate.source_offset_frame;
            clip.length_frames = candidate.length_frames;
            clip.repeat = candidate.repeat;
            self.validate_clip(&clip)?;
            if match edge {
                ClipEdge::Left => clip.end() != original.end(),
                ClipEdge::Right => clip.start_frame != original.start_frame,
            } {
                return Err(Error("Resize must keep the opposite endpoint fixed".into()));
            }
            order.push((original.start_frame, candidate.id, clip));
        }
        order.sort_by_key(|(start, id, _)| (*start, *id));
        if edge == ClipEdge::Left {
            order.reverse();
        }
        for (_, id, candidate) in &order {
            let clip = self
                .tracks
                .iter_mut()
                .flat_map(|track| &mut track.clips)
                .find(|clip| clip.id == *id)
                .ok_or_else(|| Error("Clip not found".into()))?;
            *clip = candidate.clone();
        }
        let mut processed = HashSet::<Id>::new();
        for (_, id, _) in order {
            let Some(track) = self
                .tracks
                .iter_mut()
                .find(|track| track.clips.iter().any(|clip| clip.id == id))
            else {
                continue;
            };
            let winner = track
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .cloned()
                .ok_or_else(|| Error("Clip not found".into()))?;
            processed.insert(id);
            track.clips.retain_mut(|clip| {
                if processed.contains(&clip.id)
                    || clip.end() <= winner.start_frame
                    || clip.start_frame >= winner.end()
                {
                    return true;
                }
                let (start, end) = match edge {
                    ClipEdge::Right => (winner.end(), clip.end()),
                    ClipEdge::Left => (clip.start_frame, winner.start_frame),
                };
                if end <= start {
                    return false;
                }
                *clip = clip.retained_range(start, end);
                true
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Asset, ClipLoop, ClipStretch, Edit, SAMPLE_RATE, Source, SourceMetadata};

    fn fixture() -> Project {
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
        for (start, length, offset) in [
            (100, 50, 20),
            (170, 60, 100),
            (250, 20, 300),
            (300, 100, 400),
        ] {
            project.tracks[0].clips.push(Clip {
                id: Id::new_v4(),
                asset_id: asset,
                name: format!("Clip {start}"),
                color: None,
                start_frame: start,
                source_offset_frame: offset,
                length_frames: length,
                repeat: None,
                stretch: None,
            });
        }
        project.validate().unwrap();
        project
    }

    #[test]
    fn right_expansion_removes_covered_neighbors_and_preserves_wrapped_stretched_audio() {
        let mut project = fixture();
        let neighbor = &mut project.tracks[0].clips[3];
        neighbor.repeat = Some(ClipLoop {
            length_frames: 40,
            phase_frame: 10,
        });
        neighbor.stretch = ClipStretch::new(1, 2);
        let originals = project.tracks[0].clips.clone();
        let expanded = originals[0].looped_by(ClipEdge::Right, 210);
        let preview = project
            .resized_clips(std::slice::from_ref(&expanded), ClipEdge::Right)
            .unwrap();
        project
            .edit(Edit::ResizeClips {
                clips: vec![expanded],
                edge: ClipEdge::Right,
            })
            .unwrap();
        assert_eq!(format!("{project:?}"), format!("{preview:?}"));
        assert_eq!(project.tracks[0].clips.len(), 2);
        let trimmed = &project.tracks[0].clips[1];
        assert_eq!((trimmed.start_frame, trimmed.length_frames), (360, 40));
        assert_eq!(trimmed.repeat.unwrap().phase_frame, 30);
        assert_eq!(trimmed.stretch, originals[3].stretch);
        assert_eq!(trimmed.id, originals[3].id);
        assert_eq!(trimmed.asset_id, originals[3].asset_id);
        for local in 0..40 {
            assert_eq!(
                trimmed.source_frame(local),
                originals[3].source_frame(local + 60)
            );
        }
        assert_eq!(project.assets[0].decoded_frame_count, 1000);
    }

    #[test]
    fn left_expansion_trims_neighbor_end_and_removes_fully_covered_clips() {
        let mut project = fixture();
        let template = project.tracks[0].clips[0].clone();
        let neighbor = Clip {
            id: Id::new_v4(),
            start_frame: 20,
            length_frames: 60,
            source_offset_frame: 100,
            repeat: Some(ClipLoop {
                length_frames: 30,
                phase_frame: 5,
            }),
            ..template.clone()
        };
        let covered = Clip {
            id: Id::new_v4(),
            start_frame: 90,
            length_frames: 5,
            ..template.clone()
        };
        project.tracks[0]
            .clips
            .extend([neighbor.clone(), covered.clone()]);
        let expanded = template.looped_by(ClipEdge::Left, -60);
        project
            .edit(Edit::ResizeClips {
                clips: vec![expanded],
                edge: ClipEdge::Left,
            })
            .unwrap();
        assert!(
            !project.tracks[0]
                .clips
                .iter()
                .any(|clip| clip.id == covered.id)
        );
        let remaining = project.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == neighbor.id)
            .unwrap();
        assert_eq!((remaining.start_frame, remaining.length_frames), (20, 20));
        for local in 0..20 {
            assert_eq!(remaining.source_frame(local), neighbor.source_frame(local));
        }
        project.validate().unwrap();
    }

    #[test]
    fn selected_clips_trim_each_other_in_direction_order_independent_of_payload_order() {
        for edge in [ClipEdge::Left, ClipEdge::Right] {
            let project = fixture();
            let originals = &project.tracks[0].clips;
            let delta = if edge == ClipEdge::Left { -80 } else { 70 };
            let mut candidates = vec![
                originals[0].looped_by(edge, delta),
                originals[1].looped_by(edge, delta),
            ];
            let preview = project.resized_clips(&candidates, edge).unwrap();
            candidates.reverse();
            let reversed = project.resized_clips(&candidates, edge).unwrap();
            assert_eq!(format!("{preview:?}"), format!("{reversed:?}"));
            let victim_index = if edge == ClipEdge::Left { 0 } else { 1 };
            let victim = preview.tracks[0]
                .clips
                .iter()
                .find(|clip| clip.id == originals[victim_index].id)
                .unwrap();
            let proposed = candidates.iter().find(|clip| clip.id == victim.id).unwrap();
            assert!(victim.length_frames < proposed.length_frames);
            for local in 0..victim.length_frames {
                assert_eq!(
                    victim.source_frame(local),
                    proposed.source_frame(local + victim.start_frame - proposed.start_frame)
                );
            }
            preview.validate().unwrap();
        }
    }

    #[test]
    fn failed_neighbor_resize_is_atomic_and_no_op_keeps_every_clip() {
        let mut project = fixture();
        let before = format!("{project:?}");
        let clip = project.tracks[0].clips[0].clone();
        project
            .edit(Edit::ResizeClips {
                clips: vec![clip.clone()],
                edge: ClipEdge::Right,
            })
            .unwrap();
        assert_eq!(format!("{project:?}"), before);
        let expanded = clip.looped_by(ClipEdge::Right, 400);
        let mut invalid = project.tracks[0].clips[1].clone();
        invalid.length_frames = 0;
        assert!(
            project
                .edit(Edit::ResizeClips {
                    clips: vec![expanded, invalid],
                    edge: ClipEdge::Right
                })
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
        assert!(
            project
                .resized_clips(&[clip.clone(), clip.clone()], ClipEdge::Right)
                .is_err()
        );
        let moved = Clip {
            start_frame: clip.start_frame + 1,
            ..clip
        };
        assert!(
            project
                .edit(Edit::ResizeClips {
                    clips: vec![moved],
                    edge: ClipEdge::Right
                })
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
    }
}
