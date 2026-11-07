//! Shared View-menu and toolbar panel state. Visibility is workspace state.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPanel {
    Tracks,
    Components,
    EffectsCatalog,
    Chords,
    ClipSettings,
    TrackMixer,
    EffectsLane,
    Returns,
    Master,
}

impl ViewPanel {
    pub const ALL: [Self; 9] = [
        Self::Tracks,
        Self::Components,
        Self::EffectsCatalog,
        Self::Chords,
        Self::ClipSettings,
        Self::TrackMixer,
        Self::EffectsLane,
        Self::Returns,
        Self::Master,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Components => "Components",
            Self::EffectsCatalog => "Effects Catalog",
            Self::Chords => "Chords",
            Self::ClipSettings => "Clip Settings",
            Self::TrackMixer => "Track Mixer",
            Self::EffectsLane => "Effects Lane",
            Self::Returns => "Returns",
            Self::Master => "Master",
        }
    }

    pub fn available_in(self, view: ViewMode) -> bool {
        match self {
            Self::Tracks => matches!(view, ViewMode::Instrument | ViewMode::Edit),
            Self::Components => view == ViewMode::Instrument,
            Self::EffectsCatalog | Self::EffectsLane => true,
            Self::Chords => matches!(view, ViewMode::Timeline | ViewMode::Edit),
            Self::ClipSettings | Self::TrackMixer => view == ViewMode::Timeline,
            Self::Returns | Self::Master => matches!(view, ViewMode::Timeline | ViewMode::Mixer),
        }
    }
}

impl DawUi {
    pub fn view_panel_enabled(&self, panel: ViewPanel, ctx: &egui::Context) -> bool {
        panel.available_in(self.view_mode)
            && self.effects_editing_available(ctx)
            && !self.strip_dragging
            && self.component_drag.is_none()
            && self.effects_drag.as_ref().is_none_or(|drag| !drag.active)
    }

    pub fn view_panel_selected(&self, panel: ViewPanel) -> bool {
        panel.available_in(self.view_mode)
            && match panel {
                ViewPanel::Tracks => self.tracks_panel_visible(),
                ViewPanel::Components => self.instrument_catalog_visible,
                ViewPanel::EffectsCatalog => self.effects_catalog_visible(),
                ViewPanel::Chords => self.chords_visible(),
                ViewPanel::ClipSettings => self.clip_settings_visible,
                ViewPanel::TrackMixer => self.track_mixer_visible,
                ViewPanel::EffectsLane => self.effects_lane_visible(),
                ViewPanel::Returns if self.view_mode == ViewMode::Timeline => {
                    self.timeline_returns_visible
                }
                ViewPanel::Master if self.view_mode == ViewMode::Timeline => {
                    self.timeline_master_visible
                }
                ViewPanel::Returns => self.mixer_returns_visible,
                ViewPanel::Master => self.mixer_master_visible,
            }
    }

