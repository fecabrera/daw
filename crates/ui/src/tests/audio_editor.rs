use super::*;

fn audio_fixture() -> (DawUi, egui::Context, Id, Id) {
    let (mut app, track) = fixture();
    let asset = &mut app.session.project.assets[0];
    asset.decoded_frame_count = 4800;
    let samples = (0..4800)
        .map(|frame| {
            let phase = frame as f32 * 0.09;
            [phase.sin() * 0.7, (phase * 0.73).cos() * 0.3]
        })
        .collect();
    app.session.audio.insert(
        asset.id,
        daw_media::AudioData {
            samples: std::sync::Arc::new(samples),
            peaks: Default::default(),
            metadata: asset.source_metadata.clone(),
            sample_rate_hz: 48000,
        },
    );
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.start_frame = 4800;
    clip.length_frames = 2400;
    clip.source_offset_frame = 2400;
    let id = clip.id;
    let ctx = context();
    app.open_clip_editor(id, track, &ctx);
    app.midi_editor.zoom = 1500.0;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, track, id)
}

fn shapes(app: &mut DawUi, ctx: &egui::Context) -> Vec<egui::epaint::ClippedShape> {
    frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
}

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

fn waveform_body(app: &DawUi, shapes: &[egui::epaint::ClippedShape]) -> Rect {
    let color = theme::clip_colors(app.session.project.tracks[0].color, None).body;
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill == color
                    && rect.rect.top() >= app.midi_editor.track_bounds.bottom() =>
            {
                Some(Rect::from_min_max(
                    rect.rect.min + Vec2::new(0.0, 22.0),
                    rect.rect.max,
                ))
            }
            _ => None,
        })
        .expect("enlarged waveform body")
}

fn check_peaks(app: &DawUi, shapes: &[egui::epaint::ClippedShape], body: Rect) {
    check_clip_peaks(app, shapes, body, app.selected_clip.unwrap());
}

fn check_clip_peaks(app: &DawUi, shapes: &[egui::epaint::ClippedShape], body: Rect, clip_id: Id) {
    let track = &app.session.project.tracks[0];
    let clip = track.clips.iter().find(|c| c.id == clip_id).unwrap();
    let cached = app.session.audio_for_clip(clip);
    let audio = cached.unwrap_or(&app.session.audio[&clip.asset_id]);
    let color = theme::clip_colors(track.color, clip.color).waveform;
    let frames_per_pixel = 48000.0 / f64::from(app.midi_editor.content.zoom);
    let height = body.height() / f32::from(audio.metadata.channels);
    for fraction in [0.1, 0.35, 0.7] {
        let x = (body.left() + body.width() * fraction).floor();
        let local = ((f64::from(x - body.left()) * frames_per_pixel).max(0.0)) as u64;
        for channel in 0..usize::from(audio.metadata.channels) {
            let [min, max] = waveforms::preview_extrema(
                audio,
                clip,
                local,
                frames_per_pixel.ceil() as u64 + 1,
                channel,
                if cached.is_none() { clip.stretch } else { None },
            );
            let center = body.top() + height * (channel as f32 + 0.5);
            let expected = [
                Pos2::new(x, center - max * height * 0.42),
                Pos2::new(x, center - min * height * 0.42),
            ];
            assert!(
                shapes.iter().any(|shape| match shape.shape {
                    egui::Shape::LineSegment { points, stroke } =>
                        stroke.color == color
                            && points[0].distance(expected[0]) < 0.01
                            && points[1].distance(expected[1]) < 0.01,
                    _ => false,
                }),
                "missing channel {channel} peak at {x}"
            );
        }
    }
}

