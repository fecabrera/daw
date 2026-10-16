use super::*;
use daw_core::ClipPlacement;

impl DawUi {
    pub(super) fn add_placement_track(
        &self,
        project: &mut daw_core::Project,
        clip: &Clip,
    ) -> daw_core::Result<Id> {
        let midi = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .is_some_and(|a| a.midi_pattern.is_some());
        if midi {
            let instrument = self
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.clips.iter().any(|c| c.id == clip.id))
                .or_else(|| {
                    self.session
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.clips.iter().any(|c| c.asset_id == clip.asset_id))
                })
                .and_then(|t| t.instrument.clone());
            let id = project.add_midi_track()?;
            project.tracks.last_mut().unwrap().instrument = instrument;
            Ok(id)
        } else {
            project.add_track()
        }
    }

    pub(super) fn prepare_placement_tracks(
        &self,
        project: &mut daw_core::Project,
        placements: &[(usize, &Clip)],
    ) -> daw_core::Result<()> {
        let Some(last) = placements.iter().map(|(index, _)| *index).max() else {
            return Ok(());
        };
        while project.tracks.len() <= last {
            let index = project.tracks.len();
            if let Some((_, clip)) = placements.iter().find(|(target, _)| *target == index) {
                self.add_placement_track(project, clip)?;
            } else {
                project.add_track()?;
            }
        }
        Ok(())
    }

    fn move_project(
        &self,
        previews: &[ClipPreview],
        originals: &[ClipPreview],
    ) -> daw_core::Result<daw_core::Project> {
        let mut next = self.session.project.clone();
        self.prepare_placement_tracks(
            &mut next,
            &originals
                .iter()
                .map(|p| (p.track_index, &p.clip))
                .collect::<Vec<_>>(),
        )?;
        let mut clips = Vec::with_capacity(previews.len());
        for preview in previews {
            clips.push(ClipPlacement {
                track_id: next.tracks[preview.track_index].id,
                clip: preview.clip.clone(),
            });
        }
        next.placed_clips(&clips)
    }

    pub(super) fn resolve_move_preview(
        &self,
        mut raw: Vec<ClipPreview>,
        duplicate: bool,
    ) -> Vec<ClipPreview> {
        if raw.is_empty() {
            return raw;
        }
        let mut placements = raw.clone();
        if duplicate {
            // Keep the originals in the candidate so copies can trim/split them too.
            // Raw preview IDs still identify the dragged clips for pointer handling.
            for preview in &mut placements {
                preview.clip.id = Id::new_v4();
            }
        }
        match self.move_project(&placements, &raw) {
            Ok(next) => {
                let ids = placements.iter().map(|preview| preview.clip.id).collect();
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
                if self.view_mode == ViewMode::Timeline {
                    panels::ControlPanel::paint_row_dividers(
                        &painter,
                        lane,
                        preview.track_index > 0,
                        preview.track_index + 1 == self.session.project.tracks.len(),
                    );
                } else {
                    painter.line_segment(
                        [lane.left_bottom(), lane.right_bottom()],
                        Stroke::new(1.0, theme::palette(ui.ctx()).border),
                    );
                }
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
            if let Err(error) = self.prepare_placement_tracks(
                &mut next,
                &previews
                    .iter()
                    .map(|p| (p.track_index, &p.clip))
                    .collect::<Vec<_>>(),
            ) {
                self.fail(error);
                return;
            }
            for preview in &mut previews {
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
        match self.move_project(&previews, &previews) {
            Ok(next) => {
                let track = next.tracks[previews[primary].track_index].id;
                self.selected_clip = Some(drag.clip.id);
                self.selected_clips = previews.iter().map(|preview| preview.clip.id).collect();
                self.session.project = next;
                self.select_track(track, false);
                self.prune_clip_selection();
                self.changed();
            }
            Err(error) => self.fail(error),
        }
    }
}
