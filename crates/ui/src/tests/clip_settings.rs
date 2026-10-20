use super::*;

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::WindowFocused(true),
            egui::Event::PointerMoved(point),
            button(point, true),
        ],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::WindowFocused(true), button(point, false)],
    );
}

fn key_press(app: &mut DawUi, ctx: &egui::Context, key: egui::Key, modifiers: egui::Modifiers) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::WindowFocused(true),
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
            egui::Event::ModifiersChanged(egui::Modifiers::default()),
        ],
    );
}

fn text_pos(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<Pos2> {
    fn walk(shape: &egui::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| walk(shape, label)),
            _ => None,
        }
    }
    shapes.iter().find_map(|shape| walk(&shape.shape, label))
}

fn dismiss_error(app: &mut DawUi, ctx: &egui::Context) {
    let shapes = frame_shapes(
        app,
        ctx,
        vec![egui::Event::WindowFocused(true)],
        Vec2::new(1280.0, 800.0),
    );
    let pos = text_pos(&shapes, "OK").expect("error dialog");
    click(app, ctx, pos);
    assert!(app.error.is_none());
    frame(app, ctx, vec![egui::Event::WindowFocused(true)]);
}

fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    walk(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for shape in shapes {
        walk(&shape.shape, &mut out);
    }
    out
}

fn open_field(app: &mut DawUi, ctx: &egui::Context, field: Rect) {
    let point = field.center();
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
    assert!(app.clip_settings_draft.is_none());
    frame(app, ctx, vec![button(point, true)]);
    frame(app, ctx, vec![button(point, false)]);
    assert!(
        app.clip_settings_draft.is_some(),
        "double-click did not open the field"
    );
    frame(app, ctx, vec![]);
    assert!(app.clip_settings_draft.is_some());
}

fn replace_field(app: &mut DawUi, ctx: &egui::Context, field: Rect, text: &str, key: egui::Key) {
    open_field(app, ctx, field);
    frame(
        app,
        ctx,
        vec![
            egui::Event::WindowFocused(true),
            egui::Event::Text(text.into()),
        ],
    );
    key_press(app, ctx, key, egui::Modifiers::default());
}

fn pointer(app: &mut DawUi, ctx: &egui::Context, at: Pos2, pressed: bool, shift: bool) {
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::WindowFocused(true),
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(at),
            button(at, pressed),
        ],
    );
}

fn pointer_move(app: &mut DawUi, ctx: &egui::Context, at: Pos2, shift: bool) {
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::WindowFocused(true),
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(at),
        ],
    );
}

#[test]
fn clip_settings_text_baselines_and_row_spacing_match_the_catalog_in_both_themes() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color_theme,
                ..Default::default()
            },
        );
        app.effects_catalog = effects_catalog::catalog();
        let clip = app.session.project.tracks[0].clips[0].id;
        app.select_clip(clip, track, false);
        frame(&mut app, &ctx, vec![]);
        let catalog = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let positions = ["EQ & Filters", "Dynamics", "Distortion", "Imaging"]
            .map(|label| text_pos(&catalog, label).unwrap());
        let header = text_pos(&catalog, "Effects").unwrap();
        app.timeline_catalog_visible = false;
        app.clip_settings_visible = true;
        frame(&mut app, &ctx, vec![]);
        let form = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let form_header = text_pos(&form, "Clip settings").unwrap();
        for (label, expected) in ["Name", "Source", "Gain", "Transpose"]
            .into_iter()
            .zip(positions)
        {
            let actual = text_pos(&form, label).unwrap();
            assert!(
                (actual.x - form_header.x).abs() <= 0.5,
                "{label} left inset"
            );
            assert!(
                ((actual.y - form_header.y) - (expected.y - header.y)).abs() <= 0.5,
                "{label}: form={actual:?}, catalog={expected:?}"
            );
        }

        app.selected_clip = None;
        let empty = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let prompt = text_pos(&empty, "Select a clip").unwrap();
        let header = text_pos(&empty, "Clip settings").unwrap();
        assert!((prompt.x - header.x).abs() <= 0.5);
        assert!(
            (prompt.y - header.y - (positions[0].y - text_pos(&catalog, "Effects").unwrap().y))
                .abs()
                <= 0.5
        );
    }
}

