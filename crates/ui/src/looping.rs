use super::*;
use daw_core::ClipEdge;

impl DawUi {
    pub(super) fn loop_group_preview(
        &self,
        drag: &Drag,
        pointer: Pos2,
        unsnapped: bool,
    ) -> Vec<ClipPreview> {
        let left = drag.mode == ClipDragMode::LoopLeft;
        let edge = if left {
            ClipEdge::Left
        } else {
            ClipEdge::Right
        };
        let mut lower = i128::MIN;
        let mut upper = i128::MAX;
        for entry in &drag.clips {
            let clip = &entry.clip;
            let length = i128::from(clip.length_frames);
            let minimum = clip.repeat.map_or(length, |repeat| {
                length.min(i128::from(repeat.length_frames))
            });
            // Keep a common delta within every clip's minimum and timeline bounds.
            let (min, max) = if left {
                (-i128::from(clip.start_frame), length - minimum)
            } else {
                (
                    minimum - length,
                    i128::from(u64::MAX) - i128::from(clip.end()),
                )
            };
            lower = lower.max(min);
            upper = upper.min(max);
        }
        if drag.clips.is_empty() {
            return Vec::new();
        }
        let delta = i128::from(self.drag_delta(drag, pointer));
        let original = if left {
            drag.clip.start_frame
        } else {
            drag.clip.end()
        };
        let fixed = if left {
            drag.clip.end()
        } else {
            drag.clip.start_frame
        };
        let next = self.clip_edge_frame(
            drag,
            i128::from(original) + delta,
            (i128::from(original) + lower) as u64..=(i128::from(original) + upper) as u64,
            fixed,
            unsnapped || delta == 0,
        );
        let delta = i128::from(next) - i128::from(original);
        let previews: Vec<_> = drag
            .clips
            .iter()
            .map(|entry| ClipPreview {
                track: entry.track,
                track_index: entry.track_index,
                clip: entry.clip.looped_by(edge, delta),
                valid: true,
                removed: false,
            })
            .collect();
        previews
    }
}
