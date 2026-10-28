use super::instrument_catalog::{click, key};
use super::*;
fn setup(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    app.instrument_catalog.expand_folders();
    let track = app.session.project.add_midi_track().unwrap();
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
fn main_stays_below_scrolled_layers_and_reuses_mix_controls_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = setup(theme);
        let main = app.instrument_ui.main_bounds.row;
        let label = app.instrument_ui.main_bounds.label;
        assert_eq!(main.height(), ROW_HEIGHT);
        assert_eq!(label.width(), TRACK_WIDTH);
        let [gain] = app.instrument_ui.main_bounds.mix;
        assert_eq!(gain.size(), Vec2::splat(knobs::HEADER_SIZE));
        assert_eq!(gain.center(), label.min + Vec2::new(21.0, 47.0));
        assert!(app.instrument_ui.main_bounds.collapse.right() < label.right());
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
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(viewport.center()),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0),
                    modifiers: Default::default(),
                },
            ],
        );
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(app.instrument_catalog_bounds.layer_scroll > 0.0);
        assert_eq!(app.instrument_ui.main_bounds.row, main);
        assert!(app.instrument_catalog_bounds.layer_viewport.bottom() <= main.top() + 1.0);
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
        assert_eq!(app.instrument_ui.main_bounds.row.bottom(), main.bottom());
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
fn main_accepts_shared_modulators_from_catalog_and_rejects_audio_components() {
    let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
    let point = app.instrument_ui.main_bounds.label.min + Vec2::new(25.0, 15.0);
    click(&mut app, &ctx, point, false);
    let main = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .main
        .id;
    assert_eq!(app.instrument_ui.selected_layer, Some(main));
    let toggle = app.instrument_catalog_bounds.toggle.center();
    click(&mut app, &ctx, toggle, false);
    for index in 0..crate::instrument_catalog::ENTRIES.len() {
        let point = app.instrument_catalog_bounds.entries[index].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
    }
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!(instrument.layers[0].oscillators.is_empty() && !instrument.layers[0].amplitude.present);
    assert_eq!(instrument.main.envelopes.len(), 1);
    assert_eq!(instrument.main.lfos.len(), 1);
    assert_eq!(instrument.main.macro_module.controls.len(), 1);
    assert_ne!(
        instrument.main.envelopes[0].mode,
        daw_core::EnvelopeMode::PerNote
    );
    let sources = crate::modulation_panel::sources(Some(instrument));
    assert!(sources.iter().any(|s| s.label == "Output · Envelope 1"));
    assert!(sources.iter().any(|s| s.label == "Output · LFO 1"));
    assert!(sources.iter().any(|s| s.label == "Output · Macro 1"));
    let point = app.instrument_catalog_bounds.entries[2].center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point + Vec2::new(20.0, 0.0))],
    );
    let end = app.instrument_ui.main_bounds.row.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.instrument_catalog_bounds.drop_target, Some(main));
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .main
            .lfos
            .len(),
        2
    );
    let layer = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    app.instrument_ui.selected_layer = Some(layer);
    let point = app.instrument_ui.main_bounds.lfos[0].center();
    click(&mut app, &ctx, point, false);
    assert_eq!(app.instrument_ui.selected_layer, Some(main));
    // A mixed group remains eligible through its Macro entry on a populated Main.
    let oscillator = app.instrument_catalog_bounds.entries[0].center();
    click(&mut app, &ctx, oscillator, false);
    let macro_entry = app.instrument_catalog_bounds.entries[4].center();
    click(&mut app, &ctx, macro_entry, true);
    let before = app.session.project.tracks[0].instrument.clone().unwrap();
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(macro_entry),
            button(macro_entry, true),
        ],
    );
    let end = app.instrument_ui.main_bounds.row.right_center() - Vec2::new(20.0, 0.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.instrument_catalog_bounds.drop_target, Some(main));
    assert_eq!(
        app.session.project.tracks[0].instrument.as_ref().unwrap(),
        &before
    );
    frame(&mut app, &ctx, vec![button(end, false)]);
    let mut expected = before;
    expected.main.macro_module.add();
    // Addition receives a fresh identity; all previous component data stays unchanged.
    let actual = app.session.project.tracks[0].instrument.as_ref().unwrap();
    expected.main.macro_module.controls[1].id = actual.main.macro_module.controls[1].id;
    assert_eq!(actual, &expected);
    app.session.project.validate().unwrap();
}

