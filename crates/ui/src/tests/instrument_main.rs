use super::instrument_catalog::{click, key};
use super::*;
fn setup(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    app.instrument_catalog.expand_folders();
    let track = app.session.project.add_midi_track().unwrap();
    compact_instrument_buses(app.session.project.instrument_settings_mut(track).unwrap());
    app.select_track(track, false);
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.set_instrument_subview(
        crate::instrument_workspace::InstrumentSubview::Effects,
        &ctx,
    );
    frame(&mut app, &ctx, vec![]);
    (app, ctx, track)
}
#[test]
fn modulation_lanes_use_the_same_title_panel_as_layer_and_effects() {
    use crate::instrument_workspace::InstrumentSubview;

    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, track) = setup(theme);
        let size = Vec2::new(1280.0, 950.0);
        app.set_instrument_subview(InstrumentSubview::Layer, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        let title_position = |shapes: &[egui::epaint::ClippedShape], title: &str, row: Rect| {
            shapes
                .iter()
                .find_map(|item| match &item.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == title && row.contains(text.pos) =>
                    {
                        Some(text.pos - row.min)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing title {title}"))
        };
        let (layer, lfo, envelope) = {
            let settings = app.session.project.instrument_settings(track).unwrap();
            (
                settings.layers[0].id,
                settings.lfo_lane.id,
                settings.envelope_lane.id,
            )
        };
        let expected = title_position(&shapes, "Layer", app.midi_editor.layer_bounds[&layer].row);
        for (subview, owner, title) in [
            (InstrumentSubview::Lfos, Some(lfo), "LFO"),
            (InstrumentSubview::Envelopes, Some(envelope), "Envelope"),
            (InstrumentSubview::Macros, None, "Macros"),
        ] {
            app.set_instrument_subview(subview, &ctx);
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let bounds = owner.map_or(&app.instrument_ui.macro_bounds, |owner| {
                &app.instrument_ui.modulation_lane_bounds[&owner]
            });
            let label = bounds.label;
            assert_eq!(label.width(), instrument_panel::LANE_TITLE_WIDTH);
            let header_bottom = app.instrument_catalog_bounds.track_column.top() + toolbars::HEIGHT;
            assert_eq!(bounds.row.top(), header_bottom);
            assert!(shapes.iter().any(|item| matches!(&item.shape,
                egui::Shape::LineSegment { points, stroke }
                if stroke.color == theme::palette(&ctx).border
                    && points[0] == Pos2::new(bounds.row.left(), header_bottom - 0.5)
                    && points[1] == Pos2::new(bounds.row.right(), header_bottom - 0.5))));
            let title = title_position(&shapes, title, bounds.row);
            assert_eq!(title.x, expected.x);
            assert_eq!(title.y, expected.y);
            assert!(!shapes.iter().any(|item| match &item.shape {
                egui::Shape::Mesh(mesh) => mesh.vertices.iter().any(|vertex| {
                    label.contains(vertex.pos) && vertex.pos.y < label.top() + 28.0
                }),
                _ => false,
            }));
        }
        assert!(!app.dirty);
    }
}

#[test]
fn macro_drags_keep_empty_layers_hidden_and_allow_drops_on_numbered_tabs() {
    use crate::instrument_workspace::InstrumentSubview;

    for drop_on_tab in [false, true] {
        let (mut app, ctx, track) = setup(settings::ColorTheme::Dark);
        app.set_instrument_subview(InstrumentSubview::Macros, &ctx);
        app.set_effects_lane_visible(false);
        frame(&mut app, &ctx, vec![]);
        let before = app.session.project.instrument_snapshot(track).unwrap();
        let layer = before.layers[0].id;
        let source = app.instrument_catalog_bounds.entries[4].center();
        let output = app.instrument_ui.macro_bounds.label.center();
        let target = if drop_on_tab {
            app.instrument_catalog_bounds.layer_tabs[&layer].center()
        } else {
            output
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(source), button(source, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(output)]);
        assert!(app.component_drag.as_ref().is_some_and(|drag| drag.active));
        assert!(app.midi_editor.layer_bounds.is_empty());
        assert_eq!(
            app.instrument_catalog_bounds.drop_target,
            Some(before.main.id)
        );
        assert_eq!(
            app.session.project.instrument_settings(track),
            Some(&before)
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
        frame(&mut app, &ctx, vec![button(target, false)]);
        frame(&mut app, &ctx, vec![]);
        let after = app.session.project.instrument_settings(track).unwrap();
        assert_eq!(
            after.layers[0].macro_module.controls.len(),
            usize::from(drop_on_tab)
        );
        assert_eq!(
            after.main.macro_module.controls.len(),
            usize::from(!drop_on_tab)
        );
        assert_eq!(
            app.midi_editor.layer_bounds.contains_key(&layer),
            drop_on_tab
        );
        assert_eq!(app.instrument_subview, InstrumentSubview::Macros);
        assert!(app.component_drag.is_none());
    }
}

#[test]
fn macros_title_panel_has_no_audio_controls_in_either_row_size() {
    use crate::instrument_workspace::InstrumentSubview;

    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, track) = setup(theme);
        let output = &mut app
            .session
            .project
            .instrument_settings_mut(track)
            .unwrap()
            .main;
        output.gain_db = -6.0;
        output.pan = 0.25;
        output.macro_module.add();
        app.set_instrument_subview(InstrumentSubview::Macros, &ctx);
        let size = Vec2::new(1280.0, 800.0);
        for compact in [true, false] {
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            let bounds = &app.instrument_ui.macro_bounds;
            assert_eq!(bounds.row.height(), panels::unit_height(compact));
            super::instrument_lane_toolbar::assert_toolbar(
                bounds.row,
                bounds.label,
                bounds.toolbar,
                bounds.collapse,
            );
            let below_title =
                Rect::from_min_max(bounds.label.min + Vec2::new(0.0, 29.0), bounds.label.max);
            assert!(!shapes.iter().any(|item| match &item.shape {
                egui::Shape::Rect(shape) => below_title.contains_rect(shape.rect),
                egui::Shape::Circle(shape) => below_title.contains(shape.center),
                _ => false,
            }));
            assert!(
                app.instrument_ui
                    .component_slots
                    .iter()
                    .any(|(_, unit, _)| { *unit == daw_core::InstrumentComponent::Macros })
            );
            let output = &app.session.project.instrument_settings(track).unwrap().main;
            assert_eq!(output.gain_db, -6.0);
            assert_eq!(output.pan, 0.25);
            if compact {
                let collapse = bounds.collapse.center();
                click(&mut app, &ctx, collapse, false);
            }
        }
    }
}
#[test]
fn output_accepts_macros_and_rejects_sources_and_dedicated_lane_providers() {
    let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    let main = instrument.main.id;
    let layers = instrument.layers.clone();
    app.instrument_ui.select_layer(main);
    app.set_instrument_subview(
        crate::instrument_workspace::InstrumentSubview::Modulation,
        &ctx,
    );
    frame(&mut app, &ctx, vec![]);
    for index in [1, 2, 4] {
        let point = app.instrument_catalog_bounds.entries[index].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
    }
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!(i.main.envelopes.is_empty() && i.main.lfos.is_empty());
    assert_eq!(i.main.macro_module.controls.len(), 1);
    assert_eq!(i.layers, layers);
    assert!(
        crate::modulation_panel::sources(Some(i))
            .iter()
            .any(|s| s.label == "Output · Macro 1")
    );
}

