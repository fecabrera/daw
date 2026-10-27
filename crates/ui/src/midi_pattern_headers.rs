//! Lower pattern shells reuse clip gestures while notes keep their own gestures.
use super::*;

pub(super) fn edge_width(block: Rect) -> f32 {
    // Leave a note-editing area when a short pattern is zoomed out.
    8.0_f32.min(block.width() * 0.25)
}

impl DawUi {
    pub(super) fn midi_pattern_headers(
        &mut self,
        ui: &mut egui::Ui,
        track: &daw_core::Track,
    ) -> Rect {
        let (row, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
        let lane = Rect::from_min_max(
            row.min + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
            row.max - Vec2::new(ui.spacing().scroll.allocated_width(), 0.0),
        );
        ui.painter()
            .rect_filled(row, 0.0, theme::palette(ui.ctx()).background);
        let painter = ui.painter_at(lane);
        #[cfg(test)]
        self.midi_editor.pattern_header_bounds.clear();
        for clip in &track.clips {
            let block = self.clip_block(lane, clip, clip.start_frame);
            let header = Rect::from_min_max(block.min, Pos2::new(block.right(), lane.bottom()));
            let visible = header.intersect(lane);
            if !visible.is_positive() {
                continue;
            }
            #[cfg(test)]
            self.midi_editor
                .pattern_header_bounds
                .insert(clip.id, visible);
            let response = ui
                .add_enabled_ui(self.edit_shortcuts_available(ui.ctx()), |ui| {
                    ui.interact(
                        visible,
                        ui.id().with(("midi-pattern-header", clip.id)),
                        Sense::click_and_drag(),
                    )
                })
                .inner;
            self.clip_lane_response(ui, track.id, clip, header, lane, response);
            let colors = theme::clip_colors(track.color, clip.color);
            let colors = if clip.muted {
                colors.dimmed(ui.ctx())
            } else {
                colors
            };
            clip_paint::paint_header(&painter, header, &clip.name, colors, false);
        }
        lane
    }

    pub(super) fn midi_pattern_edges(
        &mut self,
        ui: &mut egui::Ui,
        track: &daw_core::Track,
        viewport: Rect,
    ) {
        let lane = Rect::from_min_max(viewport.min - Vec2::new(0.0, 22.0), viewport.max);
        for clip in &track.clips {
            let block = self.clip_block(lane, clip, clip.start_frame);
            for (side, edge) in [
                (
                    "left",
                    Rect::from_min_max(
                        block.min,
                        Pos2::new(block.left() + edge_width(block), block.bottom()),
                    ),
                ),
                (
                    "right",
                    Rect::from_min_max(
                        Pos2::new(block.right() - edge_width(block), block.top()),
                        block.max,
                    ),
                ),
            ] {
                // Remove note hit areas from the clip edges. This also keeps egui's
                // previous-frame hit testing from capturing a fast note press.
                let mut hits = vec![edge.intersect(viewport)];
                for (_, note) in &self.midi_editor.note_regions {
                    hits = hits
                        .into_iter()
                        .flat_map(|hit| {
                            let overlap = hit.intersect(*note);
                            if !overlap.is_positive() {
                                return vec![hit];
                            }
                            vec![
                                Rect::from_min_max(hit.min, Pos2::new(hit.right(), overlap.top())),
                                Rect::from_min_max(
                                    Pos2::new(hit.left(), overlap.bottom()),
                                    hit.max,
                                ),
                                Rect::from_min_max(
                                    Pos2::new(hit.left(), overlap.top()),
                                    overlap.left_bottom(),
                                ),
                                Rect::from_min_max(
                                    overlap.right_top(),
                                    Pos2::new(hit.right(), overlap.bottom()),
                                ),
                            ]
                        })
                        .filter(Rect::is_positive)
                        .collect();
                }
                for (segment, hit) in hits
                    .into_iter()
                    .enumerate()
                    .filter(|(_, hit)| hit.is_positive())
                {
                    let response = ui
                        .add_enabled_ui(
                            self.edit_shortcuts_available(ui.ctx())
                                && !ui.input(|i| i.modifiers.command)
                                && !self.midi_editor.dragging(),
                            |ui| {
                                ui.interact(
                                    hit,
                                    ui.id().with(("midi-pattern-edge", clip.id, side, segment)),
                                    Sense::click_and_drag(),
                                )
                            },
                        )
                        .inner;
                    self.clip_lane_response(ui, track.id, clip, block, lane, response);
                }
            }
        }
    }

    pub(super) fn midi_pattern_outlines(
        &self,
        painter: &egui::Painter,
        track: &daw_core::Track,
        viewport: Rect,
    ) {
        for clip in &track.clips {
            let block = self.clip_block(viewport, clip, clip.start_frame);
            let block = Rect::from_min_max(block.min, Pos2::new(block.right(), viewport.bottom()));
            if block.intersects(viewport) {
                self.paint_midi_pattern_outline(painter, block, clip.id);
            }
        }
    }

    fn paint_midi_pattern_outline(&self, painter: &egui::Painter, block: Rect, clip: Id) {
        let stroke = if self.clip_selected(clip) {
            theme::selection_outline(painter.ctx())
        } else {
            Stroke::new(1.0, theme::palette(painter.ctx()).divider)
        };
        painter.rect_stroke(block, 0.0, stroke, StrokeKind::Inside);
    }
}
