//! Shared beat subtrack selection, headers and release-only ordering for both views.
use super::*;
use daw_core::{BeatEdit, BeatResolution, BeatSubtrack};

const STEP_INSETS: Vec2 = Vec2::new(4.0, 3.0);
const SUBTRACK_AUDITION_WIDTH: f32 = 16.0;

pub(super) fn trigger_note_name(pitch: u8) -> String {
    let name = daw_unit_core::note_name(pitch);
    if name.contains('#') {
        format!("{}#", name.replace('#', ""))
    } else {
        name
    }
}

pub(super) fn parse_trigger_note(text: &str) -> Option<f64> {
    let text = text.trim();
    let sharp = text.contains('#');
    let natural = if sharp {
        if let Some(natural) = text.strip_suffix('#') {
            natural.to_owned()
        } else {
            format!("{}{}", text.get(..1)?, text.get(1..)?.strip_prefix('#')?)
        }
    } else {
        text.to_owned()
    };
    daw_unit_core::natural_note(&natural, sharp).map(f64::from)
}

#[derive(Default)]
pub(super) struct BeatUi {
    track: Option<Id>,
    pub primary: Option<Id>,
    primary_index: usize,
    pub output_selected: bool,
    selected: HashSet<Id>,
    states: std::collections::HashMap<Id, instrument_panel::InstrumentUi>,
    pub step_index: crate::beat_grid::StepIndex,
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
    pub header_panels: std::collections::HashMap<Id, Rect>,
    #[cfg(test)]
    pub trigger_pitch_bounds: Option<[Rect; 2]>,
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
}
struct BeatDrag {
    pane: BeatDragPane,
    view: ViewMode,
    revision: u64,
    order: Vec<Id>,
    moving: Vec<Id>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BeatDragPane {
    Editor,
    Tracks,
}
impl DawUi {
    pub(super) fn cancel_beat_reorder(&mut self, pane: Option<BeatDragPane>) {
        if pane.is_none_or(|pane| self.beat_ui.drag.as_ref().is_some_and(|d| d.pane == pane)) {
            self.beat_ui.drag = None;
        }
    }
    pub(super) fn beat_reorder_in(&self, pane: BeatDragPane) -> bool {
        self.beat_ui
            .drag
            .as_ref()
            .is_some_and(|drag| drag.pane == pane)
    }
    #[cfg(test)]
    pub(super) fn beat_sampler_waveform(&self, owner: Id, layer: Id) -> Option<Rect> {
        self.instrument_ui
            .samplers
            .iter()
            .chain(
                self.beat_ui
                    .states
                    .values()
                    .flat_map(|state| &state.samplers),
            )
            .find(|bounds| bounds.track == owner && bounds.layer == layer)
            .map(|bounds| bounds.waveform)
    }
    #[cfg(test)]
    pub(super) fn cached_beat_layer_selected(&self, owner: Id, layer: Id) -> bool {
        self.beat_ui
            .states
            .get(&owner)
            .is_some_and(|state| state.selected_layers.contains(&layer))
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
            .find(|b| {
                Some(b.track) == self.selected_instrument_owner() && b.panel.contains(pointer)
            })
            .map(|b| (b.track, b.layer))
    }
    pub(super) fn selected_instrument_owner(&self) -> Option<Id> {
        let parent = self.selected_track?;
        if let Some(beat) = self.session.project.beat(parent) {
            if self.beat_ui.track != Some(parent) || self.beat_ui.output_selected {
                return Some(beat.output.id);
            }
            return self
                .beat_ui
                .primary
                .filter(|id| beat.subtracks.iter().any(|r| r.id == *id))
                .or_else(|| beat.subtracks.first().map(|r| r.id))
                .or(Some(beat.output.id));
        }
        Some(parent)
    }
    pub(super) fn reconcile_beat(&mut self, track: Id) {
        let Some(beat) = self.session.project.beat(track) else {
            return;
        };
        if self.beat_ui.track != Some(track) {
            self.beat_ui = BeatUi {
                track: Some(track),
                output_selected: true,
                ..Default::default()
            };
        }
        self.beat_ui
            .selected
            .retain(|id| beat.subtracks.iter().any(|r| r.id == *id));
        self.beat_ui
            .states
            .retain(|id, _| *id == beat.output.id || beat.subtracks.iter().any(|r| r.id == *id));
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
        if beat.subtracks.is_empty() {
            self.beat_ui.output_selected = true;
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
    pub(super) fn edit_beat(&mut self, track: Id, edit: BeatEdit) {
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
    pub(super) fn focus_beat_header(&mut self, ui: &egui::Ui, rect: Rect, focus: egui::Id) {
        self.beat_ui.focused_header = Some(focus);
        ui.ctx().memory_mut(|memory| memory.request_focus(focus));
        self.beat_ui.focus.set(ui, rect, false);
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
    /// Shared name and drag handle. The full header owns its context menu.
    pub(super) fn begin_beat_rename(&mut self, id: Id, name: String) {
        self.beat_ui.rename = Some((id, name, true));
    }
    pub(super) fn beat_renaming(&self, id: Id) -> bool {
        self.beat_ui
            .rename
            .as_ref()
            .is_some_and(|(owner, _, _)| *owner == id)
    }
    pub(super) fn beat_rename_input(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        row: &BeatSubtrack,
        font_size: f32,
    ) -> Option<egui::Response> {
        if !self.beat_renaming(row.id) {
            return None;
        }
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let (_, text, focus) = self.beat_ui.rename.as_mut().unwrap();
        let response = rows::name_input(
            ui,
            text,
            ui.make_persistent_id(("beat-name", row.id)),
            font_size,
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
        Some(response)
    }
    /// Select an owner without substituting a virtual row for the real project track.
    pub(super) fn navigate_beat_owner(&mut self, track: Id, owner: Option<Id>, shift: bool) {
        self.reconcile_beat(track);
        let Some(beat) = self.session.project.beat(track) else {
            return;
        };
        let destination = owner.unwrap_or(beat.output.id);
        if self.instrument_ui.track != Some(destination) {
            if let Some(previous) = self.instrument_ui.track
                && (previous == beat.output.id
                    || beat.subtracks.iter().any(|row| row.id == previous))
            {
                self.beat_ui
                    .states
                    .insert(previous, std::mem::take(&mut self.instrument_ui));
            }
            self.instrument_ui = self.beat_ui.states.remove(&destination).unwrap_or_default();
        }
        if let Some(owner) = owner {
            self.select_beat_subtrack(owner, shift);
        } else {
            self.beat_ui.output_selected = true;
            self.beat_ui.selected.clear();
        }
        self.clear_layer_selections_except(Some(destination));
    }
    pub(super) fn beat_name(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        row: &BeatSubtrack,
    ) -> egui::Response {
        let editing = self.beat_renaming(row.id);
        let response = self
            .beat_rename_input(ui, track, row, rows::NAME_FONT_SIZE)
            .unwrap_or_else(|| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&row.name).size(rows::NAME_FONT_SIZE))
                        .truncate()
                        .sense(Sense::click_and_drag()),
                )
            });
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
            self.begin_beat_reorder(
                ui,
                track,
                row.id,
                response.id,
                response.rect,
                BeatDragPane::Editor,
            );
        }

        response
    }
    pub(super) fn begin_beat_reorder(
        &mut self,
        ui: &egui::Ui,
        track: Id,
        id: Id,
        focus: egui::Id,
        rect: Rect,
        pane: BeatDragPane,
    ) {
        let selected = self.beat_ui.selected.clone();
        self.navigate_beat_owner(track, Some(id), false);
        if selected.contains(&id) {
            self.beat_ui.selected = selected;
        }
        self.beat_ui.focused_header = Some(focus);
        ui.memory_mut(|memory| memory.request_focus(focus));
        self.beat_ui.focus.set(ui, rect, false);
        if let Some(beat) = self.session.project.beat(track) {
            let order: Vec<_> = beat.subtracks.iter().map(|row| row.id).collect();
            let moving = order
                .iter()
                .copied()
                .filter(|id| self.beat_ui.selected.contains(id))
                .collect();
            self.beat_ui.drag = Some(BeatDrag {
                pane,
                view: self.view_mode,
                revision: self.change_revision,
                order,
                moving,
            });
        }
    }
    pub(super) fn beat_header_menu(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        row: &BeatSubtrack,
        response: &egui::Response,
        rect: Rect,
    ) {
        #[cfg(test)]
        self.beat_ui.header_panels.insert(row.id, rect);
        let enabled = ui.is_enabled() && self.editing_enabled() && !self.beat_dragging();
        let context_click = enabled
            && ui.rect_contains_pointer(rect)
            && ui.input(|i| i.focused && i.pointer.secondary_clicked());
        menus::ContextMenu::new(response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .open_memory(if !enabled {
                Some(egui::SetOpenCommand::Bool(false))
            } else {
                context_click.then_some(egui::SetOpenCommand::Bool(true))
            })
            .show(|ui| {
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
                let pitch_row = menus::value_row(ui, "Trigger pitch", 40.0, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut pitch)
                            .speed(0.1)
                            .range(0..=127)
                            .custom_formatter(|value, _| trigger_note_name(value as u8))
                            .custom_parser(parse_trigger_note)
                            .update_while_editing(false),
                    )
                    .on_hover_text(
                        "Trigger pitch: drag in semitones or type a note, such as C4 or D3#",
                    )
                });
                #[cfg(test)]
                {
                    self.beat_ui.trigger_pitch_bounds =
                        Some([pitch_row.response.rect, pitch_row.inner.rect]);
                }
                #[cfg(not(test))]
                let _ = pitch_row;
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
    }
    pub(super) fn finish_beat_rows(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        rows: &[(Id, Rect)],
        allowed: Rect,
        pane: BeatDragPane,
    ) {
        if crate::drag::cancelled(ui)
            || !self.edit_environment_available()
            || self
                .beat_ui
                .drag
                .as_ref()
                .is_some_and(|d| d.revision != self.change_revision || d.view != self.view_mode)
        {
            self.beat_ui.drag = None;
        }
        // Both panes can display the same beat. Only the gesture's origin owns
        // preview scrolling and release cleanup, including invalid outside drops.
        if let Some(drag) = self.beat_ui.drag.as_ref().filter(|drag| drag.pane == pane) {
            crate::drag::scroll_drag(ui, crate::drag::Axis::Vertical, 360.0, true, ui.clip_rect());
            let pointer = ui
                .input(|i| i.pointer.interact_pos())
                .filter(|p| allowed.contains(*p));
            ui.ctx().set_cursor_icon(if pointer.is_some() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::NotAllowed
            });
            if let Some(pointer) = pointer {
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
            if ui.input(|i| i.pointer.any_released() || !i.pointer.primary_down()) {
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
                        if panel.double_clicked()
                            && self.edit_environment_available()
                            && !self.beat_dragging()
                            && !self.beat_renaming(row.id)
                        {
                            self.begin_beat_rename(row.id, row.name.clone());
                        }
                        self.beat_header_menu(ui, track.id, row, &panel, rect);
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
                                            menus::ContextMenu::new(&response)
                                                .close_behavior(
                                                    egui::PopupCloseBehavior::CloseOnClickOutside,
                                                )
                                                .show(|ui| {
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
                self.finish_beat_rows(
                    ui,
                    track.id,
                    &row_bounds,
                    ui.clip_rect(),
                    BeatDragPane::Editor,
                );
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
    pub(super) fn beat_instrument_view(&mut self, ui: &mut egui::Ui) {
        let Some(track) = self.selected_track else {
            return;
        };
        self.reconcile_beat(track);
        let Some(beat) = self.session.project.beat(track).cloned() else {
            return;
        };
        let owner = self.selected_instrument_owner().unwrap_or(beat.output.id);
        self.beat_ui.group_viewport = Some(ui.max_rect());
        if owner != beat.output.id {
            if let Some(snapshot) = self.session.project.instrument_track(owner) {
                self.instrument_body(ui, snapshot, false);
            }
            return;
        }
        self.activate_component_drag(ui);
        let panel = panels::ControlPanel::begin(ui, ui.max_rect());
        let header = panel.header(ui, "Output", |_| {});
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        let available = ui.available_rect_before_wrap();
        let height = panels::unit_height(beat.output.collapsed).min(available.height());
        let rect = Rect::from_min_max(
            Pos2::new(available.left(), available.bottom() - height),
            available.max,
        );
        self.instrument_ui.clear_layout();
        self.instrument_ui.track = Some(owner);
        #[cfg(test)]
        {
            self.midi_editor.layer_bounds.clear();
            self.midi_editor.add_layer_bounds = Rect::NOTHING;
        }
        self.instrument_ui.component_viewports.push((owner, rect));
        let mut settings = beat.output_settings();
        let sources = modulation_panel::sources(Some(&settings));
        let providers = modulation_panel::provider_sources(&settings);
        self.instrument_main(
            ui,
            owner,
            &mut settings.main,
            crate::instrument_main::LaneRow {
                rect,
                kind: None,
                shared_sources: &[],
            },
            &sources,
            &providers,
        );
        let handled =
            self.finish_instrument_components(ui, owner, &mut settings, &[(owner, rect)], rect);
        if !handled && settings.main != beat.output {
            self.edit_beat(track, BeatEdit::SetOutput(Box::new(settings.main)));
        }
        #[cfg(test)]
        {
            self.beat_ui.final_output = Some(rect);
        }
        panel.finish(ui);
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
