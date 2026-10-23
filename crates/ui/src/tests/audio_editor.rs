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
    let track = &app.session.project.tracks[0];
    let clip = track
        .clips
        .iter()
        .find(|c| Some(c.id) == app.selected_clip)
        .unwrap();
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
fn audio_editor_waveform_handles_missing_source_and_empty_selection() {
    let (mut app, ctx, _, _) = audio_fixture();
    app.session.audio.clear();
    let painted = shapes(&mut app, &ctx);
    assert!(painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Missing source"
            && text.pos.y > app.midi_editor.track_bounds.bottom())));
    app.selected_clip = None;
    app.selected_clips.clear();
    let painted = shapes(&mut app, &ctx);
    assert!(painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Select a clip"
            && text.pos.x > app.midi_editor.panel_bounds.right()
            && text.pos.y > app.midi_editor.track_bounds.bottom())));
    assert!(!painted.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text().contains("not available yet"))));
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
