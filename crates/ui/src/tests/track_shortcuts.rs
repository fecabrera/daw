use super::*;

fn panel(app: &DawUi, id: Id) -> Rect {
    let lane = app.lane_bounds[&id];
    Rect::from_min_size(
        Pos2::new(lane.left() - TRACK_WIDTH, lane.top()),
        Vec2::new(TRACK_WIDTH, ROW_HEIGHT),
    )
}

fn click_panel(
    app: &mut DawUi,
    ctx: &egui::Context,
    id: Id,
    modifiers: egui::Modifiers,
    name: bool,
) -> Vec<egui::epaint::ClippedShape> {
    let rect = panel(app, id);
    let point = rect.min
        + if name {
            Vec2::new(110.0, 18.0)
        } else {
            Vec2::new(248.0, 40.0)
        };
    let shapes = frame_shapes(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            },
        ],
        Vec2::new(1280.0, 800.0),
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        }],
    );
    shapes
}

fn key_event(
    key: egui::Key,
    modifiers: egui::Modifiers,
    repeat: bool,
    pressed: bool,
) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers,
    }
}

fn press(app: &mut DawUi, ctx: &egui::Context, key: egui::Key) {
    frame(
        app,
        ctx,
        vec![
            key_event(key, Default::default(), false, true),
            key_event(key, Default::default(), false, false),
        ],
    );
}

#[test]
fn panel_multi_selection_toggles_with_shift_or_command_without_changing_clips() {
    for modifiers in [egui::Modifiers::SHIFT, egui::Modifiers::COMMAND] {
        let (mut app, tracks, clips) = multi_clip_fixture();
        let ctx = context();
        app.select_clip(clips[0], tracks[0], false);
        frame(&mut app, &ctx, vec![]);
        let shapes = click_panel(&mut app, &ctx, tracks[1], modifiers, true);
        assert_eq!(app.selected_tracks, HashSet::from([tracks[0], tracks[1]]));
        assert_eq!(app.selected_track, Some(tracks[1]));
        for id in &tracks[..2] {
            assert!(has_preview_outline(
                &shapes,
                panel(&app, *id),
                theme::ACCENT
            ));
        }
        let shapes = click_panel(&mut app, &ctx, tracks[1], modifiers, false);
        assert_eq!(app.selected_tracks, HashSet::from([tracks[0]]));
        assert_eq!(app.selected_track, Some(tracks[0]));
        assert!(!has_preview_outline(
            &shapes,
            panel(&app, tracks[1]),
            theme::ACCENT
        ));
        let shapes = click_panel(&mut app, &ctx, tracks[2], Default::default(), true);
        assert_eq!(app.selected_tracks, HashSet::from([tracks[2]]));
        assert!(has_preview_outline(
            &shapes,
            panel(&app, tracks[2]),
            theme::ACCENT
        ));
        assert!(!has_preview_outline(
            &shapes,
            panel(&app, tracks[0]),
            theme::ACCENT
        ));
        click_panel(&mut app, &ctx, tracks[2], modifiers, false);
        assert!(app.selected_tracks.is_empty() && app.selected_track.is_none());
        assert_eq!(app.selected_clips, HashSet::from([clips[0]]));
        assert!(!app.dirty && !app.sync_needed);
        press(&mut app, &ctx, egui::Key::M);
        assert!(!app.dirty);
    }
}

