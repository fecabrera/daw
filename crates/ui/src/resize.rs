use super::*;
use daw_core::ClipEdge;

impl ClipDragMode {
    pub(super) fn stretch_edge(self) -> Option<ClipEdge> {
        match self {
            Self::StretchLeft => Some(ClipEdge::Left),
            Self::StretchRight => Some(ClipEdge::Right),
            _ => None,
        }
    }

    pub(super) fn resize_edge(self) -> Option<ClipEdge> {
        match self {
            Self::TrimLeft | Self::LoopLeft => Some(ClipEdge::Left),
            Self::TrimRight | Self::LoopRight => Some(ClipEdge::Right),
            _ => None,
        }
    }
}

fn range(clip: &Clip) -> (u64, u64, u64, Option<daw_core::ClipLoop>) {
    (
        clip.start_frame,
        clip.source_offset_frame,
        clip.length_frames,
        clip.repeat,
    )
}

impl DawUi {
    pub(super) fn resolve_resize_preview(
        &self,
        drag: &Drag,
        mut raw: Vec<ClipPreview>,
    ) -> Vec<ClipPreview> {
        let Some(edge) = drag.mode.resize_edge().or_else(|| drag.mode.stretch_edge()) else {
            return raw;
        };
        if raw.is_empty() || raw.iter().any(|preview| !preview.valid) {
            return raw;
        }
        let clips: Vec<_> = raw.iter().map(|preview| preview.clip.clone()).collect();
        let result = if drag.mode.stretch_edge().is_some() {
            self.session.project.stretched_clips(&clips, edge)
        } else {
            self.session.project.resized_clips(&clips, edge)
        };
        let Ok(next) = result else {
            for preview in &mut raw {
                preview.valid = false;
            }
            return raw;
        };
        let requested: HashSet<_> = clips.iter().map(|clip| clip.id).collect();
        let mut previews = Vec::new();
        for (track_index, track) in self.session.project.tracks.iter().enumerate() {
            let mut originals: Vec<_> = track.clips.iter().collect();
            originals.sort_by_key(|clip| clip.start_frame);
            for original in originals {
                let result = next.tracks[track_index]
                    .clips
                    .iter()
                    .find(|clip| clip.id == original.id);
                if requested.contains(&original.id)
                    || result.is_none_or(|clip| range(clip) != range(original))
                {
                    previews.push(ClipPreview {
                        track: track.id,
                        track_index,
                        clip: result.unwrap_or(original).clone(),
                        removed: result.is_none(),
                        valid: true,
                    });
                }
            }
        }
        previews
    }

    pub(super) fn finish_clip_resize(&mut self, drag: &Drag, pointer: Pos2, unsnapped: bool) {
        let Some(edge) = drag.mode.resize_edge() else {
            return;
        };
        let raw = self.raw_clip_drag_previews(drag, pointer, unsnapped);
        if raw.is_empty() {
            return;
        }
        if raw.iter().any(|preview| !preview.valid) {
            self.fail("Cannot trim selected clips: the amount would remove a resized clip.");
            return;
        }
        let clips: Vec<_> = raw.iter().map(|preview| preview.clip.clone()).collect();
        match self.session.project.resized_clips(&clips, edge) {
            Ok(next) => {
                let changed =
                    self.session
                        .project
                        .tracks
                        .iter()
                        .zip(&next.tracks)
                        .any(|(old, new)| {
                            old.clips.len() != new.clips.len()
                                || old.clips.iter().any(|clip| {
                                    new.clips
                                        .iter()
                                        .find(|other| other.id == clip.id)
                                        .is_none_or(|other| range(other) != range(clip))
                                })
                        });
                if changed {
                    self.session.project = next;
                    self.prune_clip_selection();
                    self.changed();
                }
            }
            Err(error) => self.fail(error),
        }
    }
}
