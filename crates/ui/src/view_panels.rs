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
}

impl ViewPanel {
    pub const ALL: [Self; 8] = [
        Self::Tracks,
        Self::Components,
        Self::EffectsCatalog,
        Self::Chords,
        Self::ClipSettings,
        Self::TrackMixer,
        Self::EffectsLane,
        Self::Returns,
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
        }
    }

    pub fn available_in(self, view: ViewMode) -> bool {
        match self {
            Self::Tracks => matches!(view, ViewMode::Instrument | ViewMode::Edit),
            Self::Components => view == ViewMode::Instrument,
            Self::EffectsCatalog | Self::EffectsLane => true,
            Self::Chords => matches!(view, ViewMode::Timeline | ViewMode::Edit),
            Self::ClipSettings | Self::TrackMixer => view == ViewMode::Timeline,
            Self::Returns => view == ViewMode::Mixer,
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
                ViewPanel::Returns => self.mixer_returns_visible,
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
            ViewPanel::Returns => self.mixer_returns_visible = !self.mixer_returns_visible,
        }
        ctx.request_repaint();
    }
}
