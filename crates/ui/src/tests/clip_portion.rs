use super::*;

fn modifiers_frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::CursorIcon {
    let events = events
        .into_iter()
        .map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            },
            other => other,
        })
        .collect::<Vec<_>>();
    let mut events = events;
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

fn x_at(app: &DawUi, lane: Rect, frame: u64) -> f32 {
    lane.left()
        + ((app.session.project.seconds(frame) - app.view_scroll()) as f32) * app.view_zoom()
}

#[test]
fn command_drag_selects_clips_and_silence_for_copy_move_and_delete() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
    let first = app.session.project.tracks[0].clips[0].id;
    let template = app.session.project.tracks[0].clips[0].clone();
    let second = Id::new_v4();
    app.session.project.tracks[0].clips.push(Clip {
        id: second,
        name: "Later".into(),
        start_frame: frames(4.0),
        length_frames: frames(1.0),
        source_offset_frame: 0,
        ..template
    });
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let start = Pos2::new(x_at(&app, lane, frames(1.0)), lane.top() + 40.0);
    let end = Pos2::new(x_at(&app, lane, frames(4.5)), lane.top() + 40.0);
    let command = egui::Modifiers::COMMAND;
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    let cursor = modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    assert!(app.drag.is_none());
    assert!(app.portion_drag.is_some());
    assert_eq!(cursor, egui::CursorIcon::Crosshair);
    assert!(!app.dirty);
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    let portion = app.portion.clone().expect("time range");
    assert_eq!(portion.tracks, vec![track]);
    assert_eq!(portion.start_frame, frames(1.0));
    assert_eq!(portion.end_frame, frames(4.5));
    assert!(app.selected_clips.is_empty());
    assert!(!app.dirty);

    app.perform_clip_action(ClipAction::Copy, &ctx);
    let clipboard = app.clip_clipboard.as_ref().unwrap();
    assert_eq!(clipboard.anchor_frame, Some(frames(1.0)));
    assert_eq!(clipboard.clips.len(), 2);
    assert_eq!(clipboard.clips[0].clip.length_frames, frames(1.0));
    assert_eq!(clipboard.clips[0].clip.source_offset_frame, frames(1.0));
    assert_eq!(clipboard.clips[1].clip.start_frame, frames(4.0));
    assert_eq!(clipboard.clips[1].clip.length_frames, frames(0.5));
    assert!(
        app.session.project.tracks[0]
            .clips
            .iter()
            .any(|clip| clip.id == first)
    );

    let from = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
    let to = Pos2::new(x_at(&app, lane, frames(6.5)), lane.top() + 40.0);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            egui::Event::PointerMoved(from),
            button(from, true),
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            egui::Event::PointerMoved(to),
        ],
    );
    assert!(app.drag.is_none());
    assert!(app.portion_drag.is_some());
    assert_eq!(app.session.project.tracks[0].clips.len(), 2);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
            button(to, false),
        ],
    );
    assert!(app.error.is_none(), "{:?}", app.error);
    let clips = &app.session.project.tracks[0].clips;
    assert!(clips.iter().any(|clip| {
        clip.id == first && clip.start_frame == 0 && clip.length_frames == frames(1.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id == second && clip.start_frame == frames(4.5) && clip.length_frames == frames(0.5)
    }));
    assert!(clips.iter().any(|clip| {
        clip.start_frame == frames(6.0)
            && clip.length_frames == frames(1.0)
            && clip.source_offset_frame == frames(1.0)
    }));
    assert_eq!(app.portion.as_ref().unwrap().start_frame, frames(6.0));

    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    let clips = &app.session.project.tracks[0].clips;
    assert_eq!(clips.len(), 2);
    assert!(clips.iter().any(|clip| {
        clip.id == first && clip.start_frame == 0 && clip.length_frames == frames(1.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id == second && clip.start_frame == frames(4.5) && clip.length_frames == frames(0.5)
    }));
    assert!(app.portion.is_none());
}

#[test]
fn option_drag_duplicates_the_range_and_editor_uses_the_same_gesture() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
    let original = app.session.project.tracks[0].clips[0].id;
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let command = egui::Modifiers::COMMAND;
    let start = Pos2::new(x_at(&app, lane, frames(0.5)), lane.top() + 40.0);
    let end = Pos2::new(x_at(&app, lane, frames(1.5)), lane.top() + 40.0);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    assert_eq!(
        app.portion
            .as_ref()
            .map(|portion| (portion.start_frame, portion.end_frame)),
        Some((frames(0.5), frames(1.5)))
    );

    let copy = egui::Modifiers {
        alt: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    let from = Pos2::new(x_at(&app, lane, frames(1.0)), lane.top() + 40.0);
    let to = Pos2::new(x_at(&app, lane, frames(6.0)), lane.top() + 40.0);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        copy,
    );
    modifiers_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], copy);
    modifiers_frame(&mut app, &ctx, vec![button(to, false)], copy);
    assert!(app.error.is_none(), "{:?}", app.error);
    let clips = &app.session.project.tracks[0].clips;
    assert!(clips.iter().any(|clip| {
        clip.id == original && clip.start_frame == 0 && clip.length_frames == frames(2.0)
    }));
    assert!(clips.iter().any(|clip| {
        clip.id != original
            && clip.start_frame == frames(5.5)
            && clip.length_frames == frames(1.0)
            && clip.source_offset_frame == frames(0.5)
    }));

    app.selected_track = Some(track);
    app.set_view_mode(ViewMode::Edit, &ctx);
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let start = Pos2::new(x_at(&app, lane, frames(0.5)), lane.center().y);
    let end = Pos2::new(x_at(&app, lane, frames(1.0)), lane.center().y);
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
        command,
    );
    modifiers_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end)],
        command,
    );
    modifiers_frame(&mut app, &ctx, vec![button(end, false)], command);
    assert_eq!(app.portion.as_ref().unwrap().tracks, vec![track]);
    assert_eq!(app.portion.as_ref().unwrap().start_frame, frames(0.5));
    assert_eq!(app.portion.as_ref().unwrap().end_frame, frames(1.0));
}
