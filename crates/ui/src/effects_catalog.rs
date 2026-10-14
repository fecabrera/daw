use egui::{EventFilter, Key, Modifiers, Response, Ui};
use std::collections::HashSet;

type Entry = (&'static str, &'static str, fn() -> daw_core::EffectInstance);
pub(super) const ENTRIES: [Entry; 5] = [
    (
        daw_core::EQ_TYPE_ID,
        "3-band EQ",
        daw_core::EffectInstance::equalizer,
    ),
    (
        daw_core::DRIVE_TYPE_ID,
        "Drive",
        daw_core::EffectInstance::drive,
    ),
    (
        daw_core::AUTO_FILTER_TYPE_ID,
        "Auto-filter",
        daw_core::EffectInstance::auto_filter,
    ),
    (
        daw_core::COMPRESSOR_TYPE_ID,
        "Compressor",
        daw_core::EffectInstance::compressor,
    ),
    (daw_core::OTT_TYPE_ID, "OTT", daw_core::EffectInstance::ott),
];

/// Library selection is UI state, separate from installed units and project data.
#[derive(Default)]
pub(super) struct Catalog {
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

impl Catalog {
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
            ui.id().with(("effects-catalog-empty", index)),
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
        let type_id = ENTRIES[index].0;
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
                Some((self.cursor + 1).min(ENTRIES.len() - 1))
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
            for entry in &ENTRIES[self.anchor.min(next)..=self.anchor.max(next)] {
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
