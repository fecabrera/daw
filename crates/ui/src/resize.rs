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

    pub(super) fn edge_transaction(
        self,
        project: &daw_core::Project,
        clips: &[Clip],
    ) -> Option<daw_core::Result<daw_core::Project>> {
        if let Some(edge) = self.stretch_edge() {
            return Some(project.stretched_clips(clips, edge));
        }
        let edge = self.resize_edge()?;
        Some(if matches!(self, Self::TrimLeft | Self::TrimRight) {
            project.trimmed_clips(clips, edge)
        } else {
            project.resized_clips(clips, edge)
        })
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
    pub(super) fn update_clip_edge_mode(&mut self, ui: &egui::Ui) {
        if let Some(drag) = &mut self.drag
            && !drag.linked_boundary
            && let Some(mode) = drag.edge_mode
        {
            // Keep the press snapshot and pointer origin. Recompute each preview
            // from them, including modifier-only frames and the release frame.
            drag.mode = mode.with_stretch_modifier(ui.input(|i| clip_copy_modifier(i.modifiers)));
        }
    }

    pub(super) fn clip_trim_preview_project(&self, ui: &egui::Ui) -> Option<daw_core::Project> {
        let drag = self.drag.as_ref()?;
        if drag.linked_boundary
            || !matches!(drag.mode, ClipDragMode::TrimLeft | ClipDragMode::TrimRight)
        {
            return None;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos())?;
        let raw = self.raw_clip_drag_previews(drag, pointer, musical_time::snap_bypassed(ui));
        if raw.is_empty() || raw.iter().any(|p| !p.valid) {
            return None;
        }
        let clips: Vec<_> = raw.into_iter().map(|p| p.clip).collect();
        drag.mode
            .edge_transaction(&self.session.project, &clips)?
            .ok()
    }
    pub(super) fn resolve_resize_preview(
        &self,
        drag: &Drag,
        mut raw: Vec<ClipPreview>,
    ) -> Vec<ClipPreview> {
        if drag
            .mode
            .resize_edge()
            .or_else(|| drag.mode.stretch_edge())
            .is_none()
        {
            return raw;
        }
        if raw.is_empty() || raw.iter().any(|preview| !preview.valid) {
            return raw;
        }
        let clips: Vec<_> = raw.iter().map(|preview| preview.clip.clone()).collect();
        let result = drag
            .mode
            .edge_transaction(&self.session.project, &clips)
            .unwrap();
        let Ok(next) = result else {
            for preview in &mut raw {
                preview.valid = false;
            }
            return raw;
        };
        let requested: HashSet<_> = clips.iter().map(|clip| clip.id).collect();
        self.resize_result_previews(&next, &requested)
    }

    pub(super) fn resize_result_previews(
        &self,
        next: &daw_core::Project,
        requested: &HashSet<Id>,
    ) -> Vec<ClipPreview> {
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
        if drag.mode.resize_edge().is_none() {
            return;
        }
        if let Some(result) = self.linked_trim_result(drag, pointer, unsnapped) {
            match result {
                Ok(next) => self.publish_resize(next),
                Err(error) => self.fail(error),
            }
            return;
        }
        let raw = self.raw_clip_drag_previews(drag, pointer, unsnapped);
        if raw.is_empty() {
            return;
        }
        if raw.iter().any(|preview| !preview.valid) {
            self.fail("Cannot trim selected clips: the amount would remove a resized clip.");
            return;
        }
        let clips: Vec<_> = raw.iter().map(|preview| preview.clip.clone()).collect();
        let result = drag
            .mode
            .edge_transaction(&self.session.project, &clips)
            .unwrap();
        match result {
            Ok(next) => self.publish_resize(next),
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn publish_resize(&mut self, next: daw_core::Project) {
        let changed = self
            .session
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
                            .is_none_or(|other| {
                                range(other) != range(clip) || other.asset_id != clip.asset_id
                            })
                    })
            });
        if changed {
            if self.prepare_audio_edit(&next, None) {
                return;
            }
            self.session.project = next;
            self.prune_clip_selection();
            self.changed();
        }
    }
}
