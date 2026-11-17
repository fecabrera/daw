//! Shared catalog painting, selection, keyboard navigation, and pane focus.
use egui::{Key, Modifiers, Response, Ui};
use std::collections::HashSet;

/// Library selection is UI state, separate from installed units and project data.
#[derive(Default)]
pub(super) struct Catalog {
    entries: Vec<(&'static str, &'static str)>,
    external: Vec<daw_unit_core::HostedPluginDescriptor>,
    external_selection: Option<daw_core::PluginIdentity>,
    builtin_folders: usize,
    external_enabled: bool,
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
    pub external_rows: Vec<(usize, Response)>,
    pub external_add: Option<daw_core::PluginIdentity>,
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
        self.builtin_folders = self.folders.len();
        self
    }
    pub fn set_external(&mut self, mut plugins: Vec<daw_unit_core::HostedPluginDescriptor>) {
        plugins.sort_by_cached_key(|p| p.name.to_lowercase());
        let open: Vec<_> = self
            .folders
            .iter()
            .skip(self.builtin_folders)
            .map(|f| f.open)
            .collect();
        self.folders.truncate(self.builtin_folders);
        self.external = plugins;
        self.external_enabled = true;
        for (slot, format) in [daw_core::PluginFormat::Vst3, daw_core::PluginFormat::Clap]
            .into_iter()
            .enumerate()
        {
            self.folders.push(Folder {
                name: if format == daw_core::PluginFormat::Vst3 {
                    "VST3"
                } else {
                    "CLAP"
                },
                entries: self
                    .external
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| {
                        (p.identity.format() == format).then_some(self.entries.len() + i)
                    })
                    .collect(),
                open: open.get(slot).copied().unwrap_or(false),
            });
        }
        if let Some(selected) = &self.external_selection {
            if let Some(index) = self.external.iter().position(|p| &p.identity == selected) {
                self.cursor = Row::Entry(self.entries.len() + index);
            } else {
                self.external_selection = None;
                self.cursor = Row::Folder(self.builtin_folders);
            }
        }
    }

    pub fn enable_external(&mut self, enabled: bool) {
        self.external_enabled = enabled;
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
                Row::Entry(index)
                    if self
                        .entries
                        .get(index)
                        .is_some_and(|e| self.selection.contains(e.0)) =>
                {
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
                if let Some(e) = self.entries.get(entry) {
                    self.selection.remove(e.0);
                } else if self
                    .external
                    .get(entry - self.entries.len())
                    .is_some_and(|p| self.external_selection.as_ref() == Some(&p.identity))
                {
                    self.external_selection = None;
                }
            }
        }
        self.cursor = Row::Folder(index);
        self.anchor = self.cursor;
        self.range_base = None;
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
        let (title, _noun) = labels;
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
            external_rows: Vec::new(),
            external_add: if action == Action::Add {
                self.external_selection.clone()
            } else {
                None
            },
            rows: Vec::with_capacity(self.entries.len()),
            folders: Vec::with_capacity(self.folders.len()),
            add_selected: action == Action::Add,
            double_clicked: None,
            selected_entry: None,
        };
        let scroll = egui::ScrollArea::vertical()
            .id_salt(("catalog-rows", title))
            .max_height((bounds.bottom() - header.response.rect.bottom()).max(0.0))
            .show(ui, |ui| {
                panel.catalog_body(ui, |ui| {
                    for row in self.visible_rows() {
                        // Scope IDs by identity so collapsing an earlier folder cannot move focus.
                        let response = ui
                            .push_id(
                                match row {
                                    Row::Folder(index) => egui::Id::new(("catalog-folder", index)),
                                    Row::Entry(index) if index >= self.entries.len() => {
                                        egui::Id::new((
                                            "catalog-native-entry",
                                            &self.external[index - self.entries.len()].identity,
                                        ))
                                    }
                                    Row::Entry(index) => egui::Id::new(("catalog-entry", index)),
                                },
                                |ui| match row {
                                    Row::Folder(index) => {
                                        ui.add_enabled_ui(interaction.clear_on_empty, |ui| {
                                            panel.catalog_folder(
                                                ui,
                                                self.folders[index].name,
                                                self.folders[index].open,
                                            )
                                        })
                                        .inner
                                    }
                                    Row::Entry(index) => {
                                        let (name, selected) = if let Some((id, name)) =
                                            self.entries.get(index)
                                        {
                                            (*name, self.selection.contains(id))
                                        } else {
                                            let entry = &self.external[index - self.entries.len()];
                                            (
                                                entry.name.as_str(),
                                                self.external_selection.as_ref()
                                                    == Some(&entry.identity),
                                            )
                                        };
                                        ui.add_enabled_ui(
                                            interaction.rows_enabled
                                                && (index < self.entries.len()
                                                    || self.external_enabled),
                                            |ui| {
                                                panel
                                                    .catalog_item(
                                                        ui,
                                                        name,
                                                        selected,
                                                        !self.folders.is_empty(),
                                                    )
                                                    .on_hover_text(name)
                                            },
                                        )
                                        .inner
                                    }
                                },
                            )
                            .inner;
                        let clicked = response.clicked()
                            && ui.rect_contains_pointer(response.rect)
                            && ui.input(|input| {
                                input.pointer.button_clicked(egui::PointerButton::Primary)
                            });
                        if clicked {
                            match row {
                                Row::Folder(index) => {
                                    self.set_folder_open(index, !self.folders[index].open);
                                    ui.ctx().request_repaint();
                                }
                                Row::Entry(index) => {
                                    self.click(index, ui.input(|input| input.modifiers.shift))
                                }
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
                                if index >= self.entries.len() {
                                    if clicked && response.double_clicked() {
                                        output.external_add = Some(
                                            self.external[index - self.entries.len()]
                                                .identity
                                                .clone(),
                                        );
                                    }
                                    output.external_rows.push((index, response));
                                } else {
                                    if (clicked || (action == Action::Move && self.cursor == row))
                                        && self.selection.contains(self.entries[index].0)
                                    {
                                        output.selected_entry = Some(index);
                                    }
                                    if clicked && response.double_clicked() {
                                        output.double_clicked = Some(index);
                                    }
                                    output.rows.push((index, response));
                                }
                            }
                        }
                    }
                })
            });
        // Include folders in the occupied area; clicking one must not clear other selections.
        let bottom = output
            .rows
            .iter()
            .chain(&output.folders)
            .chain(&output.external_rows)
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
                .chain(&output.external_rows)
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
        if self.selection.is_empty() && self.external_selection.is_none() {
            return;
        }
        if self.selection_focus.lost(ctx, available, dragging) {
            self.clear();
            ctx.request_repaint();
        }
    }

    pub fn clear(&mut self) {
        self.selection.clear();
        self.external_selection = None;
        self.range_base = None;
    }

    pub fn click(&mut self, index: usize, shift: bool) {
        if index >= self.entries.len() {
            let identity = self.external[index - self.entries.len()].identity.clone();
            let deselect = shift && self.external_selection.as_ref() == Some(&identity);
            self.clear();
            if !deselect {
                self.external_selection = Some(identity);
            }
            self.cursor = Row::Entry(index);
            self.anchor = self.cursor;
            return;
        }
        self.external_selection = None;
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
            || (self.selection.is_empty()
                && self.external_selection.is_none()
                && matches!(self.cursor, Row::Entry(_)))
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
                if enter
                    && (!self.selection.is_empty()
                        || self.external_enabled && self.external_selection.is_some())
                {
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
        if modifiers.shift && matches!(rows[next],Row::Entry(i) if i>=self.entries.len()) {
            if let Row::Entry(index) = rows[next] {
                self.click(index, true);
            }
        } else if modifiers.shift {
            self.external_selection = None;
            let base = self
                .range_base
                .get_or_insert_with(|| self.selection.clone());
            self.selection.clone_from(base);
            let anchor = rows
                .iter()
                .position(|row| *row == self.anchor)
                .unwrap_or(current);
            for row in &rows[anchor.min(next)..=anchor.max(next)] {
                if let Row::Entry(index) = row
                    && let Some(entry) = self.entries.get(*index)
                {
                    self.selection.insert(entry.0);
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
        crate::selection_focus::retain_list_navigation(ui, id, !self.folders.is_empty());
    }
}

#[cfg(test)]
mod plugin_tests {
    use super::*;
    use daw_plugin_host::*;
    fn descriptor(
        format: PluginFormat,
        role: HostedRole,
        name: &str,
        tag: u8,
    ) -> HostedPluginDescriptor {
        HostedPluginDescriptor {
            identity: if format == PluginFormat::Clap {
                PluginIdentity::Clap(format!("org.daw.{tag}"))
            } else {
                PluginIdentity::Vst3([tag; 16])
            },
            name: name.into(),
            vendor: "DAW".into(),
            version: "1".into(),
            role,
        }
    }
    #[test]
    fn catalogs_group_each_format_and_filter_roles_without_changing_the_project() {
        let mut app = crate::DawUi::default();
        let before = app.session.project.clone();
        let plugins = vec![
            descriptor(PluginFormat::Vst3, HostedRole::Effect, "Effect VST", 1),
            descriptor(PluginFormat::Clap, HostedRole::Effect, "Effect CLAP", 2),
            descriptor(
                PluginFormat::Vst3,
                HostedRole::Instrument,
                "Instrument VST",
                3,
            ),
            descriptor(
                PluginFormat::Clap,
                HostedRole::Instrument,
                "Instrument CLAP",
                4,
            ),
        ];
        let mut registry = PluginRegistry::default();
        for plugin in &plugins {
            let path = std::path::PathBuf::from(format!(
                "fixture-{}.{}",
                plugin.name,
                plugin.identity.format().extension()
            ));
            registry.entries.push(RegistryEntry {
                candidate: PluginCandidate {
                    format: plugin.identity.format(),
                    path: path.clone(),
                    executable: path,
                    fingerprint: "a".repeat(64),
                },
                plugins: vec![plugin.clone()],
                state: ScanState::Ready,
                present: true,
            });
        }
        app.set_plugin_catalog(&registry);
        for (catalog, role) in [
            (&app.effects_catalog, HostedRole::Effect),
            (&app.instrument_catalog, HostedRole::Instrument),
        ] {
            assert_eq!(
                catalog
                    .folders
                    .iter()
                    .skip(catalog.builtin_folders)
                    .map(|f| f.name)
                    .collect::<Vec<_>>(),
                ["VST3", "CLAP"]
            );
            assert_eq!(catalog.external.len(), 2);
            assert!(catalog.external.iter().all(|p| p.role == role));
            for (i, format) in [PluginFormat::Vst3, PluginFormat::Clap]
                .into_iter()
                .enumerate()
            {
                let folder = &catalog.folders[catalog.builtin_folders + i];
                assert_eq!(folder.entries.len(), 1);
                assert_eq!(
                    catalog.external[folder.entries[0] - catalog.entries.len()]
                        .identity
                        .format(),
                    format
                );
            }
        }
        assert_eq!(app.session.project, before);
    }
    #[test]
    fn native_selection_survives_catalog_refresh_and_does_not_become_a_builtin_drag() {
        let plugin = descriptor(PluginFormat::Clap, HostedRole::Effect, "Owned", 2);
        let mut catalog =
            Catalog::new([("builtin", "Built in")]).with_folders([("Builtin", vec!["builtin"])]);
        catalog.set_external(vec![plugin.clone()]);
        catalog.expand_folders();
        catalog.click(1, false);
        assert!(catalog.selected_types().is_empty());
        assert_eq!(catalog.external_selection, Some(plugin.identity.clone()));
        catalog.set_external(vec![
            descriptor(PluginFormat::Clap, HostedRole::Effect, "Earlier", 3),
            plugin.clone(),
        ]);
        assert_eq!(catalog.external_selection, Some(plugin.identity));
        assert!(catalog.cursor == Row::Entry(2));
        catalog.click(0, false);
        assert_eq!(catalog.selected_types(), vec!["builtin"]);
        assert!(catalog.external_selection.is_none());
    }
}
