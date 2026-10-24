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
fn editor_and_timeline_scrollbars_use_standard_theme_handles_without_clipping() {
    for midi in [false, true] {
        for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            let (mut app, ctx, _, _, _) = fixture(midi);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            ctx.all_styles_mut(|style| {
                style.animation_time = 0.0;
                // A theme override must reach every lane instead of a local preset.
                if theme == settings::ColorTheme::Dark {
                    style.spacing.scroll.bar_width = 8.0;
                }
            });
            frame(&mut app, &ctx, vec![]);
            let panel = app.midi_editor.panel_bounds;
            let overview = Rect::from_min_max(
                Pos2::new(
                    panel.right() + track_overview::LABEL_WIDTH,
                    app.midi_editor.track_bounds.bottom() - SCROLLBAR_HEIGHT,
                ),
                Pos2::new(1280.0, app.midi_editor.track_bounds.bottom()),
            );
            let content = Rect::from_min_max(
                Pos2::new(overview.left(), panel.bottom() - SCROLLBAR_HEIGHT),
                Pos2::new(1280.0, panel.bottom()),
            );
            for view in [ViewMode::Edit, ViewMode::Timeline] {
                app.view_mode = view;
                frame(&mut app, &ctx, vec![]);
                let bands = if view == ViewMode::Edit {
                    assert_eq!(app.midi_editor.content.bounds.bottom(), panel.bottom());
                    assert_eq!(
                        app.midi_editor.content.ruler.top() - toolbars::HEIGHT,
                        overview.bottom()
                    );
                    vec![overview, content]
                } else {
                    assert_eq!(app.timeline_bounds.bottom(), app.scrollbar_bounds.bottom());
                    vec![app.scrollbar_bounds]
                };
                for band in bands {
                    let pointer = Pos2::new(band.left() + 40.0, band.bottom() - 5.0);
                    for _ in 0..3 {
                        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
                    }
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    let thickness = ctx.global_style().spacing.scroll.bar_width;
                    assert!(shapes.iter().any(|s| matches!(&s.shape,
                        egui::Shape::Rect(r)
                        if r.rect.width() > 30.0
                            && (r.rect.height() - thickness).abs() < 0.01
                            && (r.rect.intersect(s.clip_rect).intersect(band).height() - thickness).abs() < 0.01)),
                        "standard hovered scrollbar handle must remain fully visible in {band:?} (MIDI: {midi}, view: {view:?})");
                }
            }
        }
    }
}

#[test]
fn overview_click_scrolls_without_zooming_and_double_click_fits_audio_and_midi() {
    for midi in [false, true] {
        for theme in settings::ColorTheme::ALL {
            let (mut app, ctx, _, first, second) = fixture(midi);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            app.midi_editor.zoom = 100.0;
            app.midi_editor.scroll = 0.0;
            let project = app.session.project.clone();
            let playhead = app.session.project.transport.playhead_frame;
            for (clip, start, length) in [(first, 1.0, 2.0), (second, 4.0, 0.5)] {
                app.midi_editor.content.fitted = false;
                app.midi_editor.content.zoom = 300.0;
                app.midi_editor.content.scroll = 0.0;
                // Separate each single click from preceding pointer gestures.
                for _ in 0..40 {
                    frame(&mut app, &ctx, vec![]);
                }
                let point = app.midi_editor.clip_bounds[&clip].center();
                click(&mut app, &ctx, point);
                assert_eq!(app.selected_clip, Some(clip));
                assert_eq!(app.midi_editor.content.zoom, 300.0);
                assert_eq!(app.midi_editor.content.scroll, start);
                assert!(!app.midi_editor.content.fitted);
                super::midi::double_click_unit_header(&mut app, &ctx, point);
                assert_eq!(app.selected_clip, Some(clip));
                assert!(app.midi_editor.content.fitted);
                assert_eq!(app.midi_editor.content.scroll, start);
                assert!(
                    (app.midi_editor.content.zoom * length
                        - app.midi_editor.content.bounds.width())
                    .abs()
                        < 0.1
                );
                assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), (100.0, 0.0));
            }
            assert_eq!(app.session.project, project);
            assert_eq!(app.session.project.transport.playhead_frame, playhead);
            assert!(!app.dirty);
        }
    }
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

