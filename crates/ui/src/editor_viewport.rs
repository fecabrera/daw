//! Independent selected-clip viewport. Coordinates remain project seconds so
//! notes, waveform mapping, snapping, and seeking keep the shared project clock.
use super::*;

/// First source cycle, including a shortened first cycle after a loop-phase trim.
pub(super) fn original_cycle_end(clip: &Clip) -> u64 {
    clip.start_frame
        .saturating_add(clip.repeat.map_or(clip.length_frames, |repeat| {
            repeat.length_frames - repeat.phase_frame
        }))
        .min(clip.end())
}

impl DawUi {
    pub(super) fn editor_original_span(&self, lane: Rect, clip: &Clip) -> Rect {
        // Restrict only the repeat boundary. Preserve the lane clip around
        // original edges so shared fade handles remain whole at clip seams.
        let right = if clip.repeat.is_some() {
            lane.left()
                + self
                    .musical_timeline()
                    .pixel_at_frame(original_cycle_end(clip)) as f32
        } else {
            lane.right()
        };
        Rect::from_min_max(lane.min, Pos2::new(right.min(lane.right()), lane.bottom()))
    }

    pub(super) fn editor_repeat_at(
        &self,
        track: &daw_core::Track,
        lane: Rect,
        pointer: Pos2,
    ) -> bool {
        let frame = self.ruler_frame(lane, pointer);
        track.clips.iter().any(|clip| {
            clip.repeat.is_some() && frame >= original_cycle_end(clip) && frame < clip.end()
        })
    }
}

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
    #[cfg(test)]
    pub scrollbar_bounds: Rect,
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
            #[cfg(test)]
            scrollbar_bounds: Rect::NOTHING,
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
        preserve_unselected: bool,
    ) {
        let Some(clip) = clip else {
            // A lower-editor edit can remove its primary clip. Keep a manually
            // retained view through empty selection; an explicit fit restores fitted mode.
            if preserve_unselected || self.retain_on_selection || !self.fitted {
                self.clip = None;
                self.fitted = false;
                self.retain_on_selection = false;
                if preserve_unselected {
                    // Audio track clips remain navigable without a primary selection.
                    self.end = (track_end as f64 / f64::from(sample_rate_hz)).max(60.0) + 30.0;
                    self.minimum_zoom = (f64::from(width.max(1.0)) / self.end) as f32;
                    self.maximum_zoom = self.maximum_zoom.max(self.zoom);
                    self.clamp_to_content(width);
                }
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
        self.clamp_to_content(width);
    }

    /// A beat pattern uses its own source clock, never the arrangement extent.
    fn prepare_pattern(&mut self, pattern: Option<(Id, u64)>, width: f32, rate: u32) {
        let Some((id, frames)) = pattern else {
            *self = Self::default();
            return;
        };
        if self.clip != Some(id) {
            self.fitted = true;
        }
        self.clip = Some(id);
        self.retain_on_selection = false;
        self.start = 0.0;
        self.end = frames.max(1) as f64 / f64::from(rate);
        self.fit_zoom = (f64::from(width.max(1.0)) / self.end) as f32;
        self.minimum_zoom = self.fit_zoom;
        self.maximum_zoom = (self.fit_zoom * 128.0).max(self.zoom);
        if self.fitted {
            self.zoom = self.fit_zoom;
            self.scroll = 0.0;
        }
        self.clamp_to_content(width);
    }

    fn clamp_to_content(&mut self, width: f32) {
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
        .with_time_signature(project.time_signature)
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
        #[cfg(test)]
        if self.midi_editor.content_active {
            self.midi_editor.content.scrollbar_bounds = bounds;
        }
        self.timeline_scrollbar_at(ui, bounds, id);
    }

    pub(super) fn editor_content_header(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        let clip = track
            .clips
            .iter()
            .find(|clip| Some(clip.id) == self.selected_clip);
        let midi = track.kind == daw_core::TrackKind::Midi;
        let scrollbar = if midi || track.kind == daw_core::TrackKind::Beat {
            ui.spacing().scroll.allocated_width()
        } else {
            0.0
        };
        let label_width = track_overview::LABEL_WIDTH;
        let width = (ui.available_width() - label_width - scrollbar).max(1.0);
        if track.kind == daw_core::TrackKind::Beat {
            let source = clip.and_then(|clip| {
                self.session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == clip.asset_id)
                    .and_then(|a| a.beat_pattern.as_ref())
                    .map(|source| {
                        (
                            clip.id,
                            self.session
                                .project
                                .midi_frame(source.pattern.length_ticks()),
                        )
                    })
            });
            self.midi_editor.content.prepare_pattern(
                source,
                width,
                self.session.project.sample_rate_hz,
            );
        } else {
            self.midi_editor.content.prepare(
                clip,
                width,
                self.session.project.sample_rate_hz,
                track.clips.iter().map(Clip::end).max().unwrap_or(0),
                !midi,
            );
        }
        let toolbar = rows::fixed(ui, toolbars::HEIGHT, |ui| {
            toolbars::Toolbar::row_with_control_height(ui, icons::TOOLBAR_BUTTON_SIZE, |ui| {
                let enabled = (clip.is_some()
                    || (track.kind == daw_core::TrackKind::Audio && !track.clips.is_empty()))
                    && self.editing_enabled()
                    && !self.midi_editor.dragging();
                if track.kind == daw_core::TrackKind::Audio {
                    let details_enabled = self.edit_shortcuts_available(ui.ctx())
                        && !ui.ctx().text_edit_focused()
                        && self.track_name_edit.is_none();
                    if let Some(message) = self.editor_transients.toolbar(ui, details_enabled) {
                        self.diagnostic_details =
                            Some(crate::diagnostic_details::Report::Transients(message));
                    }
                } else {
                    self.editor_transients.cancel();
                }
                if track.kind == daw_core::TrackKind::Beat {
                    self.beat_pattern_options(ui, track);
                }
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

                        let fit = ui
                            .add_enabled_ui(clip.is_some(), icons::fit_clip_button)
                            .inner;
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
            ui.next_widget_position() + Vec2::new(label_width, 0.0),
            ui.max_rect().max - Vec2::new(scrollbar, 0.0),
        );
        self.timeline_pinch(ui, viewport);
        self.timeline_swipe(ui, viewport);
        self.editor_content_ruler(
            ui,
            scrollbar,
            midi || track.kind == daw_core::TrackKind::Beat,
            track.kind == daw_core::TrackKind::Beat,
            label_width,
        );
    }

    fn editor_content_ruler(
        &mut self,
        ui: &mut egui::Ui,
        scrollbar: f32,
        midi: bool,
        beat: bool,
        label_width: f32,
    ) {
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), RULER_HEIGHT),
            Sense::hover(),
        );
        let rect = Rect::from_min_max(
            row.min + Vec2::new(label_width, 0.0),
            row.max - Vec2::new(scrollbar, 0.0),
        );
        if midi {
            let gutter = Rect::from_min_max(row.min, Pos2::new(rect.left(), row.bottom()));
            let mut button_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("editor-note-audition")
                    .max_rect(Rect::from_center_size(gutter.center(), Vec2::splat(16.0)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            button_ui.add_enabled_ui(
                self.editing_enabled() && !self.midi_editor.dragging(),
                |ui| {
                    let response = icons::circular_button(
                        ui,
                        icons::Lucide::Headphones,
                        "Audition notes",
                        Some(self.midi_editor.audition_enabled),
                        16.0,
                    );
                    #[cfg(test)]
                    {
                        self.midi_editor.audition_bounds = response.rect;
                    }
                    if response.clicked() {
                        self.midi_editor.audition_enabled = !self.midi_editor.audition_enabled;
                        if !self.midi_editor.audition_enabled {
                            self.chords.audition.cancel();
                            if let Some(output) = &self.output {
                                output.stop_audition();
                            }
                        }
                    }
                },
            );
        }
        if beat {
            // Source bars are independent of project seeking and loop selection.
            let labels =
                Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - 20.0));
            let painter = ui.painter_at(rect);
            let colors = theme::palette(ui.ctx());
            painter.rect_filled(rect, 0.0, colors.background);
            painter.rect_filled(labels, 0.0, colors.panel);
            self.musical_timeline()
                .paint_ruler_ticks(&painter, rect, labels, [0, 0], false);
            ui.interact(rect, ui.id().with("beat-source-ruler"), Sense::hover())
                .on_hover_text("Select audio range");
        } else {
            self.time_ruler_at(ui, rect, ui.id().with("editor-content-ruler"));
        }
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
