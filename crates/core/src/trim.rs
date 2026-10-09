use crate::Clip;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipEdge {
    Left,
    Right,
}

impl Clip {
    /// Move one edge of a validated clip by sample frames. Reject empty ranges;
    /// clamp expansion per clip.
    /// Source bounds use the saved decoded length, converted to stretched coordinates.
    pub fn trimmed_by(&self, edge: ClipEdge, delta: i128, original_frames: u64) -> Option<Self> {
        if delta == 0 {
            return Some(self.clone());
        }
        let mut clip = self.clone();
        clip.restore_source_range();
        let length = i128::from(clip.length_frames);
        let inward = match edge {
            ClipEdge::Left => delta,
            ClipEdge::Right => delta.checked_neg()?,
        };
        if inward >= length {
            return None;
        }
        let start = i128::from(clip.start_frame);
        let offset = i128::from(clip.source_offset_frame);
        let effective = match edge {
            ClipEdge::Left => {
                let minimum = if clip.repeat.is_some() {
                    0
                } else {
                    -offset.min(start)
                };
                let effective = delta.max(minimum);
                clip.start_frame = (start + effective) as u64;
                clip.length_frames = (length - effective) as u64;
                if let Some(repeat) = clip.repeat {
                    clip.repeat = Some(repeat.shifted(effective));
                } else {
                    clip.source_offset_frame = (offset + effective) as u64;
                }
                effective
            }
            ClipEdge::Right => {
                let maximum = if clip.repeat.is_some() {
                    0
                } else {
                    let source = i128::from(clip.source_length(original_frames)?);
                    (source - offset - length).min(i128::from(u64::MAX) - start - length)
                };
                if maximum < 0 {
                    return None;
                }
                let effective = delta.min(maximum);
                clip.length_frames = (length + effective) as u64;
                effective
            }
        };
        if effective == 0 {
            return Some(self.clone());
        }
        clip.restore_source_range();
        Some(clip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipLoop, ClipStretch, Id};

    fn clip() -> Clip {
        Clip {
            id: Id::new_v4(),
            asset_id: Id::new_v4(),
            name: "Trimmed".into(),
            color: None,
            start_frame: 100,
            source_offset_frame: 20,
            length_frames: 40,
            repeat: None,
            stretch: None,
        }
    }

    #[test]
    fn trim_preserves_opposite_edge_and_clamps_source_and_timeline_bounds() {
        let original = clip();
        let left = original.trimmed_by(ClipEdge::Left, 10, 100).unwrap();
        assert_eq!(
            (
                left.start_frame,
                left.source_offset_frame,
                left.length_frames
            ),
            (110, 30, 30)
        );
        assert_eq!(left.end(), original.end());
        let left = original.trimmed_by(ClipEdge::Left, i128::MIN, 100).unwrap();
        assert_eq!(
            (
                left.start_frame,
                left.source_offset_frame,
                left.length_frames
            ),
            (80, 0, 60)
        );
        let right = original
            .trimmed_by(ClipEdge::Right, i128::MAX, 100)
            .unwrap();
        assert_eq!(
            (
                right.start_frame,
                right.source_offset_frame,
                right.length_frames
            ),
            (100, 20, 80)
        );
        assert_eq!(right.id, original.id);
        assert_eq!(right.asset_id, original.asset_id);
        for amount in [40, 41, i64::MAX as i128] {
            assert!(original.trimmed_by(ClipEdge::Left, amount, 100).is_none());
            assert!(original.trimmed_by(ClipEdge::Right, -amount, 100).is_none());
        }
        let mut near_start = original.clone();
        near_start.start_frame = 5;
        assert_eq!(
            near_start
                .trimmed_by(ClipEdge::Left, -30, 100)
                .unwrap()
                .source_offset_frame,
            15
        );
        let mut near_end = original.clone();
        near_end.start_frame = u64::MAX - 45;
        assert_eq!(
            near_end.trimmed_by(ClipEdge::Right, 30, 100).unwrap().end(),
            u64::MAX
        );
        let mut stretched = original.clone();
        stretched.stretch = ClipStretch::new(1, 2);
        let expanded = stretched.trimmed_by(ClipEdge::Right, 200, 100).unwrap();
        assert_eq!(expanded.length_frames, 180);
        assert_eq!(expanded.stretch, stretched.stretch);
    }

    #[test]
    fn group_trim_geometry_preserves_loop_phase_and_restores_source_at_one_copy() {
        let mut original = clip();
        original.repeat = Some(ClipLoop {
            length_frames: 15,
            phase_frame: 3,
        });
        let left = original.trimmed_by(ClipEdge::Left, 10, 100).unwrap();
        assert_eq!(left.repeat.unwrap().phase_frame, 13);
        for local in 0..left.length_frames {
            assert_eq!(left.source_frame(local), original.source_frame(local + 10));
        }
        let right = original.trimmed_by(ClipEdge::Right, -25, 100).unwrap();
        assert_eq!(right.repeat, None);
        assert_eq!(right.source_offset_frame, 20);
        assert_eq!(
            right
                .trimmed_by(ClipEdge::Right, 500, 100)
                .unwrap()
                .length_frames,
            80
        );
        for (edge, delta) in [(ClipEdge::Left, -50), (ClipEdge::Right, 50)] {
            assert_eq!(
                format!("{:?}", original.trimmed_by(edge, delta, 100).unwrap()),
                format!("{original:?}")
            );
        }
        let mut contiguous = original.clone();
        contiguous.length_frames = 10;
        let restored = contiguous.trimmed_by(ClipEdge::Left, -10, 100).unwrap();
        assert_eq!(restored.repeat, None);
        assert_eq!(restored.source_offset_frame, 13);
        assert_eq!(restored.length_frames, 20);
    }
}
