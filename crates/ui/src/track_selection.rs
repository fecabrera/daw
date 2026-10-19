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

    fn select_master(&mut self) {
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
        (action != TrackAction::AddMidi
            || matches!(self.view_mode, ViewMode::Timeline | ViewMode::Edit))
            && (action != TrackAction::Duplicate
                || matches!(self.view_mode, ViewMode::Timeline | ViewMode::Edit)
                || !self.session.project.tracks.iter().any(|track| {
                    self.track_selected(track.id) && track.kind == daw_core::TrackKind::Midi
                }))
            && self.clip_actions_available(ctx)
            && self.track_name_edit.is_none()
            && self.tempo_edit.is_none()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
            && self.file_hover.is_none()
            && (action != TrackAction::Rename
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
        if !self.track_action_enabled(action, ctx) {
            return;
        }
        if action == TrackAction::Rename {
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
            if matches!(action, TrackAction::Mute | TrackAction::Rename) {
                ui.separator();
            }
            let mut button = egui::Button::new(self.track_action_label(action));
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
        if !self.track_action_enabled(TrackAction::Mute, ctx)
            || ctx.input(|input| input.pointer.any_down())
        {
            return;
        }
        let (mute, solo, rename) = ctx.input_mut(|input| {
            let mut mute = false;
            let mut solo = false;
            let mut rename = false;
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                {
                    if *key == egui::Key::R && modifiers.matches_exact(egui::Modifiers::COMMAND) {
                        rename = true;
                        return false;
                    }
                    if modifiers.command || modifiers.ctrl || modifiers.alt || modifiers.mac_cmd {
                        return true;
                    }
                    match key {
                        egui::Key::M => {
                            mute = true;
                            return false;
                        }
                        egui::Key::S => {
                            solo = true;
                            return false;
                        }
                        _ => {}
                    }
                }
                true
            });
            (mute, solo, rename)
        });
        if mute {
            self.perform_track_action(TrackAction::Mute, ctx);
        }
        if solo {
            self.perform_track_action(TrackAction::Solo, ctx);
        }
        if rename {
            self.perform_track_action(TrackAction::Rename, ctx);
        }
    }

    pub(super) fn delete_selected_tracks(&mut self, ctx: &egui::Context) {
        if !self.track_action_enabled(TrackAction::Delete, ctx) {
            return;
        }
        let edits = self
            .session
            .project
            .tracks
            .iter()
            .filter(|track| self.track_selected(track.id))
            .map(|track| Edit::DeleteTrack(track.id))
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
            .session
            .project
            .tracks
            .iter()
            .filter(|track| self.track_selected(track.id))
            .map(|track| (track.id, Id::new_v4()))
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
        let action = egui::Popup::context_menu(&response)
            .show(|ui| {
                for action in [TrackAction::Add, TrackAction::AddMidi] {
                    if ui
                        .add_enabled(
                            self.track_action_enabled(action, ui.ctx()),
                            egui::Button::new(action.label()),
                        )
                        .clicked()
                    {
                        ui.close();
                        return Some(action);
                    }
                }
                None
            })
            .and_then(|response| response.inner);
        if let Some(action) = action {
            self.perform_track_action(action, ui.ctx());
        }
    }

    pub(super) fn track_context_menu(&self, ui: &mut egui::Ui) -> Option<TrackAction> {
        for action in [
            TrackAction::Rename,
            TrackAction::Duplicate,
            TrackAction::Delete,
        ] {
            let mut button = egui::Button::new(self.track_action_label(action));
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
