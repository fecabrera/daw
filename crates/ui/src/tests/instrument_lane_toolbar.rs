use super::*;

pub(super) fn assert_toolbar(row: Rect, label: Rect, rail: Rect, button: Rect) {
    assert!((rail.width() - lane_toolbar::WIDTH).abs() < 0.01);
    assert!(
        (rail.right() - row.right()).abs() < 0.75,
        "{rail:?}, row {row:?}"
    );
    assert!((rail.top() - row.top()).abs() < 0.75);
    assert!((rail.bottom() - row.bottom()).abs() < 0.75);
    assert!(rail.left() >= label.right());
    assert!(rail.contains_rect(button));
    assert_eq!(button.size(), Vec2::splat(icons::TOOLBAR_BUTTON_SIZE));
    assert!((button.left() - rail.left() - 7.0).abs() < 0.75);
    assert!((button.top() - rail.top() - 7.0).abs() < 0.75);
}

#[test]
fn each_lane_keeps_its_toolbar_fixed_outside_scrolling_components() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            let mut app = DawUi::default();
            let track = app.session.project.add_midi_track().unwrap();
            let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
            instrument.layers[0]
                .oscillators
                .resize_with(12, Default::default);
            instrument
                .layers
                .push(daw_core::InstrumentLayerSettings::default());
            instrument.main.collapsed = true;
            let layer = instrument.layers[0].id;
            let peer = instrument.layers[1].clone();
            let main = instrument.main.clone();
            app.select_track(track, false);
            let ctx = context();
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            ctx.set_pixels_per_point(scale);
            app.set_view_mode(ViewMode::Instrument, &ctx);
            let size = Vec2::new(1280.0, 1000.0);
            for _ in 0..2 {
                frame_shapes(&mut app, &ctx, vec![], size);
            }
            let before = app.session.project.clone();
            let bounds = &app.midi_editor.layer_bounds[&layer];
            assert_toolbar(bounds.row, bounds.label, bounds.toolbar, bounds.collapse);
            let rail = bounds.toolbar;
            let button_rect = bounds.collapse;
            let source_x = bounds.oscillators[0].left();
            let viewport = app
                .instrument_ui
                .component_viewports
                .iter()
                .find(|(id, _)| *id == layer)
                .unwrap()
                .1;
            assert!(viewport.right() <= rail.left());
            frame_shapes(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(viewport.center()),
                    egui::Event::MouseWheel {
                        phase: egui::TouchPhase::Move,
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(-500.0, 0.0),
                        modifiers: Default::default(),
                    },
                ],
                size,
            );
            for _ in 0..10 {
                frame_shapes(&mut app, &ctx, vec![], size);
            }
            let bounds = &app.midi_editor.layer_bounds[&layer];
            assert!(bounds.oscillators[0].left() < source_x - 20.0);
            assert_eq!(bounds.toolbar, rail);
            assert_eq!(bounds.collapse, button_rect);
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            let point = button_rect.center();
            frame_shapes(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(point), button(point, true)],
                size,
            );
            assert!(
                !app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers[0]
                    .collapsed
            );
            assert!(!app.instrument_ui.has_layer_gesture());
            frame_shapes(&mut app, &ctx, vec![button(point, false)], size);
            frame_shapes(&mut app, &ctx, vec![], size);
            let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
            assert!(instrument.layers[0].collapsed);
            assert_eq!(instrument.layers[1], peer);
            assert_eq!(instrument.main, main);
            let bounds = &app.midi_editor.layer_bounds[&layer];
            assert_toolbar(bounds.row, bounds.label, bounds.toolbar, bounds.collapse);
            assert_eq!(bounds.row.height(), ROW_HEIGHT);
            let output = &app.instrument_ui.main_bounds;
            assert_toolbar(output.row, output.label, output.toolbar, output.collapse);
            for (id, viewport) in &app.instrument_ui.component_viewports {
                let toolbar = if *id == main.id {
                    output.toolbar
                } else if let Some(lane) = app.instrument_ui.modulation_lane_bounds.get(id) {
                    lane.toolbar
                } else {
                    app.midi_editor.layer_bounds[id].toolbar
                };
                assert!(
                    viewport.right() <= toolbar.left(),
                    "component/drop viewport must exclude its toolbar"
                );
            }
            assert!(app.dirty);
        }
    }
}

