use super::*;
use daw_core::{Asset, Track};

pub(super) const LABEL_WIDTH: f32 = 52.0;
pub(super) const STRIP_HEIGHT: f32 = 56.0;

impl DawUi {
    /// Editor follows its active clip, falling back to the selected track.
    pub(super) fn track_overview_content(&self, ui: &egui::Ui) -> (Option<Track>, Vec<Asset>) {
        let preview = self.clip_trim_preview_project(ui);
        let project = preview.as_ref().unwrap_or(&self.session.project);
        let track = (self.view_mode == ViewMode::Edit)
            .then(|| {
                project.tracks.iter().find(|track| {
                    track
                        .clips
                        .iter()
                        .any(|clip| Some(clip.id) == self.selected_clip)
                })
            })
            .flatten()
            .or_else(|| {
                project
                    .tracks
                    .iter()
                    .find(|track| Some(track.id) == self.selected_track)
            })
            .cloned();
        let assets = project
            .assets
            .iter()
            .filter(|asset| {
                track
                    .as_ref()
                    .is_some_and(|track| track.clips.iter().any(|clip| clip.asset_id == asset.id))
            })
            .cloned()
            .collect();
        (track, assets)
    }

    // Selecting another clip on the same track must retain the viewport.
    pub(super) fn prepare_track_overview(&mut self, track: Option<&Track>) {
        self.midi_editor.source_origin = 0.0;
        if self.midi_editor.active_clip != self.selected_clip && !self.midi_editor.dragging() {
            self.midi_editor.clear_note_selection();
            self.midi_editor.active_clip = self.selected_clip;
        }
        if let Some(track) = track
            && self.midi_editor.active_track != Some(track.id)
        {
            self.midi_editor.cancel();
            self.midi_editor.scroll = track
                .clips
                .iter()
                .find(|clip| Some(clip.id) == self.selected_clip)
                .map_or(0.0, |clip| self.session.project.seconds(clip.start_frame));
            self.midi_editor.active_track = Some(track.id);
            self.midi_editor.active_clip = self.selected_clip;
        }
    }
    pub(super) fn track_overview_ruler(&mut self, ui: &mut egui::Ui) {
        let row = rows::centered(ui, RULER_HEIGHT, |ui| {
            ui.allocate_exact_size(Vec2::new(LABEL_WIDTH, RULER_HEIGHT), Sense::hover());
            let rect = self.time_ruler(ui);
            #[cfg(test)]
            {
                self.midi_editor.ruler_bounds = rect;
            }
            #[cfg(not(test))]
            {
                let _ = rect;
            }
        });
        let y = row.response.rect.bottom() - 0.5;
        ui.painter().line_segment(
            [
                Pos2::new(ui.max_rect().left(), y),
                Pos2::new(ui.max_rect().right(), y),
            ],
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
    }
    pub(super) fn track_overview_clip_rect(&self, band: Rect, clip: &Clip) -> Rect {
        let left = band.left()
            + (self.session.project.seconds(clip.start_frame) - self.midi_editor.scroll) as f32
                * self.midi_editor.zoom;
        Rect::from_min_size(
            Pos2::new(left, band.top()),
            Vec2::new(
                self.session.project.seconds(clip.length_frames) as f32 * self.midi_editor.zoom,
                band.height(),
            ),
        )
    }
    pub(super) fn track_overview_strip(
        &mut self,
        ui: &mut egui::Ui,
        track: &daw_core::Track,
        assets: &[daw_core::Asset],
    ) {
        let (band, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), STRIP_HEIGHT),
            Sense::hover(),
        );
        let lane = Rect::from_min_max(Pos2::new(band.left() + LABEL_WIDTH, band.top()), band.max);
        self.midi_editor.timeline_left = lane.left();
        self.lane_bounds.clear();
        self.lane_bounds.insert(track.id, lane);
        self.timeline_bounds = lane;
        self.create_midi_pattern_in_empty_lane(ui, track, lane);
        let palette = theme::palette(ui.ctx());
        ui.painter().rect_filled(band, 0.0, palette.background);
        ui.painter()
            .with_clip_rect(Rect::from_min_max(band.min, lane.left_bottom()))
            .text(
                Pos2::new(band.left() + 4.0, band.center().y),
                egui::Align2::LEFT_CENTER,
                &track.name,
                egui::FontId::proportional(10.0),
                palette.text,
            );
        #[cfg(test)]
        {
            self.midi_editor.track_bounds = lane;
            self.midi_editor.clip_bounds.clear();
        }
        let painter = ui.painter_at(lane);
        self.paint_grid(&painter, lane);
        for clip in &track.clips {
            let block = self.track_overview_clip_rect(lane, clip);
            #[cfg(test)]
            {
                self.midi_editor.clip_bounds.insert(clip.id, block);
                if Some(clip.id) == self.selected_clip {
                    self.midi_editor.pattern_bounds = block;
                }
            }
            if !block.intersects(lane) {
                continue;
            }
            let pattern = assets
                .iter()
                .find(|a| a.id == clip.asset_id)
                .and_then(|a| a.midi_pattern.as_ref())
                .map(|pattern| PreparedClipSource::Midi(self.editor_pattern(clip.id, pattern)));
            self.paint_clip(
                &painter,
                track.color,
                block,
                clip,
                self.clip_selected(clip.id),
                pattern,
            );
            self.clip_lane_interaction(ui, track.id, clip, block, lane);
        }
        self.paint_drag_preview(ui, lane);
        ui.painter().line_segment(
            [band.left_bottom(), band.right_bottom()],
            Stroke::new(1.0, palette.border),
        );
        self.paint_track_overview_playhead(&painter, lane);
    }

    pub(super) fn paint_track_overview_playhead(&self, painter: &egui::Painter, rect: Rect) {
        let x = rect.left()
            + (self
                .session
                .project
                .seconds(self.session.project.transport.playhead_frame)
                - self.midi_editor.scroll) as f32
                * self.midi_editor.zoom;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(1.0, theme::accent(painter.ctx())),
        );
    }
}