#[test]
fn mute_and_solo_keys_toggle_each_selected_track_and_ignore_repeat_or_command_keys() {
    let (mut app, tracks, _) = multi_clip_fixture();
    let ctx = context();
    app.use_native_menu();
    app.session.project.tracks[1].muted = true;
    app.session.project.tracks[1].soloed = true;
    frame(&mut app, &ctx, vec![]);
    click_panel(&mut app, &ctx, tracks[0], Default::default(), true);
    click_panel(&mut app, &ctx, tracks[1], egui::Modifiers::SHIFT, false);
    assert_eq!(app.selected_tracks, HashSet::from([tracks[0], tracks[1]]));
    assert!(
        !ctx.egui_wants_keyboard_input(),
        "panel click retained keyboard focus"
    );
    press(&mut app, &ctx, egui::Key::M);
    assert!(app.session.project.tracks[0].muted);
    assert!(!app.session.project.tracks[1].muted);
    assert!(!app.session.project.tracks[2].muted);
    assert!(app.dirty && app.sync_needed);
    press(&mut app, &ctx, egui::Key::S);
    assert!(app.session.project.tracks[0].soloed);
    assert!(!app.session.project.tracks[1].soloed);
    assert!(!app.session.project.tracks[2].soloed);
    let before = format!("{:?}", app.session.project);
    for (modifiers, repeat) in [
        (egui::Modifiers::COMMAND, false),
        (egui::Modifiers::ALT, false),
        (egui::Modifiers::CTRL, false),
    ] {
        for key in [egui::Key::M, egui::Key::S] {
            frame(
                &mut app,
                &ctx,
                vec![
                    key_event(key, modifiers, repeat, true),
                    key_event(key, modifiers, repeat, false),
                ],
            );
            assert_eq!(format!("{:?}", app.session.project), before);
        }
    }
    for key in [egui::Key::M, egui::Key::S] {
        frame(
            &mut app,
            &ctx,
            vec![key_event(key, Default::default(), false, true)],
        );
        let held = format!("{:?}", app.session.project);
        frame(
            &mut app,
            &ctx,
            vec![
                key_event(key, Default::default(), true, true),
                key_event(key, Default::default(), false, false),
            ],
        );
        assert_eq!(format!("{:?}", app.session.project), held);
        press(&mut app, &ctx, key);
    }
    press(&mut app, &ctx, egui::Key::M);
    press(&mut app, &ctx, egui::Key::S);
    assert!(!app.session.project.tracks[0].muted && !app.session.project.tracks[0].soloed);
    assert!(app.session.project.tracks[1].muted && app.session.project.tracks[1].soloed);
    click_panel(&mut app, &ctx, tracks[2], Default::default(), true);
    press(&mut app, &ctx, egui::Key::M);
    press(&mut app, &ctx, egui::Key::S);
    assert!(app.session.project.tracks[2].muted && app.session.project.tracks[2].soloed);
    assert!(!app.session.project.tracks[0].muted && !app.session.project.tracks[0].soloed);
    assert!(app.session.project.tracks[1].muted && app.session.project.tracks[1].soloed);
}

#[test]
fn track_keys_do_not_change_the_project_during_text_editing_or_blocked_interactions() {
    for blocked in [
        "name",
        "tempo",
        "gain",
        "job",
        "error",
        "settings",
        "unsaved",
        "drag",
        "unfocused",
        "disabled",
    ] {
        let (mut app, tracks, _) = multi_clip_fixture();
        let ctx = context();
        app.select_track(tracks[0], false);
        app.select_track(tracks[1], true);
        frame(&mut app, &ctx, vec![]);
        let mut sender = None;
        match blocked {
            "name" => {
                app.track_name_edit = Some(TrackNameEdit {
                    track: tracks[0],
                    text: "Track".into(),
                    focus: true,
                })
            }
            "tempo" => {
                app.tempo_edit = Some(TempoEdit {
                    text: "120".into(),
                    width: 60.0,
                    focus: true,
                })
            }
            "gain" => {
                let point = panel(&app, tracks[0]).min + Vec2::new(120.0, 47.0);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), button(point, true)],
                );
                frame(&mut app, &ctx, vec![button(point, false)]);
                assert!(ctx.egui_wants_keyboard_input());
            }
            "job" => {
                let (tx, rx) = mpsc::channel();
                sender = Some(tx);
                app.job = Some(rx);
            }
            "error" => app.error = Some("Test".into()),
            "settings" => app.settings_open = true,
            "unsaved" => app.unsaved_prompt = true,
            "drag" => app.track_drag = Some(tracks[0]),
            _ => {}
        }
        let before = format!("{:?}", app.session.project);
        let mut events = vec![
            key_event(egui::Key::M, Default::default(), false, true),
            key_event(egui::Key::S, Default::default(), false, true),
        ];
        if blocked == "drag" {
            let point = app.lane_bounds[&tracks[0]].center();
            events.push(egui::Event::PointerMoved(point));
            events.push(button(point, true));
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: blocked != "unfocused",
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(blocked != "disabled", |ui| app.show(ui));
                if blocked != "disabled" {
                    for action in TrackAction::ALL {
                        assert!(!app.track_action_enabled(action, &ctx), "{blocked}");
                        app.perform_track_action(action, &ctx);
                    }
                }
            },
        );
        output.textures_delta.clear();
        assert_eq!(format!("{:?}", app.session.project), before, "{blocked}");
        assert!(!app.dirty, "{blocked}");
        drop(sender);
    }
}