#[test]
fn audio_editor_waveform_follows_primary_selection_zoom_scroll_and_playhead() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, track, _first) = audio_fixture();
        if color_theme == settings::ColorTheme::Dark {
            let asset = &mut app.session.project.assets[0];
            asset.source_metadata.channels = 1;
            app.session
                .audio
                .get_mut(&asset.id)
                .unwrap()
                .metadata
                .channels = 1;
        }
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        let mut second = app.session.project.tracks[0].clips[0].clone();
        second.id = Id::new_v4();
        second.start_frame += second.length_frames;
        second.source_offset_frame = 0;
        let second_id = second.id;
        app.session.project.tracks[0].clips.push(second);
        app.selected_clips.insert(second_id);
        let before = app.session.project.clone();
        let painted = shapes(&mut app, &ctx);
        let body = waveform_body(&app, &painted);
        assert_eq!(body.x_range(), app.midi_editor.content.bounds.x_range());
        assert!(body.height() > app.midi_editor.track_bounds.height() * 4.0);
        check_peaks(&app, &painted, body);
        let scroll = app.midi_editor.scroll;
        app.select_clip(second_id, track, false);
        let painted = shapes(&mut app, &ctx);
        let body = waveform_body(&app, &painted);
        assert_eq!(app.midi_editor.scroll, scroll);
        assert_eq!(body.x_range(), app.midi_editor.content.bounds.x_range());
        check_peaks(&app, &painted, body);
        app.midi_editor.content.fitted = false;
        app.midi_editor.content.zoom *= 2.0;
        app.midi_editor.content.scroll += 0.025;
        app.session.project.transport.playhead_frame = 8640;
        let painted = shapes(&mut app, &ctx);
        let zoomed = waveform_body(&app, &painted);
        assert!((zoomed.width() - body.width() * 2.0).abs() < 0.1);
        assert_eq!(app.midi_editor.scroll, scroll);
        let playhead_x = app.midi_editor.content.bounds.left()
            + (0.18 - app.midi_editor.content.scroll) as f32 * app.midi_editor.content.zoom;
        assert!(painted.iter().any(|shape| matches!(shape.shape,
            egui::Shape::LineSegment { points, stroke }
                if stroke.color == theme::accent(&ctx)
                && (points[0].x - playhead_x).abs() < 0.01
                && points[0].y == app.midi_editor.content.bounds.top()
                && points[1].y == zoomed.bottom())));
        app.session.project.transport = before.transport.clone();
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.job.is_none());
    }
}

#[test]
fn audio_editor_waveform_uses_trim_loop_phase_and_prepared_pitch_and_stretch() {
    let (mut app, ctx, _, _) = audio_fixture();
    app.session.project.stretch_engine = daw_core::StretchEngine::INTEGRATED;
    let clip = &mut app.session.project.tracks[0].clips[0];
    clip.source_offset_frame = 480;
    clip.repeat = Some(daw_core::ClipLoop {
        length_frames: 960,
        phase_frame: 720,
    });
    clip.stretch = Some(daw_core::ClipStretch::new(2, 1).unwrap());
    clip.transpose = 7;
    let painted = shapes(&mut app, &ctx);
    check_peaks(&app, &painted, waveform_body(&app, &painted));
    let clip = app.session.project.tracks[0].clips[0].clone();
    let key = daw_media::ClipAudioKey::for_clip(&clip).unwrap();
    let prepared = key
        .prepare(
            &app.session.audio[&clip.asset_id],
            app.session.project.stretch_engine,
            &|| false,
        )
        .unwrap();
    app.session.stretched_audio.insert(key, prepared);
    assert!(app.session.audio_for_clip(&clip).is_some());
    let before = app.session.project.clone();
    let painted = shapes(&mut app, &ctx);
    check_peaks(&app, &painted, waveform_body(&app, &painted));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    assert!(app.job.is_none());
}

#[test]
fn audio_editor_keeps_missing_source_clips_visible_without_selection() {
    let (mut app, ctx, _, _) = audio_fixture();
    app.session.audio.clear();
    let painted = shapes(&mut app, &ctx);
    assert!(painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Missing source"
            && text.pos.y > app.midi_editor.track_bounds.bottom())));
    let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
    app.selected_clip = None;
    app.selected_clips.clear();
    let painted = shapes(&mut app, &ctx);
    assert!(painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Missing source"
            && text.pos.y > app.midi_editor.track_bounds.bottom())));
    assert!(!painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Select a clip"
            && text.pos.x > app.midi_editor.panel_bounds.right())));
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        view
    );
    assert_eq!(app.selected_clip, None);
    assert!(!app.dirty);
}