#[test]
fn empty_main_accepts_catalog_drops_across_its_label_and_body() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for index in [1, 2, 4] {
                for label in [false, true] {
                    for fast in [false, true] {
                        let (mut app, ctx, _) = setup(theme);
                        app.effects_lane_open = false;
                        app.set_instrument_subview(
                            crate::instrument_workspace::InstrumentSubview::Modulation,
                            &ctx,
                        );
                        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                        i.layers[0].collapsed = true;
                        i.main.collapsed = compact;
                        let main = i.main.id;
                        frame(&mut app, &ctx, vec![]);
                        app.dirty = false;
                        let original = serde_json::to_value(&app.session.project).unwrap();
                        let source = app.instrument_catalog_bounds.entries[index].center();
                        let row = app.instrument_ui.macro_bounds.row;
                        let end = if label {
                            app.instrument_ui.macro_bounds.label.center()
                        } else {
                            Pos2::new(
                                app.instrument_ui.macro_bounds.toolbar.left() - 20.0,
                                row.center().y,
                            )
                        };
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(source), button(source, true)],
                        );
                        assert!(app.component_drag.is_some());
                        let supported = crate::instrument_catalog::ENTRIES[index].supports_main();
                        if !fast {
                            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                            assert_eq!(
                                app.instrument_catalog_bounds.drop_target,
                                supported.then_some(main),
                                "index {index}, compact {compact}, label {label}"
                            );
                            assert_eq!(
                                serde_json::to_value(&app.session.project).unwrap(),
                                original
                            );
                            assert!(!app.dirty);
                        }
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end), button(end, false)],
                        );
                        assert!(app.component_drag.is_none());
                        assert_eq!(app.dirty, supported);
                        let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                        assert_eq!(i.main.envelopes.len(), 0);
                        assert_eq!(i.main.lfos.len(), 0);
                        assert_eq!(i.main.macro_module.controls.len(), usize::from(index == 4));
                        if let Some(e) = i.main.envelopes.first() {
                            assert_eq!(e.mode, daw_core::EnvelopeMode::Legato);
                        }
                        if !supported {
                            assert_eq!(
                                serde_json::to_value(&app.session.project).unwrap(),
                                original
                            );
                        } else {
                            assert_eq!(app.instrument_ui.selected_layer, Some(main));
                        }
                        app.session.project.validate().unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn selected_mix_headers_use_highlight_and_passive_titles_use_panel_fill() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                let (mut app, ctx, _) = setup(theme);
                app.effects_lane_open = false;
                let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                i.layers[0].collapsed = compact;
                i.main.collapsed = compact;
                let layer = i.layers[0].id;
                app.instrument_ui.selected_layer = Some(if main { i.main.id } else { layer });
                if main {
                    app.set_instrument_subview(
                        crate::instrument_workspace::InstrumentSubview::Modulation,
                        &ctx,
                    );
                }
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let label = if main {
                    app.instrument_ui.macro_bounds.label
                } else {
                    app.midi_editor.layer_bounds[&layer].label
                };
                assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == label && r.stroke == theme::selection_outline(&ctx))));
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == label && r.fill == if main { theme::palette(&ctx).panel } else { theme::selected_track_header_background(&ctx) })));
                assert!(!app.dirty);
            }
        }
    }
}