fn menu_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    enabled: bool,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
            events,
            ..Default::default()
        },
        |ui| {
            ui.add_enabled_ui(enabled, |ui| app.track_menu(ui));
        },
    );
    output.textures_delta.clear();
    output
}

#[test]
fn track_menu_clicks_use_the_same_group_actions_as_shortcuts() {
    let (mut app, tracks, _) = multi_clip_fixture();
    let ctx = context();
    app.select_track(tracks[0], false);
    app.select_track(tracks[1], true);
    app.session.project.tracks[1].muted = true;
    app.session.project.tracks[1].soloed = true;
    for action in [TrackAction::Mute, TrackAction::Solo] {
        let output = menu_frame(&mut app, &ctx, vec![], true);
        assert!(app.track_action_enabled(action, &ctx));
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == action.label() => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("track menu label");
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == action.shortcut().unwrap().logical_key.name()
        )));
        menu_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            true,
        );
        assert!(!app.dirty || action == TrackAction::Solo);
        menu_frame(&mut app, &ctx, vec![button(point, false)], true);
        assert!(app.dirty && app.sync_needed);
        assert_eq!(app.selected_tracks, HashSet::from([tracks[0], tracks[1]]));
    }
    assert!(app.session.project.tracks[0].muted && app.session.project.tracks[0].soloed);
    assert!(!app.session.project.tracks[1].muted && !app.session.project.tracks[1].soloed);
    assert!(!app.session.project.tracks[2].muted && !app.session.project.tracks[2].soloed);
    for action in [TrackAction::Mute, TrackAction::Solo] {
        app.perform_track_action(action, &ctx);
    }
    assert!(!app.session.project.tracks[0].muted && !app.session.project.tracks[0].soloed);
    assert!(app.session.project.tracks[1].muted && app.session.project.tracks[1].soloed);
}

#[test]
fn track_menu_disables_items_without_selection_or_when_ui_is_disabled() {
    let (mut app, tracks, _) = multi_clip_fixture();
    let ctx = context();
    menu_frame(&mut app, &ctx, vec![], true);
    for action in [
        TrackAction::Mute,
        TrackAction::Solo,
        TrackAction::Duplicate,
        TrackAction::Delete,
    ] {
        assert!(!app.track_action_enabled(action, &ctx));
        app.perform_track_action(action, &ctx);
    }
    assert!(!app.dirty);
    app.select_track(tracks[0], false);
    let output = menu_frame(&mut app, &ctx, vec![], false);
    for action in TrackAction::ALL {
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == action.label() => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("disabled track menu label");
        menu_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            false,
        );
        menu_frame(&mut app, &ctx, vec![button(point, false)], false);
    }
    assert!(!app.dirty);
    assert!(!app.session.project.tracks[0].muted && !app.session.project.tracks[0].soloed);
}

