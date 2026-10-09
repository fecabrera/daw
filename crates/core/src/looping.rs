use crate::{Clip, ClipEdge, ClipLoop};

impl Clip {
    /// Resize a validated clip's repeat range, preserving the opposite endpoint.
    /// The first resize uses the current trimmed length as the repeat base.
    pub fn looped_by(&self, edge: ClipEdge, delta: i128) -> Self {
        let start = i128::from(self.start_frame);
        let length = i128::from(self.length_frames);
        let minimum = self.repeat.map_or(length, |repeat| {
            length.min(i128::from(repeat.length_frames))
        });
        let effective = match edge {
            ClipEdge::Left => delta.clamp(-start, length - minimum),
            ClipEdge::Right => delta.clamp(minimum - length, i128::from(u64::MAX) - start - length),
        };
        if effective == 0 {
            return self.clone();
        }
        let repeat = self.repeat.unwrap_or(ClipLoop {
            length_frames: self.length_frames,
            phase_frame: 0,
        });
        let mut clip = self.clone();
        match edge {
            ClipEdge::Left => {
                clip.start_frame = (start + effective) as u64;
                clip.length_frames = (length - effective) as u64;
                clip.repeat = Some(repeat.shifted(effective));
            }
            ClipEdge::Right => {
                clip.length_frames = (length + effective) as u64;
                clip.repeat = Some(repeat);
            }
        }
        clip.restore_source_range();
        clip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipStretch, Id};

    #[test]
    fn looping_uses_each_trimmed_base_preserves_phase_and_restores_one_copy() {
        let original = Clip {
            id: Id::new_v4(),
            asset_id: Id::new_v4(),
            name: "Trimmed".into(),
            color: None,
            start_frame: 100,
            source_offset_frame: 20,
            length_frames: 30,
            repeat: None,
            stretch: ClipStretch::new(1, 2),
        };
        for edge in [ClipEdge::Left, ClipEdge::Right] {
            let delta = if edge == ClipEdge::Left { -40 } else { 40 };
            let expanded = original.looped_by(edge, delta);
            assert_eq!(expanded.length_frames, 70);
            assert_eq!(expanded.repeat.unwrap().length_frames, 30);
            assert_eq!(expanded.source_offset_frame, original.source_offset_frame);
            assert_eq!(expanded.stretch, original.stretch);
            assert_eq!(expanded.id, original.id);
            for frame in 0..original.length_frames {
                assert_eq!(
                    expanded.source_frame(frame + if edge == ClipEdge::Left { 40 } else { 0 }),
                    original.source_frame(frame)
                );
            }
            let restored = expanded.looped_by(edge, -delta);
            assert_eq!(format!("{restored:?}"), format!("{original:?}"));
        }
        let mut shortened = original.clone();
        shortened.repeat = Some(ClipLoop {
            length_frames: 40,
            phase_frame: 25,
        });
        for (edge, delta) in [(ClipEdge::Left, 20), (ClipEdge::Right, -20)] {
            assert_eq!(
                format!("{:?}", shortened.looped_by(edge, delta)),
                format!("{shortened:?}")
            );
        }
        assert_eq!(original.looped_by(ClipEdge::Left, i128::MIN).start_frame, 0);
        assert_eq!(
            original.looped_by(ClipEdge::Right, i128::MAX).end(),
            u64::MAX
        );
    }
}
