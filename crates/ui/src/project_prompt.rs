//! Hosts project prompts and their destination choosers on a desktop window.
use crate::{DawUi, DesktopPicker, dialogs, export_dialog, project_name_dialog};
use daw_media::ExportSettings;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPromptKind {
    Unsaved,
    Save,
    SaveAs,
    Export,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectPrompt {
    pub kind: ProjectPromptKind,
    pub generation: u64,
}

#[derive(Default)]
pub(super) struct ProjectPromptWindow {
    generation: u64,
    fallback: bool,
    pub focus_requested: bool,
    pub height: Option<u32>,
    picker: Option<(ProjectPrompt, Destination)>,
    picker_started: bool,
}

impl ProjectPromptWindow {
    pub fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.fallback = false;
        self.focus_requested = true;
        self.height = None;
    }

    pub fn picking(&self) -> bool {
        self.picker.is_some()
    }
}

enum Destination {
    Project(String),
    Export(ExportSettings),
}

#[derive(Clone, Copy)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

impl DawUi {
    pub(super) fn project_prompt_kind(&self) -> Option<ProjectPromptKind> {
        if self.unsaved_prompt {
            Some(ProjectPromptKind::Unsaved)
        } else if let Some(draft) = &self.project_name_dialog {
            Some(if draft.save_as {
                ProjectPromptKind::SaveAs
            } else {
                ProjectPromptKind::Save
            })
        } else {
            self.export_dialog
                .as_ref()
                .map(|_| ProjectPromptKind::Export)
        }
    }

    pub(super) fn native_project_prompt(&self) -> Option<ProjectPrompt> {
        if !self.native_desktop_windows_enabled() || self.project_prompt_window.fallback {
            return None;
        }
        self.project_prompt_kind().map(|kind| ProjectPrompt {
            kind,
            generation: self.project_prompt_window.generation,
        })
    }

    pub(super) fn project_prompt_blocked(&self) -> bool {
        self.error.is_some() || self.diagnostic_details.is_some()
    }

    pub(super) fn fail_native_project_prompt(&mut self, prompt: ProjectPrompt) {
        if self.native_project_prompt() == Some(prompt) {
            self.project_prompt_window.fallback = true;
            if let Some(draft) = &mut self.project_name_dialog {
                draft.restore_focus();
            }
        }
    }

    fn pick_destination(&mut self, kind: ProjectPromptKind, destination: Destination) {
        let request = ProjectPrompt {
            generation: self.project_prompt_window.generation,
            kind,
        };
        self.project_prompt_window.picker = Some((request, destination));
        self.project_prompt_window.picker_started = false;
    }

    pub(super) fn take_project_picker(&mut self) -> Option<DesktopPicker> {
        let (request, destination) = self.project_prompt_window.picker.as_ref()?;
        if std::mem::replace(&mut self.project_prompt_window.picker_started, true) {
            return None;
        }
        Some(match destination {
            Destination::Project(_) => DesktopPicker::ProjectLocation(*request),
            Destination::Export(settings) => DesktopPicker::ExportFile(*request, settings.format),
        })
    }

    pub(super) fn complete_project_picker(
        &mut self,
        request: ProjectPrompt,
        result: Result<Option<PathBuf>, String>,
    ) {
        if self
            .project_prompt_window
            .picker
            .as_ref()
            .map(|(key, _)| *key)
            != Some(request)
        {
            return;
        }
        let (_, destination) = self.project_prompt_window.picker.take().unwrap();
        let path = match result {
            Ok(path) => path,
            Err(error) => {
                self.fail(error);
                None
            }
        };
        match destination {
            Destination::Project(name) => self.save_named(name, path),
            Destination::Export(settings) => self.finish_export_picker(path, settings),
        }
    }

    fn finish_export_picker(&mut self, path: Option<PathBuf>, settings: ExportSettings) {
        self.export_dialog = None;
        if let Some(path) = path {
            self.export_to(path.with_extension(settings.format.extension()), settings);
        }
    }

    pub(super) fn cancel_project_prompt(&mut self) {
        if self.job.is_some() || self.project_prompt_window.picking() {
            return;
        }
        match self.project_prompt_kind() {
            Some(ProjectPromptKind::Unsaved) => self.apply_unsaved_choice(UnsavedChoice::Cancel),
            Some(ProjectPromptKind::Save | ProjectPromptKind::SaveAs) => {
                self.save_named(String::new(), None)
            }
            Some(ProjectPromptKind::Export) => self.export_dialog = None,
            None => {}
        }
    }

    fn apply_unsaved_choice(&mut self, choice: UnsavedChoice) {
        match choice {
            UnsavedChoice::Save => self.save(false),
            UnsavedChoice::Discard => {
                self.dirty = false;
                self.unsaved_prompt = false;
                self.perform_pending();
            }
            UnsavedChoice::Cancel => {
                self.pending = None;
                self.unsaved_prompt = false;
            }
        }
    }