#[test]
fn track_selection_survives_reordering_and_save_but_prunes_deleted_tracks_and_resets_on_open() {
    let (mut app, tracks, _) = multi_clip_fixture();
    app.select_track(tracks[0], false);
    app.select_track(tracks[2], true);
    app.edit(Edit::ReorderTrack {
        track_id: tracks[2],
        index: 0,
    });
    assert_eq!(app.selected_tracks, HashSet::from([tracks[0], tracks[2]]));
    app.edit(Edit::DeleteTrack(tracks[2]));
    assert_eq!(app.selected_tracks, HashSet::from([tracks[0]]));
    assert_eq!(app.selected_track, Some(tracks[0]));
    let loaded = app.session.clone();
    let (tx, rx) = mpsc::channel();
    app.job = Some(rx);
    tx.send(Ok(Job::Saved(loaded.clone()))).unwrap();
    app.poll();
    assert_eq!(app.selected_tracks, HashSet::from([tracks[0]]));
    app.select_track(tracks[1], true);
    let (tx, rx) = mpsc::channel();
    app.job = Some(rx);
    tx.send(Ok(Job::Loaded(loaded, false))).unwrap();
    app.poll();
    assert_eq!(app.selected_tracks, HashSet::from([tracks[0]]));
    assert_eq!(app.selected_track, Some(tracks[0]));
    app.pending = Some(Action::CloseProject);
    app.perform_pending();
    assert!(app.selected_tracks.is_empty() && app.selected_track.is_none());
}

#[test]
fn add_track_menu_item_is_first_and_works_without_a_selection() {
    let (mut app, _, clips) = multi_clip_fixture();
    let ctx = context();
    app.selected_clips.insert(clips[0]);
    let before = app.session.project.tracks.clone();
    let output = menu_frame(&mut app, &ctx, vec![], true);
    assert!(app.track_action_enabled(TrackAction::Add, &ctx));
    assert!(TrackAction::Add.shortcut().is_none());
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if [
                    "Add track",
                    "Mute",
                    "Solo",
                    "Duplicate track",
                    "Delete track",
                ]
                .contains(&text.galley.text()) =>
            {
                Some((
                    text.galley.text().to_owned(),
                    text.pos + text.galley.size() * 0.5,
                ))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        labels
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>(),
        [
            "Add track",
            "Mute",
            "Solo",
            "Duplicate track",
            "Delete track"
        ]
    );
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::LineSegment { points, .. }
        if points[0].y == points[1].y && points[0].y > labels[0].1.y && points[0].y < labels[1].1.y
    )));
    let point = labels[0].1;
    menu_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        true,
    );
    assert_eq!(app.session.project.tracks.len(), before.len());
    menu_frame(&mut app, &ctx, vec![button(point, false)], true);
    assert_eq!(app.session.project.tracks.len(), before.len() + 1);
    assert_eq!(
        format!("{:?}", &app.session.project.tracks[..before.len()]),
        format!("{before:?}")
    );
    let added = app.session.project.tracks.last().unwrap();
    assert!(added.clips.is_empty() && !added.muted && !added.soloed);
    assert_eq!(added.color, daw_core::default_track_color(before.len()));
    assert_eq!(app.selected_track, Some(added.id));
    assert_eq!(app.selected_tracks, HashSet::from([added.id]));
    assert_eq!(app.selected_clips, HashSet::from([clips[0]]));
    assert!(app.dirty && app.sync_needed);
}

fn secondary_click(
    app: &mut DawUi,
    ctx: &egui::Context,
    point: Pos2,
) -> Vec<egui::epaint::ClippedShape> {
    let event = |pressed| egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), event(true)],
    );
    frame(app, ctx, vec![event(false)]);
    frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
}