fn selectable_waveforms() -> (DawUi, egui::Context, Id, Id) {
    let (mut app, ctx, track, first) = audio_fixture();
    let original = app.session.project.tracks[0].clips[0].clone();
    let second = Clip {
        id: Id::new_v4(),
        name: "Other waveform".into(),
        start_frame: original.end(),
        source_offset_frame: 0,
        length_frames: 2_400_000,
        repeat: Some(daw_core::ClipLoop {
            length_frames: 4800,
            phase_frame: 0,
        }),
        ..original
    };
    let second_id = second.id;
    app.session.project.tracks[0].clips.push(second);
    // Pan to the long neighbor without changing the short clip's close-up zoom.
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.scroll = 0.15;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(first));
    assert_eq!(app.selected_track, Some(track));
    (app, ctx, first, second_id)
}

#[test]
fn clicking_editor_waveforms_selects_the_clip_without_changing_either_viewport() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _, second) = selectable_waveforms();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let point = app.midi_editor.content.bounds.center();
        click(&mut app, &ctx, point);
        let painted = shapes(&mut app, &ctx);
        assert_eq!(app.selected_clip, Some(second));
        assert_eq!(app.selected_clips, HashSet::from([second]));
        assert_eq!(app.midi_editor.content.clip, Some(second));
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert!(painted.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Other waveform"
                && text.pos.x < app.midi_editor.panel_bounds.right())));
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.job.is_none());
        assert!(app.drag.is_none());
    }
}

#[test]
fn editor_waveform_selection_ignores_unfocused_clicks_and_pointer_drags() {
    let (mut app, ctx, first, second) = selectable_waveforms();
    let point = app.midi_editor.content.bounds.center();
    let before = app.session.project.clone();
    let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
    for down in [true, false] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: false,
                events: vec![
                    egui::Event::WindowFocused(false),
                    egui::Event::PointerMoved(point),
                    button(point, down),
                ],
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
    }
    assert_eq!(app.selected_clip, Some(first));
    frame(&mut app, &ctx, vec![egui::Event::WindowFocused(true)]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    let end = point + Vec2::new(30.0, 0.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(app.selected_clip, Some(first));
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        view
    );
    assert_eq!(app.session.project, before);
    assert!(app.drag.is_none());
    assert!(app.job.is_none());
    assert!(!app.dirty);
    click(&mut app, &ctx, point);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(second));
}

fn waveform_context_menu(
    app: &mut DawUi,
    ctx: &egui::Context,
    point: Pos2,
) -> Vec<egui::epaint::ClippedShape> {
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
    // A context menu must still be rendered on subsequent frames, not only on open.
    shapes(app, ctx)
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
fn waveform_context_menu_matches_timeline_and_keeps_selection_and_viewports() {
    for color_theme in settings::ColorTheme::ALL {
        let (mut app, ctx, first, second) = selectable_waveforms();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.clone();
        let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
        let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
        let point = app.midi_editor.content.bounds.center();
        let painted = waveform_context_menu(&mut app, &ctx, point);
        assert!(egui::Popup::is_any_open(&ctx));
        let expected = [
            "Split at playhead",
            "Stem separation",
            "Edit clip",
            "Disable clip",
            "Delete clip",
        ];
        let labels: Vec<_> = painted
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if expected.contains(&text.galley.text()) => {
                    Some(text.galley.text())
                }
                _ => None,
            })
            .collect();
        assert_eq!(labels, expected);
        for shape in &painted {
            if let egui::Shape::Text(text) = &shape.shape
                && expected.contains(&text.galley.text())
            {
                assert_eq!(text.galley.rows.len(), 1);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size == 13.0)
                );
            }
        }
        assert!(menu_item(&painted, expected[1]).y < menu_item(&painted, expected[2]).y);
        assert_eq!(app.selected_clip, Some(first));
        assert_eq!(app.selected_clips, HashSet::from([first]));
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        let disable = menu_item(&painted, "Disable clip");
        click(&mut app, &ctx, disable);
        frame(&mut app, &ctx, vec![]);
        assert!(!egui::Popup::is_any_open(&ctx));
        assert!(!app.session.project.tracks[0].clips[0].muted);
        assert!(
            app.session.project.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == second)
                .unwrap()
                .muted
        );
        assert_eq!(app.selected_clip, Some(first));
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            content
        );
        assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
        assert!(app.dirty);
    }
}

