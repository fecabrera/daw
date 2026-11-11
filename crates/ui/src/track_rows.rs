//! Compact/regular header geometry plus the Timeline-only large sends row.
use super::*;

pub(super) struct HeaderRows {
    pub name: egui::Ui,
    pub mix: egui::Ui,
    pub sends: Option<egui::Ui>,
}

impl HeaderRows {
    pub fn new(ui: &mut egui::Ui, collapsed: bool) -> Self {
        Self::with_knobs(ui, collapsed, true, false)
    }

    pub fn track(ui: &mut egui::Ui, collapsed: bool) -> Self {
        Self::track_size(ui, collapsed, false)
    }

    pub fn track_size(ui: &mut egui::Ui, collapsed: bool, large: bool) -> Self {
        Self::with_knobs(ui, collapsed, !collapsed, large && !collapsed)
    }

    fn with_knobs(ui: &mut egui::Ui, collapsed: bool, knobs: bool, large: bool) -> Self {
        let height = if collapsed {
            knobs::SIZE
        } else if large {
            LARGE_HEIGHT - 14.0
        } else {
            ROW_HEIGHT - 14.0
        };
        let (body, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        // Two 22-point buttons plus their divider, a six-point group gap,
        // and two 28-point knob slots separated by four points.
        let mix_width = 2.0 * toolbars::CONTROL_HEIGHT
            + rows::GROUP_DIVIDER_WIDTH
            + if knobs {
                mix_controls::GROUP_GAP + 2.0 * knobs::SIZE + mix_controls::KNOB_GAP
            } else {
                0.0
            };
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
                    body.min
                        + Vec2::new(
                            0.0,
                            toolbars::CONTROL_HEIGHT
                                + 4.0
                                + if large { knobs::SIZE + 4.0 } else { 0.0 },
                        ),
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
            sends: large.then(|| {
                child(
                    Rect::from_min_size(
                        body.min + Vec2::new(0.0, toolbars::CONTROL_HEIGHT + 4.0),
                        Vec2::new(body.width(), knobs::SIZE),
                    ),
                    "track-sends-row",
                )
            }),
        }
    }
}

pub(super) const LARGE_HEIGHT: f32 = ROW_HEIGHT + knobs::SIZE + 4.0;

impl DawUi {
    pub(super) fn large_track_headers(&self) -> bool {
        self.view_mode == mixer::ViewMode::Timeline
            && !self.track_rows_collapsed
            && self.timeline_tracks_large
    }
    pub(super) fn track_row_height(&self) -> f32 {
        if self.view_mode == mixer::ViewMode::Edit {
            ROW_HEIGHT
        } else if self.track_rows_collapsed {
            MASTER_HEIGHT
        } else if self.large_track_headers() {
            LARGE_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    pub(super) fn track_size_slider(&mut self, ui: &mut egui::Ui) {
        let enabled = self.edit_shortcuts_available(ui.ctx()) && self.track_name_edit.is_none();
        let mut size = if self.track_rows_collapsed {
            0
        } else if self.timeline_tracks_large {
            2
        } else {
            1
        };
        let response = ui
            .add_enabled_ui(enabled, |ui| toolbars::track_size(ui, &mut size))
            .inner;
        #[cfg(test)]
        {
            self.track_size_bounds = response.rect;
        }
        if enabled && ui.is_enabled() && response.changed() {
            self.track_rows_collapsed = size == 0;
            self.timeline_tracks_large = size == 2;
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn track_rows_button(&mut self, ui: &mut egui::Ui) {
        let enabled = self.edit_shortcuts_available(ui.ctx()) && self.track_name_edit.is_none();
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                icons::track_rows_button(ui, self.track_rows_collapsed)
            })
            .inner;
        #[cfg(test)]
        {
            self.track_rows_bounds = response.rect;
        }
        if enabled && ui.is_enabled() && response.clicked() {
            self.track_rows_collapsed = !self.track_rows_collapsed;
            self.timeline_tracks_large = false;
            ui.ctx().request_repaint();
        }
    }
}

/// Metadata used by track headers; processing graphs and clip content stay in the project.
pub(super) fn header_snapshot(track: &daw_core::Track) -> daw_core::Track {
    daw_core::Track {
        sends: vec![],
        id: track.id,
        name: track.name.clone(),
        kind: track.kind,
        color: track.color,
        gain_db: track.gain_db,
        pan: track.pan,
        muted: track.muted,
        soloed: track.soloed,
        group_id: track.group_id,
        group_collapsed: track.group_collapsed,
        effects: Vec::new(),
        effects_compact: track.effects_compact,
        instrument: None,
        beat: None,
        midi_options: track.midi_options,
        clips: Vec::new(),
    }
}
