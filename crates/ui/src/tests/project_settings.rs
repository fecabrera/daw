use super::*;
use crate::settings::{AppSettings, ColorTheme};

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

fn click_dialog(app: &mut DawUi, ctx: &egui::Context, label: &str) {
    let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
    let point = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing label: {label}"));
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn draft_name_is_selected_and_applies_only_on_enter_or_apply() {
    for enter in [true, false] {
        let (mut app, track) = fixture();
        app.select_track(track, false);
        let project = app.session.project.clone();
        let ctx = context();
        app.open_project_settings();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Text("  Música: Mix / 2  ".into())],
        );
        assert_eq!(
            app.project_settings.as_ref().unwrap().name,
            "  Música: Mix / 2  "
        );
        assert_eq!(app.session.project.name, project.name);
        assert!(!app.dirty);
        if enter {
            frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
        } else {
            click_dialog(&mut app, &ctx, "Apply");
        }
        assert!(app.project_settings.is_none());
        assert!(app.dirty);
        let mut expected = project;
        expected.name = "Música: Mix / 2".into();
        assert_eq!(
            format!("{:?}", app.session.project),
            format!("{expected:?}")
        );
        assert_eq!(app.selected_track, Some(track));
        assert!(!app.sync_needed);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.window_title, "Música: Mix / 2 * - DAW");
    }
}

#[test]
fn cancel_escape_invalid_and_unchanged_names_preserve_project_state() {
    for cancel in [true, false] {
        let mut app = DawUi::default();
        let ctx = context();
        app.open_project_settings();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![egui::Event::Text("Draft name".into())]);
        if cancel {
            click_dialog(&mut app, &ctx, "Cancel");
        } else {
            frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
        }
        assert!(app.project_settings.is_none());
        assert_eq!(app.session.project.name, "Untitled");
        assert!(!app.dirty);
        app.open_project_settings();
        frame(&mut app, &ctx, vec![]);
        for invalid in ["", "   ", "bad\u{0}name"] {
            app.project_settings.as_mut().unwrap().name = invalid.into();
            click_dialog(&mut app, &ctx, "Apply");
            assert!(app.project_settings.is_some());
            assert_eq!(app.session.project.name, "Untitled");
            assert!(!app.dirty);
        }
        app.project_settings.as_mut().unwrap().name = "  Untitled  ".into();
        click_dialog(&mut app, &ctx, "Apply");
        assert!(app.project_settings.is_none());
        assert!(!app.dirty);
    }
}

#[test]
fn file_menu_and_native_dispatch_share_modal_guards() {
    let (mut app, track) = fixture();
    let ctx = context();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.file_menu(ui));
    output.textures_delta.clear();
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Project Settings…" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    for pressed in [true, false] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(point), button(point, pressed)],
                ..Default::default()
            },
            |ui| app.file_menu(ui),
        );
        output.textures_delta.clear();
    }
    assert!(app.project_settings.is_some());
    let before = format!("{:?}", app.session.project);
    app.select_track(track, false);
    for action in FileAction::ALL {
        assert!(!app.file_action_enabled(action));
    }
    for action in TrackAction::ALL {
        assert!(!app.track_action_enabled(action, &ctx));
    }
    for action in ClipAction::ALL {
        assert!(!app.clip_action_enabled(action, &ctx));
    }
    assert!(!app.settings_enabled());
    assert!(!app.project_settings_enabled());
    app.perform_file_action(FileAction::New);
    frame(&mut app, &ctx, vec![key(egui::Key::Space)]);
    assert!(app.output.is_none());
    assert_eq!(format!("{:?}", app.session.project), before);
    frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
    app.use_native_menu();
    app.open_project_settings();
    assert!(app.project_settings.is_some());
    app.project_settings = None;
    let (_sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    app.open_project_settings();
    assert!(app.project_settings.is_none());
    app.job = None;
    app.open_settings();
    app.open_project_settings();
    assert!(app.project_settings.is_none());
}