#[test]
fn fit_all_uses_each_panes_clip_scope_without_changing_content_or_project() {
    for midi in [false, true] {
        let (mut app, ctx, track, _, _) = fixture(midi);
        let mut distant = app.session.project.tracks[0].clips[0].clone();
        distant.id = Id::new_v4();
        distant.start_frame = 480_000;
        let other = app.session.project.add_track().unwrap();
        app.session
            .project
            .tracks
            .last_mut()
            .unwrap()
            .clips
            .push(distant);
        let before = app.session.project.clone();
        let selection = app.selected_clip;
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        app.zoom = 50.0;
        app.scroll = 2.0;
        frame(&mut app, &ctx, vec![]);
        assert!(app.fit_all_bounds.right() < app.midi_editor.zoom_bounds.left());
        let point = app.fit_all_bounds.center();
        click(&mut app, &ctx, point);
        frame(&mut app, &ctx, vec![]);
        let lane = app.midi_editor.track_bounds;
        assert!((app.midi_editor.zoom * 3.5 - lane.width()).abs() < 0.1);
        assert_eq!(app.midi_editor.scroll, 1.0);
        assert_eq!((app.zoom, app.scroll), (50.0, 2.0));
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        app.view_mode = ViewMode::Timeline;
        frame(&mut app, &ctx, vec![]);
        let point = app.fit_all_bounds.center();
        click(&mut app, &ctx, point);
        frame(&mut app, &ctx, vec![]);
        assert!((app.zoom * 11.0 - app.timeline_bounds.width()).abs() < 0.1);
        assert_eq!(app.scroll, 1.0);
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert_eq!(app.selected_clip, selection);
        assert_eq!(app.selected_track, Some(track));
        assert_ne!(track, other);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
}

#[test]
fn fit_all_keeps_exact_extreme_ranges_and_uses_current_layout_width() {
    for length in [1, 480_000_000] {
        let (mut app, ctx, _, _, _) = fixture(false);
        app.view_mode = ViewMode::Timeline;
        app.session.project.tracks[0].clips.truncate(1);
        let clip = &mut app.session.project.tracks[0].clips[0];
        clip.start_frame = 123_456_789;
        clip.length_frames = length;
        frame(&mut app, &ctx, vec![]);
        // The toolbar records the request before workspace layout resolves it.
        // Change the width on that same frame to reject stale lane measurements.
        app.fit_all_requested = true;
        frame_sized(&mut app, &ctx, vec![], Vec2::new(1400.0, 850.0));
        for _ in 0..3 {
            frame_sized(&mut app, &ctx, vec![], Vec2::new(1400.0, 850.0));
            assert!(
                (app.zoom as f64 * length as f64 / 48000.0 - app.timeline_bounds.width() as f64)
                    .abs()
                    < 0.1
            );
            assert_eq!(app.scroll, 123_456_789.0 / 48000.0);
        }
        // Returning toward the usual zoom range must not jump to its boundary.
        let zoom = app.zoom;
        let factor = if length == 1 { 0.5 } else { 2.0 };
        let point = app.timeline_bounds.center();
        frame_sized(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), egui::Event::Zoom(factor)],
            Vec2::new(1400.0, 850.0),
        );
        assert!((app.zoom - zoom * factor).abs() < zoom * 0.0001);
    }
}

#[test]
fn fit_all_is_inactive_without_clips_and_while_settings_are_open() {
    let mut app = DawUi::default();
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let before = (app.zoom, app.scroll);
    let point = app.fit_all_bounds.center();
    click(&mut app, &ctx, point);
    assert_eq!((app.zoom, app.scroll), before);
    assert!(!app.fit_all_requested);

    let (mut app, ctx, _, _, _) = fixture(false);
    app.view_mode = ViewMode::Timeline;
    app.settings_open = true;
    frame(&mut app, &ctx, vec![]);
    let before = (app.zoom, app.scroll);
    let point = app.fit_all_bounds.center();
    click(&mut app, &ctx, point);
    assert_eq!((app.zoom, app.scroll), before);
    assert!(!app.fit_all_requested);
}

