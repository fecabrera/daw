use super::*;

impl DawUi {
    pub(super) fn track_selected(&self, id: Id) -> bool {
        self.selected_track == Some(id) || self.selected_tracks.contains(&id)
    }

    pub(super) fn select_track(&mut self, id: Id, toggle: bool) {
        self.master_selected = false;
        if toggle {
            if let Some(primary) = self.selected_track {
                self.selected_tracks.insert(primary);
            }
            if !self.selected_tracks.remove(&id) {
                self.selected_tracks.insert(id);
                self.selected_track = Some(id);
            } else {
                self.selected_track = self
                    .session
                    .project
                    .tracks
                    .iter()
                    .find(|track| self.selected_tracks.contains(&track.id))
                    .map(|track| track.id);
            }
        } else {
            self.selected_tracks.clear();
            self.selected_tracks.insert(id);
            self.selected_track = Some(id);
        }
    }

    pub(super) fn select_master(&mut self) {
        self.master_selected = true;
        self.selected_track = None;
        self.selected_tracks.clear();
        self.selected_effects.clear();
        self.effects_catalog.clear();
        self.effects_drag = None;
    }

    pub(super) fn master_selection(&mut self, ui: &egui::Ui, rect: Rect) {
        self.master_selection_input(ui, rect);
        if self.master_selected {
            panels::selection_border(ui, rect);
        }
    }

    pub(super) fn master_selection_input(&mut self, ui: &egui::Ui, rect: Rect) {
        if ui.is_enabled()
            && self.editing_enabled()
            && ui.rect_contains_pointer(rect)
            && ui.input(|input| input.focused && input.pointer.primary_pressed())
            && !self.master_selected
        {
            self.select_master();
        }
    }

