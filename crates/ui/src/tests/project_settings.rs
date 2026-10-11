use super::*;

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
