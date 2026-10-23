use super::*;

fn fixture(midi: bool) -> (DawUi, egui::Context, Id, Id, Id) {
    let (mut app, track) = if midi {
        let mut app = DawUi::default();
        let track = app.session.project.add_midi_track().unwrap();
        app.session.project.add_midi_pattern(track, 0).unwrap();
        (app, track)
    } else {
        super::fixture()
    };
    let first = &mut app.session.project.tracks[0].clips[0];
    first.start_frame = 48000;
    first.length_frames = 96000;
    let first_id = first.id;
    let second = Clip {
        id: Id::new_v4(),
        start_frame: 192000,
        length_frames: 24000,
        ..first.clone()
    };
    let second_id = second.id;
    app.session.project.tracks[0].clips.push(second);
    let ctx = context();
    app.open_clip_editor(first_id, track, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, track, first_id, second_id)
}

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn selected_clip_fits_content_width_on_open_selection_and_resize_for_audio_and_midi() {
    for midi in [false, true] {
        let (mut app, ctx, track, first, second) = fixture(midi);
        // A fractional, distant project origin must not turn scrollbar rounding
        // into a manual pan or disable fitting on subsequent window resizes.
        for clip in &mut app.session.project.tracks[0].clips {
            clip.start_frame += 123_456_789;
        }
        let before = app.session.project.clone();
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        for (clip, length) in [(first, 96000), (second, 24000)] {
            app.select_clip(clip, track, false);
            for size in [Vec2::new(1280.0, 800.0), Vec2::new(900.0, 900.0)] {
                frame_sized(&mut app, &ctx, vec![], size);
                let content = &app.midi_editor.content;
                assert!(
                    (content.zoom * length as f32 / 48000.0 - content.bounds.width()).abs() < 0.1,
                    "midi={midi}: fit {} != {}",
                    content.zoom * length as f32 / 48000.0,
                    content.bounds.width()
                );
                let start = app.session.project.tracks[0]
                    .clips
                    .iter()
                    .find(|c| c.id == clip)
                    .unwrap()
                    .start_frame;
                assert_eq!(content.scroll, start as f64 / 48000.0);
                assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
                assert!(!app.midi_editor.content_active);
            }
        }
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.job.is_none());
    }
}

#[test]
fn content_and_overview_pinch_scroll_and_zoom_controls_are_independent() {
    for midi in [false, true] {
        let (mut app, ctx, _, _, _) = fixture(midi);
        let before = app.session.project.clone();
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let initial = app.midi_editor.content.zoom;
        let point = app.midi_editor.content.bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
        );
        assert!((app.midi_editor.content.zoom - initial * 2.0).abs() < 0.1);
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        let content_scroll = app.midi_editor.content.scroll;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(-60.0, 0.0),
                modifiers: Default::default(),
            }],
        );
        assert!(app.midi_editor.content.scroll > content_scroll);
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let point = app.midi_editor.track_bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
        );
        assert_eq!(app.midi_editor.zoom, overview.0 * 2.0);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        let point = app.midi_editor.content.zoom_bounds.left_center() + Vec2::new(2.0, 0.0);
        click(&mut app, &ctx, point);
        assert_ne!(app.midi_editor.content.zoom, content.0);
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let point = app.midi_editor.zoom_bounds.right_center() - Vec2::new(2.0, 0.0);
        click(&mut app, &ctx, point);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let point = app.midi_editor.content.fit_bounds.center();
        click(&mut app, &ctx, point);
        assert!(app.midi_editor.content.fitted);
        assert_eq!(
            app.midi_editor.content.zoom,
            app.midi_editor.content.fit_zoom
        );
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
}

#[test]
fn midi_zoom_out_reveals_other_track_patterns_without_changing_overview() {
    let (mut app, ctx, track, first, second) = fixture(true);
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    app.session
        .project
        .assets
        .iter_mut()
        .find(|a| a.id == asset)
        .unwrap()
        .midi_pattern = Some(daw_core::MidiPattern {
        length_ticks: 3840,
        notes: vec![daw_core::MidiNote {
            id: Id::new_v4(),
            start_tick: 0,
            length_ticks: 960,
            pitch: 60,
            velocity: 100,
        }],
    });
    let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
    let content = &mut app.midi_editor.content;
    content.fitted = false;
    content.zoom = content.fit_zoom / 4.0;
    content.scroll = 0.0;
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.midi_editor
            .track_note_bounds
            .keys()
            .any(|(clip, _)| *clip == first)
    );
    assert!(
        app.midi_editor
            .track_note_bounds
            .keys()
            .any(|(clip, _)| *clip == second)
    );
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
    let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
    let note = app
        .midi_editor
        .track_note_bounds
        .iter()
        .find(|((clip, _), _)| *clip == second)
        .unwrap()
        .1[0]
        .center();
    click(&mut app, &ctx, note);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(second));
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        view
    );
    app.select_clip(first, track, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.midi_editor.content.fitted);
}

#[test]
fn audio_zoom_out_reveals_other_track_clips_and_selection_refits_the_content() {
    let (mut app, ctx, track, first, second) = fixture(false);
    app.session.project.tracks[0].clips[0].name = "First wave".into();
    app.session.project.tracks[0].clips[1].name = "Second wave".into();
    let content = &mut app.midi_editor.content;
    content.fitted = false;
    content.zoom = content.fit_zoom / 4.0;
    content.scroll = 0.0;
    let painted = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    for name in ["First wave", "Second wave"] {
        assert!(painted.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == name
                && text.pos.y >= app.midi_editor.content.bounds.top())));
    }
    let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
    app.select_clip(second, track, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.midi_editor.content.fitted);
    assert_eq!(app.midi_editor.content.scroll, 4.0);
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
    assert_ne!(app.selected_clip, Some(first));
}

#[test]
fn content_ruler_seeks_with_its_own_zoom_and_scroll_for_audio_and_midi() {
    for midi in [false, true] {
        let (mut app, ctx, _, clip, _) = fixture(midi);
        let region = app.session.project.transport.r#loop.clone();
        let content = &mut app.midi_editor.content;
        content.fitted = false;
        content.zoom *= 2.0;
        content.scroll = 1.25;
        frame(&mut app, &ctx, vec![]);
        let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let ruler = app.midi_editor.content.ruler;
        let point = Pos2::new(ruler.left() + view.0 * 0.25, ruler.center().y);
        click(&mut app, &ctx, point);
        assert_eq!(app.session.project.transport.playhead_frame, 72000);
        assert_eq!(app.session.project.transport.r#loop, region);
        assert_eq!(app.selected_clip, Some(clip));
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            view
        );
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert!(app.dirty); // Seeking persists project navigation, as on the overview ruler.
    }
}

#[test]
fn panning_a_fitted_clip_retains_the_new_content_view_across_frames() {
    for midi in [false, true] {
        let (mut app, ctx, _, _, _) = fixture(midi);
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let initial = app.midi_editor.content.scroll;
        let point = app.midi_editor.content.bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-60.0, 0.0),
                    modifiers: Default::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
        );
        assert!(app.midi_editor.content.scroll > initial);
        assert!(!app.midi_editor.content.fitted);
        frame(&mut app, &ctx, vec![]);
        assert!(app.midi_editor.content.scroll > initial);
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert!(!app.dirty);
    }
}
