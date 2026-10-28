use super::*;

fn history_key(action: HistoryAction, mac: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Z,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: egui::Modifiers {
            command: true,
            mac_cmd: mac,
            ctrl: !mac,
            shift: action == HistoryAction::Redo,
            ..Default::default()
        },
    }
}

#[test]
fn history_shortcuts_restore_group_edits_and_branch_redo_on_both_platforms() {
    for mac in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        app.edit(Edit::Batch(vec![
            Edit::SetTrackMuted {
                track_id: track,
                muted: true,
            },
            Edit::SetTrackSoloed {
                track_id: track,
                soloed: true,
            },
        ]));
        let edited = app.session.project.clone();
        frame(
            &mut app,
            &ctx,
            vec![history_key(HistoryAction::Undo, mac, false)],
        );
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        app.perform_history_action(HistoryAction::Redo, &ctx);
        frame(
            &mut app,
            &ctx,
            vec![history_key(HistoryAction::Undo, mac, true)],
        );
        assert_eq!(
            app.session.project, edited,
            "Held key repeats must be ignored"
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        app.perform_history_action(HistoryAction::Undo, &ctx);
        frame(
            &mut app,
            &ctx,
            vec![history_key(HistoryAction::Redo, mac, false)],
        );
        assert_eq!(app.session.project, edited);
        assert!(app.dirty);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        app.perform_history_action(HistoryAction::Undo, &ctx);
        app.edit(Edit::SetTrackMuted {
            track_id: track,
            muted: true,
        });
        assert!(!app.history_action_enabled(HistoryAction::Redo, &ctx));
        let after = app.session.project.clone();
        frame(
            &mut app,
            &ctx,
            vec![history_key(HistoryAction::Redo, mac, false)],
        );
        assert_eq!(
            app.session.project, after,
            "Redo must not fall through to Undo"
        );
    }
}

#[test]
fn history_groups_live_control_changes_and_ignores_seek_selection_and_failed_jobs() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.select_track(track, false);
    app.seek(2400);
    assert!(!app.history_action_enabled(HistoryAction::Undo, &ctx));
    // A pointer gesture spans several committed, audible control updates.
    let point = Pos2::new(-100.0, -100.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    for gain in [-1.0, -3.0, -9.0] {
        app.session.project.tracks[0].gain_db = gain;
        app.changed();
        frame(&mut app, &ctx, vec![]);
    }
    frame(&mut app, &ctx, vec![button(point, false)]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tracks[0].gain_db, 0.0);
    assert_eq!(app.session.project.transport.playhead_frame, 2400);
    assert!(!app.history_action_enabled(HistoryAction::Undo, &ctx));
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    assert!(!app.history_action_enabled(HistoryAction::Redo, &ctx));
    sender.send(Err("test preparation failure".into())).unwrap();
    app.poll();
    app.error = None;
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert_eq!(app.session.project.tracks[0].gain_db, -9.0);
    app.pending = Some(Action::New);
    app.perform_pending();
    assert!(!app.history_action_enabled(HistoryAction::Undo, &ctx));
    assert!(!app.history_action_enabled(HistoryAction::Redo, &ctx));
}

#[test]
fn history_menu_native_guard_and_text_edit_ownership() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.edit(Edit::SetTrackMuted {
        track_id: track,
        muted: true,
    });
    app.use_native_menu();
    frame(
        &mut app,
        &ctx,
        vec![history_key(HistoryAction::Undo, true, false)],
    );
    assert!(
        app.session.project.tracks[0].muted,
        "Native menu owns the key equivalent"
    );
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert!(!app.session.project.tracks[0].muted);
    app.settings_open = true;
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert!(!app.session.project.tracks[0].muted);
    app.settings_open = false;
    let mut text = String::from("Text");
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.text_edit_singleline(&mut text).request_focus();
    });
    output.textures_delta.clear();
    assert!(ctx.egui_wants_keyboard_input());
    assert!(!app.history_action_enabled(HistoryAction::Redo, &ctx));
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert!(!app.session.project.tracks[0].muted);
}

#[test]
fn edit_menu_clicks_undo_and_redo_and_save_completion_keeps_the_right_checkpoint() {
    fn menu(app: &mut DawUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
                events,
                ..Default::default()
            },
            |ui| app.edit_menu(ui),
        );
        output.textures_delta.clear();
        output
    }
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.edit(Edit::SetTrackMuted {
        track_id: track,
        muted: true,
    });
    for action in HistoryAction::ALL {
        let output = menu(&mut app, &ctx, vec![]);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == action.label() => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        menu(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        menu(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(
            app.session.project.tracks[0].muted,
            action == HistoryAction::Redo
        );
    }
    let (sender, receiver) = mpsc::channel();
    let written = app.session.clone();
    app.regular_save = Some(regular_save::PendingSave {
        receiver,
        project: app.session.project.project_id,
        revision: app.change_revision,
        navigation_revision: app.navigation_revision,
    });
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert!(!app.dirty);
    sender.send(Ok(written)).unwrap();
    app.poll_regular_save();
    assert!(
        app.dirty,
        "Disk now contains the newer state that was undone"
    );
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert!(!app.dirty);
    app.seek(4800);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    app.perform_history_action(HistoryAction::Redo, &ctx);
    assert!(app.dirty, "Undo keeps an unsaved explicit seek");
}

#[test]
fn history_shortcut_works_with_a_focused_knob_after_a_drag() {
    let (mut app, track) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let point = Pos2::new(lane.left() - 47.0, lane.top() + 47.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    for offset in [10.0, 20.0, 30.0] {
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point - Vec2::new(0.0, offset))],
        );
    }
    frame(
        &mut app,
        &ctx,
        vec![button(point - Vec2::new(0.0, 30.0), false)],
    );
    let gain = app.session.project.tracks[0].gain_db;
    assert!(gain > 0.0);
    assert!(ctx.egui_wants_keyboard_input());
    assert!(!ctx.text_edit_focused());
    frame(
        &mut app,
        &ctx,
        vec![history_key(HistoryAction::Undo, true, false)],
    );
    assert_eq!(app.session.project.tracks[0].gain_db, 0.0);
    assert!(!app.history_action_enabled(HistoryAction::Undo, &ctx));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    frame(
        &mut app,
        &ctx,
        vec![history_key(HistoryAction::Redo, true, false)],
    );
    assert_eq!(app.session.project.tracks[0].gain_db, gain);
}

#[test]
fn completed_worker_edit_is_separate_from_a_new_pointer_gesture() {
    let (mut app, _) = fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let original_name = app.session.project.name.clone();
    let mut prepared = app.session.clone();
    prepared.project.name = "Prepared edit".into();
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    sender.send(Ok(Job::Loaded(prepared, true))).unwrap();
    let point = Pos2::new(-100.0, -100.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    app.session.project.master.gain_db = -6.0;
    app.changed();
    frame(&mut app, &ctx, vec![button(point, false)]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.master.gain_db, 0.0);
    assert_eq!(app.session.project.name, "Prepared edit");
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.name, original_name);
}
