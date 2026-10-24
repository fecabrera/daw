//! Independent selected-clip viewport. Coordinates remain project seconds so
//! notes, waveform mapping, snapping, and seeking keep the shared project clock.
use super::*;

pub(super) struct EditorViewport {
    pub clip: Option<Id>,
    pub zoom: f32,
    pub scroll: f64,
    pub start: f64,
    pub end: f64,
    pub minimum_zoom: f32,
    pub maximum_zoom: f32,
    pub fit_zoom: f32,
    pub fitted: bool,
    pub left: f32,
    pub retain_on_selection: bool,
    #[cfg(test)]
    pub bounds: Rect,
    #[cfg(test)]
    pub ruler: Rect,
    #[cfg(test)]
    pub zoom_bounds: Rect,
    #[cfg(test)]
    pub fit_bounds: Rect,
}

impl Default for EditorViewport {
    fn default() -> Self {
        Self {
            clip: None,
            zoom: 240.0,
            scroll: 0.0,
            start: 0.0,
            end: 1.0,
            minimum_zoom: 1.0,
            maximum_zoom: 240.0 * 128.0,
            fit_zoom: 240.0,
            fitted: true,
            left: 0.0,
            retain_on_selection: false,
            #[cfg(test)]
            bounds: Rect::NOTHING,
            #[cfg(test)]
            ruler: Rect::NOTHING,
            #[cfg(test)]
            zoom_bounds: Rect::NOTHING,
            #[cfg(test)]
            fit_bounds: Rect::NOTHING,
        }
    }
}

impl EditorViewport {
    pub fn prepare(
        &mut self,
        clip: Option<&Clip>,
        width: f32,
        sample_rate_hz: u32,
        track_end: u64,
    ) {
        let Some(clip) = clip else {
            // A lower-editor edit can remove its primary clip. Keep a manually
            // retained view through empty selection; a later overview selection fits anew.
            if self.retain_on_selection || !self.fitted {
                self.clip = None;
                self.fitted = false;
                self.retain_on_selection = false;
            } else {
                *self = Self::default();
            }
            return;
        };
        let start = clip.start_frame as f64 / f64::from(sample_rate_hz);
        let duration = clip.length_frames as f64 / f64::from(sample_rate_hz);
        if self.clip != Some(clip.id) {
            self.fitted = !self.retain_on_selection;
        }
        self.retain_on_selection = false;
        self.clip = Some(clip.id);
        self.start = start;
        self.end = (track_end as f64 / f64::from(sample_rate_hz)).max(60.0) + 30.0;
        self.fit_zoom = (f64::from(width.max(1.0)) / duration) as f32;
        self.minimum_zoom = self
            .fit_zoom
            .min((f64::from(width.max(1.0)) / self.end) as f32);
        if self.fitted {
            self.zoom = self.fit_zoom;
            self.scroll = self.start;
        }
        // Content selection can switch between clips of very different lengths.
        // Keep an existing close-up even when it exceeds the new clip's fit range.
        self.maximum_zoom = (self.fit_zoom * 128.0).max(self.zoom);
        self.zoom = self.zoom.clamp(self.minimum_zoom, self.maximum_zoom);
        self.scroll = self.scroll.clamp(
            0.0,
            (self.end - f64::from(width.max(1.0)) / f64::from(self.zoom)).max(0.0),
        );
    }

    pub fn timeline(&self, project: &daw_core::Project) -> musical_time::Timeline {
        musical_time::Timeline::at_sample_rate(
            project.tempo_bpm,
            self.zoom,
            self.scroll,
            project.sample_rate_hz,
        )
    }
}

impl DawUi {
    pub(super) fn editor_scrollbar(&mut self, ui: &mut egui::Ui, bottom: f32, id: &'static str) {
        let bounds = Rect::from_min_max(
            Pos2::new(
                ui.max_rect().left() + track_overview::LABEL_WIDTH,
                bottom - SCROLLBAR_HEIGHT,
            ),
            Pos2::new(ui.max_rect().right(), bottom),
        );
        self.timeline_scrollbar_at(ui, bounds, id);
    }

