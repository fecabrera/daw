use super::instrument_catalog::{add_component, click, key};
use super::*;

fn fixture(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.selected_track = Some(track);
    let layer = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    add_component(&mut app, &ctx, layer, 5);
    (app, ctx, layer)
}
fn state(app: &DawUi) -> &daw_core::SamplerSettings {
    app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .sampler
        .as_ref()
        .unwrap()
}
fn type_root(app: &mut DawUi, ctx: &egui::Context, text: &str, commit: egui::Key) {
    let point = app.instrument_ui.samplers[0].root.center();
    click(app, ctx, point, false);
    let id = ctx.memory(|m| m.focused()).unwrap();
    let mut edit = egui::TextEdit::load_state(ctx, id).unwrap();
    edit.cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(20),
        )));
    edit.store(ctx, id);
    frame(app, ctx, vec![egui::Event::Text(text.into())]);
    key(app, ctx, commit, false, false);
}
fn choose_duration_mode(app: &mut DawUi, ctx: &egui::Context, text: &str) {
    let menu = app.instrument_ui.samplers[0].duration_unit;
    choose_dropdown(app, ctx, menu, text);
}
fn choose_trigger(app: &mut DawUi, ctx: &egui::Context, text: &str) {
    let menu = app.instrument_ui.samplers[0].trigger;
    choose_dropdown(app, ctx, menu, text);
}
fn choose_dropdown(app: &mut DawUi, ctx: &egui::Context, menu: Rect, text: &str) {
    click(app, ctx, menu.center(), false);
    let shapes = frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0));
    let option = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text && t.pos.y > menu.bottom() => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("dropdown option");
    click(app, ctx, option, false);
    wait(app, ctx);
}
fn wait(app: &mut DawUi, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.job.is_some() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
        frame(app, ctx, vec![]);
    }
    assert!(app.job.is_none(), "sample job timed out");
    assert!(app.error.is_none(), "{:?}", app.error);
}
#[test]
fn sampler_catalog_root_sharp_controls_and_compact_layout_follow_shared_ui_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, layer) = fixture(theme);
        assert!(state(&app).input.is_none());
        assert_eq!(state(&app).root_note, 60);
        let b = &app.instrument_ui.samplers[0];
        assert!(b.panel.contains_rect(b.root) && b.panel.contains_rect(b.sharp));
        assert!(!b.gain.intersects(b.waveform));
        assert!(b.duration.is_positive());
        type_root(&mut app, &ctx, "A3", egui::Key::Enter);
        assert_eq!(state(&app).root_note, 57);
        let sharp = app.instrument_ui.samplers[0].sharp.center();
        click(&mut app, &ctx, sharp, false);
        assert_eq!(state(&app).root_note, 58);
        type_root(&mut app, &ctx, "B3", egui::Key::Enter);
        assert_eq!(state(&app).root_note, 58);
        let sharp = app.instrument_ui.samplers[0].sharp.center();
        click(&mut app, &ctx, sharp, false);
        type_root(&mut app, &ctx, "B3", egui::Key::Enter);
        assert_eq!(state(&app).root_note, 59);
        let sharp = app.instrument_ui.samplers[0].sharp.center();
        click(&mut app, &ctx, sharp, false);
        assert_eq!(state(&app).root_note, 59);
        type_root(&mut app, &ctx, "C9", egui::Key::Escape);
        assert_eq!(state(&app).root_note, 59);
        type_root(&mut app, &ctx, "G9", egui::Key::Enter);
        assert_eq!(state(&app).root_note, 127);
        let sharp = app.instrument_ui.samplers[0].sharp.center();
        click(&mut app, &ctx, sharp, false);
        assert_eq!(state(&app).root_note, 127);
        // Adding another source is a clean rejection; existing settings survive.
        let before = state(&app).clone();
        app.dirty = false;
        add_component(&mut app, &ctx, layer, 0);
        assert_eq!(state(&app), &before);
        assert!(!app.dirty);
        choose_duration_mode(&mut app, &ctx, "Time (ms)");
        assert_eq!(state(&app).mode, daw_core::SamplerMode::Time);
        assert!(app.instrument_ui.samplers[0].duration.is_positive());
        let b = &app.instrument_ui.samplers[0];
        assert!((b.root.center().y - b.sharp.center().y).abs() <= 1.0);
        assert!((b.root.center().y - b.duration.center().y).abs() <= 1.0);
        assert!((b.root.right() - b.sharp.left()).abs() < 0.01);
        assert!(b.root_group.contains_rect(b.root) && b.root_group.contains_rect(b.sharp));
        assert!(b.root_group.right() < b.duration.left());
        assert!((b.root_group.height() - toolbars::CONTROL_HEIGHT).abs() < 0.01);
        assert!(
            (b.root_group.height() - b.duration.height()).abs() < 0.01,
            "group {}, duration {}",
            b.root_group.height(),
            b.duration.height()
        );
        assert!((b.root_group.top() - b.duration.top()).abs() < 0.01);
        assert!((b.root_group.bottom() - b.duration.bottom()).abs() < 0.01);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if matches!(t.galley.text(), "Root note" | "Duration"))));

        let point = app.instrument_ui.samplers[0].root.center();
        click(&mut app, &ctx, point, false);
        let b = &app.instrument_ui.samplers[0];
        assert!(
            (b.root_group.height() - b.duration.height()).abs() < 0.01,
            "group {}, duration {}",
            b.root_group.height(),
            b.duration.height()
        );
        key(&mut app, &ctx, egui::Key::Escape, false, false);
        choose_trigger(&mut app, &ctx, "Gate");
        assert_eq!(state(&app).trigger, daw_core::SamplerTrigger::Gate);
        let layer_state = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer_state.collapsed = true;
        frame(&mut app, &ctx, vec![]);
        let b = &app.instrument_ui.samplers[0];
        assert!(!b.waveform.is_positive() && !b.root.is_positive() && !b.gain.is_positive());
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .sampler
            .as_mut()
            .unwrap()
            .collapsed = true;
        frame(&mut app, &ctx, vec![]);
        assert!(!app.instrument_ui.samplers[0].waveform.is_positive());
        let saved = serde_json::to_vec(&app.session.project).unwrap();
        let reopened: daw_core::Project = serde_json::from_slice(&saved).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            reopened.tracks[0].instrument,
            app.session.project.tracks[0].instrument
        );
    }
}

