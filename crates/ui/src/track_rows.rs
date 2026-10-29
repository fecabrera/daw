//! Shared one-row and two-row track header geometry. This is view state only.
use super::*;

pub(super) struct HeaderRows {
    pub name: egui::Ui,
    pub mix: egui::Ui,
}

impl HeaderRows {
    pub fn new(ui: &mut egui::Ui, collapsed: bool) -> Self {
        let height = if collapsed {
            knobs::SIZE
        } else {
            ROW_HEIGHT - 14.0
        };
        let (body, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        // Two 22-point buttons, their six-point gap, a six-point group gap,
        // and two 28-point knob slots separated by four points.
        let mix_width = 2.0 * toolbars::CONTROL_HEIGHT + 6.0 + 6.0 + 2.0 * knobs::SIZE + 4.0;
        let (name, mix) = if collapsed {
            let mix = Rect::from_min_max(Pos2::new(body.right() - mix_width, body.top()), body.max);
            (
                Rect::from_min_max(body.min, Pos2::new(mix.left() - 6.0, body.bottom())),
                mix,
            )
        } else {
            (
                Rect::from_min_size(body.min, Vec2::new(body.width(), toolbars::CONTROL_HEIGHT)),
                Rect::from_min_max(
                    body.min + Vec2::new(0.0, toolbars::CONTROL_HEIGHT + 4.0),
                    body.max,
                ),
            )
        };
        let mut child = |rect, salt| {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(salt)
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect.intersect(ui.clip_rect()));
            child
        };
        Self {
            name: child(name, "track-name-row"),
            mix: child(mix, "track-mix-row"),
        }
    }
}

impl DawUi {
    pub(super) fn track_row_height(&self) -> f32 {
        if self.track_rows_collapsed {
            MASTER_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    pub(super) fn track_rows_button(&mut self, ui: &mut egui::Ui) {
        let enabled = self.clip_actions_available(ui.ctx()) && self.track_name_edit.is_none();
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                icons::track_rows_button(ui, self.track_rows_collapsed)
            })
            .inner;
        #[cfg(test)]
        {
            self.track_rows_bounds = response.rect;
        }
        if response.clicked() {
            self.track_rows_collapsed = !self.track_rows_collapsed;
            ui.ctx().request_repaint();
        }
    }
}
