use super::*;

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}

#[test]
fn effects_lane_spans_the_window_with_one_status_separator_and_an_aligned_overview() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for size in [
            Vec2::new(640.0, 260.0),
            Vec2::new(640.0, 480.0),
            Vec2::new(1280.0, 800.0),
        ] {
            let (mut app, track) = fixture();
            let clip = app.session.project.tracks[0].clips[0].id;
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    ..Default::default()
                },
            );
            app.select_clip(clip, track, false);
            app.set_view_mode(ViewMode::Edit, &ctx);
            frame_sized(&mut app, &ctx, vec![], size);
            let editor_ruler = app.midi_editor.ruler_bounds;
            let editor_lane = app.lane_bounds[&track];
            let original = format!("{:?}", app.session.project);
            app.set_view_mode(ViewMode::Effects, &ctx);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let catalog = app.effects_bounds.library;
            let header = app.effects_bounds.header;
            let rack = app.effects_bounds.strip;
            let scrollbar = app.effects_bounds.scrollbar;
            let ruler = app.midi_editor.ruler_bounds;
            let lane = app.lane_bounds[&track];
            assert_eq!(catalog.width(), TRACK_WIDTH);
            assert!((catalog.top() - header.top()).abs() < 0.1);
            assert!((header.bottom() - ruler.bottom()).abs() < 0.1);
            assert!(
                (catalog.bottom() - rack.top()).abs() < 0.1,
                "size={size:?} catalog={catalog:?} rack={rack:?} scrollbar={scrollbar:?}"
            );
            assert_eq!(rack.left(), catalog.left());
            assert_eq!(rack.width(), size.x);
            assert_eq!(rack.right(), editor_lane.right());
            assert!(rack.top() >= lane.bottom());
            if size.y >= 480.0 {
                assert_eq!(rack.height(), panels::unit_height(false));
                assert!(rack.top() > lane.bottom() + 20.0);
            } else {
                assert!(rack.height() > 0.0 && rack.height() < panels::unit_height(false));
            }
            assert_eq!(ruler.height(), editor_ruler.height());
            assert_eq!(lane.height(), editor_lane.height());
            assert!((ruler.left() - editor_ruler.left() - TRACK_WIDTH).abs() < 0.1);
            assert_eq!(ruler.left(), lane.left());
            assert_eq!(ruler.bottom(), lane.top());
            assert_eq!(scrollbar.left(), ruler.left());
            assert_eq!(scrollbar.right(), ruler.right());
            assert_eq!(scrollbar.height(), SCROLLBAR_HEIGHT);
            assert_eq!(scrollbar.bottom(), rack.top());
            assert!(scrollbar.top() >= lane.bottom());
            if scrollbar.top() > lane.bottom() {
                for tick in app.musical_timeline().ticks(lane.width()) {
                    let x = lane.left() + tick.x;
                    if x < lane.left() || x > lane.right() {
                        continue;
                    }
                    let line = [Pos2::new(x, lane.bottom()), Pos2::new(x, scrollbar.top())];
                    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, .. } if *points == line)));
                }
            }
            let palette = theme::palette(&ctx);
            let line = [
                Pos2::new(rack.left(), rack.top() + 0.5),
                Pos2::new(rack.right(), rack.top() + 0.5),
            ];
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment {points, stroke} if *points == line && *stroke == Stroke::new(1.0, palette.border))));
            let bottom_lines = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if points[0].x == rack.left()
                            && points[1].x == rack.right()
                            && points[0].y == points[1].y
                            && (points[0].y - rack.bottom()).abs() <= 1.0 =>
                    {
                        Some((points, stroke))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                bottom_lines.len(),
                1,
                "one separator between the lane and status bar"
            );
            assert_eq!(*bottom_lines[0].1, Stroke::new(1.0, palette.border));
            assert_eq!(bottom_lines[0].0[0].y, rack.bottom() + 0.5);
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rack && r.fill == palette.background)));
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == catalog && r.fill == palette.panel)));
            let border = [
                Pos2::new(catalog.right() - 0.5, catalog.top()),
                Pos2::new(catalog.right() - 0.5, catalog.bottom()),
            ];
            assert_eq!(shapes.iter().filter(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke } if *points == border && *stroke == Stroke::new(1.0, palette.border))).count(), 1);
            assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Track effects are not available yet." || t.galley.text() == "Oscillator" || t.galley.text() == "Envelope")));
            assert_eq!(format!("{:?}", app.session.project), original);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn effects_overview_follows_the_processing_track_and_clears_missing_selection() {
    let (mut app, audio) = fixture();
    let audio_clip = app.session.project.tracks[0].clips[0].id;
    let midi = app.session.project.add_midi_track().unwrap();
    let midi_clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
    let ctx = context();
    app.select_clip(audio_clip, audio, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    app.select_track(midi, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.selected_clip, Some(audio_clip));
    assert_eq!(app.lane_bounds.len(), 1);
    assert!(app.lane_bounds.contains_key(&midi));
    assert!(app.midi_editor.clip_bounds.contains_key(&midi_clip));
    assert!(!app.midi_editor.clip_bounds.contains_key(&audio_clip));
    app.selected_track = None;
    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
    assert!(app.lane_bounds.is_empty());
    assert_eq!(app.timeline_bounds, Rect::NOTHING);
    assert!(app.midi_editor.clip_bounds.is_empty());
    assert!(app.effects_bounds.strip.is_positive());
    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Select a track to configure its effects.")));
    assert!(!app.dirty);
}

#[test]
fn effects_ruler_seek_loop_and_zoom_use_the_overview_viewport() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    app.midi_editor.zoom = 240.0;
    app.midi_editor.scroll = 0.5;
    let timeline = (app.zoom, app.scroll);
    frame(&mut app, &ctx, vec![]);
    let ruler = app.midi_editor.ruler_bounds;
    click(
        &mut app,
        &ctx,
        Pos2::new(ruler.left() + 180.0, ruler.bottom() - 6.0),
    );
    assert_eq!(app.session.project.transport.playhead_frame, 60_000);
    let start = Pos2::new(ruler.left() + 240.0, ruler.top() + 5.0);
    let end = start + Vec2::new(240.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let region = &app.session.project.transport.r#loop;
    assert_eq!((region.start_frame, region.end_frame), (72_000, 120_000));
    assert!(app.dirty);
    let overview = (app.midi_editor.zoom, app.midi_editor.scroll);
    let point = app.effects_bounds.strip.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!((app.midi_editor.zoom, app.midi_editor.scroll), overview);
    let point = app.lane_bounds[&track].left_center() + Vec2::new(120.0, 0.0);
    let anchor = app.view_scroll() + 120.0 / f64::from(app.view_zoom());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
    );
    assert_eq!(app.view_zoom(), 480.0);
    assert!((app.view_scroll() + 120.0 / f64::from(app.view_zoom()) - anchor).abs() < 0.001);
    let grid_point = Pos2::new(
        point.x,
        (app.lane_bounds[&track].bottom() + app.effects_bounds.scrollbar.top()) * 0.5,
    );
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(grid_point),
            egui::Event::Zoom(1.25),
        ],
    );
    assert_eq!(app.view_zoom(), 600.0);
    app.set_view_scroll(0.0);
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&track];
    let project = format!("{:?}", app.session.project);
    let start = app.effects_bounds.scrollbar.left_center() + Vec2::new(30.0, -1.0);
    let end = start + Vec2::new(80.0, 0.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert!(app.view_scroll() > 0.0);
    assert_eq!(app.lane_bounds[&track], lane);
    assert_eq!(format!("{:?}", app.session.project), project);
    assert!(!app.dirty);
    assert_eq!((app.zoom, app.scroll), timeline);
}

