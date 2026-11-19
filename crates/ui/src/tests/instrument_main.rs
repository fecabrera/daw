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
fn output_header_uses_merge_icon_and_keeps_layer_spacing_and_fixed_bus_identity() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            let (mut app, ctx, _) = setup(theme);
            let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
            instrument.main.collapsed = compact;
            instrument.main.pan = 0.35;
            let id = instrument.main.id;
            let before = instrument.clone();
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            let bounds = &app.instrument_ui.main_bounds;
            let former_pan = bounds.label.min + Vec2::new(53.0, 47.0);
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Circle(circle) if circle.center.distance(former_pan) < 1.0)));
            assert_eq!(
                bounds.icon.size(),
                Vec2::new(icons::HEADER_ICON_WIDTH, toolbars::CONTROL_HEIGHT)
            );
            assert_eq!(bounds.icon.left(), bounds.label.left() + 7.0);
            assert!((bounds.name.left() - bounds.icon.right() - 6.0).abs() < 0.1);
            let uri = icons::Lucide::Merge
                .size(icons::TOOLBAR_ICON_SIZE)
                .color(theme::palette(&ctx).secondary)
                .uri();
            let egui::load::TexturePoll::Ready { texture } = ctx
                .try_load_texture(
                    &uri,
                    egui::TextureOptions::LINEAR,
                    egui::load::SizeHint::Size {
                        width: 16,
                        height: 16,
                        maintain_aspect_ratio: false,
                    },
                )
                .unwrap()
            else {
                panic!("Output merge icon must be ready")
            };
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Mesh(mesh)
                    if mesh.texture_id == texture.id
                        && mesh.vertices.len() == 4
                        && mesh.vertices.iter().all(|vertex| {
                            let local = (vertex.uv.to_vec2() - Vec2::splat(0.5)) * icons::TOOLBAR_ICON_SIZE;
                            let expected = bounds.icon.center() + Vec2::new(-local.y, local.x);
                            vertex.pos.distance(expected) < 0.1
                        })
            )), "Output must render Merge rotated clockwise with its arrow pointing right");
            assert!(
                shapes.iter().any(
                    |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Output")
                )
            );
            assert!(
                !shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Main"))
            );
            let icon = bounds.icon.center();
            click(&mut app, &ctx, former_pan, false);
            click(&mut app, &ctx, icon, false);
            assert_eq!(app.instrument_ui.selected_layer, Some(id));
            assert!(!app.instrument_ui.dragging());
            assert!(!app.dirty);
            assert_eq!(
                app.session.project.tracks[0].instrument.as_ref().unwrap(),
                &before
            );
        }
    }
}
#[test]
fn output_scrolls_with_layers_and_reuses_mix_controls_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = setup(theme);
        let main = app.instrument_ui.main_bounds.row;
        let label = app.instrument_ui.main_bounds.label;
        assert_eq!(main.height(), ROW_HEIGHT);
        assert_eq!(label.width(), TRACK_WIDTH);
        let [gain] = app.instrument_ui.main_bounds.mix;
        assert_eq!(gain.size(), Vec2::splat(knobs::HEADER_SIZE));
        assert_eq!(
            gain.center(),
            Pos2::new(label.right() - 47.0, label.top() + 47.0)
        );
        let bounds = &app.instrument_ui.main_bounds;
        super::instrument_lane_toolbar::assert_toolbar(
            bounds.row,
            bounds.label,
            bounds.toolbar,
            bounds.collapse,
        );
        assert!(
            app.instrument_ui
                .main_bounds
                .mix
                .iter()
                .all(|r| label.contains_rect(*r)),
            "Main mix controls must stay inside its layer panel"
        );
        assert!(
            app.instrument_ui
                .main_bounds
                .mix
                .iter()
                .all(|r| r.top() >= app.instrument_ui.main_bounds.collapse.bottom() + 4.0)
        );
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        instrument
            .layers
            .extend((0..8).map(|_| daw_core::InstrumentLayerSettings::default()));
        frame(&mut app, &ctx, vec![]);
        let viewport = app.instrument_catalog_bounds.layer_viewport;
        let main = app.instrument_ui.main_bounds.row;
        assert!(viewport.bottom() <= main.top());
        frame(
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
        );
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(app.instrument_catalog_bounds.layer_scroll > 0.0);
        let scrolled_output = app.instrument_ui.main_bounds.row;
        assert!(viewport.contains_rect(scrolled_output));
        assert!(scrolled_output.top() < main.top());
        let gain = app.instrument_ui.main_bounds.mix[0].center();
        click(&mut app, &ctx, gain, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .gain_db
                > 0.0
        );
        assert!(app.dirty);
        let ids: Vec<_> = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers
            .iter()
            .map(|l| l.id)
            .collect();
        let collapse = app.instrument_ui.main_bounds.collapse.center();
        click(&mut app, &ctx, collapse, false);
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .collapsed
        );
        assert_eq!(
            app.instrument_ui.main_bounds.row.top(),
            scrolled_output.top()
        );
        assert_eq!(
            app.instrument_ui.main_bounds.row.height(),
            panels::unit_height(false)
        );
        let bounds = &app.instrument_ui.main_bounds;
        assert!(bounds.mix.iter().all(|r| bounds.label.contains_rect(*r)));
        assert!(
            bounds
                .mix
                .iter()
                .all(|r| r.top() >= bounds.collapse.bottom() + 4.0)
        );
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers
                .iter()
                .map(|l| l.id)
                .collect::<Vec<_>>(),
            ids
        );
    }
}
#[test]
fn output_accepts_macros_and_rejects_sources_and_dedicated_lane_providers() {
    let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
    let main = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .main
        .id;
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
    assert!(i.layers[0].oscillators.is_empty() && !i.layers[0].amplitude.present);
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
fn layer_and_main_selection_uses_backgrounds_without_outlines() {
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
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let label = if main {
                    app.instrument_ui.main_bounds.label
                } else {
                    app.midi_editor.layer_bounds[&layer].label
                };
                assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == label && r.stroke == theme::selection_outline(&ctx))));
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == label && r.fill == theme::selected_track_header_background(&ctx))));
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
fn output_row_draws_one_bottom_divider_across_the_canvas() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                for effects_visible in [false, true] {
                    let (mut app, ctx, _) = setup(theme);
                    let bus = &mut app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .main;
                    bus.collapsed = compact;
                    let mut effect = daw_core::EffectInstance::drive();
                    if let daw_core::EffectSettings::Drive(settings) = &mut effect.unit {
                        settings.collapsed = collapsed;
                    }
                    bus.effects.push(effect);
                    app.effects_lane_open = effects_visible;
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 1100.0));
                    let x = app.instrument_ui.effect_slots[0].2.center().x;
                    let main = app.instrument_ui.main_bounds.row;
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
                        app.instrument_ui.main_bounds.label.height(),
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
fn beat_parent_output_and_macros_have_a_bottom_divider() {
    use crate::instrument_workspace::InstrumentSubview;

    let mut app = DawUi::default();
    let track = app.session.project.add_beat_track().unwrap();
    let ctx = context();
    app.select_track(track, false);
    app.set_view_mode(ViewMode::Instrument, &ctx);
    app.effects_lane_open = false;
    for subview in [InstrumentSubview::Effects, InstrumentSubview::Modulation] {
        app.set_instrument_subview(subview, &ctx);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 900.0));
        let row = if subview == InstrumentSubview::Effects {
            app.instrument_ui.main_bounds.row
        } else {
            app.instrument_ui.macro_bounds.row
        };
        assert_eq!(row.height(), panels::unit_height(false));
        assert_final_divider(&shapes, row, &ctx);
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