#[test]
fn midi_pattern_headers_stay_visible_and_select_without_refitting() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, track, first, second) = fixture(true);
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.midi_editor.content.fitted = false;
        app.midi_editor.content.zoom = 150.0;
        app.midi_editor.content.scroll = 0.5;
        let before = app.session.project.clone();
        app.dirty = false;
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let a = app.midi_editor.pattern_header_bounds[&first];
        let b = app.midi_editor.pattern_header_bounds[&second];
        assert_eq!(a.top(), app.midi_editor.content.ruler.bottom());
        assert_eq!(a.bottom(), app.midi_editor.content.bounds.top());
        assert_eq!(b.height(), 22.0);
        for (header, selected) in [(a, true), (b, false)] {
            let stroke = if selected {
                theme::selection_outline(&ctx)
            } else {
                Stroke::new(1.0, theme::palette(&ctx).divider)
            };
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if (r.rect.left() - header.left()).abs() < 0.1
                && (r.rect.right() - header.right()).abs() < 0.1
                && (r.rect.top() - header.top()).abs() < 0.1
                && (r.rect.bottom() - app.midi_editor.content.bounds.bottom()).abs() < 0.1
                && r.rect.height() > 100.0 && r.stroke == stroke))
            );
        }
        // The header/body join must not have its own outlined rectangle or horizontal edge.
        let join = app.midi_editor.content.bounds.top();
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if [theme::selection_outline(&ctx), Stroke::new(1.0, theme::palette(&ctx).divider)].contains(&r.stroke)
                && r.rect.width() > 20.0
                && ((r.rect.top() - join).abs() < 0.1 || (r.rect.bottom() - join).abs() < 0.1))));
        // Scroll the piano keys vertically; headers remain pinned below the ruler.
        let point = app.midi_editor.content.bounds.center();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -120.0),
                    modifiers: Default::default(),
                },
            ],
        );
        assert_eq!(app.midi_editor.pattern_header_bounds[&first], a);
        assert_eq!(app.midi_editor.pattern_header_bounds[&second], b);
        let scrolled = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            scrolled
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if (r.rect.top() - a.top()).abs() < 0.1
                && (r.rect.bottom() - app.midi_editor.content.bounds.bottom()).abs() < 0.1
                && r.stroke == theme::selection_outline(&ctx)))
        );
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        click(&mut app, &ctx, b.center());
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.selected_clip, Some(second));
        assert_eq!(app.selected_track, Some(track));
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        assert_eq!(app.midi_editor.pattern_header_bounds[&second], b);
        assert!(!app.midi_editor.dragging());
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
    }
}

#[test]
fn midi_pattern_header_selection_respects_settings_and_window_focus() {
    let (mut app, ctx, _, first, second) = fixture(true);
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom = 150.0;
    app.midi_editor.content.scroll = 0.0;
    frame(&mut app, &ctx, vec![]);
    let point = app.midi_editor.pattern_header_bounds[&second].center();
    for pressed in [true, false] {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: false,
                events: vec![egui::Event::PointerMoved(point), button(point, pressed)],
                ..Default::default()
            },
            |ui| app.show(ui),
        );
    }
    assert_eq!(app.selected_clip, Some(first));
    app.settings_open = true;
    frame(&mut app, &ctx, vec![]);
    let point = app.midi_editor.pattern_header_bounds[&second].center();
    click(&mut app, &ctx, point);
    assert_eq!(app.selected_clip, Some(first));
}

fn loop_fixture(midi: bool) -> (DawUi, egui::Context, Id, Id, Id) {
    let (mut app, ctx, track, first, second) = fixture(midi);
    if midi {
        // The viewport fixture extends its clip to two seconds; match the saved pattern bounds.
        let ticks = 4 * daw_core::MIDI_TICKS_PER_BEAT;
        let frames = app.session.project.midi_frame(ticks);
        for asset in &mut app.session.project.assets {
            if let Some(pattern) = &mut asset.midi_pattern {
                pattern.length_ticks = ticks;
                asset.decoded_frame_count = frames;
            }
        }
    }
    (app, ctx, track, first, second)
}

fn loop_pointer(app: &DawUi, seconds: f64, upper: bool) -> Pos2 {
    let view = &app.midi_editor.content;
    Pos2::new(
        view.ruler.left() + ((seconds - view.scroll) * f64::from(view.zoom)) as f32,
        if upper {
            view.ruler.top() + 7.0
        } else {
            view.ruler.bottom() - 7.0
        },
    )
}
fn loop_drag(app: &mut DawUi, ctx: &egui::Context, from: f64, to: f64, shift: bool) {
    let start = loop_pointer(app, from, true);
    let end = loop_pointer(app, to, true);
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            },
        ],
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        }],
    );
}

