//! Shared catalog painting, selection, keyboard navigation, and pane focus.
use egui::{EventFilter, Key, Modifiers, Response, Ui};
use std::collections::HashSet;

/// Library selection is UI state, separate from installed units and project data.
#[derive(Default)]
pub(super) struct Catalog {
    entries: Vec<(&'static str, &'static str)>,
    pub selection: HashSet<&'static str>,
    cursor: usize,
    anchor: usize,
    range_base: Option<HashSet<&'static str>>,
    focus: Option<egui::Id>,
    selection_focus: crate::selection_focus::SelectionFocus,
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
    pub rows: Vec<Response>,
    pub add_selected: bool,
    pub double_clicked: Option<usize>,
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
            self.entries
                .iter()
                .filter(|entry| self.selection.contains(entry.0))
                .map(|entry| entry.0)
                .collect()
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

    pub fn show(
        &mut self,
        ui: &mut Ui,
        panel: &crate::panels::ControlPanel,
        bounds: egui::Rect,
        title: &str,
        noun: &str,
        interaction: Interaction,
    ) -> Output {
        self.begin_frame(ui, bounds);
        let header = panel.header(ui, title, |_| {});
        crate::panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        self.empty_region(
            ui,
            header.response.rect.intersect(bounds),
            0,
            interaction.empty_enabled,
        );
        self.retain_focus_filter(ui);
        let action = self.keyboard(ui, interaction.keyboard_enabled);
        let mut output = Output {
            header: header.response.rect,
            rows: Vec::with_capacity(self.entries.len()),
            add_selected: action == Action::Add,
            double_clicked: None,
        };
        for index in 0..self.entries.len() {
            let (type_id, name) = self.entries[index];
            let response = ui.add_enabled_ui(interaction.rows_enabled, |ui| {
                panel.catalog_item(ui, name, self.selection.contains(type_id))
                    .on_hover_text(format!("Double-click to add {name}. Enter adds selected {noun}. Shift+click toggles selection."))
            }).inner;
            let clicked = response.clicked()
                && ui.rect_contains_pointer(response.rect)
                && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary));
            if clicked {
                self.click(index, ui.input(|input| input.modifiers.shift));
                self.focus_row(ui, &response);
            }
            if action == Action::Move && self.is_cursor(index) {
                self.focus_row(ui, &response);
            }
            if clicked && response.double_clicked() {
                output.double_clicked = Some(index);
            }
            output.rows.push(response);
        }
        let bottom = output
            .rows
            .last()
            .map_or(header.response.rect.bottom(), |row| row.rect.bottom());
        let empty = egui::Rect::from_min_max(egui::pos2(bounds.left(), bottom), bounds.max);
        self.empty_region(ui, empty, self.entries.len(), interaction.empty_enabled);
        if interaction.clear_on_empty
            && ui.rect_contains_pointer(bounds)
            && !output
                .rows
                .iter()
                .any(|row| ui.rect_contains_pointer(row.rect))
            && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Primary))
        {
            self.clear();
            ui.ctx().request_repaint();
        }
        output
    }

    pub fn begin_frame(&mut self, ui: &Ui, bounds: egui::Rect) {
        self.selection_focus.set(ui, bounds, false);
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
        self.cursor = index;
        self.anchor = index;
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
        if !allowed || self.selection.is_empty() || !owns_focus {
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
            let add = ui.input_mut(|input| {
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
            if add {
                return Action::Add;
            }
        }
        let keys = if modifiers.shift {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        let movement = ui.input_mut(|input| {
            if input.consume_key(keys, Key::ArrowUp) {
                Some(self.cursor.saturating_sub(1))
            } else if input.consume_key(keys, Key::ArrowDown) {
                Some((self.cursor + 1).min(self.entries.len() - 1))
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
            for entry in &self.entries[self.anchor.min(next)..=self.anchor.max(next)] {
                self.selection.insert(entry.0);
            }
            self.cursor = next;
        } else {
            self.click(next, false);
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
                    escape: true,
                    ..Default::default()
                },
            )
        });
    }

    pub fn is_cursor(&self, index: usize) -> bool {
        self.cursor == index
    }
}
