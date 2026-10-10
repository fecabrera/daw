use super::*;
use daw_core::ClipEdge;

const SHARED_BOUNDARY_RADIUS: f32 = 3.0;

impl DawUi {
    pub(super) fn at_trim_boundary(
        &self,
        clip: &Clip,
        mode: ClipDragMode,
        block: Rect,
        pointer: Pos2,
    ) -> bool {
        let edge = match mode {
            ClipDragMode::TrimLeft => ClipEdge::Left,
            ClipDragMode::TrimRight => ClipEdge::Right,
            _ => return false,
        };
        let x = match edge {
            ClipEdge::Left => block.left(),
            ClipEdge::Right => block.left() + seconds(clip.length_frames) as f32 * self.zoom,
        };
        // Leave the rest of the 8-point edge area available for ordinary trimming.
        (pointer.x - x).abs() <= SHARED_BOUNDARY_RADIUS
            && self.session.project.trim_neighbor(clip.id, edge).is_some()
    }

    pub(super) fn linked_trim_result(
        &self,
        drag: &Drag,
        pointer: Pos2,
        unsnapped: bool,
    ) -> Option<daw_core::Result<daw_core::Project>> {
        if !drag.linked_boundary {
            return None;
        }
        let edge = match drag.mode {
            ClipDragMode::TrimLeft => ClipEdge::Left,
            ClipDragMode::TrimRight => ClipEdge::Right,
            _ => return None,
        };
        let neighbor = self.session.project.trim_neighbor(drag.clip.id, edge)?;
        let delta = self.trim_delta(
            drag,
            pointer,
            unsnapped,
            vec![neighbor.start_frame, neighbor.end()],
        );
        let mut selected: Vec<_> = drag.clips.iter().map(|entry| entry.clip.id).collect();
        if selected.is_empty() {
            selected.push(drag.clip.id);
        }
        Some(
            self.session
                .project
                .trimmed_boundary(drag.clip.id, &selected, edge, delta),
        )
    }

    fn trim_delta(&self, drag: &Drag, pointer: Pos2, unsnapped: bool, anchors: Vec<u64>) -> i128 {
        let edge = drag.mode.resize_edge().expect("trim edge");
        let source_length = |clip: &Clip| {
            self.session
                .project
                .assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .map(|asset| asset.decoded_frame_count)
        };
        let mut delta = i128::from(self.drag_delta(drag, pointer));
        let original_edge = match edge {
            ClipEdge::Left => drag.clip.start_frame,
            ClipEdge::Right => drag.clip.end(),
        };
        let raw = i128::from(original_edge) + delta;
        if !unsnapped && delta != 0 && (0..=i128::from(u64::MAX)).contains(&raw) {
            let mut anchors = anchors;
            anchors.extend([drag.clip.start_frame, drag.clip.end()]);
            if let Some(source) = source_length(&drag.clip)
                && let Some(expanded) = drag.clip.trimmed_by(
                    edge,
                    match edge {
                        ClipEdge::Left => i128::MIN,
                        ClipEdge::Right => i128::MAX,
                    },
                    source,
                )
            {
                anchors.push(match edge {
                    ClipEdge::Left => expanded.start_frame,
                    ClipEdge::Right => expanded.end(),
                });
            }
            // Snap the requested edge once. One clip's source limit must not cap the group.
            delta = i128::from(self.snap_frame(raw, 0..=u64::MAX, anchors, false))
                - i128::from(original_edge);
        }
        delta
    }

    pub(super) fn trim_group_preview(
        &self,
        drag: &Drag,
        pointer: Pos2,
        unsnapped: bool,
    ) -> Vec<ClipPreview> {
        let edge = match drag.mode {
            ClipDragMode::TrimLeft => ClipEdge::Left,
            ClipDragMode::TrimRight => ClipEdge::Right,
            _ => return Vec::new(),
        };
        let source_length = |clip: &Clip| {
            self.session
                .project
                .assets
                .iter()
                .find(|asset| asset.id == clip.asset_id)
                .map(|asset| asset.decoded_frame_count)
        };
        let delta = self.trim_delta(drag, pointer, unsnapped, Vec::new());
        let mut valid = true;
        let mut previews = Vec::with_capacity(drag.clips.len());
        for entry in &drag.clips {
            let Some(source) = source_length(&entry.clip) else {
                return Vec::new();
            };
            let clip = entry
                .clip
                .trimmed_by(edge, delta, source)
                .unwrap_or_else(|| {
                    valid = false;
                    // Keep rejected previews drawable without producing an empty clip.
                    let inward = i128::from(entry.clip.length_frames.saturating_sub(1));
                    entry
                        .clip
                        .trimmed_by(
                            edge,
                            match edge {
                                ClipEdge::Left => inward,
                                ClipEdge::Right => -inward,
                            },
                            source,
                        )
                        .unwrap_or_else(|| entry.clip.clone())
                });
            previews.push(ClipPreview {
                track: entry.track,
                track_index: entry.track_index,
                clip,
                valid: true,
                removed: false,
            });
        }
        for preview in &mut previews {
            preview.valid = valid;
        }
        previews
    }
}