#[test]
fn main_top_separator_shares_the_last_layers_bottom_pixel() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                let (mut app, ctx, _) = setup(color_theme);
                app.effects_lane_open = false;
                let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                i.layers
                    .extend((0..4).map(|_| daw_core::InstrumentLayerSettings::default()));
                i.main.collapsed = compact;
                i.main.envelopes.push(daw_core::EnvelopeInstance {
                    collapsed,
                    mode: daw_core::EnvelopeMode::Legato,
                    ..Default::default()
                });
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
                            delta: Vec2::new(0.0, -10000.0),
                            modifiers: Default::default(),
                        },
                    ],
                );
                for _ in 0..12 {
                    frame(&mut app, &ctx, vec![]);
                }
                let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                let main = app.instrument_ui.main_bounds.row;
                let x = app.instrument_ui.main_bounds.label.center().x;
                let ys = shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::LineSegment { points, stroke }
                            if *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                && points[0].x < x
                                && points[1].x > x
                                && points[0].y == points[1].y
                                && (points[0].y - main.top()).abs() <= 2.0 =>
                        {
                            Some(points[0].y)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(!ys.is_empty());
                assert!(
                    ys.iter().all(|y| *y == main.top() + 0.5),
                    "one shared pixel, got {ys:?}"
                );
                let cx = app.instrument_ui.main_bounds.label.right() + 12.0;
                assert!(!shapes.iter().any(|s| {
                    match &s.shape {
                        egui::Shape::LineSegment { points, stroke } => {
                            *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                && points[0].x < cx
                                && points[1].x > cx
                                && points
                                    .iter()
                                    .all(|p| (p.y - (main.top() - 0.5)).abs() < 0.1)
                        }
                        egui::Shape::Rect(r) => {
                            r.stroke.width > 0.0
                                && r.stroke.color != egui::Color32::TRANSPARENT
                                && r.rect.left() < cx
                                && r.rect.right() > cx
                                && (r.rect.top() - main.top()).abs() < 1.0
                        }
                        _ => false,
                    }
                }));
                assert!(!app.dirty);
            }
        }
    }
}

