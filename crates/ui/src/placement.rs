use super::*;
use daw_core::ClipPlacement;

impl DawUi {
    fn move_project(&self, previews: &[ClipPreview]) -> daw_core::Result<daw_core::Project> {
        let mut next = self.session.project.clone();
        let mut clips = Vec::with_capacity(previews.len());
        for preview in previews {
            while next.tracks.len() <= preview.track_index {
                next.add_track()?;
            }
            clips.push(ClipPlacement {
                track_id: next.tracks[preview.track_index].id,
                clip: preview.clip.clone(),
            });
        }
        next.placed_clips(&clips)
    }

    pub(super) fn resolve_move_preview(&self, mut raw: Vec<ClipPreview>) -> Vec<ClipPreview> {
        if raw.is_empty() {
            return raw;
        }
        match self.move_project(&raw) {
            Ok(next) => {
                let ids = raw.iter().map(|preview| preview.clip.id).collect();
                raw.extend(self.placement_neighbors(&next, &ids));
            }
            Err(_) => raw.iter_mut().for_each(|preview| preview.valid = false),
        }
        raw
    }

    /// Include changed originals and all new fragments; incoming clips are drawn separately.
    pub(super) fn placement_neighbors(
        &self,
        next: &daw_core::Project,
        incoming: &HashSet<Id>,
    ) -> Vec<ClipPreview> {
        let mut previews = Vec::new();
        for (index, track) in self.session.project.tracks.iter().enumerate() {
            for original in &track.clips {
                if incoming.contains(&original.id) {
                    continue;
                }
                let result = next.tracks[index]
                    .clips
                    .iter()
                    .find(|clip| clip.id == original.id);
                if result.is_none_or(|clip| {
                    (clip.start_frame, clip.length_frames)
                        != (original.start_frame, original.length_frames)
                }) {
                    previews.push(ClipPreview {
                        track: track.id,
                        track_index: index,
                        clip: result.unwrap_or(original).clone(),
                        valid: true,
                        removed: result.is_none(),
                    });
                }
            }
            for clip in &next.tracks[index].clips {
                if !incoming.contains(&clip.id)
                    && !track.clips.iter().any(|original| original.id == clip.id)
                {
                    previews.push(ClipPreview {
                        track: track.id,
                        track_index: index,
                        clip: clip.clone(),
                        valid: true,
                        removed: false,
                    });
                }
            }
        }
        previews
    }

    pub(super) fn mask_neighbor_previews(
        &self,
        ui: &egui::Ui,
        viewport: Rect,
        previews: &[ClipPreview],
    ) {
        for preview in previews {
            if let Some(lane) = self.preview_lane(preview)
                && let Some(original) = self.session.project.tracks[preview.track_index]
                    .clips
                    .iter()
                    .find(|clip| clip.id == preview.clip.id)
            {
                let block = self.clip_block(lane, original, original.start_frame);
                let painter = ui.painter_at(viewport.intersect(lane).intersect(block));
                painter.rect_filled(block, 0.0, theme::palette(ui.ctx()).background);
                self.paint_grid(&painter, lane);
                painter.line_segment(
                    [lane.left_bottom(), lane.right_bottom()],
                    Stroke::new(1.0, theme::palette(ui.ctx()).border),
                );
            }
        }
    }

    pub(super) fn finish_clip_move(&mut self, drag: &Drag, mut previews: Vec<ClipPreview>) {
        if previews.is_empty() {
            return;
        }
        let primary = previews
            .iter()
            .position(|preview| preview.clip.id == drag.clip.id)
            .unwrap_or(0);
        if drag.duplicate {
            let mut next = self.session.project.clone();
            for preview in &mut previews {
                while next.tracks.len() <= preview.track_index {
                    if let Err(error) = next.add_track() {
                        self.fail(error);
                        return;
                    }
                }
                preview.track = next.tracks[preview.track_index].id;
            }
            let clips = previews
                .into_iter()
                .map(|preview| SelectedClip {
                    track: preview.track,
                    track_index: preview.track_index,
                    clip: preview.clip,
                })
                .collect();
            self.insert_clips(next, clips, primary);
            return;
        }
        if previews.iter().all(|preview| {
            self.session.project.tracks.iter().any(|track| {
                track.id == preview.track
                    && track.clips.iter().any(|clip| {
                        clip.id == preview.clip.id && clip.start_frame == preview.clip.start_frame
                    })
            })
        }) {
            return;
        }
        match self.move_project(&previews) {
            Ok(next) => {
                self.selected_track = Some(next.tracks[previews[primary].track_index].id);
                self.selected_clip = Some(drag.clip.id);
                self.selected_clips = previews.iter().map(|preview| preview.clip.id).collect();
                self.session.project = next;
                self.prune_clip_selection();
                self.changed();
            }
            Err(error) => self.fail(error),
        }
    }
}
