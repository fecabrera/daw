//! Shared Instrument/Editor hierarchy. Selection/disclosure is UI state; moves commit on release.
use super::*;
use daw_core::{Project, TrackKind};
mod drag;
use drag::Gesture;

#[derive(Default)]
pub(super) struct State {
    pub visible: bool,
    editor_visible: bool,
    open: HashSet<Id>,
    known_beats: HashSet<Id>,
    cursor: Option<Id>,
    focus: Option<egui::Id>,
    project: Option<Id>,
    gesture: Option<Gesture>,
    rename: Option<Id>,
    #[cfg(test)]
    pub bounds: Bounds,
}
#[cfg(test)]
pub(super) struct Bounds {
    pub toggle: Rect,
    add: Rect,
    panel: Rect,
    rows: Vec<(Id, Rect)>,
    pub mix: HashMap<Id, [Rect; 4]>,
    pub disclosure: HashMap<Id, Rect>,
}
#[cfg(test)]
impl Default for Bounds {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            add: Rect::NOTHING,
            panel: Rect::NOTHING,
            rows: Vec::new(),
            mix: HashMap::new(),
            disclosure: HashMap::new(),
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
struct Mix {
    gain: f32,
    pan: f32,
    muted: bool,
    soloed: bool,
}
#[derive(Clone)]
struct Row {
    id: Id,
    parent: Option<Id>,
    beat: Option<Id>,
    name: String,
    kind: TrackKind,
    depth: usize,
    mix: Mix,
}
impl Row {
    fn branch(&self) -> bool {
        self.beat.is_none() && matches!(self.kind, TrackKind::Group | TrackKind::Beat)
    }
}
impl State {
    pub(super) fn show_children(&mut self, parent: Id) {
        self.open.insert(parent);
    }
    pub(super) fn has_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn reconcile(&mut self, project: &Project) {
        if self.project != Some(project.project_id) {
            self.open.clear();
            self.known_beats.clear();
            self.cursor = None;
            self.focus = None;
            self.gesture = None;
            self.rename = None;
            self.project = Some(project.project_id);
        }
        let beats: HashSet<_> = project
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Beat)
            .map(|t| t.id)
            .collect();
        self.open
            .extend(beats.difference(&self.known_beats).copied());
        self.known_beats = beats;
        self.open.retain(|id| {
            project
                .tracks
                .iter()
                .any(|t| t.id == *id && matches!(t.kind, TrackKind::Group | TrackKind::Beat))
        });
    }
    fn rows(&self, project: &Project) -> Vec<Row> {
        let mut parents = HashMap::new();
        let mut rows = Vec::new();
        for track in &project.tracks {
            let (visible, depth) = track.group_id.map_or((true, 0), |id| {
                parents.get(&id).copied().unwrap_or((false, 0))
            });
            if visible {
                rows.push(Row {
                    id: track.id,
                    parent: track.group_id,
                    beat: None,
                    name: track.name.clone(),
                    kind: track.kind,
                    depth,
                    mix: Mix {
                        gain: track.gain_db,
                        pan: track.pan,
                        muted: track.muted,
                        soloed: track.soloed,
                    },
                });
                if let Some(beat) = track
                    .beat
                    .as_ref()
                    .filter(|_| self.open.contains(&track.id))
                {
                    rows.extend(beat.subtracks.iter().map(|row| Row {
                        id: row.id,
                        parent: Some(track.id),
                        beat: Some(track.id),
                        name: row.name.clone(),
                        kind: TrackKind::Midi,
                        depth: depth + 1,
                        mix: Mix {
                            gain: row.instrument.main.gain_db,
                            pan: row.instrument.main.pan,
                            muted: row.muted,
                            soloed: row.soloed,
                        },
                    }));
                }
            }
            parents.insert(
                track.id,
                (visible && self.open.contains(&track.id), depth + 1),
            );
        }
        rows
    }
}
impl DawUi {
    pub(super) fn tracks_panel_visible(&self) -> bool {
        match self.view_mode {
            ViewMode::Instrument => self.track_browser.visible,
            ViewMode::Edit => self.track_browser.editor_visible,
            _ => false,
        }
    }
    pub(super) fn browser_track_renaming(&self, id: Id) -> bool {
        self.track_browser.rename == Some(id)
            && self
                .track_name_edit
                .as_ref()
                .is_some_and(|edit| edit.track == id)
    }
    fn begin_browser_track_rename(&mut self, row: &Row) {
        self.track_browser.rename = Some(row.id);
        self.track_name_edit = Some(TrackNameEdit {
            track: row.id,
            text: row.name.clone(),
            focus: true,
        });
    }
    pub(super) fn cancel_browser_interactions(&mut self) {
        self.cancel_beat_reorder(Some(crate::beat_editor::BeatDragPane::Tracks));
        if let Some(id) = self.track_browser.rename.take()
            && self
                .track_name_edit
                .as_ref()
                .is_some_and(|edit| edit.track == id)
        {
            self.track_name_edit = None;
        }
        if self
            .track_browser
            .gesture
            .take()
            .is_some_and(|gesture| gesture.owns(self.track_drag))
        {
            self.track_drag = None;
        }
    }
    pub(super) fn set_tracks_panel_visible(&mut self, visible: bool) {
        if !visible {
            self.cancel_browser_interactions();
        }
        match self.view_mode {
            ViewMode::Instrument => self.track_browser.visible = visible,
            ViewMode::Edit => self.track_browser.editor_visible = visible,
            _ => {}
        }
    }
    fn browser_select(&mut self, row: &Row, shift: bool) {
        let parent = row.beat.unwrap_or(row.id);
        let changed_parent = self.selected_track != Some(parent);
        if self.view_mode == ViewMode::Edit {
            if row.beat.is_none() || self.editor_track().is_none_or(|track| track.id != parent) {
                self.select_editor_track(parent);
            }
        } else {
            self.select_track(parent, false);
        }
        if self.session.project.beat(parent).is_some() {
            self.navigate_beat_owner(parent, row.beat.map(|_| row.id), shift);
        } else if changed_parent {
            self.instrument_ui.cancel();
            self.instrument_ui.selected_layer = None;
            self.instrument_ui.selected_layers.clear();
            self.instrument_ui.effects_layer = None;
            self.instrument_ui.track_effects_target = true;
        }
    }
    fn browser_mix(&mut self, ui: &mut egui::Ui, row: &Row, rect: Rect, enabled: bool) {
        let mut value = row.mix;
        let controls = Rect::from_center_size(
            Pos2::new(
                rect.right() - f32::from(menus::ITEM_PADDING) - mix_controls::BROWSER_WIDTH * 0.5,
                rect.center().y,
            ),
            Vec2::new(mix_controls::BROWSER_WIDTH, mix_controls::BROWSER_KNOB_SIZE),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("browser-mix", row.id))
                .max_rect(controls)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        let responses = child
            .add_enabled_ui(enabled, |ui| {
                mix_controls::browser_row(
                    ui,
                    &mut value.gain,
                    &mut value.pan,
                    &mut value.muted,
                    &mut value.soloed,
                )
            })
            .inner;
        #[cfg(test)]
        self.track_browser
            .bounds
            .mix
            .insert(row.id, std::array::from_fn(|index| responses[index].rect));
        #[cfg(not(test))]
        let _ = responses;
        if value != row.mix {
            if let Some(parent) = row.beat {
                self.edit_beat(
                    parent,
                    daw_core::BeatEdit::SetMix {
                        id: row.id,
                        gain_db: value.gain,
                        pan: value.pan,
                        muted: value.muted,
                        soloed: value.soloed,
                    },
                );
            } else if let Some(track) = self
                .session
                .project
                .tracks
                .iter_mut()
                .find(|t| t.id == row.id)
            {
                track.gain_db = value.gain;
                track.pan = value.pan;
                track.muted = value.muted;
                track.soloed = value.soloed;
                self.changed();
            }
        }
    }
    fn browser_add(&mut self, ui: &mut egui::Ui) -> Rect {
        let response = icons::button(
            ui,
            icons::Lucide::Plus,
            "Add track or beat subtrack",
            self.editing_enabled() && self.track_drag.is_none(),
            theme::palette(ui.ctx()).text,
        );
        #[cfg(test)]
        {
            self.track_browser.bounds.add = response.rect;
        }
        let action = menus::Dropdown::show_from_button(&response, |ui| self.add_track_menu(ui))
            .and_then(|response| response.inner);
        if let Some(action) = action {
            self.perform_track_action(action, ui.ctx());
        }
        response.rect
    }
    fn browser_name_input(&mut self, ui: &mut egui::Ui, row: &Row, rect: Rect, selected: bool) {
        let right = rect.right() - mix_controls::BROWSER_WIDTH - 8.0;
        let left = (rect.left()
            + f32::from(menus::ITEM_PADDING)
            + (row.depth + 1) as f32 * (icons::TOOLBAR_ICON_SIZE + f32::from(menus::ITEM_PADDING)))
        .min(right - 20.0);
        let name = Rect::from_min_max(Pos2::new(left, rect.top()), Pos2::new(right, rect.bottom()));
        ui.painter().rect_filled(
            name,
            0.0,
            if selected {
                theme::toggle_colors(ui.ctx(), true).0
            } else {
                theme::palette(ui.ctx()).panel
            },
        );
        let mut input = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("browser-rename", row.id))
                .max_rect(name),
        );
        input.set_clip_rect(name.intersect(ui.clip_rect()));
        let font = egui::TextStyle::Small.resolve(ui.style()).size;
        if let Some(parent) = row.beat {
            if let Some(source) = self
                .session
                .project
                .beat_subtrack(row.id)
                .map(|(_, row)| row.clone())
            {
                self.beat_rename_input(&mut input, parent, &source, font);
            }
        } else if let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.id == row.id)
            .cloned()
        {
            self.track_name_input(&mut input, &track, font);
        }
    }
    pub(super) fn project_tracks_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        enabled: bool,
    ) {
        if self
            .track_browser
            .rename
            .is_some_and(|id| !self.browser_track_renaming(id))
        {
            self.track_browser.rename = None;
        }
        self.track_browser.reconcile(&self.session.project);
        let rows = self.track_browser.rows(&self.session.project);
        let selected_parent = if self.view_mode == ViewMode::Edit {
            self.editor_track().map(|track| track.id)
        } else {
            self.selected_track
        };
        let selected = selected_parent.and_then(|parent| {
            self.session
                .project
                .beat(parent)
                .and_then(|_| {
                    (!self.beat_ui.output_selected)
                        .then_some(self.beat_ui.primary)
                        .flatten()
                })
                .or(Some(parent))
        });
        let header = panel.header(ui, "Tracks", |ui| self.browser_add(ui));
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        let own_gesture = self
            .track_browser
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.owns(self.track_drag));
        let available = (enabled || own_gesture)
            && self.edit_environment_available()
            && ui.is_enabled()
            && ui.input(|i| i.focused)
            && !ui.ctx().text_edit_focused();
        let mut select = None;
        if let Some(focus) = self.track_browser.focus {
            crate::selection_focus::retain_list_navigation(ui, focus, true);
        }
        let owns_focus = self
            .track_browser
            .focus
            .is_some_and(|id| ui.ctx().memory(|memory| memory.focused()) == Some(id));
        let mut moved = false;
        if available && owns_focus && !ui.input(|i| i.pointer.any_down()) && !rows.is_empty() {
            let index = self
                .track_browser
                .cursor
                .and_then(|id| rows.iter().position(|r| r.id == id))
                .unwrap_or(0);
            let key = ui.input_mut(|i| {
                [
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::Enter,
                ]
                .into_iter()
                .find(|key| i.consume_key(egui::Modifiers::NONE, *key))
            });
            if let Some(key) = key {
                let row = &rows[index];
                let next = match key {
                    egui::Key::ArrowUp => index.saturating_sub(1),
                    egui::Key::ArrowDown => (index + 1).min(rows.len() - 1),
                    egui::Key::ArrowLeft if !self.track_browser.open.remove(&row.id) => row
                        .parent
                        .and_then(|id| rows.iter().position(|r| r.id == id))
                        .unwrap_or(index),
                    egui::Key::ArrowRight if row.branch() => {
                        self.track_browser.open.insert(row.id);
                        index
                    }
                    egui::Key::Enter if row.kind == TrackKind::Group && row.beat.is_none() => {
                        if !self.track_browser.open.remove(&row.id) {
                            self.track_browser.open.insert(row.id);
                        }
                        index
                    }
                    egui::Key::Enter => {
                        select = Some(row.clone());
                        index
                    }
                    _ => index,
                };
                let row = &rows[next];
                self.track_browser.cursor = Some(row.id);
                if matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown)
                    && row.kind != TrackKind::Group
                {
                    select = Some(row.clone());
                }
                moved = true;
            }
        }
        let rows = if moved {
            self.track_browser.rows(&self.session.project)
        } else {
            rows
        };
        #[cfg(test)]
        {
            self.track_browser.bounds.rows.clear();
            self.track_browser.bounds.mix.clear();
            self.track_browser.bounds.disclosure.clear();
            self.track_browser.bounds.panel = bounds;
        }
        let mut painted = Vec::new();
        let scroll = egui::ScrollArea::vertical()
            .id_salt(("project-tracks", self.session.project.project_id))
            .auto_shrink([false, false])
            .max_height((bounds.bottom() - header.response.rect.bottom()).max(0.0))
            .show(ui, |ui| {
                if self.track_drag.is_some()
                    || self.beat_reorder_in(crate::beat_editor::BeatDragPane::Tracks)
                {
                    crate::drag::scroll_drag(
                        ui,
                        crate::drag::Axis::Vertical,
                        360.0,
                        true,
                        ui.clip_rect(),
                    );
                }
                panel.catalog_body(ui, |ui| {
                    if rows.is_empty() {
                        ui.label("No tracks");
                    }
                    for row in &rows {
                        let renaming =
                            self.beat_renaming(row.id) || self.browser_track_renaming(row.id);
                        let icon = if row.beat.is_some() {
                            icons::Lucide::AudioLines
                        } else if row.kind == TrackKind::Beat {
                            icons::Lucide::ListMusic
                        } else if row.kind == TrackKind::Midi {
                            icons::Lucide::KeyboardMusic
                        } else {
                            icons::Lucide::AudioLines
                        };
                        let open = row
                            .branch()
                            .then(|| self.track_browser.open.contains(&row.id));
                        let row_selected = if row.beat.is_some() {
                            self.beat_subtrack_selected(row.id) && selected_parent == row.beat
                        } else {
                            selected == Some(row.id)
                        };
                        let response = ui
                            .push_id(
                                ("project-track", self.session.project.project_id, row.id),
                                |ui| {
                                    ui.add_enabled_ui(available, |ui| {
                                        panel.project_track(
                                            ui,
                                            &row.name,
                                            row_selected,
                                            row.depth,
                                            (icon, open),
                                            mix_controls::BROWSER_WIDTH
                                                + 2.0 * f32::from(menus::ITEM_PADDING),
                                        )
                                    })
                                    .inner
                                },
                            )
                            .inner;
                        let rect = response.rect;
                        painted.push((row.clone(), rect));
                        #[cfg(test)]
                        {
                            self.track_browser.bounds.rows.push((row.id, rect));
                            if row.beat.is_some() {
                                self.beat_ui.headers.insert(
                                    row.id,
                                    Rect::from_min_max(
                                        rect.min,
                                        Pos2::new(
                                            rect.right() - mix_controls::BROWSER_WIDTH - 8.0,
                                            rect.bottom(),
                                        ),
                                    ),
                                );
                            }
                        }
                        let disclosure = Rect::from_center_size(
                            Pos2::new(
                                rect.left()
                                    + f32::from(menus::ITEM_PADDING)
                                    + row.depth as f32
                                        * (icons::TOOLBAR_ICON_SIZE
                                            + f32::from(menus::ITEM_PADDING))
                                    + icons::TOOLBAR_ICON_SIZE * 0.5,
                                rect.center().y,
                            ),
                            Vec2::splat(icons::TOOLBAR_ICON_SIZE),
                        );
                        #[cfg(test)]
                        if row.branch() {
                            self.track_browser
                                .bounds
                                .disclosure
                                .insert(row.id, disclosure);
                        }
                        if moved && self.track_browser.cursor == Some(row.id) {
                            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
                            response.request_focus();
                            response.scroll_to_me(None);
                            self.track_browser.focus = Some(response.id);
                            crate::selection_focus::retain_list_navigation(ui, response.id, true);
                        }
                        if response.clicked() && available && !crate::drag::cancelled(ui) {
                            response.request_focus();
                            self.track_browser.focus = Some(response.id);
                            self.track_browser.cursor = Some(row.id);
                            crate::selection_focus::retain_list_navigation(ui, response.id, true);
                            if row.branch()
                                && response
                                    .interact_pointer_pos()
                                    .is_some_and(|p| disclosure.expand(3.0).contains(p))
                            {
                                if !self.track_browser.open.remove(&row.id) {
                                    self.track_browser.open.insert(row.id);
                                }
                            } else {
                                select = Some(row.clone());
                            }
                            ui.ctx().request_repaint();
                        }
                        if available && let Some(origin) = crate::drag::press_origin(ui, &response)
                        {
                            self.track_browser.gesture = Some(Gesture::new(
                                row.clone(),
                                origin,
                                response.id,
                                rect,
                                self.change_revision,
                                self.view_mode,
                            ));
                        }
                        if response.double_clicked()
                            && available
                            && (!row.branch()
                                || response
                                    .interact_pointer_pos()
                                    .is_some_and(|p| !disclosure.expand(3.0).contains(p)))
                        {
                            if row.beat.is_some() {
                                self.begin_beat_rename(row.id, row.name.clone());
                            } else {
                                self.begin_browser_track_rename(row);
                            }
                            ui.ctx().request_repaint();
                        }
                        self.browser_mix(
                            ui,
                            row,
                            rect,
                            available
                                && self
                                    .track_browser
                                    .gesture
                                    .as_ref()
                                    .is_none_or(|g| !g.active),
                        );
                        if renaming {
                            self.browser_name_input(ui, row, rect, row_selected);
                        }
                        let menu =
                            ui.interact(rect, response.id.with("track-menu"), Sense::hover());
                        if let Some(parent) = row.beat {
                            if let Some(source) = self
                                .session
                                .project
                                .beat_subtrack(row.id)
                                .map(|(_, row)| row.clone())
                            {
                                self.beat_header_menu(ui, parent, &source, &menu, rect);
                            }
                        } else {
                            let enabled = ui.is_enabled()
                                && self.editing_enabled()
                                && !ui.ctx().text_edit_focused();
                            let context_click = enabled
                                && ui.rect_contains_pointer(rect)
                                && ui.input(|i| i.focused && i.pointer.secondary_clicked());
                            menus::ContextMenu::new(&menu)
                                .open_memory(if !enabled {
                                    Some(egui::SetOpenCommand::Bool(false))
                                } else {
                                    context_click.then_some(egui::SetOpenCommand::Bool(true))
                                })
                                .show(|ui| {
                                    if ui.add(menus::action_button("Rename")).clicked() {
                                        self.begin_browser_track_rename(row);
                                        ui.close();
                                    }
                                    if row.kind == TrackKind::Group {
                                        menus::separator(ui);
                                        if let Some(action) = self.track_creation_menu(ui) {
                                            self.add_track_to_group_from_menu(
                                                row.id,
                                                action,
                                                ui.ctx(),
                                            );
                                        }
                                    }
                                    if row.kind == TrackKind::Beat {
                                        menus::separator(ui);
                                        if ui
                                            .add_enabled(
                                                self.track_action_enabled(
                                                    TrackAction::AddBeat,
                                                    ui.ctx(),
                                                ),
                                                menus::action_button("Add subtrack"),
                                            )
                                            .clicked()
                                        {
                                            self.add_subtrack_from_menu(row.id, ui.ctx());
                                            ui.close();
                                        }
                                    }
                                });
                        }
                    }
                });
            });
        // Blank panel/header space shares the Plus menu; row menus retain their own actions.
        let context = ui.interact(bounds, ui.id().with("tracks-panel-menu"), Sense::hover());
        let creation_enabled =
            ui.is_enabled() && self.track_action_enabled(TrackAction::Add, ui.ctx());
        let blank_click = creation_enabled
            && ui.rect_contains_pointer(bounds)
            && ui.input(|i| {
                i.focused
                    && i.pointer.secondary_clicked()
                    && i.pointer.interact_pos().is_some_and(|point| {
                        !painted.iter().any(|(_, row)| row.contains(point))
                            && !header.inner.contains(point)
                    })
            });
        if let Some(action) = menus::ContextMenu::new(&context)
            .open_memory(if !creation_enabled {
                Some(egui::SetOpenCommand::Bool(false))
            } else {
                blank_click.then_some(egui::SetOpenCommand::Bool(true))
            })
            .show(|ui| self.track_creation_menu(ui))
            .and_then(|response| response.inner)
        {
            self.perform_track_action(action, ui.ctx());
        }
        if let Some(row) = select {
            self.browser_select(&row, ui.input(|i| i.modifiers.shift));
            if row.beat.is_some()
                && !self.beat_renaming(row.id)
                && let Some((_, rect)) = painted.iter().find(|(painted, _)| painted.id == row.id)
                && let Some(focus) = self.track_browser.focus
            {
                self.focus_beat_header(ui, *rect, focus);
            }
            ui.ctx().request_repaint();
        }
        self.finish_browser_drag(ui, scroll.inner_rect, &painted, available);
        if let Some(parent) = self
            .selected_track
            .filter(|id| self.session.project.beat(*id).is_some())
        {
            let subrows: Vec<_> = painted
                .iter()
                .filter(|(row, _)| row.beat == Some(parent))
                .map(|(row, rect)| (row.id, *rect))
                .collect();
            let allowed =
                subrows
                    .first()
                    .zip(subrows.last())
                    .map_or(Rect::NOTHING, |(first, last)| {
                        Rect::from_min_max(first.1.min, last.1.max).intersect(scroll.inner_rect)
                    });
            self.finish_beat_rows(
                ui,
                parent,
                &subrows,
                allowed,
                crate::beat_editor::BeatDragPane::Tracks,
            );
        }
    }
}
#[cfg(test)]
mod tests;