#[test]
fn track_context_menu_preserves_groups_and_deletes_only_the_target_selection_on_release() {
    for (group, offset) in [
        (true, Vec2::new(110.0, 18.0)),
        (true, Vec2::new(248.0, 40.0)),
        (true, Vec2::new(24.0, 48.0)),
        (true, Vec2::new(234.0, 40.0)),
        (false, Vec2::new(110.0, 18.0)),
    ] {
        let (mut app, tracks, _) = multi_clip_fixture();
        let ctx = context();
        let mut survivor = app.session.project.tracks[0].clips[0].clone();
        survivor.id = Id::new_v4();
        let selected_clip = survivor.id;
        app.session.project.tracks[2].clips.push(survivor);
        app.select_clip(selected_clip, tracks[2], false);
        app.select_track(tracks[0], false);
        app.select_track(tracks[1], true);
        frame(&mut app, &ctx, vec![]);
        let target = if group { tracks[0] } else { tracks[2] };
        let expected = if group {
            HashSet::from([tracks[0], tracks[1]])
        } else {
            HashSet::from([tracks[2]])
        };
        let before = format!("{:?}", app.session.project);
        let sources = format!("{:?}", app.session.project.assets);
        let point = panel(&app, target).min + offset;
        let shapes = secondary_click(&mut app, &ctx, point);
        assert_eq!(app.selected_tracks, expected);
        assert_eq!(format!("{:?}", app.session.project), before);
        assert!(!app.dirty && !app.sync_needed);
        let label = if group {
            "Delete selected tracks"
        } else {
            "Delete track"
        };
        let point = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("track context menu label");
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        assert_eq!(format!("{:?}", app.session.project), before);
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert!(app.dirty);
        assert_eq!(app.session.project.tracks.len(), if group { 1 } else { 2 });
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.session.project.tracks.len(), if group { 1 } else { 2 });
        assert!(
            app.session
                .project
                .tracks
                .iter()
                .all(|track| !expected.contains(&track.id))
        );
        assert_eq!(format!("{:?}", app.session.project.assets), sources);
        assert!(app.selected_tracks.is_empty() && app.selected_track.is_none());
        assert_eq!(
            app.selected_clips,
            if group {
                HashSet::from([selected_clip])
            } else {
                HashSet::new()
            }
        );
        assert_eq!(app.selected_clip, group.then_some(selected_clip));
        assert!(app.inputs.keys().all(|id| !expected.contains(id)));
        assert!(app.dirty);
    }
}

#[test]
fn dismissing_or_blocking_the_track_context_menu_keeps_tracks() {
    let (mut app, tracks, _) = multi_clip_fixture();
    let ctx = context();
    app.select_track(tracks[0], false);
    app.select_track(tracks[1], true);
    frame(&mut app, &ctx, vec![]);
    let point = panel(&app, tracks[0]).min + Vec2::new(248.0, 40.0);
    let before = format!("{:?}", app.session.project);
    secondary_click(&mut app, &ctx, point);
    press(&mut app, &ctx, egui::Key::Escape);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
    let (sender, receiver) = mpsc::channel();
    app.job = Some(receiver);
    let shapes = secondary_click(&mut app, &ctx, point);
    assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().starts_with("Delete"))));
    assert_eq!(format!("{:?}", app.session.project), before);
    app.perform_track_action(TrackAction::Delete, &ctx);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
    drop(sender);
}

#[test]
fn deleting_all_selected_tracks_clears_selection_and_requests_playback_sync() {
    let (mut app, tracks, clips) = multi_clip_fixture();
    let ctx = context();
    app.select_clip(clips[0], tracks[0], false);
    app.select_track(tracks[1], true);
    app.select_track(tracks[2], true);
    frame(&mut app, &ctx, vec![]);
    let assets = format!("{:?}", app.session.project.assets);
    app.perform_track_action(TrackAction::Delete, &ctx);
    assert!(app.session.project.tracks.is_empty());
    assert!(app.selected_tracks.is_empty() && app.selected_track.is_none());
    assert!(app.selected_clips.is_empty() && app.selected_clip.is_none());
    assert!(app.inputs.is_empty());
    assert_eq!(format!("{:?}", app.session.project.assets), assets);
    assert!(app.dirty && app.sync_needed);
    frame(&mut app, &ctx, vec![]);
    assert!(app.error.is_none());
}