#[test]
fn waveform_context_menu_dismisses_without_edits_and_does_not_open_in_empty_space() {
    let (mut app, ctx, first, _) = selectable_waveforms();
    let before = app.session.project.clone();
    let view = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
    let point = app.midi_editor.content.bounds.center();
    waveform_context_menu(&mut app, &ctx, point);
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
    frame(&mut app, &ctx, vec![]);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert_eq!(app.selected_clip, Some(first));
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        view
    );
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    app.midi_editor.content.scroll = 70.0;
    frame(&mut app, &ctx, vec![]);
    let point = app.midi_editor.content.bounds.center();
    waveform_context_menu(&mut app, &ctx, point);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert_eq!(app.session.project, before);
}

fn gesture_fixture() -> (DawUi, egui::Context, Id) {
    let (mut app, ctx, _, clip) = audio_fixture();
    app.midi_editor.content.fitted = false;
    app.midi_editor.content.zoom = 1500.0;
    app.midi_editor.content.scroll = 0.08;
    app.midi_editor.zoom = 600.0;
    app.midi_editor.scroll = 0.04;
    frame(&mut app, &ctx, vec![]);
    (app, ctx, clip)
}

fn content_block(app: &DawUi, clip: Id) -> Rect {
    let clip = app.session.project.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == clip)
        .unwrap();
    let lane = app.midi_editor.content.bounds;
    let left = lane.left()
        + (app.session.project.seconds(clip.start_frame) - app.midi_editor.content.scroll) as f32
            * app.midi_editor.content.zoom;
    Rect::from_min_max(
        Pos2::new(left, lane.top()),
        Pos2::new(
            left + app.session.project.seconds(clip.length_frames) as f32
                * app.midi_editor.content.zoom,
            lane.bottom(),
        ),
    )
}

fn gesture_button(point: Pos2, pressed: bool, stretch: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: point,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers {
            shift: true,
            alt: stretch && cfg!(target_os = "macos"),
            ctrl: stretch && !cfg!(target_os = "macos"),
            ..Default::default()
        },
    }
}

fn begin_audio_gesture(
    app: &mut DawUi,
    ctx: &egui::Context,
    id: Id,
    mode: ClipDragMode,
    body: bool,
    delta: f32,
) -> Pos2 {
    let block = content_block(app, id);
    let left = matches!(
        mode,
        ClipDragMode::TrimLeft | ClipDragMode::LoopLeft | ClipDragMode::StretchLeft
    );
    let origin = Pos2::new(
        if mode == ClipDragMode::Move {
            block.center().x
        } else if left {
            block.left() + 1.0
        } else {
            block.right() - 1.0
        },
        block.top() + if body { 60.0 } else { 10.0 },
    );
    let modifiers = match gesture_button(origin, true, mode.stretch_edge().is_some()) {
        egui::Event::PointerButton { modifiers, .. } => modifiers,
        _ => unreachable!(),
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(origin),
            gesture_button(origin, true, mode.stretch_edge().is_some()),
        ],
    );
    let end = origin + Vec2::new(delta, 0.0);
    frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.drag.as_ref().map(|drag| drag.mode), Some(mode));
    assert!(app.content_clip_drag);
    end
}

