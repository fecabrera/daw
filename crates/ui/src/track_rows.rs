//! Compact/regular header geometry plus the Timeline-only large sends row.
use super::*;

pub(super) struct HeaderRows {
    pub name: egui::Ui,
    pub mix: egui::Ui,
    pub sends: Option<egui::Ui>,
}

impl HeaderRows {
    pub fn new(ui: &mut egui::Ui, collapsed: bool) -> Self {
        Self::with_knobs(ui, collapsed, true, false, None)
    }

    pub fn track(ui: &mut egui::Ui, collapsed: bool) -> Self {
        Self::with_knobs(ui, collapsed, !collapsed, false, None)
    }

    /// Timeline uses equal 28-point slots; the Editor overview retains its own layout.
    pub fn track_size(ui: &mut egui::Ui, size: f32) -> Self {
        Self::with_knobs(ui, size < 2.0, size >= 2.0, size >= 3.0, Some(size))
    }

    fn with_knobs(
        ui: &mut egui::Ui,
        collapsed: bool,
        knobs: bool,
        large: bool,
        timeline_size: Option<f32>,
    ) -> Self {
        let timeline = timeline_size.is_some();
        let height = timeline_size.map_or_else(
            || {
                if collapsed {
                    knobs::SIZE
                } else {
                    ROW_HEIGHT - 14.0
                }
            },
            |size| knobs::SIZE * size,
        );
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
            // Keep the one-row layout centered while its surface grows.
            let body = Rect::from_center_size(body.center(), Vec2::new(body.width(), knobs::SIZE));
            let mix = Rect::from_min_max(Pos2::new(body.right() - mix_width, body.top()), body.max);
            (
                Rect::from_min_max(body.min, Pos2::new(mix.left() - 6.0, body.bottom())),
                mix,
            )
        } else {
            (
                Rect::from_min_size(
                    body.min,
                    Vec2::new(
                        body.width(),
                        if timeline {
                            knobs::SIZE
                        } else {
                            toolbars::CONTROL_HEIGHT
                        },
                    ),
                ),
                Rect::from_min_max(
                    body.min
                        + Vec2::new(
                            0.0,
                            if timeline {
                                height - knobs::SIZE
                            } else {
                                toolbars::CONTROL_HEIGHT + 4.0
                            },
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
                        body.min + Vec2::new(0.0, knobs::SIZE),
                        Vec2::new(body.width(), knobs::SIZE),
                    ),
                    "track-sends-row",
                )
            }),
        }
    }
}

/// Whole-size fixtures retain the established 28-point slot and 7-point outer padding.
#[cfg(test)]
pub(super) const REGULAR_HEIGHT: f32 = 2.0 * knobs::SIZE + 14.0;
#[cfg(test)]
pub(super) const LARGE_HEIGHT: f32 = 3.0 * knobs::SIZE + 14.0;

impl DawUi {
    pub(super) fn track_rows_collapsed(&self) -> bool {
        self.timeline_track_size < 2.0
    }
    pub(super) fn track_row_height(&self) -> f32 {
        if self.view_mode == mixer::ViewMode::Edit {
            ROW_HEIGHT
        } else if self.view_mode == mixer::ViewMode::Timeline {
            self.timeline_track_size * knobs::SIZE + 14.0
        } else if self.track_rows_collapsed() {
            MASTER_HEIGHT
        } else {
            ROW_HEIGHT
        }
    }

    pub(super) fn track_size_slider(&mut self, ui: &mut egui::Ui) {
        let enabled = self.edit_shortcuts_available(ui.ctx()) && self.track_name_edit.is_none();
        let mut size = self.timeline_track_size;
        let response = ui
            .add_enabled_ui(enabled, |ui| toolbars::track_size(ui, &mut size))
            .inner;
        #[cfg(test)]
        {
            self.track_size_bounds = response.rect;
        }
        if enabled && ui.is_enabled() && response.changed() {
            self.timeline_track_size = size;
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn track_rows_button(&mut self, ui: &mut egui::Ui) {
        let enabled = self.edit_shortcuts_available(ui.ctx()) && self.track_name_edit.is_none();
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                icons::track_rows_button(ui, self.track_rows_collapsed())
            })
            .inner;
        #[cfg(test)]
        {
            self.track_rows_bounds = response.rect;
        }
        if enabled && ui.is_enabled() && response.clicked() {
            self.timeline_track_size = if self.track_rows_collapsed() {
                2.0
            } else {
                1.0
            };
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
