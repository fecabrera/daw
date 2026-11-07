//! Timeline-only inspector rail, outside all canvas input and drop bounds.
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
        let (toggle, mixer_toggle) = timeline_toolbar::right_panels(
            ui,
            rail,
            self.clip_settings_visible,
            self.track_mixer_visible,
            allowed,
        );
        self.return_master_buttons(ui, rail);
        if toggle.clicked() {
            self.toggle_view_panel(ViewPanel::ClipSettings, ui.ctx());
        }
        if mixer_toggle.clicked() {
            self.toggle_view_panel(ViewPanel::TrackMixer, ui.ctx());
        }
        #[cfg(test)]
        {
            self.effects_bounds.track_mixer_toggle = mixer_toggle.rect;
            self.effects_bounds.clip_settings_toolbar = rail;
            self.effects_bounds.clip_settings_toggle = toggle.rect;
        }
        let settings_width = if self.clip_settings_visible || self.track_mixer_visible {
            let width = if self.track_mixer_visible {
                2.0 * mixer::STRIP_WIDTH
            } else {
                TRACK_WIDTH
            };
            width.min((rail.left() - bounds.left() - TRACK_WIDTH - 1.0).max(0.0))
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
        if self.track_mixer_visible {
            ui.painter()
                .rect_filled(settings, 0.0, theme::palette(ui.ctx()).panel);
            let mut inspector = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("timeline-track-mixer")
                    .max_rect(settings)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            inspector.set_clip_rect(settings.intersect(ui.clip_rect()));
            inspector.add_enabled_ui(allowed, |ui| self.timeline_track_mixer(ui));
            ui.painter().vline(
                settings.left() + 0.5,
                settings.y_range(),
                Stroke::new(1.0, theme::palette(ui.ctx()).border),
            );
            #[cfg(test)]
            {
                self.effects_bounds.track_mixer = settings;
            }
        }
        target
    }

    fn timeline_track_mixer(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        {
            self.mixer_bounds.clear();
            self.mixer_faders.clear();
        }
        let track = self.selected_track.and_then(|id| {
            self.session
                .project
                .tracks
                .iter()
                .find(|track| track.id == id)
                .cloned()
        });
        let panel = panels::ControlPanel::with_width(ui, ui.max_rect(), ui.available_width());
        let header = panel.header(ui, "Track mixer", |_| {});
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        if track.is_none() && !self.master_selected {
            panel.placeholder(ui, "Select a track");
            return;
        }
        let parent = track
            .as_ref()
            .and_then(|track| track.group_id)
            .and_then(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .cloned()
            });
        // Master has no parent. All other tracks show their immediate output bus.
        let strips = if track.is_some() { 2 } else { 1 };
        let height = (mixer::MIN_HEIGHT + self.mixer_send_height()).max(ui.available_height());
        egui::ScrollArea::both()
            .id_salt("timeline-track-mixer-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let (bounds, _) = ui.allocate_exact_size(
                    Vec2::new(strips as f32 * mixer::STRIP_WIDTH, height),
                    Sense::hover(),
                );
                for (index, track) in [track.as_ref(), parent.as_ref()]
                    .into_iter()
                    .take(strips)
                    .enumerate()
                {
                    let rect = Rect::from_min_size(
                        bounds.min + Vec2::new(index as f32 * mixer::STRIP_WIDTH, 0.0),
                        Vec2::new(mixer::STRIP_WIDTH, height),
                    );
                    let mut strip = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("strip", track.map(|t| t.id)))
                            .max_rect(rect),
                    );
                    strip.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.mixer_strip(&mut strip, rect, track, index > 0);
                    #[cfg(test)]
                    {
                        if let Some(track) = track {
                            self.mixer_bounds.insert(track.id, rect);
                        } else {
                            self.master_bounds = rect;
                        }
                    }
                }
            });
    }
}