#[test]
fn audio_content_gestures_preview_and_commit_without_refitting_or_replacing_source_pcm() {
    for (mode, body, delta, start, length) in [
        (ClipDragMode::Move, false, 15.0, 5280, 2400),
        (ClipDragMode::TrimLeft, true, 15.0, 5280, 1920),
        (ClipDragMode::TrimRight, true, -15.0, 4800, 1920),
        (ClipDragMode::LoopLeft, false, -15.0, 4320, 2880),
        (ClipDragMode::LoopRight, false, 15.0, 4800, 2880),
        (ClipDragMode::StretchLeft, false, -15.0, 4320, 2880),
        (ClipDragMode::StretchRight, false, 15.0, 4800, 2880),
        (ClipDragMode::StretchLeft, true, -15.0, 4320, 2880),
        (ClipDragMode::StretchRight, true, 15.0, 4800, 2880),
    ] {
        let (mut app, ctx, id) = gesture_fixture();
        let before = app.session.project.clone();
        let asset = before.tracks[0].clips[0].asset_id;
        let source = app.session.audio[&asset].samples.clone();
        let view = (
            app.midi_editor.content.zoom,
            app.midi_editor.content.scroll,
            app.midi_editor.zoom,
            app.midi_editor.scroll,
        );
        let end = begin_audio_gesture(&mut app, &ctx, id, mode, body, delta);
        assert_eq!(app.session.project, before, "preview {mode:?}");
        assert!(!app.dirty);
        let painted = shapes(&mut app, &ctx);
        let lane = app.midi_editor.content.bounds;
        let expected_left = lane.left() + (start as f32 / 48000.0 - 0.08) * 1500.0;
        let color = theme::clip_colors(before.tracks[0].color, None).body;
        assert!(
            painted
                .iter()
                .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.fill == color && (rect.rect.top() - lane.top()).abs() < 0.1
            && (rect.rect.left() - expected_left).abs() < 0.2
            && (rect.rect.width() - length as f32 / 48000.0 * 1500.0).abs() < 0.2)),
            "waveform preview {mode:?}"
        );
        frame(
            &mut app,
            &ctx,
            vec![gesture_button(end, false, mode.stretch_edge().is_some())],
        );
        wait_for_save(&mut app);
        frame(&mut app, &ctx, vec![]);
        assert!(app.error.is_none(), "{mode:?}: {:?}", app.error);
        let clip = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id == id)
            .unwrap();
        assert_eq!(
            (clip.start_frame, clip.length_frames),
            (start, length),
            "{mode:?}"
        );
        assert_eq!(clip.asset_id, asset);
        if matches!(mode, ClipDragMode::LoopLeft | ClipDragMode::LoopRight) {
            assert_eq!(clip.repeat.unwrap().length_frames, 2400);
        }
        if mode.stretch_edge().is_some() {
            assert!((clip.stretch.unwrap().ratio() - 1.2).abs() < 0.0001);
            assert!(app.session.audio_for_clip(clip).is_some());
        }
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &app.session.audio[&asset].samples
        ));
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
fn audio_content_shared_boundary_uses_lower_zoom_and_previews_both_clips() {
    let (mut app, ctx, first) = gesture_fixture();
    let neighbor = Clip {
        id: Id::new_v4(),
        start_frame: 7200,
        source_offset_frame: 1200,
        length_frames: 1200,
        ..app.session.project.tracks[0].clips[0].clone()
    };
    let second = neighbor.id;
    let track = app.session.project.tracks[0].id;
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: neighbor,
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let end = begin_audio_gesture(&mut app, &ctx, first, ClipDragMode::TrimRight, true, -15.0);
    assert!(app.drag.as_ref().unwrap().linked_boundary);
    assert_eq!(app.session.project, before);
    let painted = shapes(&mut app, &ctx);
    let lane = app.midi_editor.content.bounds;
    let color = theme::clip_colors(before.tracks[0].color, None).body;
    for (start, length) in [(4800, 1920), (6720, 1680)] {
        let left = lane.left() + (start as f32 / 48000.0 - 0.08) * 1500.0;
        assert!(painted.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.fill == color && (rect.rect.top() - lane.top()).abs() < 0.1
            && (rect.rect.left() - left).abs() < 0.2 && (rect.rect.width() - length as f32 / 48000.0 * 1500.0).abs() < 0.2)));
    }
    frame(&mut app, &ctx, vec![gesture_button(end, false, false)]);
    wait_for_save(&mut app);
    let clips = &app.session.project.tracks[0].clips;
    let a = clips.iter().find(|clip| clip.id == first).unwrap();
    let b = clips.iter().find(|clip| clip.id == second).unwrap();
    assert_eq!(
        (a.start_frame, a.length_frames, a.source_offset_frame),
        (4800, 1920, 2400)
    );
    assert_eq!(
        (b.start_frame, b.length_frames, b.source_offset_frame),
        (6720, 1680, 720)
    );
    assert_eq!(
        (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
        (1500.0, 0.08)
    );
    assert!(app.error.is_none());
}