fn eq_settings(app: &DawUi) -> &daw_core::EqSettings {
    let daw_core::EffectSettings::Eq3(settings) = &app.session.project.tracks[0].effects[0].unit
    else {
        panic!("expected EQ")
    };
    settings
}

#[test]
fn effect_catalog_colors_follow_hover_and_selection_without_editing_the_project() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for accent in [
            settings::AccentColor::DeepOrange,
            settings::AccentColor::Yellow,
        ] {
            let (mut app, track) = fixture();
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color_theme,
                    accent,
                    ..Default::default()
                },
            );
            app.select_track(track, false);
            app.set_view_mode(ViewMode::Effects, &ctx);
            let project = serde_json::to_string(&app.session.project).unwrap();
            app.dirty = false;
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerGone],
                Vec2::new(1280.0, 800.0),
            );
            let point = app.effects_bounds.entry.center();
            let palette = theme::palette(&ctx);
            let check = |shapes: &[egui::epaint::ClippedShape],
                         entry: Rect,
                         color: egui::Color32,
                         selected: bool| {
                let text = shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == "3-band EQ" => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(text.override_text_color, Some(color));
                assert_eq!(
                    text.pos,
                    entry.min + Vec2::splat(f32::from(theme::TOOLBAR_PADDING))
                );
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.font_id.size == 11.0)
                );
                let fill = theme::toggle_colors(&ctx, true).0;
                assert_eq!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == entry && rect.fill == fill)), selected);
            };
            check(&shapes, app.effects_bounds.entry, palette.secondary, false);
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point)],
                Vec2::new(1280.0, 800.0),
            );
            check(&shapes, app.effects_bounds.entry, palette.text, false);
            click(&mut app, &ctx, point);
            let shapes = frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerGone],
                Vec2::new(1280.0, 800.0),
            );
            check(&shapes, app.effects_bounds.entry, palette.text, true);
            assert_eq!(app.effects_catalog_selection, Some(daw_core::EQ_TYPE_ID));
            assert!(app.session.project.tracks[0].effects.is_empty());
            assert_eq!(
                serde_json::to_string(&app.session.project).unwrap(),
                project
            );
            assert!(!app.dirty);
        }
    }
}
#[test]
fn eq_catalog_adds_on_double_click_and_controls_save_independent_states() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    let entry = app.effects_bounds.entry.center();
    app.dirty = false;
    click(&mut app, &ctx, entry);
    assert!(app.session.project.tracks[0].effects.is_empty());
    assert!(!app.dirty);
    // Second click is the double-click release; only one instance is added.
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.session.project.tracks[0].effects.len(), 1);
    assert_eq!(eq_settings(&app).bands[0].frequency_hz, 250.0);
    assert_eq!(eq_settings(&app).bands[2].frequency_hz, 2500.0);
    assert!(app.dirty);
    let unit = &app.effects_bounds.units[0];
    assert_eq!(unit.knobs.len(), 3);
    for index in [0, 2] {
        assert!(unit.knobs[index].iter().all(Rect::is_positive));
    }
    assert!(unit.knobs[1][0].is_positive());
    assert_eq!(&unit.knobs[1][1..], &[Rect::NOTHING; 2]);
    assert_eq!(unit.bands[0].center().y, unit.bands[1].center().y);
    assert_eq!(unit.bands[1].center().y, unit.bands[2].center().y);
    let band = unit.bands[1].center();
    let original = eq_settings(&app).bands[1];
    click(&mut app, &ctx, band);
    let cut = eq_settings(&app).bands[1];
    assert!(!cut.enabled);
    assert_eq!(cut.gain_db, original.gain_db);
    assert_eq!(cut.frequency_hz, original.frequency_hz);
    assert_eq!(cut.q, original.q);
    let power = app.effects_bounds.units[0].power.center();
    click(&mut app, &ctx, power);
    assert!(!eq_settings(&app).enabled);
    assert!(!eq_settings(&app).bands[1].enabled);
    let compact = app.effects_bounds.compact.center();
    click(&mut app, &ctx, compact);
    frame(&mut app, &ctx, vec![]);
    assert!(app.session.project.tracks[0].effects_compact);
    assert_eq!(app.effects_bounds.strip.height(), ROW_HEIGHT);
    assert_eq!(app.effects_bounds.units[0].knobs, [[Rect::NOTHING; 3]; 3]);
    assert!(
        app.effects_bounds.units[0]
            .bands
            .iter()
            .all(Rect::is_positive)
    );
    let header = app.effects_bounds.units[0].header.center();
    end_click_sequence(&mut app, &ctx);
    click(&mut app, &ctx, header);
    click(&mut app, &ctx, header);
    frame(&mut app, &ctx, vec![]);
    assert!(eq_settings(&app).collapsed);
    assert!(app.effects_bounds.units[0].power.is_positive());
    assert_eq!(app.effects_bounds.units[0].bands, [Rect::NOTHING; 3]);
    let saved = serde_json::to_string(&app.session.project).unwrap();
    let restored: daw_core::Project = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        restored.tracks[0].effects,
        app.session.project.tracks[0].effects
    );
    assert!(restored.tracks[0].effects_compact);
    app.set_view_mode(ViewMode::Timeline, &ctx);
    app.set_view_mode(ViewMode::Effects, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(eq_settings(&app).collapsed);
    assert!(!eq_settings(&app).enabled);
}
#[test]
fn eq_addition_is_guarded_without_a_track_during_disabled_editing_and_focus_loss() {
    for mode in 0..3 {
        let (mut app, track) = fixture();
        let ctx = context();
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        if mode == 0 {
            app.selected_track = None;
        }
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.entry.center();
        if mode == 1 {
            app.unsaved_prompt = true;
        }
        app.dirty = false;
        if mode == 2 {
            for pressed in [true, false, true, false] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: false,
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        events: vec![egui::Event::PointerMoved(entry), button(entry, pressed)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
        } else {
            click(&mut app, &ctx, entry);
            click(&mut app, &ctx, entry);
        }
        assert!(app.session.project.tracks[0].effects.is_empty());
        assert!(!app.dirty);
    }
}

fn end_click_sequence(app: &mut DawUi, ctx: &egui::Context) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|input| input.time) + 0.6),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
}
#[test]
fn eq_knobs_edit_saved_parameters_and_multiple_instances_have_distinct_identity() {
    let (mut app, track) = fixture();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Effects, &ctx);
    app.session.project.tracks[0]
        .effects
        .push(daw_core::EffectInstance::equalizer());
    frame(&mut app, &ctx, vec![]);
    for row in 0..3 {
        let point = app.effects_bounds.units[0].knobs[0][row].center();
        let before = eq_settings(&app).bands[0];
        let end = point - Vec2::new(0.0, 30.0);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, &ctx, vec![button(end, false)]);
        let after = eq_settings(&app).bands[0];
        let values = |band: daw_core::EqBandSettings| [band.gain_db, band.frequency_hz, band.q];
        assert!(
            values(after)[row] > values(before)[row],
            "row={row} rect={:?} before={before:?} after={after:?}",
            app.effects_bounds.units[0].knobs[0][row]
        );
        assert!(after.valid());
    }
    let point = app.effects_bounds.units[0].knobs[1][0].center();
    let before = eq_settings(&app).bands[1];
    let end = point - Vec2::new(0.0, 30.0);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    frame(&mut app, &ctx, vec![button(end, false)]);
    let after = eq_settings(&app).bands[1];
    assert!(after.gain_db > before.gain_db);
    assert_eq!(after.frequency_hz, before.frequency_hz);
    assert_eq!(after.q, before.q);
    end_click_sequence(&mut app, &ctx);
    let entry = app.effects_bounds.entry.center();
    click(&mut app, &ctx, entry);
    click(&mut app, &ctx, entry);
    frame(&mut app, &ctx, vec![]);
    let effects = &app.session.project.tracks[0].effects;
    assert_eq!(effects.len(), 2);
    assert_ne!(effects[0].id, effects[1].id);
    assert_eq!(app.effects_bounds.units.len(), 2);
    let other = app.session.project.add_track().unwrap();
    app.select_track(other, false);
    frame(&mut app, &ctx, vec![]);
    assert!(app.effects_bounds.units.is_empty());
    app.select_track(track, false);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.effects_bounds.units.len(), 2);
}