#[test]
fn every_first_component_shares_one_divider_with_the_layers_column() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                for main in [false, true] {
                    for entry in crate::instrument_catalog::ENTRIES {
                        if entry.type_id == "daw.envelope" || entry.type_id == "daw.lfo" {
                            continue;
                        }
                        if main && !entry.supports_main() {
                            continue;
                        }
                        let (mut app, ctx, _) = setup(theme);
                        app.set_instrument_subview(
                            if entry.type_id == "daw.macro" {
                                crate::instrument_workspace::InstrumentSubview::Modulation
                            } else {
                                crate::instrument_workspace::InstrumentSubview::Sources
                            },
                            &ctx,
                        );

                        app.effects_lane_open = false;
                        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
                        let owner = if main {
                            instrument.main.id
                        } else {
                            instrument.layers[0].id
                        };
                        crate::instrument_catalog::add_components(
                            instrument,
                            owner,
                            &[entry.type_id],
                        );
                        let layer = &mut instrument.layers[0];
                        layer.collapsed = compact;
                        layer.oscillator_collapsed = collapsed;
                        if let Some(sampler) = &mut layer.sampler {
                            sampler.collapsed = collapsed;
                        }
                        layer.amplitude.collapsed = collapsed;
                        layer.macro_module.collapsed = collapsed;
                        for envelope in &mut layer.envelopes {
                            envelope.collapsed = collapsed;
                        }
                        for lfo in &mut layer.lfos {
                            lfo.collapsed = collapsed;
                        }
                        instrument.main.collapsed = compact;
                        instrument.main.macro_module.collapsed = collapsed;
                        for envelope in &mut instrument.main.envelopes {
                            envelope.collapsed = collapsed;
                        }
                        for lfo in &mut instrument.main.lfos {
                            lfo.collapsed = collapsed;
                        }
                        let saved = serde_json::to_value(&app.session.project).unwrap();
                        app.dirty = false;
                        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                        let label = if main {
                            app.instrument_ui.macro_bounds.label
                        } else {
                            app.midi_editor.layer_bounds[&owner].label
                        };
                        let first = app
                            .instrument_ui
                            .component_slots
                            .iter()
                            .find(|(id, _, _)| *id == owner)
                            .unwrap()
                            .2;
                        assert_eq!(
                            first.left(),
                            label.right() + panels::UNIT_GAP,
                            "{} main {main} compact {compact} collapsed {collapsed}",
                            entry.name
                        );
                        let point = Pos2::new(label.right() - 0.5, label.center().y);
                        let border = Stroke::new(1.0, theme::palette(&ctx).border);
                        let strokes = shapes
                            .iter()
                            .filter(|shape| {
                                if !shape.clip_rect.contains(point) {
                                    return false;
                                }
                                match &shape.shape {
                                    egui::Shape::LineSegment { points, stroke } => {
                                        *stroke == border
                                            && points[0].x == points[1].x
                                            && (points[0].x - point.x).abs() <= 1.0
                                            && points[0].y <= point.y
                                            && points[1].y >= point.y
                                    }
                                    egui::Shape::Rect(rect) => {
                                        rect.stroke == border
                                            && rect.rect.y_range().contains(point.y)
                                            && ((rect.rect.left() - point.x).abs() <= 1.0
                                                || (rect.rect.right() - point.x).abs() <= 1.0)
                                    }
                                    _ => false,
                                }
                            })
                            .count();
                        assert_eq!(
                            strokes, 1,
                            "{} main {main} compact {compact} collapsed {collapsed}",
                            entry.name
                        );
                        assert_eq!(serde_json::to_value(&app.session.project).unwrap(), saved);
                        assert!(!app.dirty);
                    }
                }
            }
        }
    }
}

