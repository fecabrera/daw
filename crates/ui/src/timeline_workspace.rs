//! Timeline-only right settings rail, outside all canvas input and drop bounds.
use super::*;

impl DawUi {
    pub(super) fn timeline_workspace(
        &mut self,
        ui: &mut egui::Ui,
    ) -> Option<(effects_panel::EffectTarget, Rect)> {
        let bounds = ui.max_rect();
        let rail = Rect::from_min_max(
            Pos2::new(
                (bounds.right() - timeline_toolbar::WIDTH).max(bounds.left()),
                bounds.top(),
            ),
            bounds.max,
        );
        let allowed = self.effects_editing_enabled(ui)
            && !self.strip_dragging
            && self.component_drag.is_none()
            && self.effects_drag.as_ref().is_none_or(|drag| !drag.active);
        let toggle = timeline_toolbar::clip_settings(ui, rail, self.clip_settings_visible, allowed);
        if toggle.clicked() {
            self.clip_settings_visible = !self.clip_settings_visible;
            if !self.clip_settings_visible {
                self.clear_clip_settings_edits();
            }
            ui.ctx().request_repaint();
        }
        #[cfg(test)]
        {
            self.effects_bounds.clip_settings_toolbar = rail;
            self.effects_bounds.clip_settings_toggle = toggle.rect;
        }
        let settings_width = if self.clip_settings_visible {
            TRACK_WIDTH.min((rail.left() - bounds.left() - TRACK_WIDTH - 1.0).max(0.0))
        } else {
            0.0
        };
        let settings = Rect::from_min_max(
            Pos2::new(rail.left() - settings_width, bounds.top()),
            rail.left_bottom(),
        );
        let canvas = Rect::from_min_max(bounds.min, settings.left_bottom());
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("timeline-inner-content")
                .max_rect(canvas)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        content.set_clip_rect(canvas.intersect(ui.clip_rect()));
        let target = self.timeline_content(&mut content);
        if self.clip_settings_visible {
            let mut inspector = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("timeline-right-clip-settings")
                    .max_rect(settings)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            inspector.set_clip_rect(settings.intersect(ui.clip_rect()));
            let panel = panels::ControlPanel::begin(&inspector, settings);
            self.clip_settings_panel(&mut inspector, &panel, settings, allowed);
            panel.finish_left(&inspector);
            #[cfg(test)]
            {
                self.effects_bounds.clip_settings = settings;
            }
        }
        target
    }
}