#[test]
fn clip_settings_is_exclusive_with_the_catalogs_and_edits_the_selected_clip() {
    let (mut app, track) = fixture();
    let ctx = context();
    let clip = app.session.project.tracks[0].clips[0].id;
    let file_name = std::path::Path::new(&app.session.project.assets[0].source.path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    let library = app.effects_bounds.library_toggle;
    let toggle = app.effects_bounds.clip_settings_toggle;
    assert_eq!(toggle.size(), Vec2::splat(24.0));
    assert_eq!(toggle.left(), library.left());
    assert_eq!(toggle.top() - library.bottom(), 7.0);
    assert!(app.timeline_catalog_visible);
    assert!(!app.clip_settings_visible);
    assert!(app.effects_bounds.library.width() > 0.0);

    click(&mut app, &ctx, toggle.center());
    assert!(app.clip_settings_visible);
    assert!(!app.timeline_catalog_visible);
    assert!(!app.instrument_catalog_visible);
    assert!(!app.dirty);
    assert_eq!(app.effects_bounds.library, Rect::NOTHING);
    assert_eq!(
        app.effects_bounds.clip_settings.width(),
        panels::ControlPanel::WIDTH
    );
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let text = painted_text(&shapes);
    assert!(text.iter().any(|line| line == "Clip settings"));
    assert!(text.iter().any(|line| line == "Select a clip"));

    app.select_clip(clip, track, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let text = painted_text(&shapes);
    for label in [
        "Name",
        "Source",
        "Gain",
        "Transpose",
        "Test",
        "0.0 dB",
        "0 semitones",
    ] {
        assert!(
            text.iter().any(|line| line == label),
            "missing {label} in {text:?}"
        );
    }
    let source_text = shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text)
            if text.galley.text() == file_name
                || text.galley.text().starts_with("daw-missing-") =>
        {
            Some(text)
        }
        _ => None,
    });
    let source_text = source_text.expect("missing source name");
    let gain_text = shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == "0.0 dB" => Some(text),
        _ => None,
    });
    let gain_text = gain_text.expect("missing gain");
    let palette = theme::palette(&ctx);
    let source_format = &source_text.galley.job.sections[0].format;
    let gain_format = &gain_text.galley.job.sections[0].format;
    assert_eq!(source_format.color, palette.secondary);
    assert_eq!(gain_format.color, palette.text);
    assert_eq!(source_format.font_id.size, 11.0);
    assert_eq!(source_format.font_id.size, gain_format.font_id.size);
    assert!(app.effects_bounds.clip_name.width() > 0.0);
    assert_eq!(
        app.effects_bounds.clip_gain.left(),
        app.effects_bounds.clip_name.left()
    );
    assert_eq!(
        app.effects_bounds.clip_transpose.left(),
        app.effects_bounds.clip_name.left()
    );

    let gain = app.effects_bounds.clip_gain.center();
    pointer(&mut app, &ctx, gain, true, false);
    pointer_move(&mut app, &ctx, gain + Vec2::new(20.0, 0.0), false);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 0.0);
    pointer_move(&mut app, &ctx, gain + Vec2::new(20.0, -10.0), false);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 1.0);
    pointer_move(&mut app, &ctx, gain + Vec2::new(20.0, -20.0), true);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 1.1);
    pointer(&mut app, &ctx, gain + Vec2::new(20.0, -20.0), false, false);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 0.0);

    let transpose = app.effects_bounds.clip_transpose.center();
    pointer(&mut app, &ctx, transpose, true, false);
    pointer_move(&mut app, &ctx, transpose + Vec2::new(20.0, 0.0), false);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    pointer_move(&mut app, &ctx, transpose + Vec2::new(20.0, -2.0), false);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    pointer_move(&mut app, &ctx, transpose + Vec2::new(20.0, -12.0), true);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    pointer_move(&mut app, &ctx, transpose + Vec2::new(20.0, -16.0), true);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    pointer(
        &mut app,
        &ctx,
        transpose + Vec2::new(20.0, -16.0),
        false,
        false,
    );
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 3);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);

    let name = app.effects_bounds.clip_name;
    replace_field(&mut app, &ctx, name, "  Lead  ", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].name, "Lead");
    assert!(app.dirty);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    assert_eq!(app.session.project.tracks[0].clips[0].name, "Test");

    let name = app.effects_bounds.clip_name;
    replace_field(&mut app, &ctx, name, "  Lead  ", egui::Key::Escape);
    assert_eq!(app.session.project.tracks[0].clips[0].name, "Test");

    let gain = app.effects_bounds.clip_gain;
    replace_field(&mut app, &ctx, gain, "-6.0", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, -6.0);
    let gain = app.effects_bounds.clip_gain;
    replace_field(&mut app, &ctx, gain, "99", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 12.0);
    let transpose = app.effects_bounds.clip_transpose;
    replace_field(&mut app, &ctx, transpose, "12", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 12);

    app.dirty = false;
    let gain = app.effects_bounds.clip_gain;
    replace_field(&mut app, &ctx, gain, "nope", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].gain_db, 12.0);
    assert!(app.error.is_some());
    assert!(!app.dirty);
    dismiss_error(&mut app, &ctx);

    app.dirty = false;
    let transpose = app.effects_bounds.clip_transpose;
    replace_field(&mut app, &ctx, transpose, "49", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 12);
    assert!(app.error.is_some());
    assert!(!app.dirty);
    dismiss_error(&mut app, &ctx);

    app.dirty = false;
    let name = app.effects_bounds.clip_name;
    replace_field(&mut app, &ctx, name, "   ", egui::Key::Enter);
    assert_eq!(app.session.project.tracks[0].clips[0].name, "Test");
    assert!(app.error.is_some());
    assert!(!app.dirty);
    dismiss_error(&mut app, &ctx);

    let midi = app.session.project.add_midi_track().unwrap();
    let pattern = app.session.project.add_midi_pattern(midi, 0).unwrap();
    app.select_clip(pattern, midi, false);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(
        painted_text(&shapes)
            .iter()
            .any(|line| line == "MIDI pattern")
    );

    let library_toggle = app.effects_bounds.library_toggle.center();
    click(&mut app, &ctx, library_toggle);
    assert!(app.timeline_catalog_visible);
    assert!(!app.clip_settings_visible);
    assert!(app.effects_bounds.library.width() > 0.0);
    let clip_toggle = app.effects_bounds.clip_settings_toggle.center();
    click(&mut app, &ctx, clip_toggle);
    let clip_toggle = app.effects_bounds.clip_settings_toggle.center();
    click(&mut app, &ctx, clip_toggle);
    assert!(!app.clip_settings_visible);
    assert!(!app.timeline_catalog_visible);
    assert_eq!(app.effects_bounds.clip_settings, Rect::NOTHING);
    assert_eq!(app.effects_bounds.library, Rect::NOTHING);

    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let library = app.effects_bounds.library_toggle;
    let components = app.instrument_catalog_bounds.toggle;
    let clip_toggle = app.effects_bounds.clip_settings_toggle;
    assert_eq!(components.top() - library.bottom(), 7.0);
    assert_eq!(clip_toggle, Rect::NOTHING);
    click(&mut app, &ctx, components.center());
    assert!(app.instrument_catalog_visible);
    assert!(!app.clip_settings_visible);
    assert_eq!(app.effects_bounds.clip_settings, Rect::NOTHING);
}