#[test]
fn audio_content_gestures_cancel_cleanly_on_escape_or_focus_loss() {
    for focused in [true, false] {
        for (mode, body) in [
            (ClipDragMode::Move, false),
            (ClipDragMode::TrimRight, true),
            (ClipDragMode::LoopRight, false),
            (ClipDragMode::StretchRight, true),
        ] {
            let (mut app, ctx, id) = gesture_fixture();
            let before = app.session.project.clone();
            let end = begin_audio_gesture(&mut app, &ctx, id, mode, body, 15.0);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused,
                    events: if focused {
                        vec![egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: Default::default(),
                        }]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
            frame(
                &mut app,
                &ctx,
                vec![gesture_button(end, false, mode.stretch_edge().is_some())],
            );
            assert_eq!(app.session.project, before);
            assert_eq!(
                (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                (1500.0, 0.08)
            );
            assert!(app.drag.is_none());
            assert!(!app.content_clip_drag);
            assert!(app.job.is_none());
            assert!(!app.dirty);
        }
    }
}

#[test]
fn audio_content_boundary_removal_preserves_view_through_empty_selection() {
    let (mut app, ctx, first) = gesture_fixture();
    let neighbor = Clip {
        id: Id::new_v4(),
        start_frame: 7200,
        source_offset_frame: 2400,
        length_frames: 1200,
        ..app.session.project.tracks[0].clips[0].clone()
    };
    let second = neighbor.id;
    let track = app.session.project.tracks[0].id;
    app.session
        .project
        .edit(Edit::InsertClip {
            track_id: track,
            clip: neighbor,
        })
        .unwrap();
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let end = begin_audio_gesture(&mut app, &ctx, first, ClipDragMode::TrimRight, true, -75.0);
    assert!(app.drag.as_ref().unwrap().linked_boundary);
    let painted = shapes(&mut app, &ctx);
    let lane = app.midi_editor.content.bounds;
    let color = theme::clip_colors(before.tracks[0].color, None).body;
    assert!(
        painted
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
        if rect.fill == color && (rect.rect.top() - lane.top()).abs() < 0.1
        && (rect.rect.width() - 112.5).abs() < 0.2))
    );
    assert_eq!(app.session.project, before);
    frame(&mut app, &ctx, vec![gesture_button(end, false, false)]);
    wait_for_save(&mut app);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.selected_clip, None);
        assert_eq!(
            (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
            (1500.0, 0.08)
        );
        assert_eq!(
            (app.midi_editor.zoom, app.midi_editor.scroll),
            (600.0, 0.04)
        );
    }
    let painted = shapes(&mut app, &ctx);
    waveform_body(&app, &painted);
    assert_eq!(app.selected_clip, None);
    let clips = &app.session.project.tracks[0].clips;
    assert_eq!(clips.len(), 1);
    assert_eq!(
        (
            clips[0].id,
            clips[0].start_frame,
            clips[0].length_frames,
            clips[0].source_offset_frame
        ),
        (second, 4800, 3600, 0)
    );
    // A subsequent explicit overview selection still restores automatic fitting.
    app.select_clip(second, track, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.midi_editor.content.fitted);
    assert!(
        (app.midi_editor.content.zoom * 0.075 - app.midi_editor.content.bounds.width()).abs() < 0.1
    );
}

