use super::*;

fn fixture() -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let clip = app.session.project.add_midi_pattern(track, 48_000).unwrap();
    app.session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: daw_core::MidiPattern {
                length_ticks: 3840,
                notes: vec![daw_core::MidiNote {
                    id: Id::new_v4(),
                    start_tick: 960,
                    length_ticks: 960,
                    pitch: 60,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    app.session.project.tracks[0].clips[0].length_frames = 96_000;
    let ctx = context();
    app.open_clip_editor(clip, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom = 150.0;
    app.midi_editor.content.scroll = 0.5;
    app.midi_editor.zoom = 45.0;
    app.midi_editor.scroll = 0.25;
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    (app, ctx, clip)
}

fn gesture_modifiers(stretch: bool, shift: bool) -> egui::Modifiers {
    egui::Modifiers {
        shift,
        alt: stretch && cfg!(target_os = "macos"),
        ctrl: stretch && !cfg!(target_os = "macos"),
        ..Default::default()
    }
}

fn pointer(point: Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos: point,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers,
    }
}

fn begin(
    app: &mut DawUi,
    ctx: &egui::Context,
    clip: Id,
    mode: ClipDragMode,
    body: bool,
    delta: f32,
) -> Pos2 {
    let header = app.midi_editor.pattern_header_bounds[&clip];
    let left = matches!(
        mode,
        ClipDragMode::TrimLeft | ClipDragMode::LoopLeft | ClipDragMode::StretchLeft
    );
    let x = if mode == ClipDragMode::Move {
        header.center().x
    } else if left {
        header.left() + 2.0
    } else {
        header.right() - 2.0
    };
    let origin = Pos2::new(
        x,
        if body {
            app.midi_editor.content.bounds.top() + 60.0
        } else {
            header.center().y
        },
    );
    let end = origin + Vec2::new(delta, 0.0);
    let modifiers = gesture_modifiers(mode.stretch_edge().is_some(), true);
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(origin),
            pointer(origin, true, modifiers),
        ],
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.drag.as_ref().map(|d| d.mode), Some(mode));
    assert!(app.content_clip_drag);
    end
}

#[test]
fn lower_pattern_gestures_preview_then_commit_using_content_coordinates() {
    for (mode, body, delta, start, length) in [
        (ClipDragMode::Move, false, 75.0, 72_000, 96_000),
        (ClipDragMode::TrimLeft, true, 75.0, 72_000, 72_000),
        (ClipDragMode::TrimRight, true, -75.0, 48_000, 72_000),
        (ClipDragMode::LoopLeft, false, -75.0, 24_000, 120_000),
        (ClipDragMode::LoopRight, false, 75.0, 48_000, 120_000),
        (ClipDragMode::StretchLeft, false, -75.0, 24_000, 120_000),
        (ClipDragMode::StretchRight, false, 75.0, 48_000, 120_000),
        (ClipDragMode::StretchLeft, true, -75.0, 24_000, 120_000),
        (ClipDragMode::StretchRight, true, 75.0, 48_000, 120_000),
    ] {
        let (mut app, ctx, id) = fixture();
        let before = app.session.project.clone();
        let view = (
            app.midi_editor.content.zoom,
            app.midi_editor.content.scroll,
            app.midi_editor.zoom,
            app.midi_editor.scroll,
        );
        let end = begin(&mut app, &ctx, id, mode, body, delta);
        assert_eq!(
            app.session.project, before,
            "preview changed project: {mode:?}"
        );
        assert!(!app.dirty);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let overview = app.midi_editor.track_bounds;
        let overview_left = overview.left() + (start as f32 / 48000.0 - 0.25) * 45.0;
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
            if (rect.rect.left() - overview_left).abs() < 0.2
            && (rect.rect.width() - length as f32 / 48000.0 * 45.0).abs() < 0.2
            && (rect.rect.top() - overview.top()).abs() < 0.2
            && rect.stroke.width == 2.0)),
            "overview preview {mode:?}"
        );
        let preview = app.midi_editor.pattern_header_bounds[&id];
        let expected_left =
            app.midi_editor.content.bounds.left() + (start as f32 / 48000.0 - 0.5) * 150.0;
        assert!(
            (preview.left() - expected_left).abs() < 0.2,
            "preview {mode:?}: {preview:?}"
        );
        assert!((preview.width() - length as f32 / 48000.0 * 150.0).abs() < 0.2);
        frame(
            &mut app,
            &ctx,
            vec![pointer(
                end,
                false,
                gesture_modifiers(mode.stretch_edge().is_some(), true),
            )],
        );
        wait_for_save(&mut app);
        frame(&mut app, &ctx, vec![]);
        assert!(app.error.is_none(), "{mode:?}: {:?}", app.error);
        let clip = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(
            (clip.start_frame, clip.length_frames),
            (start, length),
            "{mode:?}"
        );
        if matches!(mode, ClipDragMode::LoopLeft | ClipDragMode::LoopRight) {
            assert_eq!(clip.repeat.unwrap().length_frames, 96_000);
        }
        if mode.stretch_edge().is_some() {
            assert!((clip.stretch.unwrap().ratio() - 1.25).abs() < 0.0001);
        }
        assert_eq!(
            (
                app.midi_editor.content.zoom,
                app.midi_editor.content.scroll,
                app.midi_editor.zoom,
                app.midi_editor.scroll
            ),
            view,
            "{mode:?}"
        );
        assert!(app.drag.is_none());
        assert!(!app.content_clip_drag);
        assert!(app.dirty);
    }
}

