use super::*;

fn audio() -> (DawUi, egui::Context, Id, [Id; 3]) {
    let (mut app, ctx, track, _) = super::audio_editor::audio_fixture();
    let mut first = app.session.project.tracks[0].clips[0].clone();
    first.start_frame = 1000;
    first.length_frames = 1000;
    first.source_offset_frame = 0;
    let mut second = first.clone();
    second.id = Id::new_v4();
    second.start_frame = 4000;
    let mut third = first.clone();
    third.id = Id::new_v4();
    third.start_frame = 8000;
    let ids = [first.id, second.id, third.id];
    app.session.project.tracks[0].clips = vec![first, second, third];
    app.select_clip(ids[2], track, false);
    app.midi_editor.zoom = 1000.0;
    app.midi_editor.scroll = 0.0;
    app.midi_editor.content.zoom = 1000.0;
    app.midi_editor.content.scroll = 0.0;
    app.retain_content_view();
    app.history = None;
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    (app, ctx, track, ids)
}
fn bounds(app: &DawUi, overview: bool) -> Rect {
    if overview {
        app.midi_editor.track_bounds
    } else {
        app.midi_editor.content.bounds
    }
}
fn rectangle(app: &DawUi, overview: bool) -> (Pos2, Pos2) {
    let lane = bounds(app, overview);
    let (zoom, scroll) = if overview {
        (app.midi_editor.zoom, app.midi_editor.scroll)
    } else {
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll)
    };
    let x = |frame| {
        lane.left() + ((app.session.project.seconds(frame) - scroll) * f64::from(zoom)) as f32
    };
    (
        Pos2::new(x(6000), lane.center().y),
        Pos2::new(x(500), lane.top() + 4.0),
    )
}
fn start(app: &mut DawUi, ctx: &egui::Context, origin: Pos2, end: Pos2, shift: bool) {
    shift_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
        shift,
    );
    shift_frame(app, ctx, vec![egui::Event::PointerMoved(end)], shift);
}
#[test]
fn editor_marquee_selects_and_toggles_audio_groups_in_both_panes() {
    for overview in [false, true] {
        for shift in [false, true] {
            let (mut app, ctx, track, ids) = audio();
            if shift {
                app.select_clip(ids[1], track, true);
                app.retain_content_view();
                frame(&mut app, &ctx, vec![]);
            }
            let before = app.session.project.clone();
            let clock = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
            let (origin, end) = rectangle(&app, overview);
            start(&mut app, &ctx, origin, end, shift);
            assert!(app.marquee.is_some(), "overview={overview}");
            let expected = if shift {
                HashSet::from([ids[0], ids[2]])
            } else {
                HashSet::from([ids[0], ids[1]])
            };
            assert_eq!(
                app.selected_clips, expected,
                "overview={overview}, shift={shift}"
            );
            shift_frame(&mut app, &ctx, vec![button(end, false)], shift);
            frame(&mut app, &ctx, vec![]);
            assert!(app.marquee.is_none());
            assert_eq!(
                app.selected_clips, expected,
                "overview={overview}, shift={shift}"
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            assert_eq!(
                (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                clock
            );
            assert!(app.midi_editor.delete_clips);
        }
    }
}
#[test]
fn editor_marquee_cancel_restores_the_prior_selection() {
    for overview in [false, true] {
        for focus_loss in [false, true] {
            let (mut app, ctx, _, ids) = audio();
            let before = app.session.project.clone();
            let (origin, end) = rectangle(&app, overview);
            start(&mut app, &ctx, origin, end, false);
            assert!(app.marquee.is_some(), "overview={overview}");
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused: !focus_loss,
                    events: if focus_loss {
                        vec![]
                    } else {
                        vec![egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Default::default(),
                        }]
                    },
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
            assert!(app.marquee.is_none());
            assert_eq!(app.selected_clip, Some(ids[2]));
            assert_eq!(app.selected_clips, HashSet::from([ids[2]]));
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
        }
    }
}
#[test]
fn editor_marquee_in_midi_header_strip_selects_clips_without_notes() {
    let (mut app, track, first, _) = super::midi::fixture();
    let ctx = context();
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.start_frame = 48_000;
    clip.length_frames = 24_000;
    let mut second = clip.clone();
    second.id = Id::new_v4();
    second.start_frame = 96_000;
    let mut third = clip.clone();
    third.id = Id::new_v4();
    third.start_frame = 144_000;
    app.session.project.tracks[0]
        .clips
        .extend([second.clone(), third]);
    app.open_clip_editor(first, track, &ctx);
    app.midi_editor.content.zoom = 100.0;
    app.midi_editor.content.scroll = 0.0;
    app.retain_content_view();
    frame(&mut app, &ctx, vec![]);
    let header = app.midi_editor.pattern_header_bounds[&first];
    let left = header.left() - 100.0;
    let origin = Pos2::new(left + 130_000.0 / 48000.0 * 100.0, header.bottom() - 2.0);
    let end = Pos2::new(left + 40_000.0 / 48000.0 * 100.0, header.top() + 2.0);
    start(&mut app, &ctx, origin, end, false);
    assert!(app.marquee.is_some());
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.selected_clips, HashSet::from([first, second.id]));
    assert!(app.midi_editor.selected_notes().is_empty());
}

#[test]
fn editor_overview_command_drag_keeps_time_range_selection() {
    let (mut app, ctx, _, _) = audio();
    let before = app.session.project.clone();
    let (origin, end) = rectangle(&app, true);
    let modifiers = egui::Modifiers {
        command: true,
        mac_cmd: cfg!(target_os = "macos"),
        ctrl: !cfg!(target_os = "macos"),
        shift: true,
        ..Default::default()
    };
    let input = |app: &mut DawUi, mut events: Vec<egui::Event>| {
        for event in &mut events {
            if let egui::Event::PointerButton {
                modifiers: value, ..
            } = event
            {
                *value = modifiers;
            }
        }
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        frame(app, &ctx, events);
    };
    input(
        &mut app,
        vec![egui::Event::PointerMoved(origin), button(origin, true)],
    );
    input(&mut app, vec![egui::Event::PointerMoved(end)]);
    assert!(app.marquee.is_none());
    assert!(app.portion_drag.is_some());
    input(&mut app, vec![button(end, false)]);
    assert!(app.portion.is_some());
    assert!(app.marquee.is_none());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}