#[test]
fn track_menu_deletes_single_or_multiple_tracks_through_the_shared_action() {
    for group in [false, true] {
        let (mut app, tracks, clips) = multi_clip_fixture();
        let ctx = context();
        app.select_clip(clips[0], tracks[0], false);
        if group {
            app.select_track(tracks[1], true);
        }
        let before = format!("{:?}", app.session.project);
        let assets = format!("{:?}", app.session.project.assets);
        let survivor = format!("{:?}", app.session.project.tracks.last().unwrap());
        let output = menu_frame(&mut app, &ctx, vec![], true);
        assert!(app.track_action_enabled(TrackAction::Delete, &ctx));
        assert!(TrackAction::Delete.shortcut().is_none());
        let label = if group {
            "Delete selected tracks"
        } else {
            "Delete track"
        };
        assert_eq!(app.track_action_label(TrackAction::Delete), label);
        let text_point = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("track menu label")
        };
        let point = text_point(label);
        let solo = text_point("Solo");
        assert!(point.y > solo.y);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::LineSegment { points, .. }
            if points[0].y == points[1].y && points[0].y > solo.y && points[0].y < point.y
        )));
        menu_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            true,
        );
        assert_eq!(format!("{:?}", app.session.project), before);
        menu_frame(&mut app, &ctx, vec![button(point, false)], true);
        assert_eq!(app.session.project.tracks.len(), if group { 1 } else { 2 });
        assert_eq!(
            format!("{:?}", app.session.project.tracks.last().unwrap()),
            survivor
        );
        assert_eq!(format!("{:?}", app.session.project.assets), assets);
        assert!(app.selected_tracks.is_empty() && app.selected_track.is_none());
        assert!(app.selected_clips.is_empty() && app.selected_clip.is_none());
        assert!(app.dirty && app.sync_needed);
    }
}