#[test]
fn lower_pattern_gestures_cancel_without_edits_or_view_changes() {
    for (mode, body) in [
        (ClipDragMode::Move, false),
        (ClipDragMode::TrimRight, true),
        (ClipDragMode::LoopRight, false),
        (ClipDragMode::StretchRight, true),
    ] {
        let (mut app, ctx, id) = fixture();
        let before = app.session.project.clone();
        let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let end = begin(&mut app, &ctx, id, mode, body, 75.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        frame(
            &mut app,
            &ctx,
            vec![pointer(end, false, Default::default())],
        );
        assert_eq!(app.session.project, before);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            view
        );
        assert!(app.drag.is_none());
        assert!(!app.content_clip_drag);
        assert!(app.job.is_none());
        assert!(!app.dirty);
    }
}

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(point),
            pointer(point, true, Default::default()),
        ],
    );
    frame(app, ctx, vec![pointer(point, false, Default::default())]);
}

fn menu(app: &mut DawUi, ctx: &egui::Context, point: Pos2) -> Vec<egui::epaint::ClippedShape> {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
}

fn menu_item(shapes: &[egui::epaint::ClippedShape], label: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing menu item {label}"))
}

#[test]
fn lower_pattern_context_menus_keep_views_and_include_midi_note_controls() {
    for body in [false, true] {
        let (mut app, ctx, id) = fixture();
        if body {
            let mut pattern = app
                .session
                .project
                .assets
                .iter()
                .find(|a| a.id == app.session.project.tracks[0].clips[0].asset_id)
                .unwrap()
                .midi_pattern
                .clone()
                .unwrap();
            pattern.notes[0].start_tick = 0;
            pattern.notes[0].length_ticks = 3840;
            app.session
                .project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: id,
                    pattern,
                })
                .unwrap();
            frame(&mut app, &ctx, vec![]);
        }
        let pattern = app
            .session
            .project
            .assets
            .iter()
            .find(|a| a.id == app.session.project.tracks[0].clips[0].asset_id)
            .unwrap()
            .midi_pattern
            .as_ref()
            .unwrap();
        let point = app.midi_editor.note_bounds[&pattern.notes[0].id].center();
        let note_y = point.y;
        click(&mut app, &ctx, point);
        let header = app.midi_editor.pattern_header_bounds[&id];
        let point = if body {
            // Notes own their hit areas even at a clip edge. Open the clip
            // menu on the empty row beside the selected note instead.
            Pos2::new(header.right() - 2.0, note_y - 18.0)
        } else {
            header.center()
        };
        let before = app.session.project.clone();
        let view = (
            app.midi_editor.content.zoom,
            app.midi_editor.content.scroll,
            app.midi_editor.zoom,
            app.midi_editor.scroll,
        );
        let shapes = menu(&mut app, &ctx, point);
        assert!(
            egui::Popup::is_any_open(&ctx),
            "body={body}, drag={}, note-drag={}",
            app.drag.is_some(),
            app.midi_editor.dragging()
        );
        for label in [
            "Split at playhead",
            "Stem-separation",
            "Edit clip",
            "Disable clip",
            "Delete clip",
            "Velocity",
            "Delete note",
        ] {
            menu_item(&shapes, label);
        }
        assert_eq!(app.session.project, before);
        assert_eq!(app.selected_clip, Some(id));
        assert!(!app.dirty);
        assert_eq!(
            (
                app.midi_editor.content.zoom,
                app.midi_editor.content.scroll,
                app.midi_editor.zoom,
                app.midi_editor.scroll
            ),
            view
        );
        click(&mut app, &ctx, menu_item(&shapes, "Disable clip"));
        frame(&mut app, &ctx, vec![]);
        assert!(app.session.project.tracks[0].clips[0].muted);
        assert_eq!(
            (
                app.midi_editor.content.zoom,
                app.midi_editor.content.scroll,
                app.midi_editor.zoom,
                app.midi_editor.scroll
            ),
            view
        );
        assert!(app.drag.is_none());
        assert!(!app.content_clip_drag);
    }
}

#[test]
fn lower_pattern_group_moves_share_snapping_and_shift_bypass() {
    for shift in [false, true] {
        let (mut app, ctx, id) = fixture();
        let original = app.session.project.tracks[0].clips[0].clone();
        let second = Clip {
            id: Id::new_v4(),
            start_frame: 192_000,
            ..original
        };
        let track = app.session.project.tracks[0].id;
        app.session
            .project
            .edit(Edit::InsertClip {
                track_id: track,
                clip: second.clone(),
            })
            .unwrap();
        app.selected_clips.insert(second.id);
        frame(&mut app, &ctx, vec![]);
        let origin = app.midi_editor.pattern_header_bounds[&id].center();
        let end = origin + Vec2::new(70.0, 0.0);
        let modifiers = gesture_modifiers(false, shift);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerMoved(origin),
                pointer(origin, true, modifiers),
            ],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.drag.as_ref().unwrap().clips.len(), 2);
        frame(&mut app, &ctx, vec![pointer(end, false, modifiers)]);
        frame(&mut app, &ctx, vec![]);
        let expected_delta = if shift { 22_400 } else { 24_000 };
        assert_eq!(
            app.session.project.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .start_frame,
            48_000 + expected_delta
        );
        assert_eq!(
            app.session.project.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == second.id)
                .unwrap()
                .start_frame,
            192_000 + expected_delta
        );
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            (150.0, 0.5)
        );
    }
}