#[test]
fn renamed_project_saves_in_its_existing_folder_and_reopens_with_new_name() {
    let mut app = DawUi::default();
    let folder = std::env::temp_dir().join(format!("daw-project-settings-{}", Id::new_v4()));
    app.session.save(&folder).unwrap();
    let original_folder = app.session.folder.clone();
    let ctx = context();
    app.open_project_settings();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("Renamed project".into())],
    );
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    assert_eq!(app.session.folder, original_folder);
    assert_eq!(Session::open(&folder).unwrap().project.name, "Untitled");
    app.save(false);
    wait_for_save(&mut app);
    assert!(!app.dirty);
    assert_eq!(app.session.folder, original_folder);
    assert_eq!(
        Session::open(&folder).unwrap().project.name,
        "Renamed project"
    );
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn audio_dropdowns_stage_cancel_and_apply_as_one_worker_transaction() {
    let (mut app, track) = fixture();
    app.select_track(track, false);
    let before = format!("{:?}", app.session.project);
    let ctx = context();
    app.open_project_settings();
    frame(&mut app, &ctx, vec![]);
    click_dialog(&mut app, &ctx, "48000 Hz");
    click_dialog(&mut app, &ctx, "96000 Hz");
    click_dialog(&mut app, &ctx, "512 samples");
    click_dialog(&mut app, &ctx, "128 samples");
    click_dialog(&mut app, &ctx, "32-bit float");
    click_dialog(&mut app, &ctx, "24-bit signed integer");
    assert_eq!(
        app.project_settings.as_ref().unwrap().output_sample_format,
        daw_core::SampleFormat::I24
    );
    assert_eq!(
        app.project_settings.as_ref().unwrap().sample_rate_hz,
        96_000
    );
    assert_eq!(
        app.project_settings.as_ref().unwrap().buffer_size_frames,
        128
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    click_dialog(&mut app, &ctx, "Cancel");
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
    app.open_project_settings();
    let draft = app.project_settings.as_mut().unwrap();
    assert_eq!(draft.sample_rate_hz, 48_000);
    assert_eq!(draft.buffer_size_frames, 512);
    draft.name = "Audio settings".into();
    draft.sample_rate_hz = 96_000;
    draft.buffer_size_frames = 128;
    draft.output_sample_format = daw_core::SampleFormat::I24;
    click_dialog(&mut app, &ctx, "Apply");
    // The worker cannot publish its result until the next poll.
    assert!(app.job.is_some());
    assert_eq!(format!("{:?}", app.session.project), before);
    wait_for_save(&mut app);
    assert!(app.error.is_none());
    assert_eq!(app.session.project.sample_rate_hz, 96_000);
    assert_eq!(app.session.project.buffer_size_frames, 128);
    assert_eq!(app.session.project.name, "Audio settings");
    assert_eq!(
        app.session.project.output_sample_format,
        daw_core::SampleFormat::I24
    );
    assert!(app.dirty);
    assert_eq!(app.selected_track, Some(track));
    assert!(!app.sync_needed);
}

#[test]
fn failed_audio_settings_retain_name_timing_and_modified_state() {
    let (mut app, _) = fixture();
    app.session.project.sample_rate_hz = 192_000;
    app.session.project.tracks[0].clips[0].length_frames = 1;
    let before = format!("{:?}", app.session.project);
    let ctx = context();
    app.open_project_settings();
    let draft = app.project_settings.as_mut().unwrap();
    draft.name = "Must not commit".into();
    draft.sample_rate_hz = 44_100;
    draft.buffer_size_frames = 64;
    frame(&mut app, &ctx, vec![]);
    click_dialog(&mut app, &ctx, "Apply");
    wait_for_save(&mut app);
    assert!(app.error.is_some());
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
}

#[test]
fn settings_dialog_fields_share_left_and_right_edges() {
    for theme in [ColorTheme::Dark, ColorTheme::Light] {
        for project_settings in [false, true] {
            let (mut app, _) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            let labels = if project_settings {
                app.open_project_settings();
                vec!["Untitled", "48000 Hz", "512 samples", "Rubber Band"]
            } else {
                app.settings_open = true;
                vec![
                    theme.name(),
                    app.settings.accent.name(),
                    #[cfg(any(target_os = "macos", target_os = "windows"))]
                    app.settings.titlebar.name(),
                ]
            };
            for _ in 0..3 {
                frame(&mut app, &ctx, vec![]);
            }
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let fields: Vec<_> = labels
                .iter()
                .map(|label| {
                    let point = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == *label => {
                                Some(text.pos + text.galley.size() * 0.5)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing field: {label}"));
                    shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect)
                                if rect.rect.contains(point)
                                    && rect.rect.width() > 100.0
                                    && rect.rect.height() < 30.0 =>
                            {
                                Some(rect.rect)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing field border: {label}"))
                })
                .collect();
            for rect in &fields[1..] {
                assert!((rect.left() - fields[0].left()).abs() < 0.1, "{fields:?}");
                assert!((rect.right() - fields[0].right()).abs() < 0.1, "{fields:?}");
            }
        }
    }
}

#[test]
fn engine_selector_draft_cancel_apply_and_noop() {
    for theme in [ColorTheme::Dark, ColorTheme::Light] {
        let (mut app, track) = fixture();
        app.selected_track = Some(track);
        app.selected_tracks.insert(track);
        let ctx = context();
        app.set_settings(
            &ctx,
            AppSettings {
                theme,
                ..Default::default()
            },
        );
        app.open_project_settings();
        assert_eq!(
            app.project_settings.as_ref().unwrap().stretch_engine,
            daw_core::StretchEngine::RUBBER_BAND
        );
        app.project_settings.as_mut().unwrap().stretch_engine = daw_core::StretchEngine::INTEGRATED;
        frame(&mut app, &ctx, vec![]);
        click_dialog(&mut app, &ctx, "Cancel");
        assert_eq!(
            app.session.project.stretch_engine,
            daw_core::StretchEngine::RUBBER_BAND
        );
        assert!(!app.dirty);
        app.open_project_settings();
        app.project_settings.as_mut().unwrap().stretch_engine = daw_core::StretchEngine::INTEGRATED;
        frame(&mut app, &ctx, vec![]);
        click_dialog(&mut app, &ctx, "Apply");
        assert!(app.job.is_some());
        wait_for_save(&mut app);
        assert!(app.error.is_none());
        assert_eq!(
            app.session.project.stretch_engine,
            daw_core::StretchEngine::INTEGRATED
        );
        assert_eq!(app.selected_track, Some(track));
        assert!(app.dirty);
        app.dirty = false;
        app.open_project_settings();
        frame(&mut app, &ctx, vec![]);
        click_dialog(&mut app, &ctx, "Apply");
        assert!(app.job.is_none());
        assert!(!app.dirty);
    }
}

#[test]
fn cancelling_settings_worker_discards_late_publication() {
    let (mut app, _) = fixture();
    let before = format!("{:?}", app.session.project);
    let ctx = context();
    app.open_project_settings();
    app.project_settings.as_mut().unwrap().stretch_engine = daw_core::StretchEngine::INTEGRATED;
    frame(&mut app, &ctx, vec![]);
    click_dialog(&mut app, &ctx, "Apply");
    let token = app.settings_cancel.take().unwrap();
    token.store(true, Ordering::Relaxed);
    app.job = None;
    app.busy.clear();
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
}

#[test]
fn project_output_format_only_changes_commit_and_undo_as_audio_settings() {
    let (mut app, _) = fixture();
    let ctx = context();
    let before = app.session.project.audio_quality();
    app.open_project_settings();
    app.project_settings.as_mut().unwrap().output_sample_format = daw_core::SampleFormat::I16;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    click_dialog(&mut app, &ctx, "Apply");
    assert!(app.job.is_some());
    assert_eq!(app.session.project.audio_quality(), before);
    wait_for_save(&mut app);
    assert!(app.error.is_none());
    assert_eq!(
        app.session.project.output_sample_format,
        daw_core::SampleFormat::I16
    );
    assert_eq!(app.session.project.sample_rate_hz, before.sample_rate_hz);
    assert_eq!(
        app.session.project.buffer_size_frames,
        before.buffer_size_frames
    );
    assert!(app.dirty);
    app.record_history();
    frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    if let Some(id) = ctx.memory(|m| m.focused()) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.audio_quality(), before);
    assert!(
        !app.sync_needed,
        "Changing the stream format through undo must recreate output"
    );
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert_eq!(
        app.session.project.output_sample_format,
        daw_core::SampleFormat::I16
    );
    assert!(!app.sync_needed);
}