#[test]
fn empty_main_accepts_catalog_drops_across_its_label_and_body() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for index in 0..crate::instrument_catalog::ENTRIES.len() {
                for label in [false, true] {
                    for fast in [false, true] {
                        let (mut app, ctx, _) = setup(theme);
                        app.timeline_effects_visible = false;
                        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                        i.layers[0].collapsed = true;
                        i.main.collapsed = compact;
                        let main = i.main.id;
                        frame(&mut app, &ctx, vec![]);
                        let toggle = app.instrument_catalog_bounds.toggle.center();
                        click(&mut app, &ctx, toggle, false);
                        app.dirty = false;
                        let original = serde_json::to_value(&app.session.project).unwrap();
                        let source = app.instrument_catalog_bounds.entries[index].center();
                        let row = app.instrument_ui.main_bounds.row;
                        let end = if label {
                            app.instrument_ui.main_bounds.label.center()
                        } else {
                            Pos2::new(row.right() - 20.0, row.center().y)
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
                        assert_eq!(i.main.envelopes.len(), usize::from(index == 1));
                        assert_eq!(i.main.lfos.len(), usize::from(index == 2));
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
fn layer_and_main_selection_strokes_stay_flush_and_paint_after_shared_dividers() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                let (mut app, ctx, _) = setup(theme);
                app.timeline_effects_visible = false;
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
                let outline = shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Rect(r)
                            if r.stroke == theme::selection_outline(&ctx)
                                && label.contains_rect(r.rect) =>
                        {
                            Some((s.clip_rect, r))
                        }
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(outline.1.rect, label);
                assert_eq!(outline.1.stroke_kind, egui::StrokeKind::Inside);
                assert!(
                    outline.0.contains_rect(outline.1.rect),
                    "main {main} compact {compact}: clip {:?}, rect {:?}, label {:?}",
                    outline.0,
                    outline.1.rect,
                    label
                );
                assert_eq!(outline.1.stroke.width, 2.0);
                let selection_index=shapes.iter().position(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect==label && r.stroke==theme::selection_outline(&ctx))).unwrap();
                // Neither the final column divider nor later row fills may cut the stroke.
                assert!(!shapes[selection_index + 1..].iter().any(|s| {
                    match &s.shape {
                        egui::Shape::LineSegment { points, stroke } => {
                            *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                && points[0].x == label.right() - 0.5
                                && points[1].x == label.right() - 0.5
                                && points[0].y < label.center().y
                                && points[1].y > label.center().y
                        }
                        egui::Shape::Rect(r) => {
                            r.fill != egui::Color32::TRANSPARENT
                                && r.rect
                                    .intersect(s.clip_rect)
                                    .contains(label.right_center() - Vec2::new(1.0, 0.0))
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
fn every_first_component_shares_one_divider_with_the_layers_column() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                for main in [false, true] {
                    for entry in crate::instrument_catalog::ENTRIES {
                        if main && !entry.supports_main() {
                            continue;
                        }
                        let (mut app, ctx, _) = setup(theme);
                        app.timeline_effects_visible = false;
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
                            app.instrument_ui.main_bounds.label
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
                            label.right() - 1.0,
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

#[test]
fn main_components_share_the_bottom_separator_with_the_lane_or_status_panel() {
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
                    bus.envelopes.push(daw_core::EnvelopeInstance {
                        collapsed,
                        mode: daw_core::EnvelopeMode::Legato,
                        ..Default::default()
                    });
                    bus.lfos.push(daw_core::LfoInstance {
                        collapsed,
                        ..Default::default()
                    });
                    app.timeline_effects_visible = effects_visible;
                    let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
                    let headers = &app.instrument_ui.main_bounds.envelopes;
                    let x = headers[0].center().x;
                    let main = app.instrument_ui.main_bounds.row;
                    let y = main.bottom() - 0.5;
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
fn fixed_layers_header_keeps_its_separator_above_scrolled_content() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = setup(color_theme);
        app.timeline_effects_visible = false;
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
fn main_top_separator_shares_the_last_layers_bottom_pixel() {
    for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for collapsed in [false, true] {
                let (mut app, ctx, _) = setup(color_theme);
                app.timeline_effects_visible = false;
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
                    ys.iter().all(|y| *y == main.top() - 0.5),
                    "one shared pixel, got {ys:?}"
                );
                let header = app.instrument_ui.main_bounds.envelopes[0];
                let cx = header.center().x;
                assert!(!shapes.iter().any(|s| {
                    match &s.shape {
                        egui::Shape::LineSegment { points, stroke } => {
                            *stroke == Stroke::new(1.0, theme::palette(&ctx).border)
                                && points[0].x < cx
                                && points[1].x > cx
                                && points
                                    .iter()
                                    .all(|p| (p.y - (main.top() + 0.5)).abs() < 0.1)
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
                    app.timeline_effects_visible = false;
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
                                    if r.fill == fill && r.rect.contains(point) =>
                                {
                                    Some(r.rect)
                                }
                                _ => None,
                            })
                            .expect("unit header fill");
                        assert_eq!(
                            cap.top(),
                            main.row.top(),
                            "Main header must not expose the omitted frame edge"
                        );
                        assert_eq!(
                            header.top(),
                            main.row.top() + 1.0,
                            "retain frame and control geometry"
                        );
                        assert_eq!(cap.bottom(), header.top() + panels::HEADER_HEIGHT);
                    }
                    assert_eq!(main.label.height(), panels::unit_height(compact));
                    assert!(!app.dirty);
                }
            }
        }
    }
}