#[test]
fn unselected_audio_track_keeps_all_waveforms_navigation_and_selection() {
    for color_theme in settings::ColorTheme::ALL {
        for fitted in [false, true] {
            let (mut app, ctx, track, first) = audio_fixture();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            let mut neighbor = app.session.project.tracks[0].clips[0].clone();
            neighbor.id = Id::new_v4();
            neighbor.start_frame = neighbor.end();
            neighbor.source_offset_frame = 0;
            neighbor.name = "Neighbor waveform".into();
            let second = neighbor.id;
            app.session.project.tracks[0].clips.push(neighbor);
            app.midi_editor.content.fitted = false;
            app.midi_editor.content.zoom = 1500.0;
            app.midi_editor.content.scroll = 0.08;
            frame(&mut app, &ctx, vec![]);
            // Cover a fitted view as well as an existing manual view.
            app.midi_editor.content.fitted = fitted;
            let before_shapes = shapes(&mut app, &ctx);
            let before = app.session.project.clone();
            let content = (app.midi_editor.content.zoom, app.midi_editor.content.scroll);
            let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
            let body = waveform_body(&app, &before_shapes);
            check_peaks(&app, &before_shapes, body);
            app.selected_clip = None;
            app.selected_clips.clear();
            for _ in 0..3 {
                let painted = shapes(&mut app, &ctx);
                assert_eq!(waveform_body(&app, &painted), body);
                // Verify the same waveform peaks even with no primary selection.
                check_clip_peaks(&app, &painted, body, first);
                if !fitted {
                    assert!(painted.iter().any(|shape| matches!(&shape.shape,
                        egui::Shape::Text(text) if text.galley.text() == "Neighbor waveform"
                            && text.pos.y >= app.midi_editor.content.bounds.top())));
                }
                assert_eq!(
                    (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                    content
                );
                assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
                assert!(app.selected_clips.is_empty());
            }
            // Fit requires a selected clip. Zoom remains active without one.
            let fit = app.midi_editor.content.fit_bounds.center();
            click(&mut app, &ctx, fit);
            assert_eq!(
                (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                content
            );
            let zoom = app.midi_editor.content.zoom_bounds.right_center() - Vec2::new(2.0, 0.0);
            click(&mut app, &ctx, zoom);
            assert_ne!(app.midi_editor.content.zoom, content.0);
            // Restore the view to select a visible waveform with a normal click.
            app.midi_editor.content.zoom = content.0;
            app.midi_editor.content.scroll = content.1;
            frame(&mut app, &ctx, vec![]);
            let point = app.midi_editor.content.bounds.min
                + Vec2::new(((0.125 - content.1) as f32 * content.0).max(1.0), 50.0);
            click(&mut app, &ctx, point);
            assert_eq!(app.selected_clip, Some(first));
            assert_eq!(app.selected_track, Some(track));
            assert_eq!(
                (app.midi_editor.content.zoom, app.midi_editor.content.scroll),
                content
            );
            assert!(
                app.session.project.tracks[0]
                    .clips
                    .iter()
                    .any(|clip| clip.id == second)
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            assert!(app.job.is_none());
        }
    }
}

#[test]
fn audio_track_without_initial_clip_selection_has_waveforms_and_full_scroll_extent() {
    let (mut app, ctx, track, _) = audio_fixture();
    let mut distant = app.session.project.tracks[0].clips[0].clone();
    distant.id = Id::new_v4();
    distant.start_frame = 48_000 * 120;
    app.session.project.tracks[0].clips.push(distant);
    app.selected_clip = None;
    app.selected_clips.clear();
    app.midi_editor.content = Default::default();
    let before = app.session.project.clone();
    let painted = shapes(&mut app, &ctx);
    waveform_body(&app, &painted);
    assert_eq!(app.selected_clip, None);
    assert_eq!(app.selected_track, Some(track));
    assert!(app.midi_editor.content.end > 120.0);
    app.midi_editor.content.scroll = 119.9;
    let painted = shapes(&mut app, &ctx);
    waveform_body(&app, &painted);
    assert_eq!(app.midi_editor.content.scroll, 119.9);
    app.session.project.tracks[0].clips.clear();
    let painted = shapes(&mut app, &ctx);
    assert!(!painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Select a clip"
            && text.pos.x > app.midi_editor.panel_bounds.right())));
    app.session.project = before.clone();
    app.selected_track = None;
    let painted = shapes(&mut app, &ctx);
    let color = theme::clip_colors(before.tracks[0].color, None).body;
    assert!(!painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if rect.fill == color)));
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
    assert!(app.job.is_none());
}