    pub(super) fn prune_track_selection(&mut self) {
        self.selected_tracks.retain(|id| {
            self.session
                .project
                .tracks
                .iter()
                .any(|track| track.id == *id)
        });
        self.selected_track = self
            .selected_track
            .filter(|id| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .any(|track| track.id == *id)
            })
            .or_else(|| {
                self.session
                    .project
                    .tracks
                    .iter()
                    .find(|track| self.selected_tracks.contains(&track.id))
                    .map(|track| track.id)
            });
        if let Some(primary) = self.selected_track {
            self.selected_tracks.insert(primary);
        }
    }

    pub fn track_action_enabled(&self, action: TrackAction, ctx: &egui::Context) -> bool {
        // Non-text controls can retain keyboard focus outside the Tracks pane.
        let input_available = match action {
            TrackAction::Rename if self.selected_rename_layer().is_some() => {
                !ctx.text_edit_focused()
                    && ctx.dragged_id().is_none()
                    && !self.instrument_ui.has_layer_gesture()
            }
            TrackAction::Add | TrackAction::AddMidi | TrackAction::Duplicate => {
                !ctx.text_edit_focused() && ctx.dragged_id().is_none()
            }
            TrackAction::Mute | TrackAction::Solo
                if matches!(self.view_mode, ViewMode::Timeline | ViewMode::Instrument) =>
            {
                !ctx.text_edit_focused()
            }
            _ => !ctx.egui_wants_keyboard_input(),
        };
        (action != TrackAction::Ungroup || self.selected_group().is_some())
            && (action != TrackAction::AddMidi
                || matches!(self.view_mode, ViewMode::Timeline | ViewMode::Edit))
            && (action != TrackAction::Duplicate
                || matches!(self.view_mode, ViewMode::Timeline | ViewMode::Edit)
                || !self.session.project.tracks.iter().any(|track| {
                    self.track_selected(track.id) && track.kind == daw_core::TrackKind::Midi
                }))
            && input_available
            && self.selection_shortcut_environment_available(ctx)
            && (action != TrackAction::Rename
                || self.selected_rename_layer().is_some()
                || self
                    .session
                    .project
                    .tracks
                    .iter()
                    .filter(|track| self.track_selected(track.id))
                    .count()
                    == 1)
            && (matches!(action, TrackAction::Add | TrackAction::AddMidi)
                || self
                    .session
                    .project
                    .tracks
                    .iter()
                    .any(|track| self.track_selected(track.id)))
    }

    pub fn perform_track_action(&mut self, action: TrackAction, ctx: &egui::Context) {
        // Native key equivalents call this entry directly. Keep menu buttons
        // available on press, but execute mix actions only after pointer release.
        if !self.track_action_enabled(action, ctx)
            || (matches!(action, TrackAction::Mute | TrackAction::Solo)
                && ctx.input(|input| input.pointer.any_down()))
        {
            return;
        }
        self.ensure_history();
        if self.view_mode == ViewMode::Instrument
            && matches!(action, TrackAction::Mute | TrackAction::Solo)
        {
            self.instrument_mix_action(action);
            return;
        }
        if matches!(action, TrackAction::Group | TrackAction::Ungroup) {
            self.perform_group_action(action);
            return;
        }
        if action == TrackAction::Rename {
            if self.rename_selected_layer() {
                return;
            }
            if let Some(track) = self
                .session
                .project
                .tracks
                .iter()
                .find(|track| self.track_selected(track.id))
            {
                self.track_name_edit = Some(TrackNameEdit {
                    track: track.id,
                    text: track.name.clone(),
                    focus: true,
                });
            }
            return;
        }
        if action == TrackAction::Delete {
            self.delete_selected_tracks(ctx);
            return;
        }
        if action == TrackAction::Duplicate {
            self.duplicate_selected_tracks();
            return;
        }
        if matches!(action, TrackAction::Add | TrackAction::AddMidi) {
            let result = if action == TrackAction::AddMidi {
                self.session.project.add_midi_track()
            } else {
                self.session.project.add_track()
            };
            match result {
                Ok(id) => {
                    self.select_track(id, false);
                    self.changed();
                }
                Err(error) => self.fail(error),
            }
            return;
        }
        let edits = self
            .session
            .project
            .tracks
            .iter()
            .filter(|track| self.track_selected(track.id))
            .map(|track| {
                if action == TrackAction::Mute {
                    Edit::SetTrackMuted {
                        track_id: track.id,
                        muted: !track.muted,
                    }
                } else {
                    Edit::SetTrackSoloed {
                        track_id: track.id,
                        soloed: !track.soloed,
                    }
                }
            })
            .collect();
        self.edit(Edit::Batch(edits));
    }

    pub(super) fn track_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
        for action in TrackAction::ALL {
            if action.starts_menu_group() {
                crate::menus::separator(ui);
            }
            let mut button = crate::menus::action_button(self.track_action_label(action));
            if let Some(shortcut) = action.shortcut() {
                button = button.shortcut_text(toolbars::menu_shortcut_text(ui.ctx(), &shortcut));
            }
            let response = ui.add_enabled(self.track_action_enabled(action, ui.ctx()), button);
            if response.clicked() {
                self.perform_track_action(action, ui.ctx());
                ui.close();
            }
        }
    }

    pub(super) fn track_shortcuts(&mut self, ctx: &egui::Context) {
        let enabled = TrackAction::ALL.map(|action| self.track_action_enabled(action, ctx));
        if !enabled.iter().any(|enabled| *enabled) || ctx.input(|input| input.pointer.any_down()) {
            return;
        }
        let pressed = ctx.input_mut(|input| {
            let mut pressed = [false; TrackAction::ALL.len()];
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                {
                    for (index, action) in TrackAction::ALL.iter().enumerate() {
                        if enabled[index] && action.matches_shortcut(*key, *modifiers) {
                            pressed[index] = true;
                            return false;
                        }
                    }
                }
                true
            });
            pressed
        });
        for (index, action) in TrackAction::ALL.into_iter().enumerate() {
            if pressed[index] {
                self.perform_track_action(action, ctx);
            }
        }
    }

    pub(super) fn delete_selected_tracks(&mut self, ctx: &egui::Context) {
        if !self.track_action_enabled(TrackAction::Delete, ctx) {
            return;
        }
        let edits = self
            .selected_track_roots()
            .into_iter()
            .map(Edit::DeleteTrack)
            .collect();
        self.edit(Edit::Batch(edits));
        self.inputs.retain(|id, _| {
            self.session
                .project
                .tracks
                .iter()
                .any(|track| track.id == *id)
        });
    }

    fn duplicate_selected_tracks(&mut self) {
        let copies: Vec<_> = self
            .selected_track_roots()
            .into_iter()
            .map(|id| (id, Id::new_v4()))
            .collect();
        self.edit(Edit::Batch(
            copies
                .iter()
                .map(|(track_id, new_track_id)| Edit::DuplicateTrack {
                    track_id: *track_id,
                    new_track_id: *new_track_id,
                })
                .collect(),
        ));
        if copies.iter().all(|(_, id)| {
            self.session
                .project
                .tracks
                .iter()
                .any(|track| track.id == *id)
        }) {
            self.selected_tracks = copies.iter().map(|(_, id)| *id).collect();
            self.selected_track = copies.first().map(|(_, id)| *id);
        }
    }

    pub fn track_action_label(&self, action: TrackAction) -> &'static str {
        if action == TrackAction::Rename && self.selected_rename_layer().is_some() {
            return "Rename layer";
        }
        if matches!(action, TrackAction::Delete | TrackAction::Duplicate)
            && self
                .session
                .project
                .tracks
                .iter()
                .filter(|track| self.track_selected(track.id))
                .count()
                > 1
        {
            match action {
                TrackAction::Duplicate => "Duplicate selected tracks",
                _ => "Delete selected tracks",
            }
        } else {
            action.label()
        }
    }

    fn add_track_menu(&self, ui: &mut egui::Ui) -> Option<TrackAction> {
        for action in [TrackAction::Add, TrackAction::AddMidi] {
            if ui
                .add_enabled(
                    self.track_action_enabled(action, ui.ctx()),
                    menus::action_button(action.label()),
                )
                .clicked()
            {
                ui.close();
                return Some(action);
            }
        }
        None
    }

    pub(super) fn add_track_dropdown(&mut self, ui: &mut egui::Ui) {
        let response = icons::button(
            ui,
            icons::Lucide::Plus,
            "Add track",
            self.track_action_enabled(TrackAction::Add, ui.ctx()),
            theme::palette(ui.ctx()).text,
        );
        let action = menus::Dropdown::show_from_button(&response, |ui| self.add_track_menu(ui))
            .and_then(|response| response.inner);
        if let Some(action) = action {
            self.perform_track_action(action, ui.ctx());
        }
    }

    pub(super) fn empty_tracks_context_menu(&mut self, ui: &mut egui::Ui) {
        if !ui.is_enabled() || !self.track_action_enabled(TrackAction::Add, ui.ctx()) {
            return;
        }
        let rect = egui::Rect::from_min_max(
            egui::Pos2::new(ui.min_rect().left(), ui.cursor().top()),
            egui::Pos2::new(ui.min_rect().left() + TRACK_WIDTH, ui.clip_rect().bottom()),
        )
        .intersect(ui.clip_rect());
        if !rect.is_positive() {
            return;
        }
        let response = ui.interact(rect, ui.id().with("empty-tracks"), egui::Sense::click());
        let action = crate::menus::ContextMenu::new(&response)
            .show(|ui| self.add_track_menu(ui))
            .and_then(|response| response.inner);
        if let Some(action) = action {
            self.perform_track_action(action, ui.ctx());
        }
    }

    pub(super) fn track_context_menu(&self, ui: &mut egui::Ui) -> Option<TrackAction> {
        for (index, action) in [
            TrackAction::Rename,
            TrackAction::Duplicate,
            TrackAction::Group,
            TrackAction::Ungroup,
            TrackAction::Delete,
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 && action.starts_menu_group() {
                crate::menus::separator(ui);
            }
            let mut button = crate::menus::action_button(self.track_action_label(action));
            if let Some(shortcut) = action.shortcut() {
                button = button.shortcut_text(toolbars::menu_shortcut_text(ui.ctx(), &shortcut));
            }
            let response = ui.add_enabled(self.track_action_enabled(action, ui.ctx()), button);
            if response.clicked() {
                ui.close();
                return Some(action);
            }
        }
        None
    }
}