#[test]
fn sampler_file_drop_and_region_drag_prepare_on_release_and_cancel_without_edits() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let file = TestWav::new();
        let pointer = app.instrument_ui.samplers[0].waveform.center();
        file_frame(&mut app, &ctx, &file.0, pointer, true);
        wait(&mut app, &ctx);
        assert!(app.session.project.tracks[0].clips.is_empty());
        assert!(state(&app).input.is_some());
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let name = &state(&app).input.as_ref().unwrap().name;
        let titles: Vec<_> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == name => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(titles.len(), 1);
        let header_top = app.instrument_ui.samplers[0].ruler.bottom();
        assert!((header_top..header_top + 22.0).contains(&titles[0].pos.y));
        let sample = state(&app).id;
        assert_eq!(app.session.audio[&sample].samples.len(), 48000);
        let original_loop = serde_json::to_value(&app.session.project.transport.r#loop).unwrap();
        let bar = app.instrument_ui.samplers[0].range;
        let start = Pos2::new(bar.right(), bar.center().y);
        let end = Pos2::new(bar.center().x, bar.center().y);
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(state(&app).end_frame, 48000);
        assert!(!app.dirty && app.job.is_none());
        key(&mut app, &ctx, egui::Key::Escape, false, false);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app).end_frame, 48000);
        assert!(!app.dirty && app.job.is_none());
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        wait(&mut app, &ctx);
        assert!(state(&app).end_frame < 48000 && app.dirty);
        assert_eq!(
            app.session.audio[&sample].samples.len() as u64,
            state(&app).end_frame
        );
        assert_eq!(
            serde_json::to_value(&app.session.project.transport.r#loop).unwrap(),
            original_loop
        );
    }
}

