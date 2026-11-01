//! Shared beat subtrack selection, headers and release-only ordering for both views.
use super::*;
use daw_core::{BeatEdit, BeatResolution, BeatSubtrack};

const STEP_INSETS: Vec2 = Vec2::new(4.0, 3.0);
const SUBTRACK_AUDITION_WIDTH: f32 = 16.0;

#[derive(Default)]
pub(super) struct BeatUi {
    track: Option<Id>,
    pub primary: Option<Id>,
    primary_index: usize,
    pub output_selected: bool,
    selected: HashSet<Id>,
    pub owner_override: Option<Id>,
    states: std::collections::HashMap<Id, instrument_panel::InstrumentUi>,
    pub step_index: crate::beat_grid::StepIndex,
    collapsed_groups: HashSet<Id>,
    rename: Option<(Id, String, bool)>,
    drag: Option<BeatDrag>,
    length_drag: Option<MonitorDrag>,
    length_preview: Option<(Id, u64, u64)>,
    length_cancelled: bool,
    focus: selection_focus::SelectionFocus,
    focused_header: Option<egui::Id>,
    group_viewport: Option<Rect>,
    #[cfg(test)]
    pub headers: std::collections::HashMap<Id, Rect>,
    #[cfg(test)]
    pub steps: std::collections::HashMap<(Id, u32), Rect>,
    #[cfg(test)]
    pub disabled_steps: Vec<(Id, Rect)>,
    #[cfg(test)]
    pub audition_steps: std::collections::HashMap<Id, Rect>,
    #[cfg(test)]
    pub final_output: Option<Rect>,
    #[cfg(test)]
    pub grid_options: Option<Rect>,
    #[cfg(test)]
    pub subdivision_bounds: Option<Rect>,
    #[cfg(test)]
    pub subdivision_items: Vec<(BeatResolution, Rect)>,
    #[cfg(test)]
    pub mix_bounds: std::collections::HashMap<Id, [Rect; 4]>,
    #[cfg(test)]
    pub groups: std::collections::HashMap<Id, (Rect, Rect)>,
    #[cfg(test)]
    pub add_layers: std::collections::HashMap<Id, Rect>,
}
struct BeatDrag {
    revision: u64,
    order: Vec<Id>,
    moving: Vec<Id>,
}
impl DawUi {
    pub(super) fn beat_group_collapsed(&self, id: Id) -> bool {
        self.beat_ui.collapsed_groups.contains(&id)
    }
    pub(super) fn toggle_beat_group(&mut self, id: Id) {
        if !self.beat_ui.collapsed_groups.remove(&id) {
            self.beat_ui.collapsed_groups.insert(id);
        }
    }
    pub(super) fn beat_dragging(&self) -> bool {
        self.beat_ui.drag.is_some() || self.beat_ui.length_drag.is_some()
    }
    pub(super) fn beat_sampler_target(&self, pointer: Pos2) -> Option<(Id, Id)> {
        if !self
            .beat_ui
            .group_viewport
            .is_some_and(|r| r.contains(pointer))
        {
            return None;
        }
        self.instrument_ui
            .samplers
            .iter()
            .chain(self.beat_ui.states.values().flat_map(|s| &s.samplers))
            .find(|b| b.panel.contains(pointer))
            .map(|b| (b.track, b.layer))
    }
    pub(super) fn selected_instrument_owner(&self) -> Option<Id> {
        if let Some(owner) = self.beat_ui.owner_override {
            return Some(owner);
        }
        let parent = self.selected_track?;
        if let Some(beat) = self.session.project.beat(parent) {
            if self.beat_ui.output_selected {
                return Some(beat.output.id);
            }
            return self
                .beat_ui
                .primary
                .filter(|id| beat.subtracks.iter().any(|r| r.id == *id))
                .or_else(|| beat.subtracks.first().map(|r| r.id));
        }
        Some(parent)
    }
    fn reconcile_beat(&mut self, track: Id) {
        let Some(beat) = self.session.project.beat(track) else {
            return;
        };
        if self.beat_ui.track != Some(track) {
            self.beat_ui = BeatUi {
                track: Some(track),
                ..Default::default()
            };
        }
        self.beat_ui
            .selected
            .retain(|id| beat.subtracks.iter().any(|r| r.id == *id));
        self.beat_ui
            .collapsed_groups
            .retain(|id| beat.subtracks.iter().any(|r| r.id == *id));
        self.beat_ui
            .states
            .retain(|id, _| beat.subtracks.iter().any(|r| r.id == *id));
        if self
            .beat_ui
            .primary
            .is_none_or(|id| !beat.subtracks.iter().any(|r| r.id == id))
        {
            self.beat_ui.primary = beat
                .subtracks
                .iter()
                .find(|r| self.beat_ui.selected.contains(&r.id))
                .or_else(|| {
                    beat.subtracks.get(
                        self.beat_ui
                            .primary_index
                            .min(beat.subtracks.len().saturating_sub(1)),
                    )
                })
                .map(|r| r.id);
        }
        if let Some(index) = beat
            .subtracks
            .iter()
            .position(|r| Some(r.id) == self.beat_ui.primary)
        {
            self.beat_ui.primary_index = index;
        }
        if self
            .beat_ui
            .rename
            .as_ref()
            .is_some_and(|(id, _, _)| !beat.subtracks.iter().any(|r| r.id == *id))
        {
            self.beat_ui.rename = None;
        }
    }
    fn edit_beat(&mut self, track: Id, edit: BeatEdit) {
        self.edit(Edit::Beat {
            track_id: track,
            edit,
        });
    }
    pub(super) fn add_beat_subtrack(&mut self, track: Id) {
        if !self.editing_enabled() {
            return;
        }
        let Some(beat) = self.session.project.beat(track) else {
            return;
        };
        let mut next = beat.clone();
        match next.add_subtrack() {
            Ok(id) => {
                let row = next.subtracks.pop().unwrap();
                self.edit_beat(
                    track,
                    BeatEdit::Add {
                        subtrack: Box::new(row),
                        index: beat.subtracks.len(),
                    },
                );
                self.beat_ui.output_selected = false;
                self.beat_ui.primary = Some(id);
                self.beat_ui.selected = HashSet::from([id]);
            }
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn clear_layer_selections_except(&mut self, owner: Option<Id>) {
        if owner.is_none() || self.instrument_ui.track != owner {
            self.instrument_ui.clear_layer_selection();
        }
        for (id, state) in &mut self.beat_ui.states {
            if Some(*id) != owner {
                state.clear_layer_selection();
            }
        }
    }
    pub(super) fn deselect_beat_subtracks(&mut self) {
        self.beat_ui.selected.clear();
    }
    pub(super) fn beat_subtrack_selected(&self, id: Id) -> bool {
        self.beat_ui.selected.contains(&id)
    }
    pub(super) fn beat_select(&mut self, ui: &egui::Ui, id: Id, rect: Rect, focus: egui::Id) {
        self.beat_ui.focused_header = Some(focus);
        ui.ctx().memory_mut(|m| m.request_focus(focus));
        self.select_beat_subtrack(id, ui.input(|i| i.modifiers.shift));
        self.beat_ui.focus.set(ui, rect, false);
    }
    fn select_beat_subtrack(&mut self, id: Id, shift: bool) {
        self.beat_ui.output_selected = false;
        self.beat_ui.primary = Some(id);
        crate::unit_selection::select(&mut self.beat_ui.selected, id, shift);
    }
    /// Shared name, context menu and drag handle. Unit controls keep their own focus.
    pub(super) fn beat_name(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        row: &BeatSubtrack,
    ) -> egui::Response {
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let editing = self
            .beat_ui
            .rename
            .as_ref()
            .is_some_and(|(id, _, _)| *id == row.id);
        let response = if editing {
            let (_, text, focus) = self.beat_ui.rename.as_mut().unwrap();
            let response = rows::name_input(
                ui,
                text,
                ui.make_persistent_id(("beat-name", row.id)),
                rows::NAME_FONT_SIZE,
                focus,
            );
            response.widget_info(|| {
                egui::WidgetInfo::text_edit(
                    ui.is_enabled(),
                    &row.name,
                    text.as_str(),
                    "Beat subtrack name",
                )
            });
            let action = if escape_pressed {
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                Some(InlineEditAction::Cancel)
            } else {
                inline_edit_action(ui, &response)
            };
            if let Some(action) = action {
                response.surrender_focus();
                if let Some((id, name, _)) = self.beat_ui.rename.take()
                    && action == InlineEditAction::Commit
                    && !name.trim().is_empty()
                    && name != row.name
                {
                    self.edit_beat(track, BeatEdit::Rename { id, name });
                }
            }
            response
        } else {
            ui.add(
                egui::Label::new(egui::RichText::new(&row.name).size(rows::NAME_FONT_SIZE))
                    .truncate()
                    .sense(Sense::click_and_drag()),
            )
        };
        #[cfg(test)]
        {
            self.beat_ui.headers.insert(row.id, response.rect);
        }
        if response.clicked() {
            response.request_focus();
            self.beat_select(ui, row.id, response.rect, response.id);
        }
        if response.double_clicked() {
            self.beat_ui.rename = Some((row.id, row.name.clone(), true));
        }
        if !editing && response.drag_started() && self.edit_environment_available() {
            response.request_focus();
            if !self.beat_ui.selected.contains(&row.id) {
                self.beat_select(ui, row.id, response.rect, response.id);
            }
            if let Some(beat) = self.session.project.beat(track) {
                let order: Vec<_> = beat.subtracks.iter().map(|r| r.id).collect();
                let moving = order
                    .iter()
                    .copied()
                    .filter(|id| self.beat_ui.selected.contains(id))
                    .collect();
                self.beat_ui.drag = Some(BeatDrag {
                    revision: self.change_revision,
                    order,
                    moving,
                });
            }
        }
        menus::ContextMenu::new(&response).show(|ui| {
            if ui.add(menus::action_button("Rename")).clicked() {
                self.beat_ui.rename = Some((row.id, row.name.clone(), true));
                ui.close();
            }
            if ui
                .add(menus::action_button("Delete selected subtracks"))
                .clicked()
            {
                let ids = if self.beat_ui.selected.contains(&row.id) {
                    self.beat_ui.selected.iter().copied().collect()
                } else {
                    vec![row.id]
                };
                self.edit_beat(track, BeatEdit::Remove { ids });
                ui.close();
            }
            menus::separator(ui);
            let mut pitch = row.trigger_pitch;
            ui.horizontal(|ui| {
                ui.label("Trigger pitch");
                ui.add(egui::DragValue::new(&mut pitch).range(0..=127));
            });
            if pitch != row.trigger_pitch {
                self.edit_beat(track, BeatEdit::SetPitch { id: row.id, pitch });
            }
            ui.menu_button("Stops", |ui| {
                let siblings = self
                    .session
                    .project
                    .beat(track)
                    .map(|b| b.subtracks.clone())
                    .unwrap_or_default();
                for other in siblings.iter().filter(|r| r.id != row.id) {
                    let mut selected = row.stops.contains(&other.id);
                    if ui.checkbox(&mut selected, &other.name).changed() {
                        let mut targets = row.stops.clone();
                        if selected {
                            targets.push(other.id);
                        } else {
                            targets.retain(|id| *id != other.id);
                        }
                        self.edit_beat(
                            track,
                            BeatEdit::SetStops {
                                id: row.id,
                                targets,
                            },
                        );
                    }
                }
            });
        });
        response
    }
    fn finish_beat_rows(&mut self, ui: &mut egui::Ui, track: Id, rows: &[(Id, Rect)]) {
        if crate::drag::cancelled(ui)
            || !self.edit_environment_available()
            || self
                .beat_ui
                .drag
                .as_ref()
                .is_some_and(|d| d.revision != self.change_revision)
        {
            self.beat_ui.drag = None;
        }
        if let Some(drag) = &self.beat_ui.drag {
            crate::drag::scroll_drag(ui, crate::drag::Axis::Vertical, 360.0, true, ui.clip_rect());
            if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
                let slot = rows.partition_point(|(_, r)| pointer.y > r.center().y);
                let y = rows.get(slot).map_or_else(
                    || rows.last().map_or(ui.min_rect().top(), |(_, r)| r.bottom()),
                    |(_, r)| r.top(),
                );
                ui.painter().hline(
                    ui.clip_rect().x_range(),
                    y,
                    Stroke::new(2.0, ui.visuals().selection.bg_fill),
                );
                if ui.input(|i| i.pointer.any_released()) && ui.clip_rect().contains(pointer) {
                    let current: Vec<_> = self
                        .session
                        .project
                        .beat(track)
                        .map(|b| b.subtracks.iter().map(|r| r.id).collect())
                        .unwrap_or_default();
                    if current == drag.order {
                        let (index, changed) =
                            crate::drag::group_insertion(&drag.order, &drag.moving, slot);
                        if changed {
                            self.edit_beat(
                                track,
                                BeatEdit::Reorder {
                                    ids: drag.moving.clone(),
                                    index,
                                },
                            );
                        }
                    }
                }
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.beat_ui.drag = None;
            }
        }
        let delete = self.beat_ui.drag.is_none()
            && !ui.input(|i| i.pointer.any_down())
            && !self.beat_ui.selected.is_empty()
            && self.beat_ui.focus.owns(ui.ctx())
            && self.beat_ui.focused_header == ui.ctx().memory(|m| m.focused())
            && ui.input(|i| i.focused)
            && self.beat_ui.rename.is_none()
            && !ui.ctx().text_edit_focused()
            && self.edit_environment_available()
            && ui
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete {
            ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
                i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace);
            });
            self.edit_beat(
                track,
                BeatEdit::Remove {
                    ids: self.beat_ui.selected.iter().copied().collect(),
                },
            );
        }
    }
    pub(super) fn beat_pattern_options(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        self.reconcile_beat(track.id);
        let active = track
            .clips
            .iter()
            .find(|c| Some(c.id) == self.selected_clip)
            .cloned();
        let mut pattern = active
            .as_ref()
            .and_then(|c| {
                self.session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)
            })
            .and_then(|a| a.beat_pattern.as_ref())
            .map(|s| s.pattern.clone());
        let original = pattern.clone();
        if active.is_none() || ui.input(|i| i.pointer.primary_pressed()) {
            self.beat_ui.length_drag = None;
            self.beat_ui.length_preview = None;
            self.beat_ui.length_cancelled = false;
        }
        if let Some(pattern) = &mut pattern {
            let clip = active.as_ref().unwrap();
            let cancelled = self.beat_ui.length_cancelled
                || !ui.is_enabled()
                || !ui.input(|i| i.focused)
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                || !self.edit_environment_available()
                || self
                    .beat_ui
                    .length_preview
                    .is_some_and(|(id, revision, _)| {
                        id != clip.id || revision != self.change_revision
                    });
            if cancelled {
                self.beat_ui.length_cancelled = true;
                self.beat_ui.length_drag = None;
                self.beat_ui.length_preview = None;
            }
            let frame = self
                .beat_ui
                .length_preview
                .map_or(clip.length_frames, |(_, _, frames)| frames);
            let tempo = self.session.project.tempo_bpm;
            let rate = self.session.project.sample_rate_hz;
            let (minimum, maximum) = if let Some(repeat) = clip.repeat {
                (
                    repeat.length_frames.min(clip.length_frames),
                    u64::MAX - clip.start_frame,
                )
            } else {
                (
                    1,
                    self.session
                        .project
                        .pattern_trim_bounds(clip, daw_core::ClipEdge::Right)
                        .map_or(clip.length_frames, |b| {
                            b.frame_limits().end() - clip.start_frame
                        }),
                )
            };
            ui.label("Length");
            let monitor = transport_monitor_control(
                ui,
                &musical_time::duration_monitor_at_rate(frame, tempo, rate),
                &format!(
                    "Clip length in bars and beats (4/4): {:.6} seconds, {frame} samples",
                    self.session.project.seconds(frame)
                ),
                0,
                Sense::hover(),
            );
            let responses = musical_sections(ui, &monitor, 0);
            #[cfg(test)]
            {
                self.beat_ui.grid_options = Some(monitor.response.rect);
            }
            if !cancelled
                && let Some(frames) = monitor_drag_position(
                    ui,
                    &responses,
                    &mut self.beat_ui.length_drag,
                    clip.length_frames,
                    &musical_steps(tempo, rate),
                    minimum..=maximum,
                )
            {
                self.beat_ui.length_preview = Some((clip.id, self.change_revision, frames));
            }
            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
                && let Some((_, _, frames)) = self.beat_ui.length_preview.take()
            {
                self.set_beat_clip_length(clip, frames);
            }
            let mut resolution = pattern.resolution;
            #[cfg(test)]
            {
                self.beat_ui.subdivision_items.clear();
            }
            let dropdown = menus::Dropdown::from_id_salt(("beat-grid", track.id))
                .selected_text(resolution_name(resolution, clip.stretch))
                .show_ui(ui, |ui| {
                    for choice in BeatResolution::ALL {
                        let response = ui.selectable_value(
                            &mut resolution,
                            choice,
                            resolution_name(choice, clip.stretch),
                        );
                        #[cfg(test)]
                        {
                            self.beat_ui.subdivision_items.push((choice, response.rect));
                        }
                        #[cfg(not(test))]
                        let _ = response;
                    }
                });
            #[cfg(test)]
            {
                self.beat_ui.subdivision_bounds = Some(dropdown.response.rect);
            }
            dropdown.response.on_hover_text(
                "Step length as a fraction of one bar (4/4), including clip stretch.",
            );
            if resolution != pattern.resolution
                && let Ok(next) = pattern.with_grid(pattern.bars, resolution)
            {
                *pattern = next;
            }
        }

        if pattern != original
            && let (Some(clip), Some(pattern)) = (active, pattern)
        {
            self.edit(Edit::ReplaceBeatPattern {
                clip_id: clip.id,
                pattern,
            });
        }
    }
    fn set_beat_clip_length(&mut self, clip: &Clip, frames: u64) {
        if !self.editing_enabled() {
            return;
        }
        let frames = frames.clamp(1, u64::MAX - clip.start_frame);
        if frames == clip.length_frames {
            return;
        }
        let edge = daw_core::ClipEdge::Right;
        let next = if clip.repeat.is_some() {
            let candidate =
                clip.looped_by(edge, i128::from(frames) - i128::from(clip.length_frames));
            self.session.project.resized_clips(&[candidate], edge)
        } else {
            let mut candidate = clip.clone();
            candidate.length_frames = frames;
            self.session.project.trimmed_clips(&[candidate], edge)
        };
        match next {
            Ok(next) => self.publish_resize(next),
            Err(error) => self.fail(error),
        }
    }
    pub(super) fn beat_grid(&mut self, ui: &mut egui::Ui, track: &daw_core::Track) {
        self.reconcile_beat(track.id);
        let Some(beat) = self.session.project.beat(track.id).cloned() else {
            return;
        };
        let active = track
            .clips
            .iter()
            .find(|c| Some(c.id) == self.selected_clip)
            .cloned();
        let mut pattern = active
            .as_ref()
            .and_then(|c| {
                self.session
                    .project
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)
            })
            .and_then(|a| a.beat_pattern.as_ref())
            .map(|s| s.pattern.clone());
        let editable = pattern.is_some();
        if !editable {
            pattern = Some(daw_core::BeatPattern::default());
        }
        let original = pattern.clone();
        let mut audition = None;
        let mut direct_audition = None;
        if let Some(pattern) = &pattern {
            self.beat_ui
                .step_index
                .prepare(active.as_ref().map(|c| c.asset_id), pattern);
        }
        #[cfg(test)]
        {
            self.beat_ui.headers.clear();
            self.beat_ui.mix_bounds.clear();
            self.beat_ui.steps.clear();
            self.beat_ui.disabled_steps.clear();
            self.beat_ui.audition_steps.clear();
        }
        let clock = self.musical_timeline();
        let width = (ui.available_width()
            - TRACK_WIDTH
            - track_overview::LABEL_WIDTH
            - ui.spacing().scroll.allocated_width())
        .max(1.0);
        let first_frame = clock.frame_at_pixel(0.0).max(0) as u64;
        let last_frame = clock.frame_at_pixel(f64::from(width)).max(0) as u64;
        // Use the clip's saved source mapping for trims, repeats and stretches.
        let mut cells = Vec::new();
        if editable && let (Some(clip), Some(pattern)) = (&active, &pattern) {
            let clock = self.musical_timeline();
            let visible = clock.frame_at_pixel(0.0).max(0) as u64;
            let base = clip.repeat.map_or(clip.length_frames, |r| r.length_frames);
            let first = visible.saturating_sub(clip.start_frame) / base;
            let count = clip.repeat.map_or(1, |_| clip.length_frames / base + 2);
            let visible_frames = clock.frame_delta(f64::from(ui.available_width())).max(0) as u64;
            let last = count.min(first.saturating_add(visible_frames / base + 3));
            let repeat_pixels = clock.pixels_for_frames(i128::from(base));
            let stride = (0.5 / repeat_pixels).ceil().max(1.0) as usize;
            for repetition in (first..last).step_by(stride) {
                let step_ticks = pattern.resolution.step_ticks();
                let phase = clip.repeat.map_or(0, |r| r.phase_frame);
                let origin = i128::from(clip.start_frame)
                    - i128::from(clip.source_offset_frame)
                    - i128::from(phase)
                    + i128::from(repetition) * i128::from(base);
                let slots = crate::beat_grid::visible_slots(
                    pattern.length_ticks() / step_ticks,
                    i128::from(first_frame)..i128::from(last_frame),
                    |slot| {
                        let frame = self.session.project.midi_frame(slot * step_ticks);
                        origin
                            + i128::from(
                                clip.stretch.map_or(frame, |s| s.scale(frame).unwrap_or(0)),
                            )
                    },
                );
                for slot in slots {
                    let tick = slot * step_ticks;
                    let start = self.session.project.midi_frame(tick);
                    let end = self
                        .session
                        .project
                        .midi_frame(tick + pattern.resolution.step_ticks());
                    let mut start = clip.stretch.map_or(start, |s| s.scale(start).unwrap_or(0));
                    let mut end = clip.stretch.map_or(end, |s| s.scale(end).unwrap_or(0));
                    if clip.repeat.is_some() {
                        start = start.max(clip.source_offset_frame);
                        end = end.min(clip.source_offset_frame.saturating_add(base));
                    }
                    if end <= start {
                        continue;
                    }
                    let local = i128::from(start)
                        - i128::from(clip.source_offset_frame)
                        - i128::from(phase)
                        + i128::from(repetition) * i128::from(base);
                    let finish = local + i128::from(end - start);
                    if finish <= 0 || local >= i128::from(clip.length_frames) {
                        continue;
                    }
                    cells.push((
                        tick,
                        clip.start_frame + local.max(0) as u64,
                        clip.start_frame + finish.min(i128::from(clip.length_frames)) as u64,
                        true,
                    ));
                }
            }
        }
        // Keep an empty disabled grid outside the selected clip as well as when
        // no clip is selected. Other clips never supply content to this display.
        let step_ticks = pattern
            .as_ref()
            .map_or(BeatResolution::Sixteenth.step_ticks(), |p| {
                p.resolution.step_ticks()
            });
        let resolution = pattern
            .as_ref()
            .map_or(BeatResolution::Sixteenth, |p| p.resolution);
        let (numerator, denominator) =
            step_length_in_beats(resolution, active.as_ref().and_then(|c| c.stretch));
        let steps_per_beat = denominator as f64 / numerator as f64;
        let grid_steps = pattern
            .as_ref()
            .map_or(16, |p| p.length_ticks() / step_ticks)
            .max(1);
        let first = (clock.beats(first_frame) * steps_per_beat).floor() as u64;
        let last = ((clock.beats(last_frame) * steps_per_beat).ceil() as u64).saturating_add(1);
        let stride = last.saturating_sub(first).div_ceil(2048).max(1) as usize;
        let seconds_per_step = 60.0 / f64::from(self.session.project.tempo_bpm) / steps_per_beat;
        for index in (first..last).step_by(stride) {
            let start = self.session.project.frames(index as f64 * seconds_per_step);
            let end = self
                .session
                .project
                .frames((index + 1) as f64 * seconds_per_step);
            if end <= start {
                continue;
            }
            let tick = (index % u64::from(grid_steps)) as u32 * step_ticks;
            if editable && let Some(clip) = &active {
                if start < clip.start_frame {
                    cells.push((tick, start, end.min(clip.start_frame), false));
                }
                if end > clip.end() {
                    cells.push((tick, start.max(clip.end()), end, false));
                }
            } else {
                cells.push((tick, start, end, false));
            }
        }
        let viewport = ui.available_rect_before_wrap();
        egui::ScrollArea::vertical()
            .id_salt(("beat-rows", track.id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut row_bounds = Vec::new();
                for row in &beat.subtracks {
                    let top = ui.next_widget_position();
                    let height = 32.0;
                    let rect = Rect::from_min_size(top, Vec2::new(ui.available_width(), height));
                    row_bounds.push((row.id, rect));
                    // Keep full-list drag geometry while omitting offscreen widgets.
                    // Retain active controls and renames until their normal gesture/focus cleanup.
                    let active_row = self.beat_ui.primary == Some(row.id)
                        && (ui.input(|i| i.pointer.any_down())
                            || ui.ctx().memory(|m| m.focused().is_some()));
                    if !rect.intersects(ui.clip_rect())
                        && !active_row
                        && !self
                            .beat_ui
                            .rename
                            .as_ref()
                            .is_some_and(|(id, _, _)| *id == row.id)
                    {
                        ui.allocate_space(rect.size());
                        continue;
                    }
                    let mut row_ui =
                        ui.new_child(egui::UiBuilder::new().id_salt(row.id).max_rect(rect));
                    row_ui.horizontal_top(|ui| {
                        let rect = Rect::from_min_size(
                            ui.next_widget_position(),
                            Vec2::new(TRACK_WIDTH, height),
                        );
                        // The background claims empty header space; child controls keep their
                        // own pointer and keyboard interactions above it.
                        let panel = ui.interact(
                            rect,
                            ui.id().with(("beat-header", row.id)),
                            Sense::click(),
                        );
                        let mut header = ui.new_child(
                            egui::UiBuilder::new()
                                .id_salt(row.id)
                                .max_rect(rect.shrink2(Vec2::new(7.0, 2.0)))
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        header.spacing_mut().item_spacing = Vec2::new(mix_controls::GROUP_GAP, 4.0);
                        let mut bus = row.instrument.main.clone();
                        let (mut muted, mut soloed) = (row.muted, row.soloed);
                        let levels = self
                            .output
                            .as_ref()
                            .and_then(|o| o.layer_meter(row.id, row.instrument.main.id))
                            .map_or([(0.0, false); 2], |meter| meter.levels());
                        let ((response, knobs, toggles), cleared) =
                            meters::stereo_panel_with_content(
                                &mut header,
                                height - 4.0,
                                levels,
                                |header| {
                                    header
                                        .add_enabled_ui(
                                            !self.beat_dragging()
                                                && self.edit_environment_available(),
                                            |ui| {
                                                mix_controls::named_row(
                                                    ui,
                                                    &mut bus,
                                                    &mut muted,
                                                    &mut soloed,
                                                    |ui| self.beat_name(ui, track.id, row),
                                                )
                                            },
                                        )
                                        .inner
                                },
                            );
                        meters::reset_on_click(
                            ui,
                            self.output
                                .as_ref()
                                .and_then(|o| o.layer_meter(row.id, row.instrument.main.id)),
                            cleared,
                        );
                        #[cfg(test)]
                        {
                            self.beat_ui.headers.insert(row.id, response.rect);
                            self.beat_ui.mix_bounds.insert(
                                row.id,
                                [
                                    toggles[0].rect,
                                    toggles[1].rect,
                                    knobs.bounds[1],
                                    knobs.bounds[0],
                                ],
                            );
                        }
                        let press = ui.input(|i| {
                            i.pointer
                                .primary_pressed()
                                .then(|| i.pointer.press_origin())
                                .flatten()
                        });
                        if ui.is_enabled()
                            && !self.beat_dragging()
                            && self.edit_environment_available()
                            && let Some(pointer) =
                                press.filter(|p| rect.intersect(ui.clip_rect()).contains(*p))
                            && !response.rect.contains(pointer)
                        {
                            let on_control = toggles.iter().any(|r| r.rect.contains(pointer))
                                || knobs.bounds.iter().any(|r| r.contains(pointer));
                            self.select_beat_subtrack(row.id, ui.input(|i| i.modifiers.shift));
                            self.beat_ui.focus.set(ui, rect, false);
                            if on_control {
                                // Keep knob/button keyboard focus and guard header-only Delete.
                                self.beat_ui.focused_header = None;
                            } else {
                                self.beat_ui.focused_header = Some(panel.id);
                                panel.request_focus();
                            }
                        }
                        #[cfg(not(test))]
                        let _ = (response, knobs, toggles);
                        if bus != row.instrument.main || muted != row.muted || soloed != row.soloed
                        {
                            self.edit_beat(
                                track.id,
                                BeatEdit::SetMix {
                                    id: row.id,
                                    gain_db: bus.gain_db,
                                    pan: bus.pan,
                                    muted,
                                    soloed,
                                },
                            );
                        }
                        ui.advance_cursor_after_rect(rect);
                        let preview_cell = Rect::from_center_size(
                            Pos2::new(
                                rect.right() + track_overview::LABEL_WIDTH * 0.5,
                                top.y + height * 0.5,
                            ),
                            Vec2::new(SUBTRACK_AUDITION_WIDTH + 2.0 * STEP_INSETS.x, height),
                        );
                        let preview = ui
                            .add_enabled_ui(self.editing_enabled(), |ui| {
                                beat_step_button(ui, preview_cell, "", false)
                            })
                            .inner
                            .on_hover_text(format!("Audition {}", row.name));
                        preview.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                preview.enabled(),
                                format!("Audition {}", row.name),
                            )
                        });
                        #[cfg(test)]
                        {
                            self.beat_ui.audition_steps.insert(row.id, preview.rect);
                        }
                        if preview.clicked() {
                            direct_audition = Some(row.id);
                        }
                        let grid_clip = Rect::from_min_max(
                            Pos2::new(rect.right() + track_overview::LABEL_WIDTH, top.y),
                            Pos2::new(ui.clip_rect().right(), top.y + height),
                        );
                        ui.set_clip_rect(grid_clip.intersect(ui.clip_rect()));
                        ui.add_enabled_ui(editable, |ui| {
                            if let Some(pattern) = &mut pattern {
                                for &(tick, frame, end, cell_editable) in &cells {
                                    ui.scope(|ui| {
                                        if !cell_editable {
                                            ui.disable();
                                        }
                                        let timeline = self.musical_timeline();
                                        let left = self.midi_editor.content.left
                                            + timeline.pixel_at_frame(frame) as f32;
                                        let right = self.midi_editor.content.left
                                            + timeline.pixel_at_frame(end) as f32;
                                        let cell = Rect::from_min_max(
                                            Pos2::new(left, top.y),
                                            Pos2::new(right, top.y + height),
                                        );
                                        if !cell.intersects(ui.clip_rect()) {
                                            return;
                                        }
                                        let step = cell_editable
                                            .then(|| {
                                                self.beat_ui.step_index.get(row.id, tick).cloned()
                                            })
                                            .flatten();
                                        let response = beat_step_button(
                                            ui,
                                            cell,
                                            if tick.is_multiple_of(daw_core::MIDI_TICKS_PER_BEAT) {
                                                "·"
                                            } else {
                                                ""
                                            },
                                            step.is_some(),
                                        );
                                        if cell_editable && let Some(clip) = &active {
                                            let frame =
                                                self.session.project.transport.playhead_frame;
                                            if frame >= clip.start_frame && frame < clip.end() {
                                                let position =
                                                    clip.source_frame(frame - clip.start_frame);
                                                let source = clip.stretch.map_or(position, |s| {
                                                    s.original_frame(position)
                                                });
                                                if source >= self.session.project.midi_frame(tick)
                                                    && source
                                                        < self.session.project.midi_frame(
                                                            tick + pattern.resolution.step_ticks(),
                                                        )
                                                {
                                                    ui.painter().rect_stroke(
                                                        response.rect,
                                                        2.0,
                                                        Stroke::new(
                                                            1.0,
                                                            ui.visuals().selection.bg_fill,
                                                        ),
                                                        egui::StrokeKind::Inside,
                                                    );
                                                }
                                            }
                                        }
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::WidgetType::Button,
                                                ui.is_enabled(),
                                                step.is_some(),
                                                format!(
                                                    "{} step {}",
                                                    row.name,
                                                    tick / pattern.resolution.step_ticks() + 1
                                                ),
                                            )
                                        });
                                        #[cfg(test)]
                                        {
                                            if cell_editable || !editable {
                                                self.beat_ui
                                                    .steps
                                                    .insert((row.id, tick), response.rect);
                                            }
                                            if !cell_editable {
                                                self.beat_ui
                                                    .disabled_steps
                                                    .push((row.id, response.rect));
                                            }
                                        }
                                        // Egui's hit radius can reach past the clip end into
                                        // disabled cells. Only pointer clicks inside this cell edit it.
                                        let pointer_inside = ui.input(|i| {
                                            !i.pointer.button_clicked(egui::PointerButton::Primary)
                                                || i.pointer.interact_pos().is_some_and(|p| {
                                                    cell.contains(p) && p.x < cell.right()
                                                })
                                        });
                                        if response.clicked()
                                            && pointer_inside
                                            && let Ok(next) = pattern.with_step(
                                                row.id,
                                                tick,
                                                if step.is_some() { None } else { Some(100) },
                                            )
                                        {
                                            *pattern = next;
                                            self.beat_ui.step_index.edited(pattern);
                                            if step.is_none() && self.midi_editor.audition_enabled {
                                                audition = Some(row.id);
                                            }
                                        }
                                        if let Some(step) = step {
                                            menus::ContextMenu::new(&response).show(|ui| {
                                                let mut velocity = step.velocity;
                                                ui.label("Velocity");
                                                ui.add(
                                                    egui::DragValue::new(&mut velocity)
                                                        .range(1..=127),
                                                );
                                                if velocity != step.velocity
                                                    && let Ok(next) = pattern.with_step(
                                                        row.id,
                                                        tick,
                                                        Some(velocity),
                                                    )
                                                {
                                                    *pattern = next;
                                                    self.beat_ui.step_index.edited(pattern);
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                        });
                    });
                    ui.advance_cursor_after_rect(rect);
                }
                self.finish_beat_rows(ui, track.id, &row_bounds);
                for (id, row) in &row_bounds {
                    if self.beat_subtrack_selected(*id) {
                        panels::ControlPanel::selected_row(
                            ui,
                            Rect::from_min_size(row.min, Vec2::new(TRACK_WIDTH, row.height())),
                            ui.clip_rect(),
                        );
                    }
                }
            });
        let grid = Rect::from_min_max(
            viewport.min + Vec2::new(TRACK_WIDTH + track_overview::LABEL_WIDTH, 0.0),
            viewport.max,
        );
        panels::ControlPanel::row_left_divider(ui, grid, viewport);
        if pattern != original
            && let (Some(clip), Some(pattern)) = (active, pattern)
        {
            self.edit(Edit::ReplaceBeatPattern {
                clip_id: clip.id,
                pattern,
            });
        }
        if let Some(row) = audition {
            self.audition_beat_step(track.id, row);
        }
        if let Some(row) = direct_audition {
            self.audition_beat_subtrack(track.id, row);
        }
    }
    pub(super) fn beat_output_mix(
        &mut self,
        ui: &mut egui::Ui,
        owner: Id,
        bus: &mut daw_core::InstrumentBusSettings,
    ) -> mix_controls::GainPanResponse {
        let Some((parent, row)) = self
            .session
            .project
            .beat_subtrack(owner)
            .map(|(p, r)| (p, r.clone()))
        else {
            return mix_controls::GainPanResponse {
                bounds: [Rect::NOTHING; 2],
            };
        };
        let (mut muted, mut soloed) = (row.muted, row.soloed);
        let (knobs, toggles) = mix_controls::header_row(
            ui,
            &mut bus.gain_db,
            &mut bus.pan,
            &mut muted,
            &mut soloed,
            ["Mute subtrack", "Solo within this beat instrument"],
        );
        #[cfg(test)]
        {
            self.beat_ui.mix_bounds.insert(
                owner,
                [
                    toggles[0].rect,
                    toggles[1].rect,
                    knobs.bounds[1],
                    knobs.bounds[0],
                ],
            );
        }
        #[cfg(not(test))]
        let _ = toggles;
        if muted != row.muted || soloed != row.soloed {
            self.edit(Edit::Beat {
                track_id: parent,
                edit: BeatEdit::SetMix {
                    id: owner,
                    gain_db: bus.gain_db,
                    pan: bus.pan,
                    muted,
                    soloed,
                },
            });
        }
        knobs
    }
    fn beat_group_header(&mut self, ui: &mut egui::Ui, parent: Id, row: &BeatSubtrack, rect: Rect) {
        let label = Rect::from_min_size(rect.min, Vec2::new(TRACK_WIDTH, MASTER_HEIGHT));
        let mut header = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((row.id, "group-header"))
                .max_rect(label)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let mut bus = row.instrument.main.clone();
        panels::PanelStyle {
            margin: 6,
            ..Default::default()
        }
        .track_frame(ui.ctx())
        .show(&mut header, |ui| {
            ui.set_min_width(TRACK_WIDTH - 14.0);
            ui.set_min_height(MASTER_HEIGHT - 14.0);
            ui.spacing_mut().item_spacing = Vec2::new(mix_controls::GROUP_GAP, 4.0);
            if self.beat_group_collapsed(row.id) {
                let levels = self
                    .output
                    .as_ref()
                    .and_then(|o| o.layer_meter(row.id, row.instrument.main.id))
                    .map_or([(0.0, false); 2], |m| m.levels());
                let cleared = meters::stereo_panel(ui, MASTER_HEIGHT - 14.0, levels, |ui| {
                    let mut rows = track_rows::HeaderRows::new(ui, true);
                    rows::centered(&mut rows.name, knobs::SIZE, |ui| {
                        let collapsed = self.beat_group_collapsed(row.id);
                        let response = icons::disclosure_button(
                            ui,
                            !collapsed,
                            if collapsed {
                                "Expand subtrack"
                            } else {
                                "Collapse subtrack"
                            },
                            self.editing_enabled(),
                            None,
                            icons::HEADER_ICON_WIDTH,
                        );
                        #[cfg(test)]
                        {
                            self.beat_ui.groups.insert(row.id, (label, response.rect));
                        }
                        if response.clicked() {
                            self.toggle_beat_group(row.id);
                        }
                        self.beat_name(ui, parent, row);
                    });
                    self.beat_output_mix(&mut rows.mix, row.id, &mut bus);
                });
                meters::reset_on_click(
                    ui,
                    self.output
                        .as_ref()
                        .and_then(|o| o.layer_meter(row.id, row.instrument.main.id)),
                    cleared,
                );
            } else {
                rows::centered(ui, knobs::SIZE, |ui| {
                    let response = icons::disclosure_button(
                        ui,
                        true,
                        "Collapse subtrack",
                        self.editing_enabled(),
                        None,
                        icons::HEADER_ICON_WIDTH,
                    );
                    #[cfg(test)]
                    {
                        self.beat_ui.groups.insert(row.id, (label, response.rect));
                    }
                    if response.clicked() {
                        self.toggle_beat_group(row.id);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let add = icons::button(
                            ui,
                            icons::Lucide::Plus,
                            "Add layer",
                            self.editing_enabled() && self.component_drag.is_none(),
                            theme::palette(ui.ctx()).text,
                        );
                        #[cfg(test)]
                        {
                            self.beat_ui.add_layers.insert(row.id, add.rect);
                        }
                        if add.clicked()
                            && let Some(mut settings) =
                                self.session.project.instrument_settings(row.id).cloned()
                        {
                            self.append_instrument_layer(&mut settings);
                            self.edit(Edit::SetInstrument {
                                track_id: row.id,
                                instrument: Some(settings),
                            });
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            self.beat_name(ui, parent, row)
                        });
                    });
                });
            }
        });
        if bus.gain_db != row.instrument.main.gain_db || bus.pan != row.instrument.main.pan {
            self.edit_beat(
                parent,
                BeatEdit::SetMix {
                    id: row.id,
                    gain_db: bus.gain_db,
                    pan: bus.pan,
                    muted: row.muted,
                    soloed: row.soloed,
                },
            );
        }
        panels::GroupPanel::row_dividers(ui, rect, 0, true, false);
        if !self.beat_group_collapsed(row.id) {
            panels::GroupPanel::row_dividers(ui, rect, 1, false, true);
        }
        ui.advance_cursor_after_rect(rect);
    }
    pub(super) fn beat_instrument_view(&mut self, ui: &mut egui::Ui) {
        let Some(track) = self.selected_track else {
            return;
        };
        self.reconcile_beat(track);
        let Some(beat) = self.session.project.beat(track).cloned() else {
            return;
        };

        if let Some(owner) = self.instrument_ui.track
            && self.session.project.instrument_snapshot(owner).is_some()
        {
            self.beat_ui
                .states
                .insert(owner, std::mem::take(&mut self.instrument_ui));
        }
        let column_bounds = ui.max_rect();
        let panel = panels::ControlPanel::begin(ui, column_bounds);
        ui.spacing_mut().item_spacing.y = -1.0;
        let header = self.instrument_lanes_header(
            ui,
            &panel,
            "Subtracks",
            "Add beat subtrack",
            self.editing_enabled(),
        );
        if header.inner.clicked() {
            self.add_beat_subtrack(track);
        }
        let final_height = panels::unit_height(beat.output.collapsed).min(ui.available_height());
        let available = ui.available_rect_before_wrap();
        let final_rect = Rect::from_min_max(
            Pos2::new(available.left(), available.bottom() - final_height),
            available.max,
        );
        if self.editing_enabled()
            && ui.is_enabled()
            && ui.rect_contains_pointer(final_rect)
            && ui.input(|i| {
                i.focused
                    && i.pointer.primary_pressed()
                    && i.pointer
                        .press_origin()
                        .is_some_and(|p| final_rect.intersect(ui.clip_rect()).contains(p))
            })
        {
            self.beat_ui.output_selected = true;
            if !ui.input(|i| i.modifiers.shift) {
                self.clear_layer_selections_except(Some(beat.output.id));
            }
        }
        let groups = egui::ScrollArea::vertical()
            .id_salt(("beat-instruments", track))
            .max_height((available.height() - final_height).max(0.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut row_bounds = Vec::new();
                #[cfg(test)]
                self.beat_ui.add_layers.clear();
                // Resolve the clicked owner before painting any group, so stale
                // cached selections cannot flash in groups rendered earlier.
                let mut position = ui.next_widget_position();
                let rows: Vec<_> = beat
                    .subtracks
                    .iter()
                    .map(|row| {
                        // Source/provider rows share one border pixel; Output retains its full height.
                        let children = if self.beat_group_collapsed(row.id) {
                            0.0
                        } else {
                            row.instrument
                                .layers
                                .iter()
                                .map(|l| panels::unit_height(l.collapsed) - 1.0)
                                .sum::<f32>()
                                + (panels::unit_height(row.instrument.lfo_lane.collapsed) - 1.0)
                                + (panels::unit_height(row.instrument.envelope_lane.collapsed)
                                    - 1.0)
                                + panels::unit_height(row.instrument.main.collapsed)
                        };
                        let height = MASTER_HEIGHT + children;
                        let rect =
                            Rect::from_min_size(position, Vec2::new(ui.available_width(), height));
                        position.y = rect.bottom() + ui.spacing().item_spacing.y;
                        (row, rect)
                    })
                    .collect();
                if self.editing_enabled()
                    && ui.is_enabled()
                    && let Some(pointer) = ui.input(|i| {
                        (i.focused && i.pointer.primary_pressed())
                            .then(|| i.pointer.press_origin())
                            .flatten()
                    })
                    && let Some((row, _)) = rows.iter().find(|(_, rect)| {
                        rect.intersect(ui.clip_rect()).contains(pointer)
                            && ui.rect_contains_pointer(*rect)
                    })
                {
                    self.beat_ui.primary = Some(row.id);
                    self.beat_ui.output_selected = false;
                    if !ui.input(|i| i.modifiers.shift) {
                        self.clear_layer_selections_except(Some(row.id));
                    }
                }
                for (row, rect) in rows {
                    self.instrument_ui = self.beat_ui.states.remove(&row.id).unwrap_or_default();
                    self.beat_ui.owner_override = Some(row.id);
                    let mut group = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(row.id)
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    let header_rect =
                        Rect::from_min_size(rect.min, Vec2::new(rect.width(), MASTER_HEIGHT));
                    self.beat_group_header(&mut group, track, row, header_rect);
                    if !self.beat_group_collapsed(row.id)
                        && let Some(snapshot) = self.session.project.instrument_track(row.id)
                    {
                        self.instrument_body(&mut group, snapshot, true);
                    } else {
                        self.instrument_ui.clear_layout();
                        if let Some(mut settings) = self.session.project.instrument_snapshot(row.id)
                        {
                            self.finish_component_drag(
                                &group,
                                row.id,
                                &mut settings,
                                &[],
                                header_rect,
                            );
                        }
                    }
                    panels::GroupPanel::row_dividers(ui, rect, 0, false, true);
                    row_bounds.push((row.id, rect));
                    ui.advance_cursor_after_rect(rect);
                    self.beat_ui
                        .states
                        .insert(row.id, std::mem::take(&mut self.instrument_ui));
                }
                self.finish_beat_rows(ui, track, &row_bounds);
                row_bounds
            });
        self.beat_ui.group_viewport = Some(groups.inner_rect);
        self.beat_ui.owner_override = None;
        let active = self.beat_ui.primary;
        let active_state = active
            .and_then(|id| self.beat_ui.states.remove(&id))
            .unwrap_or_default();
        self.instrument_ui = self
            .beat_ui
            .states
            .remove(&beat.output.id)
            .unwrap_or_default();
        self.instrument_ui.track = Some(beat.output.id);
        self.beat_ui.owner_override = Some(beat.output.id);
        if ui.input(|i| {
            i.pointer.primary_pressed()
                && i.pointer
                    .press_origin()
                    .is_some_and(|p| final_rect.contains(p))
        }) {
            self.beat_ui.output_selected = true;
        }
        let mut settings = beat.output_settings();
        let sources = modulation_panel::sources(Some(&settings));
        let providers = modulation_panel::provider_sources(&settings);
        self.instrument_main(
            ui,
            beat.output.id,
            &mut settings.main,
            crate::instrument_main::LaneRow {
                rect: final_rect,
                kind: None,
                shared_sources: &[],
            },
            &sources,
            &providers,
        );
        let handled = self.finish_instrument_components(
            ui,
            beat.output.id,
            &mut settings,
            &[(beat.output.id, final_rect)],
            final_rect,
        );
        if !handled && settings.main != beat.output {
            self.edit_beat(track, BeatEdit::SetOutput(Box::new(settings.main)));
        }
        self.beat_ui.owner_override = None;
        if self.beat_ui.output_selected {
            if let Some(id) = active {
                self.beat_ui.states.insert(id, active_state);
            }
        } else {
            self.beat_ui.states.insert(
                beat.output.id,
                std::mem::replace(&mut self.instrument_ui, active_state),
            );
        }
        #[cfg(test)]
        {
            self.beat_ui.final_output = Some(final_rect);
        }
        panel.finish(ui);
        ui.painter().hline(
            column_bounds.x_range(),
            header.response.rect.bottom() - 0.5,
            Stroke::new(1.0, theme::palette(ui.ctx()).border),
        );
        // Match Tracks: restore selection strokes after the neutral column/header borders.
        for (id, row) in &groups.inner {
            if self.beat_subtrack_selected(*id) {
                panels::ControlPanel::selected_row(
                    ui,
                    Rect::from_min_size(row.min, Vec2::new(TRACK_WIDTH, row.height())),
                    groups.inner_rect,
                );
            }
        }
        ui.advance_cursor_after_rect(available);
    }
}
/// Exact audible interval in beats, shared with placeholder geometry.
fn step_length_in_beats(
    resolution: BeatResolution,
    stretch: Option<daw_core::ClipStretch>,
) -> (u128, u128) {
    let (output, input) = stretch.map_or((1, 1), |s| {
        (u128::from(s.output_frames), u128::from(s.source_frames))
    });
    let numerator = u128::from(resolution.step_ticks()) * output;
    let denominator = u128::from(daw_core::MIDI_TICKS_PER_BEAT) * input;
    reduced_fraction(numerator, denominator)
}
fn reduced_fraction(numerator: u128, denominator: u128) -> (u128, u128) {
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (numerator / a, denominator / a)
}
fn resolution_name(resolution: BeatResolution, stretch: Option<daw_core::ClipStretch>) -> String {
    let (numerator, denominator) = step_length_in_beats(resolution, stretch);
    let (numerator, denominator) = reduced_fraction(
        numerator,
        denominator * u128::from(musical_time::BEATS_PER_BAR),
    );
    if denominator == 1 {
        return numerator.to_string();
    }
    if numerator <= 999 && denominator <= 999 {
        return format!("{numerator}/{denominator}");
    }
    // Frame-derived stretches can have large exact fractions. Keep captions compact.
    format!("{:.4}", numerator as f64 / denominator as f64)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

/// Share cell appearance and outer insets between pattern steps and row audition.
fn beat_step_button(ui: &mut egui::Ui, cell: Rect, label: &str, selected: bool) -> egui::Response {
    ui.put(
        cell.shrink2(STEP_INSETS),
        egui::Button::new(label).selected(selected),
    )
}
