//! Audio clip editing on the independent editor content viewport.
use super::*;

impl DawUi {
    pub(super) fn audio_editor(&mut self, ui: &mut egui::Ui) {
        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ui.available_height().max(0.0)),
            Sense::hover(),
        );
        let lane = Rect::from_min_max(
            canvas.min + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
            canvas.max,
        );
        let palette = theme::palette(ui.ctx());
        ui.painter().rect_filled(canvas, 0.0, palette.background);
        let painter = ui.painter_at(lane);
        self.paint_grid(&painter, lane);

        // Use the same candidate transaction for lower MIDI and audio previews.
        let (track, _) = self.track_overview_content(ui);
        let preview = self.editor_clip_preview(ui);
        let track = track.as_ref().map(|track| {
            preview
                .as_ref()
                .and_then(|project| {
                    project
                        .tracks
                        .iter()
                        .find(|candidate| candidate.id == track.id)
                })
                .unwrap_or(track)
        });
        // A boundary preview can remove the primary clip; still paint its surviving neighbor.
        let selected = track.filter(|_| self.selected_clip.is_some());
        if let Some(track) = selected {
            for clip in &track.clips {
                let block = self.clip_block(lane, clip, clip.start_frame);
                let visible = block.intersect(lane).intersect(ui.clip_rect());
                if !visible.is_positive() {
                    continue;
                }
                let response = ui
                    .add_enabled_ui(self.edit_shortcuts_available(ui.ctx()), |ui| {
                        ui.interact(
                            visible,
                            ui.id().with(("audio-editor-clip", clip.id)),
                            Sense::click_and_drag(),
                        )
                    })
                    .inner
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let clicked = response.clicked();
                self.clip_lane_response(ui, track.id, clip, block, lane, response);
                if clicked {
                    ui.ctx().request_repaint();
                }
            }
            for clip in &track.clips {
                let block = self.clip_block(lane, clip, clip.start_frame);
                if block.intersect(lane).is_positive() {
                    self.paint_clip(
                        &painter,
                        track.color,
                        block,
                        clip,
                        self.clip_selected(clip.id),
                        None,
                    );
                }
            }
        } else {
            painter.text(
                lane.center(),
                egui::Align2::CENTER_CENTER,
                "Select a clip",
                egui::TextStyle::Body.resolve(ui.style()),
                palette.secondary,
            );
        }
        let x = lane.left()
            + (self
                .session
                .project
                .seconds(self.session.project.transport.playhead_frame)
                - self.view_scroll()) as f32
                * self.view_zoom();
        painter.line_segment(
            [Pos2::new(x, lane.top()), Pos2::new(x, lane.bottom())],
            Stroke::new(1.0, theme::accent(ui.ctx())),
        );
        #[cfg(test)]
        {
            self.midi_editor.content.bounds = lane;
        }
        ui.painter().vline(
            lane.left(),
            canvas.y_range(),
            Stroke::new(1.0, palette.border),
        );
        self.editor_scrollbar(ui, canvas.bottom(), "editor_content_scrollbar");
        if self.drag.is_some() && self.content_clip_drag {
            self.finish_clip_drag(ui);
            self.retain_content_view();
        }
    }
}