#[test]
fn sampler_audio_clip_drag_opens_instrument_and_assigns_on_drop_without_moving_the_clip() {
    let (mut app, ctx, _) = fixture(settings::ColorTheme::Dark);
    let midi_track = app.selected_track.unwrap();
    let file = TestWav::new();
    let audio_track = app.session.project.add_track().unwrap();
    app.session.import(&file.0, Some(audio_track), 0).unwrap();
    let source = app.session.project.tracks[1].clips[0].clone();
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.select_track(audio_track, false);
    frame(&mut app, &ctx, vec![]);
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    let instrument = shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Instrument" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("Instrument view button");
    let start = app
        .clip_block(app.lane_bounds[&audio_track], &source, 0)
        .min
        + Vec2::new(20.0, 12.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::new(10.0, 0.0))],
    );
    assert!(app.drag.is_some());
    app.dirty = false;
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(instrument)]);
    assert_eq!(app.view_mode, ViewMode::Instrument);
    assert_eq!(app.selected_track, Some(midi_track));
    frame(&mut app, &ctx, vec![]);
    let target = app.instrument_ui.samplers[0].waveform.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
    assert!(state(&app).input.is_none() && !app.dirty);
    frame(&mut app, &ctx, vec![button(target, false)]);
    wait(&mut app, &ctx);
    assert_eq!(
        state(&app).input.as_ref().unwrap().asset_id,
        source.asset_id
    );
    assert!(app.dirty && app.session.project.tracks[0].clips.is_empty());
    assert_eq!(
        serde_json::to_value(&app.session.project.tracks[1].clips[0]).unwrap(),
        serde_json::to_value(source).unwrap()
    );
}

#[test]
fn sampler_cancelled_preparation_cannot_publish_a_late_result() {
    let (mut app, ctx, _) = fixture(settings::ColorTheme::Light);
    app.dirty = false;
    let (send, receive) = mpsc::channel();
    let mut next = app.session.clone();
    next.project.name = "Must not publish".into();
    app.run_sampler_job("Preparing sample", move |_| {
        receive.recv().unwrap();
        Ok(next)
    });
    assert!(app.settings_cancel.is_some());
    app.settings_cancel
        .take()
        .unwrap()
        .store(true, Ordering::Relaxed);
    app.job = None;
    app.busy.clear();
    send.send(()).unwrap();
    for _ in 0..5 {
        frame(&mut app, &ctx, vec![]);
    }
    assert_ne!(app.session.project.name, "Must not publish");
    assert!(!app.dirty);
}