    pub fn toggle_view_panel(&mut self, panel: ViewPanel, ctx: &egui::Context) {
        if !self.view_panel_enabled(panel, ctx) {
            return;
        }
        match panel {
            ViewPanel::Tracks => {
                self.set_tracks_panel_visible(!self.tracks_panel_visible());
                if self.tracks_panel_visible() {
                    if self.view_mode == ViewMode::Instrument {
                        self.instrument_catalog_visible = false;
                    }
                    self.instrument_catalog.clear();
                    self.set_effects_catalog_visible(false);
                    self.effects_catalog.clear();
                    self.set_chords_visible(false);
                    self.chords.catalog.clear();
                }
            }
            ViewPanel::Chords => {
                self.set_chords_visible(!self.chords_visible());
                self.chords.catalog.clear();
                if self.chords_visible() {
                    self.set_tracks_panel_visible(false);
                    self.set_effects_catalog_visible(false);
                    self.effects_catalog.clear();
                }
            }
            ViewPanel::EffectsCatalog => {
                self.set_effects_catalog_visible(!self.effects_catalog_visible());
                self.effects_catalog.clear();
                if self.effects_catalog_visible() {
                    self.set_tracks_panel_visible(false);
                    self.set_chords_visible(false);
                    self.chords.catalog.clear();
                    if self.view_mode == ViewMode::Instrument {
                        self.instrument_catalog_visible = false;
                    }
                    self.instrument_catalog.clear();
                }
            }
            ViewPanel::Components => {
                self.instrument_catalog_visible = !self.instrument_catalog_visible;
                self.instrument_catalog.clear();
                if self.instrument_catalog_visible {
                    self.track_browser.visible = false;
                    self.set_effects_catalog_visible(false);
                    self.effects_catalog.clear();
                }
            }
            ViewPanel::EffectsLane => {
                self.set_effects_lane_visible(!self.effects_lane_visible());
                self.selected_effects.clear();
                self.effects_drag = None;
            }
            ViewPanel::ClipSettings => {
                self.clip_settings_visible = !self.clip_settings_visible;
                if self.clip_settings_visible {
                    self.track_mixer_visible = false;
                }
                if !self.clip_settings_visible {
                    self.clear_clip_settings_edits();
                }
            }
            ViewPanel::TrackMixer => {
                self.track_mixer_visible = !self.track_mixer_visible;
                if self.track_mixer_visible {
                    self.clip_settings_visible = false;
                    self.clear_clip_settings_edits();
                }
            }
            ViewPanel::Returns if self.view_mode == ViewMode::Timeline => {
                self.timeline_returns_visible = !self.timeline_returns_visible
            }
            ViewPanel::Master if self.view_mode == ViewMode::Timeline => {
                self.timeline_master_visible = !self.timeline_master_visible
            }
            ViewPanel::Returns => self.mixer_returns_visible = !self.mixer_returns_visible,
            ViewPanel::Master => self.mixer_master_visible = !self.mixer_master_visible,
        }
        ctx.request_repaint();
    }
}

impl DawUi {
    /// Stack Returns above Master at the bottom of the reserved right rail.
    pub(super) fn return_master_buttons(&mut self, ui: &mut egui::Ui, bounds: Rect) {
        let inset = (timeline_toolbar::WIDTH - icons::TOOLBAR_BUTTON_SIZE) * 0.5;
        let buttons = Rect::from_min_max(
            Pos2::new(
                bounds.left(),
                bounds.bottom() - 2.0 * (icons::TOOLBAR_BUTTON_SIZE + inset),
            ),
            bounds.max,
        );
        let mut rail = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("bus-visibility-toolbar")
                .max_rect(buttons),
        );
        rail.set_clip_rect(buttons.intersect(ui.clip_rect()));
        for (panel, icon, name, selected) in [
            (
                ViewPanel::Master,
                icons::Lucide::BoomBox,
                "Show or hide Master track",
                self.view_panel_selected(ViewPanel::Master),
            ),
            (
                ViewPanel::Returns,
                icons::Lucide::CornerDownLeft,
                "Show or hide Return tracks",
                self.view_panel_selected(ViewPanel::Returns),
            ),
        ] {
            let enabled = self.view_panel_enabled(panel, ui.ctx());
            let offset = if panel == ViewPanel::Master {
                0.0
            } else {
                icons::TOOLBAR_BUTTON_SIZE + inset
            };
            let response = timeline_toolbar::toggle_at(
                &mut rail,
                Pos2::new(
                    bounds.left() + inset,
                    bounds.bottom() - inset - icons::TOOLBAR_BUTTON_SIZE - offset,
                ),
                if panel == ViewPanel::Master {
                    "master-visibility"
                } else {
                    "returns-visibility"
                },
                icon,
                name,
                selected,
                enabled,
            );
            #[cfg(test)]
            match panel {
                ViewPanel::Master => self.mixer_master_bounds = response.rect,
                ViewPanel::Returns => self.mixer_returns_bounds = response.rect,
                _ => unreachable!(),
            }
            if response.clicked() {
                self.toggle_view_panel(panel, ui.ctx());
            }
        }
    }
}