#[test]
fn main_unit_header_fills_reach_the_shared_top_edge_without_moving_controls() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                for enabled in [false, true] {
                    let (mut app, ctx, _) = setup(color_theme);
                    app.effects_lane_open = false;
                    let bus = &mut app.session.project.tracks[0]
                        .instrument
                        .as_mut()
                        .unwrap()
                        .main;
                    bus.collapsed = compact;
                    bus.envelopes.push(daw_core::EnvelopeInstance {
                        enabled,
                        collapsed,
                        mode: daw_core::EnvelopeMode::Legato,
                        ..Default::default()
                    });
                    bus.lfos.push(daw_core::LfoInstance {
                        enabled,
                        collapsed,
                        ..Default::default()
                    });
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    let main = &app.instrument_ui.main_bounds;
                    let fill = theme::unit_header_fill(&ctx, enabled);
                    for header in main.envelopes.iter().chain(&main.lfos) {
                        let point = Pos2::new(
                            header.center().x,
                            header.top() + panels::HEADER_HEIGHT * 0.5,
                        );
                        let cap = shapes
                            .iter()
                            .find_map(|s| match &s.shape {
                                egui::Shape::Rect(r)
                                    if r.fill == fill
                                        && r.rect == *header
                                        && r.rect.contains(point) =>
                                {
                                    Some(r.rect)
                                }
                                _ => None,
                            })
                            .expect("unit header fill");
                        assert_eq!(
                            cap.top(),
                            main.row.top() + panels::UNIT_VERTICAL_MARGIN,
                            "Main header must not expose the omitted frame edge"
                        );
                        assert_eq!(
                            header.top(),
                            main.row.top() + panels::UNIT_VERTICAL_MARGIN,
                            "retain frame and control geometry"
                        );
                        assert_eq!(cap, *header);
                    }
                    assert_eq!(main.label.height(), panels::unit_height(compact));
                    assert!(!app.dirty);
                }
            }
        }
    }
}
