//! Shared catalog painting, selection, keyboard navigation, and pane focus.
use egui::{EventFilter, Key, Modifiers, Response, Ui};
use std::collections::HashSet;

/// Library selection is UI state, separate from installed units and project data.
#[derive(Default)]
pub(super) struct Catalog {
    entries: Vec<(&'static str, &'static str)>,
    pub selection: HashSet<&'static str>,
    folders: Vec<Folder>,
    folder_filter: Option<usize>,
    cursor: Row,
    anchor: Row,
    range_base: Option<HashSet<&'static str>>,
    focus: Option<egui::Id>,
    selection_focus: crate::selection_focus::SelectionFocus,
}

/// Folders belong to the catalog, never to a saved project.
struct Folder {
    name: &'static str,
    entries: Vec<usize>,
    open: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Folder(usize),
    Entry(usize),
}
impl Default for Row {
    fn default() -> Self {
        Self::Entry(0)
    }
}

#[derive(Default, PartialEq)]
pub(super) enum Action {
    #[default]
    None,
    Move,
    Add,
}

pub(super) struct Interaction {
    pub rows_enabled: bool,
    pub keyboard_enabled: bool,
    pub clear_on_empty: bool,
    pub empty_enabled: bool,
}

pub(super) struct Output {
    pub header: egui::Rect,
    pub rows: Vec<(usize, Response)>,
    pub folders: Vec<(usize, Response)>,
    pub add_selected: bool,
    pub double_clicked: Option<usize>,
    /// Entry selected by a click or keyboard movement in this frame.
    pub selected_entry: Option<usize>,
}

/// Selection is captured at the initial press, before any project mutation.
pub(super) struct DragStart {
    pub types: Vec<&'static str>,
    pub index: usize,
    pub origin: egui::Pos2,
    pub focus: egui::Id,
}

impl Catalog {
    pub fn start_drag(&mut self, ui: &Ui, response: &Response, index: usize) -> Option<DragStart> {
        let origin = crate::drag::press_origin(ui, response)?;
        self.focus_row(ui, response);
        let id = self.entries[index].0;
        let types = if self.selection.contains(id) {
            self.selected_types()
        } else {
            vec![id]
        };
        Some(DragStart {
            types,
            index,
            origin,
            focus: response.id,
        })
    }

    pub fn select_dragged(&mut self, index: usize) {
        if !self.selection.contains(self.entries[index].0) {
            self.click(index, false);
        }
    }

