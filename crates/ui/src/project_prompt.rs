//! Hosts project prompts and their destination choosers on a desktop window.
use crate::dialogs::FooterAction;
use crate::{DawUi, DesktopPicker, dialogs, export_dialog, project_name_dialog};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPromptKind {
    Unsaved,
    Save,
    SaveAs,
    Export,
    ReplaceExport,
    ExportStatus,
    StretchUpgrade,
    Arrangement,
    ProjectSettings,
    Normalize,
    Bounce,
    Separation,
    CollectionError,
    Recovery,
    MemoryDetails,
    TransientDetails,
    Error,
    ModelValidation,
    SettingsMessage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProjectPrompt {
    pub kind: ProjectPromptKind,
    pub generation: u64,
}

#[derive(Default)]
pub(super) struct ProjectPromptWindow {
    generation: u64,
    active_kind: Option<ProjectPromptKind>,
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
    Export(export_dialog::Request),
}

#[derive(Clone, Copy)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

impl DawUi {
    pub(super) fn sync_project_prompt(&mut self) {
        let kind = self.project_prompt_kind();
        if self.project_prompt_window.active_kind != kind {
            self.project_prompt_window.active_kind = kind;
            self.project_prompt_window.begin();
            if kind == Some(ProjectPromptKind::ProjectSettings)
                && let Some(draft) = self.project_settings.as_mut()
            {
                draft.restore_focus();
            }
        }
    }

    pub(super) fn project_prompt_kind(&self) -> Option<ProjectPromptKind> {
        use ProjectPromptKind::*;
        if self.settings_picker_active() {
            None
        } else if let Some((prompt, _)) = &self.project_prompt_window.picker {
            Some(prompt.kind)
        } else if self.error.is_some() {
            Some(Error)
        } else if let Some(report) = &self.diagnostic_details {
            Some(match report {
                crate::diagnostic_details::Report::Memory => MemoryDetails,
                crate::diagnostic_details::Report::Transients(_) => TransientDetails,
            })
        } else if self.unsaved_prompt {
            Some(Unsaved)
        } else if let Some(draft) = &self.project_name_dialog {
            Some(if draft.save_as { SaveAs } else { Save })
        } else if self.export_dialog.is_some() {
            Some(Export)
        } else if self.overwrite.is_some() {
            Some(ReplaceExport)
        } else if self.export_status.is_some() {
            Some(ExportStatus)
        } else if self.background_media.stretch_upgrade_notice.is_some() {
            Some(StretchUpgrade)
        } else if self.arrangement.draft.is_some() {
            Some(Arrangement)
        } else if self.project_settings.is_some() {
            Some(ProjectSettings)
        } else if self.normalize_dialog.is_some() {
            Some(Normalize)
        } else if self.bounce_status.is_some() {
            Some(Bounce)
        } else if self.separation_status.is_some() {
            Some(Separation)
        } else if self.collection_error_visible() {
            Some(CollectionError)
        } else if self.midi.recovery_prompt() {
            Some(Recovery)
        } else if self.settings_open
            && self.separation_models.running()
            && self.settings_cancel.is_some()
        {
            Some(ModelValidation)
        } else if self.settings_open && self.settings_window.message.is_some() {
            Some(SettingsMessage)
        } else {
            None
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

    pub(super) fn fail_native_project_prompt(&mut self, prompt: ProjectPrompt) {
        if self.native_project_prompt() == Some(prompt) {
            self.project_prompt_window.fallback = true;
            if let Some(draft) = &mut self.project_name_dialog {
                draft.restore_focus();
            }
            if let Some(draft) = &mut self.project_settings {
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
            Destination::Export(export) => {
                DesktopPicker::ExportFile(*request, export.settings.format)
            }
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

    fn finish_export_picker(&mut self, path: Option<PathBuf>, request: export_dialog::Request) {
        self.export_dialog = None;
        if let Some(path) = path {
            self.export_to(
                path.with_extension(request.settings.format.extension()),
                request,
            );
        }
    }

    pub(super) fn cancel_project_prompt(&mut self) {
        use ProjectPromptKind as Kind;
        let kind = self.project_prompt_kind();
        if self.project_prompt_window.picking() {
            return;
        }
        match kind {
            Some(Kind::Error) => self.error = None,
            Some(Kind::MemoryDetails | Kind::TransientDetails) => self.diagnostic_details = None,
            Some(Kind::SettingsMessage) => self.settings_window.message = None,
            _ if self.job.is_some() => {}
            Some(ProjectPromptKind::Unsaved) => self.apply_unsaved_choice(UnsavedChoice::Cancel),
            Some(ProjectPromptKind::Save | ProjectPromptKind::SaveAs) => {
                self.save_named(String::new(), None)
            }
            Some(ProjectPromptKind::Export) => self.export_dialog = None,
            Some(ProjectPromptKind::ReplaceExport) => self.overwrite = None,
            Some(ProjectPromptKind::StretchUpgrade) => {
                self.background_media.stretch_upgrade_notice = None
            }
            Some(ProjectPromptKind::ExportStatus)
                if self
                    .export_status
                    .as_ref()
                    .is_some_and(|status| status.state != crate::export_status::State::Running) =>
            {
                self.export_status = None;
            }
            Some(Kind::Arrangement) => self.arrangement.draft = None,
            Some(Kind::ProjectSettings) => self.project_settings = None,
            Some(Kind::Normalize) => self.normalize_dialog = None,
            Some(Kind::Bounce)
                if self
                    .bounce_status
                    .as_ref()
                    .is_some_and(|status| status.state != crate::job_report::State::Running) =>
            {
                self.dismiss_bounce()
            }
            Some(Kind::Separation)
                if self
                    .separation_status
                    .as_ref()
                    .is_some_and(|status| status.state != crate::job_report::State::Running) =>
            {
                self.separation_status = None
            }
            Some(Kind::CollectionError) => self.acknowledge_collection_error(),
            Some(
                Kind::ExportStatus
                | Kind::Bounce
                | Kind::Separation
                | Kind::Recovery
                | Kind::ModelValidation,
            )
            | None => {}
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
        use ProjectPromptKind as Kind;
        match self.project_prompt_kind() {
            Some(Kind::Error) => {
                if let Some(error) = self.error.as_deref()
                    && dialogs::Dialog::new("Error", "", &[FooterAction::accept("OK", ())])
                        .show_surface(ctx, surface, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    ui.add(egui::Label::new(error).wrap());
                                });
                        })
                        .is_some()
                {
                    self.error = None;
                }
                return;
            }
            Some(Kind::MemoryDetails | Kind::TransientDetails) => {
                self.show_diagnostic_details(ctx, surface);
                return;
            }
            Some(Kind::Arrangement) => {
                self.arrangement_dialog(ctx, surface);
                return;
            }
            Some(Kind::ProjectSettings) => {
                if let Some(mut draft) = self.project_settings.take() {
                    match draft.show(ctx, surface) {
                        Some(crate::project_settings::Action::Apply(values)) => {
                            self.apply_project_settings(values)
                        }
                        Some(crate::project_settings::Action::Cancel) => {}
                        None => self.project_settings = Some(draft),
                    }
                }
                return;
            }
            Some(Kind::Normalize) => {
                self.show_normalization(ctx, surface);
                return;
            }
            Some(Kind::Bounce) => {
                self.show_bounce(ctx, surface);
                return;
            }
            Some(Kind::Separation) => {
                if self
                    .separation_status
                    .as_ref()
                    .is_some_and(|status| status.show(ctx, surface))
                {
                    self.separation_status = None;
                }
                return;
            }
            Some(Kind::CollectionError) => {
                self.show_collection_error(ctx, surface);
                return;
            }
            Some(Kind::Recovery) => {
                self.midi_dialogs(ctx, surface);
                return;
            }
            Some(Kind::ModelValidation) => {
                if let Some(cancel) = &self.settings_cancel {
                    use std::sync::atomic::Ordering;
                    if dialogs::Dialog::new(
                        "Model validation",
                        &self.busy,
                        &[FooterAction::cancel("Cancel", ())],
                    )
                    .actions_enabled(!cancel.load(Ordering::Relaxed))
                    .show_message(ctx, surface)
                    .is_some()
                    {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                return;
            }
            Some(Kind::SettingsMessage) => {
                if let Some(message) = self.settings_window.message.as_deref()
                    && dialogs::Dialog::new(
                        "Settings window",
                        message,
                        &[FooterAction::accept("OK", ())],
                    )
                    .show_message(ctx, surface)
                    .is_some()
                {
                    self.settings_window.message = None;
                }
                return;
            }
            None => return,
            _ => {}
        }
        if self.project_prompt_window.picking() {
            let (title, message) = if self.export_dialog.is_some() {
                ("Export audio", "Choosing an export destination…")
            } else {
                ("Save project", "Choosing a project location…")
            };
            dialogs::Dialog::new(title, message, &[FooterAction::cancel("Cancel", ())])
                .actions_enabled(false)
                .show_message(ctx, surface);
            return;
        }
        if self.unsaved_prompt {
            let choice = dialogs::Dialog::new(
                "Unsaved changes",
                "Save changes before continuing?",
                &[
                    FooterAction::accept("Save", UnsavedChoice::Save),
                    FooterAction::deny("Discard", UnsavedChoice::Discard),
                    FooterAction::cancel("Cancel", UnsavedChoice::Cancel),
                ],
            )
            .actions_enabled(self.job.is_none())
            .show_message(ctx, surface);
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
        } else if let Some(mut draft) = self.export_dialog.take() {
            match export_dialog::show(ctx, &mut draft, &self.session.project, surface) {
                Some(export_dialog::Action::Cancel) => self.export_dialog = None,
                Some(export_dialog::Action::Export) if self.native_desktop_windows_enabled() => {
                    let request = draft.request;
                    self.export_dialog = Some(draft);
                    self.pick_destination(ProjectPromptKind::Export, Destination::Export(request));
                }
                Some(export_dialog::Action::Export) => {
                    let format = draft.request.settings.format;
                    let path = rfd::FileDialog::new()
                        .set_file_name(format!("mix.{}", format.extension()))
                        .add_filter(format.label(), &[format.extension()])
                        .save_file();
                    self.finish_export_picker(path, draft.request);
                }
                None => self.export_dialog = Some(draft),
            }
        } else if let Some((path, _)) = &self.overwrite {
            let message = format!("Replace {}?", path.display());
            let replace = dialogs::Dialog::new(
                "Replace export?",
                &message,
                &[
                    FooterAction::accept("Replace", true),
                    FooterAction::cancel("Cancel", false),
                ],
            )
            .show_message(ctx, surface);
            if let Some(replace) = replace
                && let Some((path, request)) = self.overwrite.take()
                && replace
            {
                self.export(path, true, request);
            }
        } else if let Some(status) = &self.export_status {
            if status.show(ctx, surface) {
                self.export_status = None;
            }
        } else if let Some(notice) = &self.background_media.stretch_upgrade_notice {
            let acknowledged = dialogs::Dialog::new(
                "Time stretch upgrade",
                notice,
                &[FooterAction::accept("OK", ())],
            )
            .show_message(ctx, surface);
            if acknowledged.is_some()
                || ctx
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                self.background_media.stretch_upgrade_notice = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, DesktopPicker, DesktopWindow};
    use daw_media::ExportSettings;

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

    #[test]
    fn native_error_preserves_the_project_settings_draft() {
        let mut app = DawUi::default();
        app.enable_native_windows();
        let mut draft = crate::project_settings::ProjectSettings::new(&app.session.project);
        draft.name = "Pending name".into();
        app.project_settings = Some(draft);
        app.sync_project_prompt();

        let root = egui::Context::default();
        let settings_window = app.desktop_window().unwrap();
        assert_eq!(settings_window.title(), "Project Settings");
        let shapes = child_frame(
            &mut app,
            &root,
            &egui::Context::default(),
            settings_window,
            vec![],
            false,
        );
        text_bounds(&shapes, "Project name");
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == settings_window.title())));

        app.fail("Could not apply project settings.");
        app.sync_project_prompt();
        let error_window = app.desktop_window().unwrap();
        assert_eq!(error_window.title(), "Error");
        child_frame(
            &mut app,
            &root,
            &egui::Context::default(),
            error_window,
            vec![],
            true,
        );

        app.sync_project_prompt();
        let resumed = app.desktop_window().unwrap();
        assert_eq!(resumed.title(), "Project Settings");
        assert_ne!(resumed, settings_window);
        assert_eq!(app.project_settings.as_ref().unwrap().name, "Pending name");
        child_frame(
            &mut app,
            &root,
            &egui::Context::default(),
            resumed,
            vec![],
            true,
        );
        assert!(app.desktop_window().is_none());
        assert_ne!(app.session.project.name, "Pending name");
    }

    #[test]
    fn native_replace_export_requires_confirmation_before_writing() {
        struct Output(PathBuf);
        impl Drop for Output {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let output = Output(
            std::env::temp_dir().join(format!("daw-replace-export-{}.wav", daw_core::Id::new_v4())),
        );
        std::fs::write(&output.0, b"previous export").unwrap();
        let settings = ExportSettings {
            wav_codec: daw_media::WavCodec::Float32,
            ..Default::default()
        };
        let mut app = DawUi::default();
        app.enable_native_windows();
        let track = app.session.project.add_midi_track().unwrap();
        app.session.project.add_midi_pattern(track, 0).unwrap();
        let root = egui::Context::default();
        let child = egui::Context::default();

        app.export_to(output.0.clone(), app.export_request(settings));
        let window = app.desktop_window().unwrap();
        assert_eq!(window.title(), "Replace export?");
        assert!(!app.edit_environment_available());
        child_frame(&mut app, &root, &child, window, vec![], true);
        assert!(app.desktop_window().is_none() && app.job.is_none());
        assert_eq!(std::fs::read(&output.0).unwrap(), b"previous export");

        app.export_to(output.0.clone(), app.export_request(settings));
        let next = app.desktop_window().unwrap();
        assert_ne!(next, window);
        child_frame(&mut app, &root, &child, next, vec![], false);
        let shapes = child_frame(&mut app, &root, &child, next, vec![], false);
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == next.title())));
        assert_button_padding(&app, next, &shapes, "Cancel");
        let point = text_bounds(&shapes, "Replace").center();
        let button = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        child_frame(
            &mut app,
            &root,
            &child,
            next,
            vec![egui::Event::PointerMoved(point), button(true)],
            false,
        );
        child_frame(&mut app, &root, &child, next, vec![button(false)], false);
        assert!(app.overwrite.is_none());
        assert_eq!(app.desktop_window().unwrap().title(), "Export status");
        assert_eq!(app.export_status.as_ref().unwrap().settings, settings);
        for _ in 0..400 {
            app.poll();
            if app.job.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            app.export_status.unwrap().state,
            crate::export_status::State::Complete
        );
        assert_eq!(
            hound::WavReader::open(&output.0)
                .unwrap()
                .spec()
                .sample_format,
            hound::SampleFormat::Float
        );
    }

    #[test]
    fn native_export_status_uses_the_shared_body_and_can_close_only_after_export() {
        use crate::export_status::{ExportStatus, State};

        let mut app = DawUi::default();
        app.enable_native_windows();
        app.export_status = Some(ExportStatus::new(
            PathBuf::from("mix.wav"),
            ExportSettings::default(),
            48_000,
            4096,
            vec!["Example export warning".to_owned()],
        ));
        let window = app.desktop_window().unwrap();
        assert_eq!(window.title(), "Export status");
        assert!(!app.edit_environment_available());

        let root = egui::Context::default();
        let child = egui::Context::default();
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        let has_label = |shapes: &[egui::epaint::ClippedShape], label| {
            shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == label)
            })
        };
        for label in [
            "Rendering and encoding audio…",
            "mix.wav",
            "Example export warning",
        ] {
            assert!(has_label(&shapes, label), "Missing {label}");
        }
        assert!(has_label(&shapes, "Close"));
        for label in [window.title(), "Cancel"] {
            assert!(!has_label(&shapes, label), "Unexpected {label}");
        }
        child_frame(
            &mut app,
            &root,
            &child,
            window,
            vec![key(egui::Key::Escape)],
            false,
        );
        child_frame(&mut app, &root, &child, window, vec![], true);
        assert_eq!(app.desktop_window(), Some(window));
        assert!(!app.edit_environment_available());

        let status = app.export_status.as_mut().unwrap();
        status.state = State::Complete;
        status.warnings = (0..100).map(|i| format!("Export warning {i}")).collect();
        child_frame(&mut app, &root, &child, window, vec![], false);
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert!(has_label(&shapes, "Export complete"));
        assert!(has_label(&shapes, "100%"));
        assert_button_padding(&app, window, &shapes, "Close");
        child_frame(&mut app, &root, &child, window, vec![], true);
        assert!(app.desktop_window().is_none());
        assert!(app.edit_environment_available());
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
        label: &str,
    ) {
        let bounds = text_bounds(shapes, label);
        let gap = app.desktop_window_size(window)[1] as f32 - bounds.bottom();
        assert!((8.0..=16.0).contains(&gap), "Gap below {label} text: {gap}");
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
        assert_button_padding(&app, window, &shapes, "Cancel");
        for expected in [
            "Save changes before continuing?",
            "Save",
            "Discard",
            "Cancel",
        ] {
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == expected)));
        }
        let save = text_bounds(&shapes, "Save").center();
        let discard = text_bounds(&shapes, "Discard").center();
        let accent = crate::theme::accent(&child);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.fill == accent
                && rect.rect.contains(save) && !rect.rect.contains(discard)
        )));
        let message = text_bounds(&shapes, "Save changes before continuing?");
        let padding = f32::from(dialogs::PADDING);
        assert!(shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
                if (padding - 1.0..=padding + 2.0).contains(&(points[0].y - message.bottom()))
                    && points[0].y == points[1].y
                    && (points[1].x - points[0].x) >= 350.0
        )));
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
        let mut output = root.run_ui(egui::RawInput::default(), |ui| {
            app.midi_dialogs(ui.ctx(), None)
        });
        output.textures_delta.clear();
        assert!(
            output.shapes.is_empty(),
            "the project prompt keeps priority"
        );
        app.midi.recovery_running = false;
        let child = egui::Context::default();
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert_button_padding(&app, window, &shapes, "Cancel");
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
        crate::fonts::configure(&child);
        child_frame(&mut app, &root, &child, window, vec![], false);
        let wav_height = app.desktop_window_size(window)[1];
        let draft = app.export_dialog.as_mut().unwrap();
        draft.request.settings.format = ExportFormat::Mp3;
        draft.request.region.start_frame = 100;
        draft.request.region.include_tail = false;
        let expected = draft.request;
        child_frame(&mut app, &root, &child, window, vec![], false);
        let shapes = child_frame(&mut app, &root, &child, window, vec![], false);
        assert!(app.desktop_window_size(window)[1] > wav_height);
        assert_button_padding(&app, window, &shapes, "Cancel");
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == window.title())));

        let point = text_bounds(&shapes, "Export").center();
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
        assert!(matches!(app.project_prompt_window.picker.as_ref(),
            Some((_, Destination::Export(export))) if *export == expected));
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