#[test]
fn lane_toolbars_reject_catalog_drops_and_dialog_blocked_toggles() {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.select_track(track, false);
    app.instrument_catalog.expand_folders();
    app.instrument_catalog_visible = true;
    let ctx = context();
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    let layer = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    for output in [false, true] {
        let rail = if output {
            app.instrument_ui.main_bounds.toolbar
        } else {
            app.midi_editor.layer_bounds[&layer].toolbar
        };
        let before = app.session.project.clone();
        let source = app.instrument_catalog_bounds.entries[1].center();
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source), button(source, true)],
        );
        assert!(app.component_drag.is_some());
        let point = rail.center();
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
        assert_eq!(app.instrument_catalog_bounds.drop_target, None);
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        let point = if output {
            app.instrument_ui.main_bounds.collapse.center()
        } else {
            app.midi_editor.layer_bounds[&layer].collapse.center()
        };
        app.settings_open = true;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        frame(&mut app, &ctx, vec![button(point, false)]);
        assert_eq!(app.session.project, before);
        app.settings_open = false;
        frame(&mut app, &ctx, vec![]);
    }
}

#[test]
fn selected_effect_outlines_stop_at_their_lane_toolbar() {
    for output in [false, true] {
        let mut app = DawUi::default();
        let track = app.session.project.add_midi_track().unwrap();
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let (owner, effects) = if output {
            (instrument.main.id, &mut instrument.main.effects)
        } else {
            (instrument.layers[0].id, &mut instrument.layers[0].effects)
        };
        effects.extend((0..8).map(|_| daw_core::EffectInstance::drive()));
        app.select_track(track, false);
        let ctx = context();
        app.set_view_mode(ViewMode::Instrument, &ctx);
        frame(&mut app, &ctx, vec![]);
        let viewport = app
            .instrument_ui
            .component_viewports
            .iter()
            .find(|(id, _)| *id == owner)
            .unwrap()
            .1;
        let (_, effect, rect) = app
            .instrument_ui
            .effect_slots
            .iter()
            .find(|(id, _, r)| {
                *id == owner && r.left() < viewport.right() && r.right() > viewport.right()
            })
            .copied()
            .expect("partly visible effect at the toolbar edge");
        let point = Pos2::new(
            (rect.left() + 24.0 + viewport.right()) * 0.5,
            rect.top() + 12.0,
        );
        super::instrument_catalog::click(&mut app, &ctx, point, false);
        assert!(app.selected_effects.contains(&effect));
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let outlines: Vec<_> = shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx))).collect();
        assert!(!outlines.is_empty());
        assert!(
            outlines
                .iter()
                .all(|shape| shape.clip_rect.right() <= viewport.right())
        );
    }
}

#[test]
fn output_toolbar_shares_one_top_separator_with_the_whole_pinned_row() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5] {
            for compact in [false, true] {
                for effects_visible in [false, true] {
                    let mut app = DawUi::default();
                    let track = app.session.project.add_midi_track().unwrap();
                    let bus = &mut app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .main;
                    bus.collapsed = compact;
                    bus.envelopes.push(daw_core::EnvelopeInstance {
                        mode: daw_core::EnvelopeMode::Legato,
                        ..Default::default()
                    });
                    app.timeline_effects_visible = effects_visible;
                    app.select_track(track, false);
                    let ctx = context();
                    app.set_settings(
                        &ctx,
                        settings::AppSettings {
                            theme,
                            ..Default::default()
                        },
                    );
                    ctx.set_pixels_per_point(scale);
                    app.set_view_mode(ViewMode::Instrument, &ctx);
                    let size = Vec2::new(1280.0, 1000.0);
                    for _ in 0..2 {
                        frame_shapes(&mut app, &ctx, vec![], size);
                    }
                    let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                    let bounds = &app.instrument_ui.main_bounds;
                    for x in [
                        bounds.label.center().x,
                        bounds.envelopes[0].center().x,
                        bounds.toolbar.left() - 12.0,
                        bounds.toolbar.center().x,
                    ] {
                        let lines: Vec<_> = shapes
                            .iter()
                            .filter_map(|s| match &s.shape {
                                egui::Shape::LineSegment { points, stroke }
                                    if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                        && points[0].x < x
                                        && points[1].x > x
                                        && points[0].y == points[1].y
                                        && (points[0].y - bounds.row.top()).abs() <= 1.0 =>
                                {
                                    Some(points[0].y)
                                }
                                _ => None,
                            })
                            .collect();
                        assert!(!lines.is_empty(), "missing Output separator at {x}");
                        assert!(
                            lines
                                .iter()
                                .all(|y| (*y - (bounds.row.top() - 0.5)).abs() < 0.01),
                            "duplicate Output edge at {x}: {lines:?}"
                        );
                    }
                }
            }
        }
    }
}
