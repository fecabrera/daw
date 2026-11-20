//! Timeline-only inspector rail, outside all canvas input and drop bounds.
use super::*;

const EDITOR_RESIZE_HIT_HEIGHT: f32 = 6.0;

pub(super) struct EditorPane {
    mode: LowerPane,
    height: f32,
    resize: Option<EditorResize>,
    #[cfg(test)]
    pub(super) toggle_bounds: Rect,
    #[cfg(test)]
    pub(super) divider_bounds: Rect,
}

#[derive(Default, PartialEq)]
enum LowerPane {
    #[default]
    Effects,
    Editor,
    Hidden,
}

struct EditorResize {
    previous_height: f32,
    visible_height: f32,
    origin_y: f32,
}

impl Default for EditorPane {
    fn default() -> Self {
        Self {
            mode: LowerPane::default(),
            height: 300.0,
            resize: None,
            #[cfg(test)]
            toggle_bounds: Rect::NOTHING,
            #[cfg(test)]
            divider_bounds: Rect::NOTHING,
        }
    }
}

impl EditorPane {
    pub(super) fn resizing(&self) -> bool {
        self.resize.is_some()
    }

    pub(super) fn shows_effects(&self) -> bool {
        self.mode == LowerPane::Effects
    }

    pub(super) fn show_effects(&mut self) {
        self.mode = LowerPane::Effects;
    }

    pub(super) fn layout(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        maximum: f32,
        enabled: bool,
    ) -> (f32, bool) {
        let minimum = midi_editor::LOWER_MIN_HEIGHT.min(maximum);
        let height = self.height.clamp(minimum, maximum);
        let divider = |height: f32| {
            Rect::from_min_size(
                Pos2::new(bounds.left(), bounds.bottom() - height),
                Vec2::new(bounds.width(), EDITOR_RESIZE_HIT_HEIGHT.min(height)),
            )
        };
        let mut blocked = self.resizing();
        let (focused, escape) =
            ui.input(|input| (input.focused, input.key_pressed(egui::Key::Escape)));
        let response = (enabled && focused && height > 0.0).then(|| {
            ui.interact(
                divider(height),
                ui.id().with("timeline-editor-divider"),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        });
        if escape || response.is_none() {
            if let Some(resize) = self.resize.take() {
                self.height = resize.previous_height;
            }
        } else if let Some(response) = &response {
            let button = egui::PointerButton::Primary;
            if response.drag_started_by(button)
                && let Some(origin) = ui.input(|input| input.pointer.press_origin())
            {
                self.resize = Some(EditorResize {
                    previous_height: self.height,
                    visible_height: height,
                    origin_y: origin.y,
                });
                blocked = true;
            }
            if let Some(resize) = &self.resize {
                if let Some(pointer) = response.interact_pointer_pos() {
                    self.height = (resize.visible_height + resize.origin_y - pointer.y)
                        .clamp(minimum, maximum);
                }
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                if response.drag_stopped_by(button)
                    || !ui.input(|input| input.pointer.primary_down())
                {
                    self.resize = None;
                }
            }
        }
        let height = self.height.clamp(minimum, maximum);
        #[cfg(test)]
        {
            self.divider_bounds = divider(height);
        }
        (height, blocked)
    }
}

impl DawUi {
    pub(super) fn timeline_editor_visible(&self) -> bool {
        self.view_mode == ViewMode::Timeline && self.timeline_editor.mode == LowerPane::Editor
    }

    pub(super) fn lower_editor_visible(&self) -> bool {
        self.view_mode.has_track_overview() || self.timeline_editor_visible()
    }

    pub(super) fn set_timeline_editor_visible(&mut self, visible: bool) {
        self.timeline_editor.mode = if visible {
            LowerPane::Editor
        } else {
            LowerPane::Hidden
        };
        if visible {
            self.selected_effects.clear();
            self.effects_drag = None;
        }
        self.midi_editor.cancel();
        self.midi_editor.delete_clips = true;
        self.cancel_beat_reorder(None);
        self.clear_clip_settings_edits();
    }

    pub(super) fn timeline_editor_pane(&mut self, ui: &mut egui::Ui, bounds: Rect, resize: bool) {
        if !bounds.is_positive() {
            return;
        }
        ui.painter()
            .rect_filled(bounds, 0.0, theme::palette(ui.ctx()).background);
        let mut pane = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("timeline-editor-pane")
                .max_rect(bounds)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        pane.set_clip_rect(bounds.intersect(ui.clip_rect()));
        pane.spacing_mut().item_spacing = Vec2::ZERO;
        pane.add_enabled_ui(!resize, |ui| {
            let (track, assets) = self.track_overview_content(ui);
            self.prepare_track_overview(track.as_ref());
            let Some(track) = track else {
                self.midi_editor.delete_clips = true;
                if !self.clip_settings_visible {
                    self.clear_clip_settings_edits();
                }
                ui.centered_and_justified(|ui| ui.label("Select an audio, MIDI, or beat track"));
                return;
            };
            self.update_editor_note_gestures(ui);
            self.midi_editor_panel(ui, bounds, false);
            let canvas = Rect::from_min_max(bounds.min + Vec2::new(TRACK_WIDTH, 0.0), bounds.max);
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("editor-canvas")
                    .max_rect(canvas)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            content.set_clip_rect(canvas.intersect(ui.clip_rect()));
            if let Some(beat) = self.lower_editor(&mut content, bounds, &track, &assets) {
                Self::paint_beat_editor_dividers(ui, bounds, canvas.left(), beat);
            }
        });
        panels::ControlPanel::row_dividers(ui, bounds, true, false);
    }

    pub(super) fn timeline_workspace(
        &mut self,
        ui: &mut egui::Ui,
    ) -> Option<(effects_panel::EffectTarget, Rect)> {
        let bounds = ui.max_rect();
        let rail = timeline_toolbar::right_bounds(bounds);
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
            self.track_mixer_panel(ui, settings, allowed);
        }
        target
    }

    pub(super) fn track_mixer_panel(&mut self, ui: &mut egui::Ui, bounds: Rect, enabled: bool) {
        if bounds.width() <= 0.0 {
            return;
        }
        ui.painter()
            .rect_filled(bounds, 0.0, theme::palette(ui.ctx()).panel);
        let mut inspector = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("timeline-track-mixer")
                .max_rect(bounds)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        inspector.set_clip_rect(bounds.intersect(ui.clip_rect()));
        inspector.add_enabled_ui(enabled, |ui| self.track_mixer_strips(ui));
        ui.painter().vline(
            bounds.left() + 0.5,
            bounds.y_range(),
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
        #[cfg(test)]
        {
            self.effects_bounds.track_mixer = bounds;
        }
    }

    fn track_mixer_strips(&mut self, ui: &mut egui::Ui) {
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
        let height = (mixer::MIN_HEIGHT + self.mixer_send_height() + self.mixer_gutter_height())
            .max(ui.available_height());
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
