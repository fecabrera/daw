//! Shared project undo/redo actions for keyboard, native menus, and the app menu.
use super::*;
use daw_project::SessionHistory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryAction {
    Undo,
    Redo,
}
impl HistoryAction {
    pub const ALL: [Self; 2] = [Self::Undo, Self::Redo];
    pub fn label(self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }
    pub fn shortcut(self) -> egui::KeyboardShortcut {
        egui::KeyboardShortcut::new(
            if self == Self::Redo {
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::COMMAND
            },
            egui::Key::Z,
        )
    }
}

impl DawUi {
    pub(super) fn history_memory_notice(&mut self) {
        if self.job.is_some() {
            return;
        }
        if let Some(history) = &mut self.history {
            let removed = history.take_memory_notice();
            if removed > 0 {
                self.notices.push(format!("Removed {removed} older undo/redo entries to limit memory use. The current project is unchanged."));
                self.repaint_until = Some(std::time::Instant::now() + Duration::from_millis(250));
            }
        }
    }
    pub(super) fn ensure_history(&mut self) {
        if self
            .history
            .as_ref()
            .is_none_or(|h| h.project_id() != self.session.project.project_id)
        {
            match SessionHistory::new(&self.session, !self.dirty) {
                Ok(history) => self.history = Some(history),
                Err(error) => self.fail(error),
            }
        }
    }
    pub(super) fn record_history(&mut self) {
        if let Some(history) = &mut self.history {
            if let Err(error) = history.record(&self.session) {
                self.fail(error);
            } else {
                self.dirty = !history.is_saved()
                    || self.navigation_revision != self.saved_navigation_revision;
            }
        }
    }
    pub(super) fn history_saved(&mut self, snapshot: &Session) {
        if let Some(history) = &mut self.history
            && let Err(error) = history.mark_saved(snapshot)
        {
            self.fail(error);
        }
    }
    pub fn history_action_enabled(&self, action: HistoryAction, ctx: &egui::Context) -> bool {
        self.editing_enabled()
            && self.track_drag.is_none()
            && self.loop_drag.is_none()
            && self.marquee.is_none()
            && self.portion_drag.is_none()
            && self.track_name_edit.is_none()
            && self.tempo_edit.is_none()
            && self.time_drag.is_none()
            && self.musical_drag.is_none()
            && self.selection_drag.iter().all(Option::is_none)
            && self.file_hover.is_none()
            && !self.instrument_ui.editing_name()
            && !ctx.text_edit_focused()
            && ctx.input(|i| i.focused)
            && self.history.as_ref().is_some_and(|h| match action {
                HistoryAction::Undo => h.can_undo(),
                HistoryAction::Redo => h.can_redo(),
            })
    }
    pub fn perform_history_action(&mut self, action: HistoryAction, ctx: &egui::Context) {
        if !self.history_action_enabled(action, ctx) {
            return;
        }
        if let Some(output) = &self.output {
            self.session.project.transport.playhead_frame = output.transport_intent().frame;
        }
        let history = self.history.as_mut().unwrap();
        let restored = match action {
            HistoryAction::Undo => history.undo(&self.session),
            HistoryAction::Redo => history.redo(&self.session),
        };
        let Some(session) = restored else {
            return;
        };
        let settings_changed = self.session.project.sample_rate_hz
            != session.project.sample_rate_hz
            || self.session.project.buffer_size_frames != session.project.buffer_size_frames;
        if settings_changed {
            self.output = None;
            self.clip_clipboard = None;
        }
        self.session = session;
        self.dirty =
            !history.is_saved() || self.navigation_revision != self.saved_navigation_revision;
        self.change_revision = self.change_revision.wrapping_add(1);
        self.sync_needed = !settings_changed;
        self.inputs.clear();
        self.master_gain_input = faders::format_gain(self.session.project.master.gain_db);
        self.midi_editor.cancel();
        self.instrument_ui.cancel();
        self.selected_effects.clear();
        self.portion = None;
        self.portion_drag = None;
        self.prune_clip_selection();
        self.prune_track_selection();
        self.notices = self.session.warnings.clone();
        ctx.request_repaint();
    }
    pub(super) fn history_shortcuts(&mut self, ctx: &egui::Context) {
        if self.native_menu || ctx.text_edit_focused() {
            return;
        }
        let mut requested = None;
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key: egui::Key::Z,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                {
                    for action in HistoryAction::ALL {
                        if modifiers.matches_exact(action.shortcut().modifiers) {
                            if !repeat {
                                requested = Some(action);
                            }
                            return false;
                        }
                    }
                }
                true
            })
        });
        if let Some(action) = requested
            && ctx.input(|i| !i.pointer.any_down())
        {
            self.perform_history_action(action, ctx);
        }
    }
    pub(super) fn history_menu(&mut self, ui: &mut egui::Ui) {
        for action in HistoryAction::ALL {
            let button = egui::Button::new(action.label())
                .shortcut_text(toolbars::menu_shortcut_text(ui.ctx(), &action.shortcut()));
            if ui
                .add_enabled(self.history_action_enabled(action, ui.ctx()), button)
                .clicked()
            {
                self.perform_history_action(action, ui.ctx());
                ui.close();
            }
        }
        ui.separator();
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    #[test]
    fn history_memory_notice_is_visible_once_and_does_not_change_the_project() {
        let mut app = DawUi::new(None);
        app.ensure_history();
        app.session.project.name = "Edit".into();
        app.record_history();
        let before = app.session.project.clone();
        app.history.as_mut().unwrap().release_optional_memory();
        app.history_memory_notice();
        assert_eq!(app.notices.len(), 1);
        assert!(app.notices[0].contains("undo/redo"));
        app.history_memory_notice();
        assert_eq!(app.notices.len(), 1);
        assert_eq!(app.session.project, before);
    }
}