#[test]
fn both_track_menus_duplicate_settings_without_clips_and_select_the_copies() {
    for context_menu in [false, true] {
        for group in [false, true] {
            let (mut app, tracks, clips) = multi_clip_fixture();
            let ctx = context();
            app.session.project.tracks[0].name = "Bass".into();
            app.session.project.tracks[0].color = daw_core::RgbColor {
                r: 21,
                g: 45,
                b: 90,
            };
            app.session.project.tracks[0].gain_db = -8.0;
            app.session.project.tracks[0].pan = -0.35;
            app.session.project.tracks[0].muted = true;
            app.session.project.tracks[1].soloed = true;
            app.select_clip(clips[0], tracks[0], false);
            if group {
                app.select_track(tracks[1], true);
            }
            let originals = app.session.project.tracks.clone();
            let before = format!("{:?}", app.session.project);
            let assets = format!("{:?}", app.session.project.assets);
            let shapes = if context_menu {
                frame(&mut app, &ctx, vec![]);
                let point = panel(&app, tracks[0]).min + Vec2::new(110.0, 18.0);
                secondary_click(&mut app, &ctx, point)
            } else {
                menu_frame(&mut app, &ctx, vec![], true).shapes
            };
            let label = if group {
                "Duplicate selected tracks"
            } else {
                "Duplicate track"
            };
            assert_eq!(app.track_action_label(TrackAction::Duplicate), label);
            assert!(TrackAction::Duplicate.shortcut().is_none());
            let point = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("duplicate track menu label");
            let press = vec![egui::Event::PointerMoved(point), button(point, true)];
            if context_menu {
                frame(&mut app, &ctx, press);
            } else {
                menu_frame(&mut app, &ctx, press, true);
            }
            assert_eq!(format!("{:?}", app.session.project), before);
            let release = vec![button(point, false)];
            if context_menu {
                frame(&mut app, &ctx, release);
            } else {
                menu_frame(&mut app, &ctx, release, true);
            }
            let count = if group { 2 } else { 1 };
            assert_eq!(app.session.project.tracks.len(), 3 + count);
            let copied: Vec<_> = app
                .session
                .project
                .tracks
                .iter()
                .filter(|track| !tracks.contains(&track.id))
                .collect();
            assert_eq!(copied.len(), count);
            for (index, copy) in copied.iter().enumerate() {
                let original = &originals[index];
                let mut expected = original.clone();
                expected.id = copy.id;
                expected.clips.clear();
                assert_eq!(format!("{copy:?}"), format!("{expected:?}"));
                let position = app
                    .session
                    .project
                    .tracks
                    .iter()
                    .position(|track| track.id == original.id)
                    .unwrap();
                assert_eq!(app.session.project.tracks[position + 1].id, copy.id);
            }
            for original in &originals {
                let retained = app
                    .session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == original.id)
                    .unwrap();
                assert_eq!(format!("{retained:?}"), format!("{original:?}"));
            }
            assert_eq!(
                app.selected_tracks,
                copied.iter().map(|track| track.id).collect()
            );
            assert_eq!(app.selected_track, Some(copied[0].id));
            assert_eq!(app.selected_clips, HashSet::from([clips[0]]));
            assert_eq!(app.selected_clip, Some(clips[0]));
            assert_eq!(format!("{:?}", app.session.project.assets), assets);
            assert!(app.dirty);
            let previous_ids: HashSet<_> = app
                .session
                .project
                .tracks
                .iter()
                .map(|track| track.id)
                .collect();
            app.perform_track_action(TrackAction::Duplicate, &ctx);
            assert_eq!(app.session.project.tracks.len(), 3 + 2 * count);
            assert!(
                app.selected_tracks
                    .iter()
                    .all(|id| !previous_ids.contains(id))
            );
            assert!(app.sync_needed);
        }
    }
}

#[test]
fn failed_track_duplication_keeps_the_original_project_and_selection() {
    let (mut app, tracks, _) = multi_clip_fixture();
    let ctx = context();
    app.select_track(tracks[0], false);
    app.select_track(tracks[1], true);
    frame(&mut app, &ctx, vec![]);
    app.session.project.tracks[0].pan = 2.0;
    let before = format!("{:?}", app.session.project);
    let selection = app.selected_tracks.clone();
    app.perform_track_action(TrackAction::Duplicate, &ctx);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert_eq!(app.selected_tracks, selection);
    assert_eq!(app.selected_track, Some(tracks[1]));
    assert!(!app.dirty && !app.sync_needed);
    assert!(app.error.is_some());
}

#[test]
fn copy_plus_header_button_is_disabled_without_selection_and_duplicates_the_group() {
    let (mut app, tracks, clips) = multi_clip_fixture();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let point = Pos2::new(
        TRACK_WIDTH - 19.0,
        app.lane_bounds[&tracks[0]].top() - RULER_HEIGHT / 2.0,
    );
    let before = format!("{:?}", app.session.project);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert_eq!(format!("{:?}", app.session.project), before);
    assert!(!app.dirty);
    app.select_clip(clips[0], tracks[0], false);
    app.select_track(tracks[1], true);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    assert_eq!(format!("{:?}", app.session.project), before);
    frame(&mut app, &ctx, vec![button(point, false)]);
    assert_eq!(app.session.project.tracks.len(), 5);
    assert_eq!(
        app.selected_tracks,
        HashSet::from([
            app.session.project.tracks[1].id,
            app.session.project.tracks[3].id
        ])
    );
    assert!(
        app.session
            .project
            .tracks
            .iter()
            .filter(|track| app.selected_tracks.contains(&track.id))
            .all(|track| track.clips.is_empty())
    );
    assert_eq!(app.selected_clips, HashSet::from([clips[0]]));
    assert!(app.dirty && app.error.is_none());
}