    pub fn new(entries: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Keep entry indices stable while folders define display and navigation order.
    pub fn with_folders(
        mut self,
        folders: impl IntoIterator<Item = (&'static str, Vec<&'static str>)>,
    ) -> Self {
        self.folders = folders
            .into_iter()
            .map(|(name, types)| Folder {
                name,
                entries: self
                    .entries
                    .iter()
                    .enumerate()
                    .filter_map(|(index, entry)| types.contains(&entry.0).then_some(index))
                    .collect(),
                open: false,
            })
            .collect();
        self
    }

    /// Set up control tests that require visible entries; startup tests use the default.
    #[cfg(test)]
    pub fn expand_folders(&mut self) {
        for folder in &mut self.folders {
            folder.open = true;
        }
    }

    /// A filter keeps entry identities and folder expansion independent of selection.
    pub fn filter_folder(&mut self, folder: Option<usize>) {
        if self.folder_filter == folder {
            return;
        }
        self.clear();
        self.folder_filter = folder;
        let index = folder.unwrap_or(0);
        if let Some(folder) = folder.and_then(|index| self.folders.get_mut(index)) {
            folder.open = true;
        }
        self.cursor = Row::Folder(index);
        self.anchor = self.cursor;
    }

    fn visible_rows(&self) -> Vec<Row> {
        if self.folders.is_empty() {
            return (0..self.entries.len()).map(Row::Entry).collect();
        }
        self.folders
            .iter()
            .enumerate()
            .filter(|(index, _)| self.folder_filter.is_none_or(|filter| filter == *index))
            .flat_map(|(index, folder)| {
                std::iter::once(Row::Folder(index)).chain(
                    folder
                        .entries
                        .iter()
                        .copied()
                        .filter(|_| folder.open)
                        .map(Row::Entry),
                )
            })
            .collect()
    }

    pub fn selected_types(&self) -> Vec<&'static str> {
        self.visible_rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Entry(index) if self.selection.contains(self.entries[index].0) => {
                    Some(self.entries[index].0)
                }
                _ => None,
            })
            .collect()
    }

    fn set_folder_open(&mut self, index: usize, open: bool) {
        self.folders[index].open = open;
        if !open {
            // Hidden entries cannot be added or included in a later group drag.
            for &entry in &self.folders[index].entries {
                self.selection.remove(self.entries[entry].0);
            }
        }
        self.cursor = Row::Folder(index);
        self.anchor = self.cursor;
        self.range_base = None;
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        panel: &crate::panels::ControlPanel,
        bounds: egui::Rect,
        title: &str,
        noun: &str,
        interaction: Interaction,
    ) -> Output {
        self.show_with_header(ui, panel, bounds, (title, noun), interaction, |_| {
            egui::Rect::NOTHING
        })
    }

    pub fn show_with_header(
        &mut self,
        ui: &mut Ui,
        panel: &crate::panels::ControlPanel,
        bounds: egui::Rect,
        labels: (&str, &str),
        interaction: Interaction,
        actions: impl FnOnce(&mut Ui) -> egui::Rect,
    ) -> Output {
        let (title, noun) = labels;
        self.begin_frame(ui, bounds);
        let header = panel.header(ui, title, actions);
        crate::panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        self.empty_region(
            ui,
            egui::Rect::from_min_max(
                header.response.rect.min,
                egui::pos2(
                    header.inner.left().min(header.response.rect.right()),
                    header.response.rect.bottom(),
                ),
            )
            .intersect(bounds),
            0,
            interaction.empty_enabled,
        );
        self.retain_focus_filter(ui);
        let action = self.keyboard(ui, interaction.keyboard_enabled);
        let mut output = Output {
            header: header.response.rect,
            rows: Vec::with_capacity(self.entries.len()),
            folders: Vec::with_capacity(self.folders.len()),
            add_selected: action == Action::Add,
            double_clicked: None,
            selected_entry: None,
        };
        let scroll = egui::ScrollArea::vertical()
            .id_salt(("catalog-rows", title))
            .max_height((bounds.bottom() - header.response.rect.bottom()).max(0.0))
            .show(ui, |ui| panel.catalog_body(ui, |ui| {
                for row in self.visible_rows() {
                    // Scope IDs by identity so collapsing an earlier folder cannot move focus.
                    let response = ui.push_id(match row {
                        Row::Folder(index) => ("catalog-folder", index),
                        Row::Entry(index) => ("catalog-entry", index),
                    }, |ui| match row {
                        Row::Folder(index) => ui.add_enabled_ui(interaction.clear_on_empty, |ui| {
                            panel.catalog_folder(ui, self.folders[index].name, self.folders[index].open)
                        }).inner,
                        Row::Entry(index) => {
                            let (type_id, name) = self.entries[index];
                            ui.add_enabled_ui(interaction.rows_enabled, |ui| {
                                panel.catalog_item(ui, name, self.selection.contains(type_id), !self.folders.is_empty())
                                    .on_hover_text(format!("Double-click to add {name}. Enter adds selected {noun}. Shift+click toggles selection."))
                            }).inner
                        }
                    }).inner;
                    let clicked = response.clicked()
                        && ui.rect_contains_pointer(response.rect)
                        && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary));
                    if clicked {
                        match row {
                            Row::Folder(index) => {
                                self.set_folder_open(index, !self.folders[index].open);
                                ui.ctx().request_repaint();
                            }
                            Row::Entry(index) => self.click(index, ui.input(|input| input.modifiers.shift)),
                        }
                        self.focus_row(ui, &response);
                    }
                    if action == Action::Move && self.cursor == row {
                        self.focus_row(ui, &response);
                        response.scroll_to_me(None);
                    }
                    match row {
                        Row::Folder(index) => output.folders.push((index, response)),
                        Row::Entry(index) => {
                            if (clicked || (action == Action::Move && self.cursor == row))
                                && self.selection.contains(self.entries[index].0) {
                                output.selected_entry = Some(index);
                            }
                            if clicked && response.double_clicked() {
                                output.double_clicked = Some(index);
                            }
                            output.rows.push((index, response));
                        }
                    }
                }
            }));
        // Include folders in the occupied area; clicking one must not clear other selections.
        let bottom = output
            .rows
            .iter()
            .chain(&output.folders)
            .map(|(_, response)| response.rect.bottom())
            .fold(header.response.rect.bottom(), f32::max)
            .min(scroll.inner_rect.bottom());
        let empty = egui::Rect::from_min_max(egui::pos2(bounds.left(), bottom), bounds.max);
        self.empty_region(ui, empty, self.entries.len(), interaction.empty_enabled);
        if interaction.clear_on_empty
            && ui.rect_contains_pointer(bounds)
            && !output
                .rows
                .iter()
                .chain(&output.folders)
                .any(|(_, row)| ui.rect_contains_pointer(row.rect))
            && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary))
        {
            self.clear();
            ui.ctx().request_repaint();
        }
        output
    }

    pub fn begin_frame(&mut self, ui: &Ui, bounds: egui::Rect) {
        self.selection_focus.set_vertical_scroll(ui, bounds);
    }

    /// Claim only blank regions so egui's hit radius cannot activate adjacent controls.
    pub fn empty_region(&mut self, ui: &Ui, rect: egui::Rect, index: usize, allowed: bool) {
        if !rect.is_positive() {
            return;
        }
        let response = ui.interact(
            rect,
            ui.id().with(("catalog-empty", index)),
            if allowed {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::hover()
            },
        );
        if allowed && response.is_pointer_button_down_on() {
            response.request_focus();
        }
    }

    pub fn clear_if_unfocused(&mut self, ctx: &egui::Context, available: bool, dragging: bool) {
        if self.selection.is_empty() {
            return;
        }
        if self.selection_focus.lost(ctx, available, dragging) {
            self.clear();
            ctx.request_repaint();
        }
    }

    pub fn clear(&mut self) {
        self.selection.clear();
        self.range_base = None;
    }

    pub fn click(&mut self, index: usize, shift: bool) {
        let type_id = self.entries[index].0;
        if shift {
            if !self.selection.remove(type_id) {
                self.selection.insert(type_id);
            }
        } else {
            self.selection = HashSet::from([type_id]);
        }
        self.cursor = Row::Entry(index);
        self.anchor = self.cursor;
        self.range_base = None;
    }

    /// Consume vertical arrows only while a library row owns keyboard focus.
    pub fn keyboard(&mut self, ui: &Ui, allowed: bool) -> Action {
        // Escape can release egui focus before this view is drawn.
        let escape = ui.input(|input| input.key_pressed(Key::Escape));
        let owns_focus = self.focus.is_some_and(|id| {
            ui.memory(|memory| {
                memory.has_focus(id)
                    || (escape && memory.focused().is_none() && memory.had_focus_last_frame(id))
            })
        });
        if !allowed
            || !owns_focus
            || (self.selection.is_empty() && matches!(self.cursor, Row::Entry(_)))
        {
            return Action::None;
        }
        let modifiers = ui.input(|input| input.modifiers);
        if modifiers.alt || modifiers.ctrl || modifiers.command || modifiers.mac_cmd {
            return Action::None;
        }
        if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            self.clear();
            return Action::None;
        }
        if !modifiers.shift {
            let enter = ui.input_mut(|input| {
                let pressed = input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            repeat: false,
                            ..
                        }
                    )
                });
                let consumed = input.consume_key(Modifiers::NONE, Key::Enter);
                pressed && consumed
            });
            if let Row::Folder(index) = self.cursor {
                let toggle = enter;
                let close =
                    ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowLeft));
                let open =
                    ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowRight));
                if toggle || close || open {
                    let open = if toggle {
                        !self.folders[index].open
                    } else {
                        open
                    };
                    self.set_folder_open(index, open);
                    return Action::Move;
                }
            } else {
                if enter && !self.selection.is_empty() {
                    return Action::Add;
                }
                if let Row::Entry(entry) = self.cursor
                    && let Some(folder) = self
                        .folders
                        .iter()
                        .position(|folder| folder.entries.contains(&entry))
                    && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowLeft))
                {
                    self.set_folder_open(folder, false);
                    return Action::Move;
                }
            }
        }
        let rows = self.visible_rows();
        let Some(current) = rows.iter().position(|row| *row == self.cursor) else {
            return Action::None;
        };
        let keys = if modifiers.shift {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        let movement = ui.input_mut(|input| {
            if input.consume_key(keys, Key::ArrowUp) {
                Some(current.saturating_sub(1))
            } else if input.consume_key(keys, Key::ArrowDown) {
                Some((current + 1).min(rows.len() - 1))
            } else {
                None
            }
        });
        let Some(next) = movement else {
            return Action::None;
        };
        if modifiers.shift {
            let base = self
                .range_base
                .get_or_insert_with(|| self.selection.clone());
            self.selection.clone_from(base);
            let anchor = rows
                .iter()
                .position(|row| *row == self.anchor)
                .unwrap_or(current);
            for row in &rows[anchor.min(next)..=anchor.max(next)] {
                if let Row::Entry(index) = row {
                    self.selection.insert(self.entries[*index].0);
                }
            }
            self.cursor = rows[next];
        } else {
            match rows[next] {
                Row::Entry(index) => self.click(index, false),
                row @ Row::Folder(_) => {
                    self.clear();
                    self.cursor = row;
                    self.anchor = row;
                }
            }
        }
        Action::Move
    }

    pub fn focus_row(&mut self, ui: &Ui, response: &Response) {
        // egui computes default arrow navigation before widgets consume the event.
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        response.request_focus();
        self.focus = Some(response.id);
        self.retain_focus_filter(ui);
    }

    pub fn retain_focus_filter(&self, ui: &Ui) {
        let Some(id) = self.focus else { return };
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                id,
                EventFilter {
                    vertical_arrows: true,
                    horizontal_arrows: !self.folders.is_empty(),
                    escape: true,
                    ..Default::default()
                },
            )
        });
    }
}