fn assert_final_divider(shapes: &[egui::epaint::ClippedShape], row: Rect, ctx: &egui::Context) {
    let y = row.bottom() - 0.5;
    let stroke = Stroke::new(1.0, theme::palette(ctx).border);
    let dividers = shapes.iter().filter(|shape| {
        shape.clip_rect.contains(Pos2::new(row.center().x, y))
            && matches!(&shape.shape, egui::Shape::LineSegment { points, stroke: line }
                if *line == stroke
                    && *points == [Pos2::new(row.left(), y), Pos2::new(row.right(), y)])
    });
    assert_eq!(dividers.count(), 1, "final row {row:?}");
}

#[test]
fn last_layer_effects_row_draws_one_bottom_divider_across_the_canvas() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                for effects_visible in [false, true] {
                    let (mut app, ctx, _) = setup(theme);
                    let layer = &mut app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .layers[0];
                    layer.collapsed = compact;
                    let mut effect = daw_core::EffectInstance::drive();
                    if let daw_core::EffectSettings::Drive(settings) = &mut effect.unit {
                        settings.collapsed = collapsed;
                    }
                    layer.effects.push(effect);
                    app.effects_lane_open = effects_visible;
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 1100.0));
                    let owner = app.session.project.tracks[0]
                        .instrument
                        .as_ref()
                        .unwrap()
                        .layers[0]
                        .id;
                    let x = app.instrument_ui.effect_slots[0].2.center().x;
                    let main = app.midi_editor.layer_bounds[&owner].row;
                    let y = main.bottom() - 0.5;
                    assert_final_divider(&shapes, main, &ctx);
                    assert!(
                        !shapes.iter().any(|s| match &s.shape {
                            egui::Shape::Rect(r) =>
                                r.stroke.width > 0.0
                                    && r.stroke.color != egui::Color32::TRANSPARENT
                                    && r.rect.left() < x
                                    && r.rect.right() > x
                                    && (r.rect.bottom() - y).abs() < 1.0,
                            egui::Shape::LineSegment { points, stroke } =>
                                stroke.color == theme::palette(&ctx).border
                                    && points[0].x < x
                                    && points[1].x > x
                                    && points[0].x > main.left()
                                    && points.iter().all(|p| (p.y - y).abs() < 0.4),
                            _ => false,
                        }),
                        "unit must omit its bottom edge: compact {compact}, collapsed {collapsed}, effects {effects_visible}"
                    );
                    // The left panel remains the same height as its components.
                    assert_eq!(
                        app.midi_editor.layer_bounds[&owner].label.height(),
                        panels::unit_height(compact)
                    );
                    assert!(!app.dirty);
                }
            }
        }
    }
}