    pub(super) fn draw_project_prompt(
        &mut self,
        ctx: &egui::Context,
        surface: Option<&mut egui::Ui>,
    ) {
        if self.project_prompt_blocked() {
            return;
        }
        if self.project_prompt_window.picking() {
            let (title, message) = if self.export_dialog.is_some() {
                ("Export audio", "Choosing an export destination…")
            } else {
                ("Save project", "Choosing a project location…")
            };
            let waiting = dialogs::Dialog::<()>::new(title, message, &[]);
            match surface {
                Some(ui) => waiting.body(ui, |_| {}),
                None => waiting.show(ctx),
            };
            return;
        }
        if self.unsaved_prompt {
            let dialog = dialogs::Dialog::new(
                "Unsaved changes",
                "Save changes before continuing?",
                &[
                    ("Save", UnsavedChoice::Save),
                    ("Discard", UnsavedChoice::Discard),
                    ("Cancel", UnsavedChoice::Cancel),
                ],
            )
            .actions_enabled(self.job.is_none());
            let choice = match surface {
                Some(ui) => dialog.body(ui, |_| {}),
                None => dialog.show(ctx),
            };
            if self.job.is_none() {
                if ctx
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                {
                    self.cancel_project_prompt();
                } else if let Some(choice) = choice {
                    self.apply_unsaved_choice(choice);
                }
            }
            return;
        }
        if let Some(mut draft) = self.project_name_dialog.take() {
            match draft.show(ctx, surface) {
                Some(project_name_dialog::Action::Cancel) => self.save_named(String::new(), None),
                Some(project_name_dialog::Action::Continue(name))
                    if self.native_desktop_windows_enabled() =>
                {
                    self.project_name_dialog = Some(draft);
                    self.pick_destination(
                        self.project_prompt_kind().unwrap(),
                        Destination::Project(name),
                    );
                }
                Some(project_name_dialog::Action::Continue(name)) => {
                    let parent = rfd::FileDialog::new()
                        .set_title("Choose project location")
                        .pick_folder();
                    self.save_named(name, parent);
                }
                None => self.project_name_dialog = Some(draft),
            }
        } else if let Some(mut settings) = self.export_dialog {
            match export_dialog::show(
                ctx,
                &mut settings,
                self.session.project.sample_rate_hz,
                surface,
            ) {
                Some(export_dialog::Action::Cancel) => self.export_dialog = None,
                Some(export_dialog::Action::Export) if self.native_desktop_windows_enabled() => {
                    self.export_dialog = Some(settings);
                    self.pick_destination(ProjectPromptKind::Export, Destination::Export(settings));
                }
                Some(export_dialog::Action::Export) => {
                    let format = settings.format;
                    let path = rfd::FileDialog::new()
                        .set_file_name(format!("mix.{}", format.extension()))
                        .add_filter(format.label(), &[format.extension()])
                        .save_file();
                    self.finish_export_picker(path, settings);
                }
                None => self.export_dialog = Some(settings),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, DesktopPicker, DesktopWindow};

    fn child_frame(
        app: &mut DawUi,
        root: &egui::Context,
        child: &egui::Context,
        window: DesktopWindow,
        events: Vec<egui::Event>,
        close: bool,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                app.desktop_window_size(window)
                    .map(|size| size as f32)
                    .into(),
            )),
            events,
            ..Default::default()
        };
        if close {
            input
                .viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .events
                .push(egui::ViewportEvent::Close);
        }
        let mut output = child.run_ui(input, |ui| app.show_desktop_window(window, root, ui));
        output.textures_delta.clear();
        output.shapes
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn text_bounds(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Rect {
        shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing dialog item: {label}"))
    }

    fn assert_button_padding(
        app: &DawUi,
        window: DesktopWindow,
        shapes: &[egui::epaint::ClippedShape],
    ) {
        let cancel = text_bounds(shapes, "Cancel");
        let gap = app.desktop_window_size(window)[1] as f32 - cancel.bottom();
        assert!((8.0..=16.0).contains(&gap), "Gap below Cancel text: {gap}");
    }

    #[test]
    fn native_unsaved_prompt_uses_client_body_and_close_cancels_the_transition() {
        let mut app = DawUi::default();
        app.enable_native_windows();
        app.dirty = true;
        app.request(Action::New);
        let window = app.desktop_window().unwrap();
        assert_eq!(window.title(), "Unsaved changes");
        assert!(!window.resizable());
        assert!(app.take_desktop_focus(window));
        assert!(!app.edit_environment_available());
        let root = egui::Context::default();
        let child = egui::Context::default();
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert_button_padding(&app, window, &shapes);
        for expected in [
            "Save changes before continuing?",
            "Save",
            "Discard",
            "Cancel",
        ] {
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == expected)));
        }
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == window.title())));
        app.request_desktop_close();
        assert!(app.take_desktop_focus(window));
        assert!(matches!(app.pending, Some(Action::New)));
        child_frame(&mut app, &root, &child, window, vec![], true);
        assert!(app.desktop_window().is_none());
        assert!(app.dirty && app.pending.is_none() && !app.allow_close);
    }

    #[test]
    fn save_prompt_keeps_its_generation_and_chooser_owner_until_cancel() {
        let mut app = DawUi::default();
        app.enable_native_windows();
        app.dirty = true;
        app.request(Action::New);
        let unsaved = app.desktop_window().unwrap();
        app.apply_unsaved_choice(UnsavedChoice::Save);
        let window = app.desktop_window().unwrap();
        assert_eq!(window.title(), "Save project");
        assert_ne!(window, unsaved);
        app.midi.recovery_running = true;
        assert!(!app.desktop_window_blocked(window));
        app.close_desktop_window(unsaved);
        assert_eq!(app.desktop_window(), Some(window));

        let root = egui::Context::default();
        let mut output = root.run_ui(egui::RawInput::default(), |ui| app.midi_dialogs(ui.ctx()));
        output.textures_delta.clear();
        assert!(
            output.shapes.is_empty(),
            "the project prompt keeps priority"
        );
        app.midi.recovery_running = false;
        let child = egui::Context::default();
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert_button_padding(&app, window, &shapes);
        let valid_height = app.desktop_window_size(window)[1];
        app.project_name_dialog.as_mut().unwrap().name.clear();
        child_frame(&mut app, &root, &child, window, vec![], false);
        assert!(app.desktop_window_size(window)[1] > valid_height);
        child_frame(
            &mut app,
            &root,
            &child,
            window,
            vec![egui::Event::Text("Música".into())],
            false,
        );
        assert_eq!(app.project_name_dialog.as_ref().unwrap().name, "Música");
        child_frame(
            &mut app,
            &root,
            &child,
            window,
            vec![key(egui::Key::Enter)],
            false,
        );
        let request = app.take_desktop_picker().unwrap();
        assert!(matches!(request, DesktopPicker::ProjectLocation(_)));
        assert_eq!(request.owner(), window);
        assert!(app.take_desktop_picker().is_none());
        app.close_desktop_window(window);
        assert_eq!(app.desktop_window(), Some(window));
        app.complete_desktop_picker(request, Ok(None));
        assert!(app.desktop_window().is_none());
        assert!(app.dirty && app.pending.is_none());

        app.save(true);
        let save_as = app.desktop_window().unwrap();
        assert_eq!(save_as.title(), "Save project as");
        app.fail_desktop_window(window);
        assert_eq!(app.desktop_window(), Some(save_as));
        app.project_name_dialog.as_mut().unwrap().name = "Retained draft".into();
        app.fail_desktop_window(save_as);
        assert!(app.desktop_window().is_none());
        assert_eq!(app.project_name_dialog.unwrap().name, "Retained draft");
    }

    #[test]
    fn native_export_resizes_for_codecs_and_owns_the_file_picker() {
        use daw_media::ExportFormat;
        let mut app = DawUi::default();
        app.enable_native_windows();
        let track = app.session.project.add_midi_track().unwrap();
        app.session.project.add_midi_pattern(track, 0).unwrap();
        app.perform_file_action(crate::FileAction::Export);
        let window = app.desktop_window().unwrap();
        assert_eq!(window.title(), "Export audio");
        assert!(!app.edit_environment_available());

        let root = egui::Context::default();
        let child = egui::Context::default();
        child_frame(&mut app, &root, &child, window, vec![], false);
        let wav_height = app.desktop_window_size(window)[1];
        app.export_dialog.as_mut().unwrap().format = ExportFormat::Mp3;
        child_frame(&mut app, &root, &child, window, vec![], false);
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert!(app.desktop_window_size(window)[1] > wav_height);
        assert_button_padding(&app, window, &shapes);
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == window.title())));

        let point = text_bounds(&shapes, "Export…").center();
        let mouse = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        child_frame(
            &mut app,
            &root,
            &child,
            window,
            vec![egui::Event::PointerMoved(point), mouse(true)],
            false,
        );
        child_frame(&mut app, &root, &child, window, vec![mouse(false)], false);
        let request = app.take_desktop_picker().unwrap();
        assert!(matches!(
            request,
            DesktopPicker::ExportFile(_, ExportFormat::Mp3)
        ));
        assert_eq!(request.owner(), window);
        app.close_desktop_window(window);
        assert_eq!(app.desktop_window(), Some(window));
        app.complete_desktop_picker(request, Ok(None));
        assert!(app.desktop_window().is_none());
        assert!(!app.dirty && app.job.is_none());
        assert_eq!(app.export_settings, ExportSettings::default());
    }
}
