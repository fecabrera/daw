//! Selected-clip waveform on the independent editor content viewport.
use super::*;

impl DawUi {
    pub(super) fn audio_editor(&mut self, ui: &mut egui::Ui) {
        let (canvas, _) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                (ui.available_height() - SCROLLBAR_HEIGHT).max(0.0),
            ),
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

        // Resolve after overview interactions so selection, trims, and deletion appear
        // in this frame. The shared painter resolves prepared audio and source mapping.
        let (track, _) = self.track_overview_content(ui);
        let selected = track.as_ref().and_then(|track| {
            track
                .clips
                .iter()
                .find(|clip| Some(clip.id) == self.selected_clip)
                .map(|clip| (track, clip))
        });
        if let Some((track, _)) = selected {
            if self.editing_enabled() && ui.is_enabled() && ui.input(|input| input.focused) {
                let menu_open = egui::Popup::is_any_open(ui.ctx());
                for clip in &track.clips {
                    let visible = self
                        .clip_block(lane, clip, clip.start_frame)
                        .intersect(lane)
                        .intersect(ui.clip_rect());
                    if !visible.is_positive() {
                        continue;
                    }
                    let response = ui
                        .interact(
                            visible,
                            ui.id().with(("audio-editor-clip", clip.id)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    // Keep this popup alive across frames while blocking waveforms
                    // beneath other menus. Timeline's menu owns targets and actions.
                    if !menu_open || response.context_menu_opened() {
                        response.context_menu(|ui| self.clip_context_menu(ui, clip.id));
                    }
                    if !menu_open && response.clicked() {
                        self.editor_select_clip(clip.id, track.id);
                        ui.ctx().request_repaint(); // Refresh the options panel painted before this lane.
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            ui.is_enabled(),
                            Some(clip.id) == self.selected_clip,
                            &clip.name,
                        )
                    });
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
                        Some(clip.id) == self.selected_clip,
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
        rows::centered(ui, SCROLLBAR_HEIGHT, |ui| {
            ui.allocate_exact_size(
                Vec2::new(track_overview::LABEL_WIDTH, SCROLLBAR_HEIGHT),
                Sense::hover(),
            );
            self.timeline_scrollbar(ui);
        });
    }
}
