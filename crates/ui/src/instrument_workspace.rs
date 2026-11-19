use super::*;
use daw_core::InstrumentComponent;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum InstrumentSubview {
    #[default]
    Sources,
    Effects,
    Modulation,
}

impl InstrumentSubview {
    const ALL: [Self; 3] = [Self::Sources, Self::Effects, Self::Modulation];

    fn label(self) -> &'static str {
        match self {
            Self::Sources => "Sources",
            Self::Effects => "Effects",
            Self::Modulation => "Modulation",
        }
    }

    pub(super) fn shows(self, component: InstrumentComponent) -> bool {
        use InstrumentComponent::*;
        match self {
            Self::Sources => component.is_source() || component == Amplitude,
            Self::Effects => matches!(component, Effect(_)),
            Self::Modulation => matches!(component, Envelope(_) | Lfo(_) | Macros),
        }
    }
}

impl DawUi {
    fn instrument_subview_tabs(&mut self, ui: &mut egui::Ui) {
        let enabled = self.view_panel_enabled(ViewPanel::TrackMixer, ui.ctx())
            && !self.instrument_ui.editing_name()
            && self.effects_drag.is_none();
        ui.spacing_mut().item_spacing.y = 0.0;
        toolbars::Toolbar::row(ui, |ui| {
            let group = rows::input_group(ui, |ui| {
                InstrumentSubview::ALL.map(|subview| {
                    let response = ui
                        .scope(|ui| {
                            match subview {
                                InstrumentSubview::Sources => rows::input_group_edge(ui, false),
                                InstrumentSubview::Effects => {}
                                InstrumentSubview::Modulation => rows::input_group_edge(ui, true),
                            }
                            ui.add_enabled(
                                enabled,
                                egui::Button::new(subview.label())
                                    .selected(self.instrument_subview == subview)
                                    .min_size(Vec2::new(0.0, toolbars::CONTROL_HEIGHT)),
                            )
                        })
                        .inner;
                    #[cfg(test)]
                    {
                        self.instrument_catalog_bounds.subviews[subview as usize] = response.rect;
                    }
                    if response.clicked() {
                        self.set_instrument_subview(subview, ui.ctx());
                    }
                    response
                })
            });
            for response in &group.inner[1..] {
                rows::group_separator(ui, group.response.rect, response.rect.left());
            }
        });
    }

    pub(super) fn set_instrument_subview(
        &mut self,
        subview: InstrumentSubview,
        ctx: &egui::Context,
    ) {
        if self.instrument_subview == subview {
            return;
        }
        self.instrument_subview = subview;
        self.instrument_ui.cancel();
        self.instrument_ui.effects_layer = None;
        self.selected_effects.clear();
        self.instrument_catalog.clear();
        if !self.tracks_panel_visible() {
            if subview == InstrumentSubview::Effects && self.instrument_catalog_visible {
                self.toggle_view_panel(ViewPanel::EffectsCatalog, ctx);
            } else if subview != InstrumentSubview::Effects && self.effects_catalog_visible() {
                self.toggle_view_panel(ViewPanel::Components, ctx);
            }
        }
        ctx.request_repaint();
    }

    pub(super) fn instrument_workspace(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.max_rect();
        let rail = timeline_toolbar::right_bounds(bounds);
        timeline_toolbar::paint_right_rail(ui, rail);
        let size = timeline_toolbar::BUTTON_SIZE;
        let inset = (timeline_toolbar::WIDTH - size) * 0.5;
        let mut toolbar = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-right-toolbar")
                .max_rect(rail),
        );
        toolbar.set_clip_rect(rail.intersect(ui.clip_rect()));
        let panel = ViewPanel::TrackMixer;
        let response = timeline_toolbar::toggle_at(
            &mut toolbar,
            rail.min + Vec2::splat(inset),
            panel.label(),
            icons::Lucide::SlidersVertical,
            "Show track mixer",
            self.view_panel_selected(panel),
            self.view_panel_enabled(panel, ui.ctx()),
        );
        if response.clicked() {
            self.toggle_view_panel(panel, ui.ctx());
        }
        #[cfg(test)]
        {
            self.effects_bounds.track_mixer_toggle = response.rect;
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
        let compact_header = canvas.width() < TRACK_WIDTH + 225.0;
        let tabs = Rect::from_min_max(
            Pos2::new(
                if compact_header {
                    canvas.left()
                } else {
                    canvas.left() + TRACK_WIDTH
                },
                canvas.top(),
            ),
            Pos2::new(canvas.right(), canvas.top() + toolbars::HEIGHT),
        );
        let mut tabs_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-subviews")
                .max_rect(tabs)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        tabs_ui.set_clip_rect(tabs.intersect(ui.clip_rect()));
        self.instrument_subview_tabs(&mut tabs_ui);
        let mut lanes = canvas;
        if compact_header {
            lanes.min.y = tabs.bottom().min(canvas.bottom());
        }
        let mut lanes_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("instrument-lanes")
                .max_rect(lanes)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lanes_ui.set_clip_rect(lanes.intersect(ui.clip_rect()));
        self.instrument_view(&mut lanes_ui);
        if self.instrument_track_mixer_visible {
            self.track_mixer_panel(
                ui,
                mixer,
                self.view_panel_enabled(ViewPanel::TrackMixer, ui.ctx()),
            );
        }
    }
}