#[test]
fn sampler_dropdowns_replace_mode_knobs_and_preserve_standard_strip_geometry() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(color);
        for (mode, active) in [
            (daw_core::SamplerMode::Off, "Off"),
            (daw_core::SamplerMode::Time, "ms"),
            (daw_core::SamplerMode::Bars, "bars"),
        ] {
            app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0]
                .sampler
                .as_mut()
                .unwrap()
                .mode = mode;
            frame(&mut app, &ctx, vec![]);
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let text = |name: &str| {
                shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.text() == name => Some(t),
                        _ => None,
                    })
                    .unwrap()
            };
            let rect = |name: &str| {
                let t = text(name);
                Rect::from_min_size(t.pos, t.galley.size())
            };
            let b = &app.instrument_ui.samplers[0];
            assert!(
                b.duration_unit
                    .contains(text(active).pos + text(active).galley.size() * 0.5)
            );
            assert!(b.trigger.contains_rect(rect("One-shot")));
            assert!(
                !shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Gate"))
            );
            assert!((b.trigger.height() - toolbars::CONTROL_HEIGHT).abs() < 0.01);
            assert!((b.trigger.center().y - b.duration.center().y).abs() < 0.01);
            assert!((b.trigger.width() - 76.0).abs() < 0.01);
            assert!(b.duration.right() < b.trigger.left());
            assert!(
                b.trigger.right() <= b.waveform.right(),
                "root {:?} duration {:?} trigger {:?} wave {:?}",
                b.root_group,
                b.duration,
                b.trigger,
                b.waveform
            );
            let strip = b.controls;
            let footer = rect("Sample");
            assert!((strip.width() - midi_editor::UNIT_STRIP_WIDTH).abs() < 0.01);
            assert!((footer.center().x - strip.center().x).abs() <= 1.0);
            let footer_y = strip.bottom()
                - f32::from(panels::PanelStyle::default().margin)
                - icons::COMPACT_BUTTON_SIZE * 0.5;
            assert!((footer.center().y - footer_y).abs() <= 1.0);
            assert!((b.gain.center().x - strip.center().x).abs() <= 1.0);
            assert!(b.gain.bottom() < footer.top());
            assert!(!strip.intersects(b.trigger));
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke }
                if (points[0].x - strip.left()).abs() < 0.01 && (points[1].x - strip.left()).abs() < 0.01
                    && (points[0].y - strip.top()).abs() < 0.01 && (points[1].y - strip.bottom()).abs() < 0.01
                    && stroke.width == 1.0 && stroke.color == theme::unit_separator(&ctx))));
        }
        // Off keeps the dropdown available, but its original-duration readout cannot edit.
        choose_duration_mode(&mut app, &ctx, "Off");
        app.dirty = false;
        let saved = state(&app).clone();
        let value = app.instrument_ui.samplers[0].duration_value.center();
        click(&mut app, &ctx, value, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        assert_eq!(state(&app), &saved);
        assert!(!app.dirty);
        let menu = app.instrument_ui.samplers[0].duration_unit.center();
        click(&mut app, &ctx, menu, false);
        key(&mut app, &ctx, egui::Key::Escape, false, false);
        assert_eq!(state(&app), &saved);
        assert!(!app.dirty);
        choose_duration_mode(&mut app, &ctx, "Off");
        assert!(!app.dirty);
        let file = TestWav::new();
        let target = app.instrument_ui.samplers[0].waveform.center();
        file_frame(&mut app, &ctx, &file.0, target, true);
        wait(&mut app, &ctx);
        let id = state(&app).id;
        choose_duration_mode(&mut app, &ctx, "Bars");
        assert_eq!(state(&app).mode, daw_core::SamplerMode::Bars);
        assert_eq!(state(&app).bars, 1);
        assert_eq!(app.session.audio[&id].samples.len(), 96000);
        choose_duration_mode(&mut app, &ctx, "Time (ms)");
        assert_eq!(state(&app).mode, daw_core::SamplerMode::Time);
        assert_eq!(state(&app).time_ms, 2000.0);
        choose_duration_mode(&mut app, &ctx, "Off");
        assert_eq!(state(&app).mode, daw_core::SamplerMode::Off);
        assert_eq!(app.session.audio[&id].samples.len(), 48000);
        app.dirty = false;
        let saved = state(&app).clone();
        let menu = app.instrument_ui.samplers[0].trigger.center();
        click(&mut app, &ctx, menu, false);
        key(&mut app, &ctx, egui::Key::Escape, false, false);
        assert_eq!(state(&app), &saved);
        assert!(!app.dirty);
        choose_trigger(&mut app, &ctx, "One-shot");
        assert!(!app.dirty);
        let prepared = app.session.audio[&id].samples.clone();
        choose_trigger(&mut app, &ctx, "Gate");
        assert_eq!(state(&app).trigger, daw_core::SamplerTrigger::Gate);
        assert!(app.dirty);
        assert!(std::sync::Arc::ptr_eq(
            &prepared,
            &app.session.audio[&id].samples
        ));
        choose_trigger(&mut app, &ctx, "One-shot");
        assert_eq!(state(&app).trigger, daw_core::SamplerTrigger::OneShot);
    }
}

#[test]
fn sampler_root_drag_steps_through_accidentals_clamps_and_cancels_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let start = app.instrument_ui.samplers[0].root.center();
        let end = start + Vec2::new(10.0, 0.0);
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(state(&app).root_note, 60);
        assert!(!app.dirty && app.job.is_none());
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app).root_note, 61);
        assert!(app.dirty && app.job.is_none());
        type_root(&mut app, &ctx, "D4", egui::Key::Enter);
        assert_eq!(state(&app).root_note, 63);
        let start = app.instrument_ui.samplers[0].root.center();
        let end = start - Vec2::new(30.0, 0.0);
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        key(&mut app, &ctx, egui::Key::Escape, false, false);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app).root_note, 63);
        assert!(!app.dirty);
        // Full MIDI bounds remain reachable, with no accidental beyond G9.
        let sharp = app.instrument_ui.samplers[0].sharp.center();
        click(&mut app, &ctx, sharp, false);
        type_root(&mut app, &ctx, "G9", egui::Key::Enter);
        let start = app.instrument_ui.samplers[0].root.center();
        let end = start + Vec2::new(30.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app).root_note, 127);
    }
}