#[test]
fn lower_ruler_moves_resizes_creates_and_toggles_the_shared_loop_in_both_editors() {
    for midi in [false, true] {
        for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            for (zoom, scroll) in [(150.0, 0.5), (180.0, 1.0)] {
                let (mut app, ctx, _, selected, _) = loop_fixture(midi);
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme,
                        ..Default::default()
                    },
                );
                app.midi_editor.content.fitted = false;
                app.midi_editor.content.zoom = zoom;
                app.midi_editor.content.scroll = scroll;
                app.midi_editor.zoom = 70.0;
                app.midi_editor.scroll = 0.25;
                app.session.project.transport.playhead_frame = 180000;
                app.session.project.transport.r#loop = daw_core::Loop {
                    start_frame: 72000,
                    end_frame: 144000,
                    enabled: true,
                };
                app.history = None;
                app.dirty = false;
                frame(&mut app, &ctx, vec![]);
                let original = app.session.project.transport.r#loop.clone();
                let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
                let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
                loop_drag(&mut app, &ctx, 2.0, 2.5, false);
                assert_eq!(
                    (
                        app.session.project.transport.r#loop.start_frame,
                        app.session.project.transport.r#loop.end_frame
                    ),
                    (96000, 168000)
                );
                assert!(app.dirty);
                app.perform_history_action(HistoryAction::Undo, &ctx);
                assert_eq!(app.session.project.transport.r#loop, original);
                app.perform_history_action(HistoryAction::Redo, &ctx);
                loop_drag(&mut app, &ctx, 2.0, 2.5, false);
                assert_eq!(
                    (
                        app.session.project.transport.r#loop.start_frame,
                        app.session.project.transport.r#loop.end_frame
                    ),
                    (120000, 168000)
                );
                loop_drag(&mut app, &ctx, 3.5, 4.0, false);
                assert_eq!(
                    (
                        app.session.project.transport.r#loop.start_frame,
                        app.session.project.transport.r#loop.end_frame
                    ),
                    (120000, 192000)
                );
                let point = loop_pointer(&app, 3.0, true);
                click(&mut app, &ctx, point);
                assert!(!app.session.project.transport.r#loop.enabled);
                click(&mut app, &ctx, point);
                assert!(app.session.project.transport.r#loop.enabled);
                loop_drag(&mut app, &ctx, 4.5, 5.0, false);
                assert_eq!(
                    (
                        app.session.project.transport.r#loop.start_frame,
                        app.session.project.transport.r#loop.end_frame
                    ),
                    (216000, 240000)
                );
                assert_eq!(app.session.project.transport.playhead_frame, 180000);
                assert_eq!(app.selected_clip, Some(selected));
                assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
                assert_eq!(
                    (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                    content
                );
                let restored: daw_core::Project =
                    serde_json::from_value(serde_json::to_value(&app.session.project).unwrap())
                        .unwrap();
                assert_eq!(
                    restored.transport.r#loop,
                    app.session.project.transport.r#loop
                );
                app.session.project.validate().unwrap();
            }
        }
    }
}

#[test]
fn lower_ruler_loop_shift_bypasses_snapping_and_tick_seeking_does_not_edit_the_loop() {
    for midi in [false, true] {
        let (mut app, ctx, _, _, _) = loop_fixture(midi);
        app.midi_editor.content.fitted = false;
        app.midi_editor.content.zoom = 200.0;
        app.midi_editor.content.scroll = 0.5;
        app.session.project.transport.r#loop = daw_core::Loop {
            start_frame: 48000,
            end_frame: 96000,
            enabled: true,
        };
        frame(&mut app, &ctx, vec![]);
        loop_drag(&mut app, &ctx, 1.5, 1.625, true);
        assert_eq!(
            (
                app.session.project.transport.r#loop.start_frame,
                app.session.project.transport.r#loop.end_frame
            ),
            (54000, 102000)
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::ModifiersChanged(Default::default())],
        );
        let region = app.session.project.transport.r#loop.clone();
        let point = loop_pointer(&app, 2.5, false);
        click(&mut app, &ctx, point);
        assert_eq!(app.session.project.transport.playhead_frame, 120000);
        assert_eq!(app.session.project.transport.r#loop, region);
    }
}

#[test]
fn lower_ruler_loop_cancel_and_guards_preserve_the_project_and_viewports() {
    for midi in [false, true] {
        for guard in ["escape", "focus", "settings", "disabled"] {
            let (mut app, ctx, _, _, _) = loop_fixture(midi);
            app.midi_editor.content.fitted = false;
            app.midi_editor.content.zoom = 200.0;
            app.midi_editor.content.scroll = 0.5;
            app.session.project.transport.r#loop = daw_core::Loop {
                start_frame: 48000,
                end_frame: 96000,
                enabled: true,
            };
            app.history = None;
            app.dirty = false;
            frame(&mut app, &ctx, vec![]);
            let before = app.session.project.clone();
            let view = (
                app.midi_editor.content.zoom,
                app.midi_editor.content.scroll,
                app.midi_editor.zoom,
                app.midi_editor.scroll,
            );
            let start = loop_pointer(&app, 1.5, true);
            let end = loop_pointer(&app, 2.0, true);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert_ne!(
                app.session.project.transport.r#loop,
                before.transport.r#loop
            );
            assert!(!app.dirty);
            let mut focused = true;
            let mut enabled = true;
            let mut events = vec![];
            match guard {
                "escape" => events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }),
                "focus" => {
                    focused = false;
                    events.push(egui::Event::WindowFocused(false));
                }
                "settings" => app.settings_open = true,
                "disabled" => enabled = false,
                _ => unreachable!(),
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(enabled, |ui| app.show(ui));
                },
            );
            output.textures_delta.clear();
            assert_eq!(app.session.project, before, "{midi} {guard}");
            assert!(app.loop_drag.is_none());
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
        }
    }
}