    pub(super) fn editor_content_header(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        let clip = track
            .clips
            .iter()
            .find(|clip| Some(clip.id) == self.selected_clip);
        let midi = track.kind == daw_core::TrackKind::Midi;
        let scrollbar = if midi {
            ui.spacing().scroll.allocated_width()
        } else {
            0.0
        };
        let width = (ui.available_width() - track_overview::LABEL_WIDTH - scrollbar).max(1.0);
        self.midi_editor.content.prepare(
            clip,
            width,
            self.session.project.sample_rate_hz,
            track.clips.iter().map(Clip::end).max().unwrap_or(0),
        );
        let toolbar = rows::fixed(ui, toolbars::HEIGHT, |ui| {
            toolbars::Toolbar::row_with_control_height(ui, icons::TOOLBAR_BUTTON_SIZE, |ui| {
                let enabled =
                    clip.is_some() && self.editing_enabled() && !self.midi_editor.dragging();
                ui.add_enabled_ui(enabled, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut zoom = self.view_zoom();
                        let response = toolbars::horizontal_zoom(
                            ui,
                            &mut zoom,
                            self.midi_editor.content.minimum_zoom
                                ..=self.midi_editor.content.maximum_zoom,
                        );
                        #[cfg(test)]
                        {
                            self.midi_editor.content.zoom_bounds = response.rect;
                        }
                        if response.changed() {
                            self.set_view_zoom(zoom);
                        }

                        let fit = icons::fit_clip_button(ui);
                        #[cfg(test)]
                        {
                            self.midi_editor.content.fit_bounds = fit.rect;
                        }
                        if fit.clicked() {
                            self.midi_editor.content.fitted = true;
                            self.midi_editor.content.zoom = self.midi_editor.content.fit_zoom;
                            self.midi_editor.content.scroll = self.midi_editor.content.start;
                        }
                    });
                });
            });
        });
        panels::ControlPanel::pinned_row_divider(ui, toolbar.response.rect);
        panels::ControlPanel::row_dividers(ui, toolbar.response.rect, false, true);
        let viewport = Rect::from_min_max(
            ui.next_widget_position() + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
            ui.max_rect().max - Vec2::new(scrollbar, 0.0),
        );
        self.timeline_pinch(ui, viewport);
        self.timeline_swipe(ui, viewport);
        self.editor_content_ruler(ui, scrollbar);
    }

    fn editor_content_ruler(&mut self, ui: &mut egui::Ui, scrollbar: f32) {
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), RULER_HEIGHT),
            Sense::hover(),
        );
        let rect = Rect::from_min_max(
            row.min + Vec2::new(track_overview::LABEL_WIDTH, 0.0),
            row.max - Vec2::new(scrollbar, 0.0),
        );
        self.time_ruler_at(ui, rect, ui.id().with("editor-content-ruler"));
        let palette = theme::palette(ui.ctx());
        ui.painter().hline(
            row.x_range(),
            row.bottom() - 0.5,
            Stroke::new(1.0, palette.border),
        );
        self.midi_editor.content.left = rect.left();
        #[cfg(test)]
        {
            self.midi_editor.content.ruler = rect;
        }
    }
}

impl DawUi {
    pub(super) fn retain_content_view(&mut self) {
        if self.midi_editor.active_clip != self.selected_clip {
            self.midi_editor.clear_note_selection();
        }
        self.midi_editor.active_clip = self.selected_clip;
        self.midi_editor.content.fitted = false;
        self.midi_editor.content.retain_on_selection = true;
    }

    // Core owns overwrite/trim/loop/stretch resolution. This adapter only supplies
    // the shared preview ranges to either content editor, including changed neighbors.
    pub(super) fn editor_clip_preview(&self, ui: &egui::Ui) -> Option<daw_core::Project> {
        let drag = self.drag.as_ref().filter(|_| self.content_clip_drag)?;
        let pointer = ui.input(|i| i.pointer.latest_pos())?;
        if let Some(result) =
            self.linked_trim_result(drag, pointer, musical_time::snap_bypassed(ui))
        {
            return result.ok();
        }
        let raw = self.raw_clip_drag_previews(drag, pointer, musical_time::snap_bypassed(ui));
        if raw.is_empty() || raw.iter().any(|p| !p.valid) {
            return None;
        }
        let mut clips: Vec<_> = raw.into_iter().map(|p| (p.track, p.clip)).collect();
        if drag.mode == ClipDragMode::Move {
            if drag.duplicate {
                for (_, clip) in &mut clips {
                    clip.id = Id::new_v4();
                }
            }
            return self
                .session
                .project
                .placed_clips(
                    &clips
                        .into_iter()
                        .map(|(track_id, clip)| daw_core::ClipPlacement { track_id, clip })
                        .collect::<Vec<_>>(),
                )
                .ok();
        }
        let clips: Vec<_> = clips.into_iter().map(|(_, clip)| clip).collect();
        if let Some(edge) = drag.mode.stretch_edge() {
            self.session.project.stretched_clips(&clips, edge).ok()
        } else if let Some(edge) = drag.mode.resize_edge() {
            if matches!(drag.mode, ClipDragMode::TrimLeft | ClipDragMode::TrimRight) {
                self.session.project.trimmed_clips(&clips, edge).ok()
            } else {
                self.session.project.resized_clips(&clips, edge).ok()
            }
        } else {
            None
        }
    }
}