#[test]
fn sampler_bars_duration_has_whole_bar_drag_steps_and_a_one_bar_minimum() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .sampler
            .as_mut()
            .unwrap()
            .mode = daw_core::SamplerMode::Bars;
        frame(&mut app, &ctx, vec![]);
        let start = app.instrument_ui.samplers[0].duration_value.center();
        let end = start + Vec2::new(10.0, 0.0);
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(state(&app).bars, 1);
        assert!(!app.dirty && app.job.is_none());
        let shows = |shapes: &[egui::epaint::ClippedShape], expected: &str| {
            shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == expected))
        };
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(
            shows(&shapes, "2"),
            "{:?}",
            shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some(t.galley.text()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(end + Vec2::new(10.0, 0.0))],
        );
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(shows(&shapes, "3"));
        frame(
            &mut app,
            &ctx,
            vec![button(end + Vec2::new(10.0, 0.0), false)],
        );
        assert_eq!(state(&app).bars, 3);
        let start = app.instrument_ui.samplers[0].duration_value.center();
        let end = start - Vec2::new(50.0, 0.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert_eq!(state(&app).bars, 1);
    }
}

#[test]
fn sampler_preview_reuses_standard_local_grid_and_ruler_at_project_tempo() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(theme);
        let file = TestWav::new();
        let target = app.instrument_ui.samplers[0].waveform.center();
        file_frame(&mut app, &ctx, &file.0, target, true);
        wait(&mut app, &ctx);
        for tempo in [60.0, 120.0, 210.0] {
            for (mode, time_ms, bars) in [
                (daw_core::SamplerMode::Off, 1000.0, 1),
                (daw_core::SamplerMode::Time, 750.0, 1),
                (daw_core::SamplerMode::Bars, 1000.0, 1),
                (daw_core::SamplerMode::Bars, 1000.0, 2),
            ] {
                let sample = app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .sampler
                    .as_mut()
                    .unwrap();
                sample.mode = mode;
                sample.time_ms = time_ms;
                sample.bars = bars;
                app.session.project.tempo_bpm = tempo;
                app.dirty = false;
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let b = &app.instrument_ui.samplers[0];
                let body = Rect::from_min_max(
                    b.ruler.left_bottom() + Vec2::new(0.0, 22.0),
                    b.waveform.max,
                );
                let sample = state(&app);
                let duration = sample
                    .output_frames(app.session.project.sample_rate_hz, tempo)
                    .unwrap();
                let ratio = duration as f64 / (sample.end_frame - sample.start_frame) as f64;
                let timeline = musical_time::Timeline::at_sample_rate(
                    tempo,
                    (f64::from(body.width()) * f64::from(app.session.project.sample_rate_hz)
                        / (sample.input.as_ref().unwrap().length_frames as f64 * ratio))
                        as f32,
                    0.0,
                    app.session.project.sample_rate_hz,
                );
                if mode == daw_core::SamplerMode::Bars {
                    let one_beat = (f64::from(app.session.project.sample_rate_hz) * 60.0
                        / f64::from(tempo))
                    .round() as u64;
                    assert!(
                        (timeline.pixel_at_frame(one_beat) as f32
                            - body.width() / (4.0 * bars as f32))
                            .abs()
                            < 0.01
                    );
                }
                let palette = theme::palette(&ctx);
                for tick in timeline.ticks(body.width()) {
                    let x = body.left() + tick.x;
                    let color = if tick.bar {
                        palette.border
                    } else if tick.whole_beat {
                        palette.grid
                    } else {
                        palette.grid_subdivision
                    };
                    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke }
                    if (points[0].x - x).abs() < 0.01 && (points[1].x - x).abs() < 0.01
                        && (points[0].y - body.top()).abs() < 0.01 && (points[1].y - body.bottom()).abs() < 0.01
                        && stroke.width == 1.0 && stroke.color == color)));
                    if let Some(label) = tick.label.filter(|_| tick.x + 40.0 < body.width()) {
                        assert!(
                            shapes
                                .iter()
                                .any(|s| matches!(&s.shape, egui::Shape::Text(t)
                        if t.galley.text() == label && (t.pos.x - x - 3.0).abs() < 0.01
                            && b.range.contains(t.pos + t.galley.size() * 0.5)))
                        );
                    }
                }
                assert!(!app.dirty && app.job.is_none());
            }
        }
    }
}