#[test]
fn clip_settings_is_timeline_only_and_leaving_discards_the_open_draft() {
    for mode in [ViewMode::Instrument, ViewMode::Edit, ViewMode::Mixer] {
        let (mut app, track) = fixture();
        let ctx = context();
        let clip = app.session.project.tracks[0].clips[0].id;
        app.select_clip(clip, track, false);
        app.timeline_catalog_visible = false;
        app.clip_settings_visible = true;
        frame(&mut app, &ctx, vec![]);
        let name = app.effects_bounds.clip_name;
        open_field(&mut app, &ctx, name);
        app.dirty = false;
        app.set_view_mode(mode, &ctx);
        assert_eq!(app.view_mode, mode);
        assert!(!app.clip_settings_visible);
        assert!(app.clip_settings_draft.is_none());
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            !painted_text(&shapes)
                .iter()
                .any(|text| text == "Clip settings")
        );
        if mode == ViewMode::Instrument {
            assert_eq!(app.effects_bounds.clip_settings_toggle, Rect::NOTHING);
            assert_eq!(app.effects_bounds.clip_settings, Rect::NOTHING);
        }
        app.set_view_mode(ViewMode::Timeline, &ctx);
        frame(&mut app, &ctx, vec![]);
        assert!(app.effects_bounds.clip_settings_toggle.is_finite());
        assert!(!app.clip_settings_visible);
        assert_eq!(app.selected_clip, Some(clip));
        assert!(!app.dirty);
    }
}

