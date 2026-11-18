use super::*;

impl DawUi {
    pub(super) fn instrument_workspace(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.max_rect();
        let rail = timeline_toolbar::right_bounds(bounds);
        timeline_toolbar::paint_right_rail(ui, rail);
        let size = timeline_toolbar::BUTTON_SIZE;
        let inset = (timeline_toolbar::WIDTH - size) * 0.5;
        let bottom = rail.bottom() - inset - size;
        let mut toolbar = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-right-toolbar")
                .max_rect(rail),
        );
        toolbar.set_clip_rect(rail.intersect(ui.clip_rect()));
        for (panel, icon, name, top) in [
            (
                ViewPanel::TrackMixer,
                icons::Lucide::SlidersVertical,
                "Show track mixer",
                rail.top() + inset,
            ),
            (
                ViewPanel::ModulationLanes,
                icons::Lucide::Activity,
                "Show LFO and Envelopes",
                bottom - size - inset,
            ),
            (
                ViewPanel::InstrumentOutput,
                icons::Lucide::Merge,
                "Show instrument Output",
                bottom,
            ),
        ] {
            let response = timeline_toolbar::toggle_at(
                &mut toolbar,
                Pos2::new(rail.left() + inset, top),
                panel.label(),
                icon,
                name,
                self.view_panel_selected(panel),
                self.view_panel_enabled(panel, ui.ctx()),
            );
            if response.clicked() {
                self.toggle_view_panel(panel, ui.ctx());
            }
            #[cfg(test)]
            match panel {
                ViewPanel::TrackMixer => self.effects_bounds.track_mixer_toggle = response.rect,
                ViewPanel::ModulationLanes => {
                    self.instrument_catalog_bounds.modulation_toggle = response.rect
                }
                ViewPanel::InstrumentOutput => {
                    self.instrument_catalog_bounds.output_toggle = response.rect
                }
                _ => unreachable!(),
            }
        }
        let mixer_width = if self.instrument_track_mixer_visible {
            (2.0 * mixer::STRIP_WIDTH)
                .min((rail.left() - bounds.left() - TRACK_WIDTH - lane_toolbar::WIDTH).max(0.0))
        } else {
            0.0
        };
        let mixer = Rect::from_min_max(
            rail.left_top() - Vec2::new(mixer_width, 0.0),
            rail.left_bottom(),
        );
        let canvas = Rect::from_min_max(bounds.min, mixer.left_bottom());
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-canvas")
                .max_rect(canvas)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        content.set_clip_rect(canvas.intersect(ui.clip_rect()));
        self.instrument_view(&mut content);
        if self.instrument_track_mixer_visible {
            self.track_mixer_panel(
                ui,
                mixer,
                self.view_panel_enabled(ViewPanel::TrackMixer, ui.ctx()),
            );
        }
    }
}
