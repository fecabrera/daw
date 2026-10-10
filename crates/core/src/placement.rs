use crate::{Clip, Edit, Error, Id, Project, Result};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct ClipPlacement {
    pub track_id: Id,
    pub clip: Clip,
}

impl Project {
    /// Moves retain IDs; new placements use new IDs. Incoming clips win overlaps.
    pub fn placed_clips(&self, clips: &[ClipPlacement]) -> Result<Self> {
        let mut next = self.clone();
        next.edit(Edit::OverwriteClips(clips.to_vec()))?;
        Ok(next)
    }

    pub(crate) fn apply_placements(&mut self, clips: &[ClipPlacement]) -> Result<()> {
        let mut incoming = HashSet::new();
        for placement in clips {
            if !incoming.insert(placement.clip.id) {
                return Err(Error("Duplicate placed clip ID".into()));
            }
            if !self
                .tracks
                .iter()
                .any(|track| track.id == placement.track_id)
            {
                return Err(Error("Track not found".into()));
            }
            self.validate_clip(&placement.clip)?;
        }
        // Remove the full moving group first, so original ranges cannot trim each other.
        for track in &mut self.tracks {
            track.clips.retain(|clip| !incoming.contains(&clip.id));
            let mut winners: Vec<_> = clips
                .iter()
                .filter(|placement| placement.track_id == track.id)
                .collect();
            winners.sort_by_key(|placement| (placement.clip.start_frame, placement.clip.id));
            let mut survivors = Vec::new();
            for original in &track.clips {
                let mut pieces = vec![original.clone()];
                for winner in &winners {
                    let start = winner.clip.start_frame;
                    let end = winner.clip.end();
                    pieces = pieces
                        .into_iter()
                        .flat_map(|piece| piece.uncovered_ranges(start, end))
                        .collect();
                }
                survivors.extend(pieces);
            }
            survivors.extend(winners.into_iter().map(|placement| placement.clip.clone()));
            survivors.sort_by_key(|clip| (clip.start_frame, clip.id));
            track.clips = survivors;
        }
        // Final validation rejects overlaps within the incoming group atomically.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Asset, ClipLoop, ClipStretch, SAMPLE_RATE, Source, SourceMetadata};

    fn fixture() -> (Project, Clip, Clip) {
        let mut project = Project::default();
        project.add_track().unwrap();
        project.add_track().unwrap();
        let asset_id = Id::new_v4();
        project.assets.push(Asset {
            id: asset_id,
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
        let neighbor = Clip {
            id: Id::new_v4(),
            asset_id,
            name: "Neighbor".into(),
            color: Some(crate::DEFAULT_TRACK_COLOR),
            start_frame: 100,
            source_offset_frame: 40,
            length_frames: 400,
            repeat: Some(ClipLoop {
                length_frames: 70,
                phase_frame: 13,
            }),
            stretch: ClipStretch::new(1, 2),
        };
        let moving = Clip {
            id: Id::new_v4(),
            name: "Moving".into(),
            start_frame: 0,
            length_frames: 100,
            repeat: None,
            stretch: None,
            ..neighbor.clone()
        };
        project.tracks[0].clips.push(neighbor.clone());
        project.tracks[1].clips.push(moving.clone());
        project.validate().unwrap();
        (project, neighbor, moving)
    }

    #[test]
    fn middle_move_splits_looped_stretched_neighbor_without_changing_remaining_samples() {
        let (mut project, neighbor, mut moving) = fixture();
        moving.start_frame = 200;
        let before = format!("{project:?}");
        let placements = vec![ClipPlacement {
            track_id: project.tracks[0].id,
            clip: moving.clone(),
        }];
        let preview = project.placed_clips(&placements).unwrap();
        assert_eq!(format!("{project:?}"), before);
        project.edit(Edit::OverwriteClips(placements)).unwrap();
        assert!(project.tracks[1].clips.is_empty());
        assert_eq!(project.tracks[0].clips.len(), 3);
        for result in [project, preview] {
            let clips = &result.tracks[0].clips;
            assert_eq!(clips[0].id, neighbor.id);
            assert_eq!(clips[1].id, moving.id);
            assert_ne!(clips[2].id, neighbor.id);
            assert_eq!((clips[0].start_frame, clips[0].end()), (100, 200));
            assert_eq!((clips[2].start_frame, clips[2].end()), (300, 500));
            for retained in [&clips[0], &clips[2]] {
                assert_eq!(retained.stretch, neighbor.stretch);
                assert_eq!(retained.color, neighbor.color);
                assert_eq!(retained.asset_id, neighbor.asset_id);
                for local in 0..retained.length_frames {
                    assert_eq!(
                        retained.source_frame(local),
                        neighbor.source_frame(local + retained.start_frame - neighbor.start_frame)
                    );
                }
            }
            result.validate().unwrap();
        }
    }

    #[test]
    fn placements_trim_either_edge_remove_full_coverage_and_allow_touching_boundaries() {
        for (start, length, expected) in [
            (50, 100, Some((150, 500))),
            (450, 100, Some((100, 450))),
            (50, 500, None),
            (0, 100, Some((100, 500))),
        ] {
            let (mut project, neighbor, mut moving) = fixture();
            moving.start_frame = start;
            moving.length_frames = length;
            project
                .edit(Edit::OverwriteClips(vec![ClipPlacement {
                    track_id: project.tracks[0].id,
                    clip: moving,
                }]))
                .unwrap();
            let retained = project.tracks[0]
                .clips
                .iter()
                .find(|clip| clip.id == neighbor.id);
            assert_eq!(
                retained.map(|clip| (clip.start_frame, clip.end())),
                expected
            );
            if let Some(retained) = retained {
                for local in 0..retained.length_frames {
                    assert_eq!(
                        retained.source_frame(local),
                        neighbor.source_frame(local + retained.start_frame - neighbor.start_frame)
                    );
                }
            }
        }
    }

    #[test]
    fn moving_group_preserves_its_gaps_and_splits_one_neighbor_multiple_times() {
        let (mut project, neighbor, mut moving) = fixture();
        moving.start_frame = 200;
        moving.length_frames = 50;
        let second = Clip {
            id: Id::new_v4(),
            start_frame: 350,
            ..moving.clone()
        };
        project.tracks[1].clips.push(Clip {
            start_frame: 150,
            ..second.clone()
        });
        let placements = vec![
            ClipPlacement {
                track_id: project.tracks[0].id,
                clip: second,
            },
            ClipPlacement {
                track_id: project.tracks[0].id,
                clip: moving,
            },
        ];
        project.edit(Edit::OverwriteClips(placements)).unwrap();
        let retained: Vec<_> = project.tracks[0]
            .clips
            .iter()
            .filter(|clip| clip.name == neighbor.name)
            .collect();
        assert_eq!(
            retained
                .iter()
                .map(|clip| (clip.start_frame, clip.end()))
                .collect::<Vec<_>>(),
            vec![(100, 200), (250, 350), (400, 500)]
        );
        assert_eq!(retained[0].id, neighbor.id);
        for clip in retained {
            for local in 0..clip.length_frames {
                assert_eq!(
                    clip.source_frame(local),
                    neighbor.source_frame(local + clip.start_frame - neighbor.start_frame)
                );
            }
        }
        assert!(project.tracks[1].clips.is_empty());
    }

    #[test]
    fn invalid_incoming_group_rolls_back_moves_splits_trims_and_removals() {
        let (mut project, _, mut moving) = fixture();
        moving.start_frame = 200;
        let other = Clip {
            id: Id::new_v4(),
            start_frame: 250,
            ..moving.clone()
        };
        let before = format!("{project:?}");
        let placements = vec![
            ClipPlacement {
                track_id: project.tracks[0].id,
                clip: moving.clone(),
            },
            ClipPlacement {
                track_id: project.tracks[0].id,
                clip: other,
            },
        ];
        assert!(project.edit(Edit::OverwriteClips(placements)).is_err());
        assert_eq!(format!("{project:?}"), before);
        assert!(
            project
                .placed_clips(&[ClipPlacement {
                    track_id: Id::new_v4(),
                    clip: moving
                }])
                .is_err()
        );
        assert_eq!(format!("{project:?}"), before);
    }
}