#[test]
fn eq_footer_pairs_match_oscillator_spacing_in_both_modes_and_share_the_status_separator() {
    for compact in [false, true] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.session.project.tracks[0]
            .effects
            .push(daw_core::EffectInstance::equalizer());
        app.session.project.tracks[0].effects_compact = compact;
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Effects, &ctx);
        frame(&mut app, &ctx, vec![]);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        for (index, label) in ["Low", "Mid", "High"].into_iter().enumerate() {
            let button = app.effects_bounds.units[0].bands[index];
            let text = shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                    _ => None,
                })
                .unwrap();
            let text_bounds = text.galley.rect.translate(text.pos.to_vec2());
            assert!((text_bounds.left() - button.right() - 8.0).abs() < 0.51);
            assert!((text_bounds.center().y - button.center().y).abs() < 0.51);
            assert_eq!(button.size(), Vec2::splat(icons::COMPACT_BUTTON_SIZE));
            assert_eq!(
                text.galley.job.sections[0].format.font_id.size,
                panels::HEADER_FONT_SIZE
            );
        }
        // The unit frame reserves its border geometry but omits the bottom stroke.
        let rack = app.effects_bounds.strip;
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.width > 0.0 && rect.stroke.color != egui::Color32::TRANSPARENT && (rect.rect.bottom() - rack.bottom()).abs() < 0.1 && rect.rect.width() > 200.0)));
    }
}