fn prepared_audio_fixture(engine: daw_core::StretchEngine) -> (DawUi, egui::Context, Id) {
    let (mut app, track) = fixture();
    app.session.project.stretch_engine = engine;
    let asset = &mut app.session.project.assets[0];
    asset.decoded_frame_count = 24000;
    let audio = daw_media::AudioData {
        samples: std::sync::Arc::new(
            (0..24000)
                .map(|frame| {
                    let value =
                        (frame as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 0.2;
                    [value, -value]
                })
                .collect(),
        ),
        peaks: Default::default(),
        metadata: asset.source_metadata.clone(),
        sample_rate_hz: 48000,
    };
    app.session.audio.insert(asset.id, audio);
    app.session.project.tracks[0].clips[0].length_frames = 24000;
    let clip = app.session.project.tracks[0].clips[0].id;
    app.select_clip(clip, track, false);
    app.clip_settings_visible = true;
    app.timeline_catalog_visible = false;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    (app, ctx, clip)
}

#[test]
fn audio_clip_transpose_drag_prepares_only_on_release_and_publishes_one_undoable_edit() {
    for engine in [
        daw_core::StretchEngine::RUBBER_BAND,
        daw_core::StretchEngine::INTEGRATED,
    ] {
        let (mut app, ctx, clip) = prepared_audio_fixture(engine);
        let asset = app.session.project.tracks[0].clips[0].asset_id;
        let original = app.session.audio[&asset].samples.clone();
        app.session.project.transport.playhead_frame = 4000;
        app.dirty = false;
        let point = app.effects_bounds.clip_transpose.center();
        pointer(&mut app, &ctx, point, true, false);
        pointer_move(&mut app, &ctx, point - Vec2::new(0.0, 12.0), false);
        assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
        assert!(app.job.is_none());
        assert!(app.clip_transpose_drag.is_some());
        assert!(!app.dirty);
        pointer(&mut app, &ctx, point - Vec2::new(0.0, 12.0), false, false);
        assert!(app.job.is_some());
        assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
        wait_for_save(&mut app);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.session.project.tracks[0].clips[0].transpose, 12);
        assert_eq!(app.session.project.transport.playhead_frame, 4000);
        assert_eq!(app.selected_clip, Some(clip));
        assert!(app.dirty);
        let current = app.session.project.tracks[0].clips[0].clone();
        let pitched = app
            .session
            .audio_for_clip(&current)
            .unwrap()
            .samples
            .clone();
        assert!(!std::sync::Arc::ptr_eq(&original, &pitched));
        assert_eq!(pitched.len(), original.len());
        app.perform_history_action(HistoryAction::Undo, &ctx);
        assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &app.session.audio[&asset].samples
        ));
        app.perform_history_action(HistoryAction::Redo, &ctx);
        assert!(std::sync::Arc::ptr_eq(
            &pitched,
            &app.session.audio_for_clip(&current).unwrap().samples
        ));
    }
}

#[test]
fn audio_clip_transpose_cancelled_gestures_workers_and_failed_preparation_keep_the_project() {
    let (mut app, ctx, clip) = prepared_audio_fixture(daw_core::StretchEngine::INTEGRATED);
    app.dirty = false;
    let point = app.effects_bounds.clip_transpose.center();
    pointer(&mut app, &ctx, point, true, false);
    pointer_move(&mut app, &ctx, point - Vec2::new(0.0, 12.0), false);
    key_press(&mut app, &ctx, egui::Key::Escape, Default::default());
    pointer(&mut app, &ctx, point - Vec2::new(0.0, 12.0), false, false);
    assert!(app.job.is_none());
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    assert!(!app.dirty);
    // The existing cancellation action drops the receiver, rejecting late results.
    app.edit(Edit::SetClipTranspose {
        clip_id: clip,
        transpose: 12,
    });
    assert!(app.job.is_some());
    app.settings_cancel
        .take()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::Relaxed);
    app.job = None;
    app.poll();
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    assert!(!app.dirty);
    let asset = app.session.project.tracks[0].clips[0].asset_id;
    app.session.audio.get_mut(&asset).unwrap().samples =
        std::sync::Arc::new(vec![[f32::NAN; 2]; 24000]);
    app.edit(Edit::SetClipTranspose {
        clip_id: clip,
        transpose: -12,
    });
    wait_for_save(&mut app);
    assert!(app.error.is_some());
    assert_eq!(app.session.project.tracks[0].clips[0].transpose, 0);
    assert!(app.session.stretched_audio.is_empty());
    assert!(!app.dirty);
}