#[test]
fn sampler_duration_input_keeps_fixed_width_and_centered_text_in_display_and_editing() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = fixture(color);
        for (mode, time_ms, bars) in [
            (daw_core::SamplerMode::Bars, 1000.0, 1),
            (daw_core::SamplerMode::Bars, 1000.0, 128),
            (daw_core::SamplerMode::Bars, 1000.0, u32::MAX),
            (daw_core::SamplerMode::Time, 1.0, 1),
            (daw_core::SamplerMode::Time, 123.45, 1),
            (daw_core::SamplerMode::Time, 1e12, 1),
        ] {
            let sample = app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0]
                .sampler
                .as_mut()
                .unwrap();
            sample.mode = mode;
            sample.time_ms = time_ms;
            sample.bars = bars;
            for editing in [false, true] {
                frame(&mut app, &ctx, vec![]);
                if editing {
                    let center = app.instrument_ui.samplers[0].duration_value.center();
                    click(&mut app, &ctx, center, false);
                }
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let bounds = &app.instrument_ui.samplers[0];
                let field = bounds.duration;
                assert!(
                    (field.width() - 120.0).abs() < 0.01,
                    "{field:?}, editing {editing}"
                );
                assert!((field.height() - toolbars::CONTROL_HEIGHT).abs() < 0.01);
                assert!((field.height() - bounds.root_group.height()).abs() < 0.01);
                assert!((bounds.duration_value.right() - bounds.duration_unit.left()).abs() < 0.01);
                assert!((bounds.duration_unit.width() - 60.0).abs() < 0.01);
                assert!(
                    field.contains_rect(bounds.duration_value)
                        && field.contains_rect(bounds.duration_unit)
                );
                for component in [bounds.root, bounds.sharp, bounds.duration_value] {
                    let text = shapes
                        .iter()
                        .filter_map(|s| match &s.shape {
                            egui::Shape::Text(t) => {
                                let rect = t.galley.rect.translate(t.pos.to_vec2());
                                if rect.intersects(component) && !t.galley.text().is_empty() {
                                    assert!(t.galley.job.sections.iter().all(|s| {
                                        if component == bounds.duration_value {
                                            s.format.font_id.size > 0.0
                                                && s.format.font_id.size <= 11.0
                                        } else {
                                            (s.format.font_id.size - 11.0).abs() < 0.01
                                        }
                                    }));
                                    Some(rect)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .reduce(|a, b| a.union(b))
                        .unwrap();
                    assert!(
                        (text.center().x - component.center().x).abs() <= 1.0,
                        "{text:?}, component {component:?}, editing {editing}"
                    );
                }
                let label = if mode == daw_core::SamplerMode::Bars {
                    "bars"
                } else {
                    "ms"
                };
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label && bounds.duration_unit.contains(t.pos + t.galley.size() * 0.5))));
                if editing {
                    key(&mut app, &ctx, egui::Key::Escape, false, false);
                }
            }
            app.dirty = false;
            let saved = state(&app).clone();
            let label = app.instrument_ui.samplers[0].duration_unit.center();
            click(&mut app, &ctx, label, false);
            assert_eq!(state(&app), &saved);
            assert!(!app.dirty && app.job.is_none());
            key(&mut app, &ctx, egui::Key::Escape, false, false);
        }
    }
}