#[test]
fn modulation_divider_follows_the_last_visible_row() {
    use crate::instrument_workspace::InstrumentSubview;

    for has_layer_macros in [false, true] {
        let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
        app.effects_lane_open = false;
        app.set_instrument_subview(InstrumentSubview::Modulation, &ctx);
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let layer = &mut instrument.layers[0];
        layer.collapsed = false;
        if has_layer_macros {
            layer.macro_module.add();
        }
        let layer = layer.id;
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 900.0));
        let last = if has_layer_macros {
            app.midi_editor.layer_bounds[&layer].row
        } else {
            app.instrument_ui.macro_bounds.row
        };
        assert_final_divider(&shapes, last, &ctx);
        assert!(!app.dirty);
    }
}

#[test]
fn fixed_layers_header_keeps_its_separator_above_scrolled_content() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = setup(color_theme);
        app.effects_lane_open = false;
        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
        i.layers
            .extend((0..5).map(|_| daw_core::InstrumentLayerSettings::default()));
        for l in &mut i.layers {
            l.oscillators.push(Default::default());
        }
        frame(&mut app, &ctx, vec![]);
        let viewport = app.instrument_catalog_bounds.layer_viewport;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(viewport.center()),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -120.0),
                    modifiers: Default::default(),
                },
            ],
        );
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert!(app.instrument_catalog_bounds.layer_scroll > 0.0);
        let header = app.midi_editor.instrument_header_bounds;
        let y = header.bottom() - 0.5;
        let index=shapes.iter().position(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke} if *stroke==Stroke::new(1.0,theme::palette(&ctx).border) && points[0].y==y && points[1].y==y && points[0].x==header.left() && points[1].x>header.right())).expect("full-width pinned header separator");
        // Scrolled row fills must be behind the fixed line, including the shared top pixel.
        assert!(!shapes[index + 1..].iter().any(|s| {
            match &s.shape {
                egui::Shape::Rect(r) => {
                    r.fill != egui::Color32::TRANSPARENT
                        && r.rect
                            .intersect(s.clip_rect)
                            .contains(Pos2::new(header.left() + 20.0, y))
                }
                _ => false,
            }
        }));
        assert!(!app.dirty);
    }
}

#[test]
fn expanded_modulation_lanes_share_the_regular_layer_scroll_viewport() {
    let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
    app.set_instrument_subview(
        crate::instrument_workspace::InstrumentSubview::Modulation,
        &ctx,
    );
    let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
    instrument.lfo_lane.collapsed = false;
    instrument.envelope_lane.collapsed = false;
    let lfo = instrument.lfo_lane.id;
    let envelope = instrument.envelope_lane.id;
    let size = Vec2::new(1000.0, 600.0);
    frame_sized(&mut app, &ctx, vec![], size);
    let rows = |app: &DawUi| {
        [
            app.instrument_ui.modulation_lane_bounds[&lfo].row,
            app.instrument_ui.modulation_lane_bounds[&envelope].row,
            app.instrument_ui.macro_bounds.row,
        ]
    };
    let before = rows(&app);
    let viewport = app.instrument_catalog_bounds.layer_viewport;
    assert!(before[2].top() >= viewport.bottom());
    for pair in before.windows(2) {
        assert_eq!(pair[0].bottom() - 1.0, pair[1].top());
    }
    frame_sized(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(viewport.center()),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -10000.0),
                modifiers: Default::default(),
            },
        ],
        size,
    );
    for _ in 0..10 {
        frame_sized(&mut app, &ctx, vec![], size);
    }
    let after = rows(&app);
    let offset = Vec2::new(0.0, after[2].top() - before[2].top());
    assert!(offset.y < 0.0);
    for (before, after) in before.into_iter().zip(after) {
        assert_eq!(before.translate(offset), after);
    }
    assert!(viewport.contains_rect(after[2]));
    assert_eq!(app.instrument_catalog_bounds.layer_viewport, viewport);
    assert!(!app.dirty);
}
