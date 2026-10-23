//! Enlarged selected-clip waveform on the Editor's shared project clock.
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
        if let Some((track, clip)) = selected {
            let block = self.track_overview_clip_rect(lane, clip);
            if block.intersects(lane) {
                self.paint_clip(&painter, track.color, block, clip, false, None);
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
        self.paint_track_overview_playhead(&painter, lane);
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
