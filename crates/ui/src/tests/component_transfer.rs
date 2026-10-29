use super::*;
use daw_core::InstrumentComponent;

fn setup(theme: settings::ColorTheme, compact: bool) -> (DawUi, egui::Context, [Id; 2]) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.layers.push(daw_core::InstrumentLayerSettings::default());
    for l in &mut i.layers {
        l.collapsed = compact;
    }
    let a = &mut i.layers[0];
    a.oscillators = vec![Default::default(), Default::default()];
    a.envelopes[0].present = true;
    a.envelopes[0].mode = daw_core::EnvelopeMode::Legato;
    a.lfos.push(Default::default());
    a.amplitude.present = true;
    let owners = [i.layers[0].id, i.layers[1].id];
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
    app.timeline_catalog_visible = false;
    app.timeline_effects_visible = false;
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, owners)
}
fn modifier(copy: bool) -> egui::Modifiers {
    egui::Modifiers {
        alt: copy,
        ctrl: copy,
        ..Default::default()
    }
}
fn event(pos: Pos2, pressed: bool, copy: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: modifier(copy),
    }
}
fn source(app: &DawUi, owner: Id, c: InstrumentComponent) -> Pos2 {
    let r = app
        .instrument_ui
        .component_bounds
        .iter()
        .rev()
        .find(|(id, kind, _)| *id == owner && *kind == c)
        .unwrap()
        .2;
    if r.width() < 40.0 {
        r.center()
    } else if matches!(c, InstrumentComponent::Oscillator(_)) {
        egui::pos2(r.left() + 2.0, r.center().y)
    } else {
        egui::pos2(r.left() + 35.0, r.top() + 10.0)
    }
}

fn order_setup(
    theme: settings::ColorTheme,
    compact: bool,
    main: bool,
    collapsed: bool,
) -> (DawUi, egui::Context, Id, InstrumentComponent) {
    let (mut app, ctx, owners) = setup(theme, compact);
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.layers[1].collapsed = true;
    i.layers[0].oscillators.truncate(1);
    i.layers[0].amplitude.present = collapsed;
    i.layers[0].amplitude.collapsed = collapsed;
    i.layers[0].envelopes[0].collapsed = collapsed;
    i.layers[0].lfos[0].collapsed = collapsed;
    let owner;
    let moving;
    if main {
        i.main.collapsed = compact;
        i.main.envelopes = vec![i.layers[0].envelopes[0].clone()];
        i.main.lfos.push(daw_core::LfoInstance {
            collapsed,
            ..Default::default()
        });
        owner = i.main.id;
        if collapsed {
            i.main.macro_module.add();
            i.main.macro_module.collapsed = true;
            moving = InstrumentComponent::Macros;
        } else {
            moving = InstrumentComponent::Lfo(i.main.lfos[0].id);
        }
        i.layers[0].collapsed = true;
    } else {
        owner = owners[0];
        moving = if collapsed {
            InstrumentComponent::Amplitude
        } else {
            InstrumentComponent::Lfo(i.layers[0].lfos[0].id)
        };
    }
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    app.dirty = false;
    (app, ctx, owner, moving)
}

#[test]
fn component_units_reorder_with_effects_lane_preview_and_sources_pinned_first() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                for collapsed in [false, true] {
                    for fast in [false, true] {
                        let (mut app, ctx, owner, moving) =
                            order_setup(theme, compact, main, collapsed);
                        let before = app.session.project.tracks[0].instrument.clone().unwrap();
                        let slots: Vec<_> = app
                            .instrument_ui
                            .component_slots
                            .iter()
                            .filter(|(id, _, _)| *id == owner)
                            .map(|(_, c, r)| (*c, *r))
                            .collect();
                        let first = usize::from(!main);
                        let end = slots[0].1.left_center() + egui::vec2(2.0, 0.0);
                        let p = source(&app, owner, moving);
                        start(&mut app, &ctx, p);
                        if !fast {
                            let shapes = frame_shapes(
                                &mut app,
                                &ctx,
                                vec![egui::Event::PointerMoved(end)],
                                Vec2::new(1280.0, 800.0),
                            );
                            assert_eq!(app.instrument_catalog_bounds.drop_target, Some(owner));
                            assert_eq!(
                                app.session.project.tracks[0].instrument.as_ref().unwrap(),
                                &before
                            );
                            assert!(!app.dirty);
                            let line_x = slots[first].1.left().max(
                                app.instrument_catalog_bounds.drop_bounds.left()
                                    + TRACK_WIDTH
                                    + 1.0,
                            );
                            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { points, stroke }
                                if stroke.width == 2.0 && stroke.color == theme::accent(&ctx)
                                && (points[0].x - line_x).abs() < 0.1 && points[0].x == points[1].x)), "main {main} compact {compact} collapsed {collapsed}");
                            let bounds = app.instrument_catalog_bounds.drop_bounds;
                            assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == bounds && r.stroke == theme::selection_outline(&ctx))));
                        }
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end), event(end, false, false)],
                        );
                        assert!(app.component_drag.is_none());
                        assert!(app.error.is_none(), "{:?}", app.error);
                        assert!(app.dirty);
                        let actual = app.session.project.tracks[0].instrument.as_ref().unwrap();
                        let mut expected = before;
                        let order = if main {
                            &actual.main.component_order
                        } else {
                            &actual.layers[0].component_order
                        };
                        if main {
                            expected.main.component_order = order.clone();
                        } else {
                            expected.layers[0].component_order = order.clone();
                        }
                        assert_eq!(actual, &expected);
                        let visible = if main {
                            actual.main.visible_components()
                        } else {
                            actual.layers[0].visible_components()
                        };
                        assert_eq!(visible[first], moving);
                        if !main {
                            assert_eq!(visible[0], InstrumentComponent::Oscillators);
                        }
                        frame(&mut app, &ctx, vec![]);
                        assert_eq!(
                            app.instrument_ui
                                .component_slots
                                .iter()
                                .filter(|(id, _, _)| *id == owner)
                                .map(|(_, c, _)| *c)
                                .collect::<Vec<_>>(),
                            visible
                        );
                        app.session.project.validate().unwrap();
                        let saved = serde_json::to_vec(&app.session.project).unwrap();
                        assert_eq!(
                            serde_json::from_slice::<daw_core::Project>(&saved)
                                .unwrap()
                                .tracks[0]
                                .instrument,
                            app.session.project.tracks[0].instrument
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn component_reorder_no_op_cancel_and_pinned_source_drags_stay_clean() {
    for main in [false, true] {
        for mode in ["same", "escape", "focus", "outside", "source"] {
            if main && mode == "source" {
                continue;
            }
            let (mut app, ctx, owner, mut moving) =
                order_setup(settings::ColorTheme::Dark, true, main, true);
            if mode == "source" {
                moving = InstrumentComponent::Oscillators;
            }
            let original = app.session.project.tracks[0].instrument.clone();
            let slots: Vec<_> = app
                .instrument_ui
                .component_slots
                .iter()
                .filter(|(id, _, _)| *id == owner)
                .map(|(_, c, r)| (*c, *r))
                .collect();
            let p = source(&app, owner, moving);
            let end = if mode == "same" {
                slots
                    .iter()
                    .find(|(c, _)| *c == moving)
                    .unwrap()
                    .1
                    .left_center()
                    + egui::vec2(2.0, 0.0)
            } else if mode == "source" {
                slots.last().unwrap().1.right_center() - egui::vec2(2.0, 0.0)
            } else {
                slots[0].1.left_center() + egui::vec2(2.0, 0.0)
            };
            start(&mut app, &ctx, p);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            match mode {
                "escape" => frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                ),
                "focus" => {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            focused: false,
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                egui::vec2(1280.0, 800.0),
                            )),
                            ..Default::default()
                        },
                        |ui| app.show(ui),
                    );
                    output.textures_delta.clear();
                }
                "outside" => frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(egui::pos2(1.0, 1.0)),
                        event(egui::pos2(1.0, 1.0), false, false),
                    ],
                ),
                _ => frame(&mut app, &ctx, vec![event(end, false, false)]),
            }
            assert!(app.component_drag.is_none(), "{mode}");
            assert_eq!(app.session.project.tracks[0].instrument, original, "{mode}");
            assert!(!app.dirty, "{mode}");
        }
    }
}
#[test]
fn component_reorder_scrolls_both_owner_viewports_without_editing_the_preview() {
    for main in [false, true] {
        let (mut app, ctx, owner, moving) =
            order_setup(settings::ColorTheme::Dark, false, main, false);
        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
        let extra = (0..4).map(|_| daw_core::EnvelopeInstance {
            present: true,
            mode: daw_core::EnvelopeMode::Legato,
            ..Default::default()
        });
        if main {
            i.main.envelopes.extend(extra);
            i.main.component_order = vec![moving];
        } else {
            i.layers[0].envelopes.extend(extra);
            i.layers[0].component_order = vec![InstrumentComponent::Oscillators, moving];
        }
        let others = (0..4).map(|_| daw_core::EnvelopeInstance {
            present: true,
            mode: daw_core::EnvelopeMode::Legato,
            ..Default::default()
        });
        if main {
            i.layers[0].envelopes.extend(others);
        } else {
            i.layers[1].envelopes.extend(others);
        }
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let before = app.session.project.tracks[0].instrument.clone();
        let position = |app: &DawUi| {
            app.instrument_ui
                .component_slots
                .iter()
                .find(|(id, c, _)| *id == owner && *c == moving)
                .unwrap()
                .2
                .left()
        };
        let other_positions = |app: &DawUi| {
            app.instrument_ui
                .component_slots
                .iter()
                .filter(|(id, _, _)| *id != owner)
                .map(|(id, unit, rect)| (*id, *unit, rect.left()))
                .collect::<Vec<_>>()
        };
        let others_before = other_positions(&app);
        let x = position(&app);
        let p = source(&app, owner, moving);
        let viewport = app
            .instrument_ui
            .component_viewports
            .iter()
            .find(|(id, _)| *id == owner)
            .unwrap()
            .1;
        let edge = egui::pos2(viewport.right() - 3.0, p.y);
        start(&mut app, &ctx, p);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(edge)]);
        for _ in 0..40 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(position(&app) < x - 20.0, "Main: {main}");
        assert_eq!(
            other_positions(&app),
            others_before,
            "Unhovered lanes must not scroll"
        );
        assert!(app.component_drag.as_ref().is_some_and(|d| d.active));
        assert_eq!(app.session.project.tracks[0].instrument, before);
        assert!(!app.dirty);
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
        assert!(app.component_drag.is_none());
        assert_eq!(app.session.project.tracks[0].instrument, before);
        assert!(!app.dirty);
    }
}

#[test]
fn component_click_selection_uses_effect_outlines_and_stays_out_of_project_data() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                for collapsed in [false, true] {
                    let (mut app, ctx, owner, moving) =
                        order_setup(theme, compact, main, collapsed);
                    let before = app.session.project.tracks[0].instrument.clone();
                    let p = source(&app, owner, moving);
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(p), event(p, true, false)],
                    );
                    let shapes = frame_shapes(
                        &mut app,
                        &ctx,
                        vec![event(p, false, false)],
                        Vec2::new(1280.0, 800.0),
                    );
                    assert_eq!(
                        app.instrument_ui.selected_components,
                        HashSet::from([(owner, moving)]),
                        "main {main} compact {compact} collapsed {collapsed}"
                    );
                    let rect = app
                        .instrument_ui
                        .component_slots
                        .iter()
                        .find(|(id, c, _)| *id == owner && *c == moving)
                        .unwrap()
                        .2;
                    assert!(
                        shapes
                            .iter()
                            .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                        if r.rect == rect && r.stroke == theme::selection_outline(&ctx)))
                    );
                    frame(&mut app, &ctx, vec![]);
                    assert_eq!(
                        app.instrument_ui.selected_components,
                        HashSet::from([(owner, moving)])
                    );
                    assert_eq!(app.session.project.tracks[0].instrument, before);
                    assert!(!app.dirty);
                    let label = if main {
                        app.instrument_ui.main_bounds.label.center()
                    } else {
                        app.midi_editor.layer_bounds[&owner].label.center()
                    };
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(label), event(label, true, false)],
                    );
                    frame(&mut app, &ctx, vec![event(label, false, false)]);
                    assert!(app.instrument_ui.selected_components.is_empty());
                    assert_eq!(app.session.project.tracks[0].instrument, before);
                    assert!(!app.dirty);
                }
            }
        }
    }
}

#[test]
fn cross_lane_components_use_effect_insertion_preview_and_select_the_result() {
    for main in [false, true] {
        for empty in [false, true] {
            for copy in [false, true] {
                let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
                let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                i.layers[0].envelopes[0].collapsed = true;
                i.layers[0].lfos[0].collapsed = true;
                let component = InstrumentComponent::Envelope(i.layers[0].envelopes[0].id);
                let destination = if main {
                    i.main.collapsed = true;
                    i.main.id
                } else {
                    owners[1]
                };
                if !empty {
                    if main {
                        i.main.lfos.push(Default::default());
                    } else {
                        i.layers[1].oscillators.push(Default::default());
                        i.layers[1].lfos.push(Default::default());
                    }
                }
                frame(&mut app, &ctx, vec![]);
                frame(&mut app, &ctx, vec![]);
                let before = app.session.project.tracks[0].instrument.clone();
                app.dirty = false;
                let viewport = app
                    .instrument_ui
                    .component_viewports
                    .iter()
                    .find(|(id, _)| *id == destination)
                    .unwrap()
                    .1;
                let end = egui::pos2(viewport.left() + 2.0, viewport.center().y);
                let p = source(&app, owners[0], component);
                start(&mut app, &ctx, p);
                let shapes = frame_shapes(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::ModifiersChanged(modifier(copy)),
                        egui::Event::PointerMoved(end),
                    ],
                    Vec2::new(1280.0, 800.0),
                );
                assert_eq!(
                    app.instrument_catalog_bounds.drop_target,
                    Some(destination),
                    "main {main} empty {empty} copy {copy} end {end:?} viewport {viewport:?}"
                );
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment {points, stroke}
                    if stroke.width == 2.0 && stroke.color == theme::accent(&ctx) && points[0].x == points[1].x)));
                let drop = app.instrument_catalog_bounds.drop_bounds;
                assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == drop && r.stroke == theme::selection_outline(&ctx))));
                assert_eq!(app.session.project.tracks[0].instrument, before);
                assert!(!app.dirty);
                frame(&mut app, &ctx, vec![event(end, false, copy)]);
                assert!(app.error.is_none(), "{:?}", app.error);
                assert!(app.dirty);
                frame(&mut app, &ctx, vec![]);
                let selected = *app.instrument_ui.selected_components.iter().next().unwrap();
                assert_eq!(selected.0, destination);
                assert!(matches!(selected.1, InstrumentComponent::Envelope(_)));
                let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                let order = if main {
                    i.main.visible_components()
                } else {
                    i.layers[1].visible_components()
                };
                assert_eq!(order[usize::from(!main && !empty)], selected.1);
                if copy || !main {
                    assert_ne!(selected.1, component);
                } else {
                    assert_eq!(selected.1, component);
                }
                app.session.project.validate().unwrap();
            }
        }
    }
}

fn start(app: &mut DawUi, ctx: &egui::Context, p: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(p), event(p, true, false)],
    );
    assert!(app.component_drag.is_some(), "component press at {p:?}");
}
#[test]
fn installed_component_headers_and_source_gaps_move_and_copy_on_release() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for kind in 0..6 {
                for copy in [false, true] {
                    let (mut app, ctx, owners) = setup(theme, compact);
                    // Keep the transferred component visible without horizontal scrolling.
                    if kind == 5 {
                        let module = &mut app.session.project.tracks[0]
                            .instrument
                            .as_mut()
                            .unwrap()
                            .layers[0]
                            .macro_module;
                        module.add();
                        module.add();
                    }
                    if kind != 2 {
                        app.session.project.tracks[0]
                            .instrument
                            .as_mut()
                            .unwrap()
                            .layers[0]
                            .envelopes[0]
                            .present = false;
                    }
                    frame(&mut app, &ctx, vec![]);
                    let l = &app.session.project.tracks[0]
                        .instrument
                        .as_ref()
                        .unwrap()
                        .layers[0];
                    let c = match kind {
                        0 => InstrumentComponent::Oscillators,
                        1 => InstrumentComponent::Oscillator(l.oscillators[0].id),
                        2 => InstrumentComponent::Envelope(l.envelopes[0].id),
                        3 => InstrumentComponent::Lfo(l.lfos[0].id),
                        4 => InstrumentComponent::Amplitude,
                        _ => InstrumentComponent::Macros,
                    };
                    let p = source(&app, owners[0], c);
                    let end = app.midi_editor.layer_bounds[&owners[1]].label.center();
                    let original = serde_json::to_value(&app.session.project).unwrap();
                    start(&mut app, &ctx, p);
                    frame(
                        &mut app,
                        &ctx,
                        vec![
                            egui::Event::ModifiersChanged(modifier(copy)),
                            egui::Event::PointerMoved(end),
                        ],
                    );
                    assert_eq!(
                        app.instrument_catalog_bounds.drop_target,
                        Some(owners[1]),
                        "kind {kind} compact {compact}"
                    );
                    assert_eq!(
                        serde_json::to_value(&app.session.project).unwrap(),
                        original
                    );
                    assert!(!app.dirty);
                    frame(&mut app, &ctx, vec![event(end, false, copy)]);
                    assert!(app.component_drag.is_none());
                    assert!(app.dirty, "kind {kind} compact {compact}");
                    assert!(app.error.is_none(), "{:?}", app.error);
                    app.session.project.validate().unwrap();
                    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                    match kind {
                        0 | 1 => {
                            assert_eq!(
                                i.layers[1].oscillators.len(),
                                if kind == 0 { 2 } else { 1 }
                            );
                            assert_eq!(
                                i.layers[0].oscillators.len(),
                                if copy {
                                    2
                                } else if kind == 0 {
                                    0
                                } else {
                                    1
                                }
                            );
                        }
                        2 => {
                            assert_eq!(
                                i.layers[0].envelopes.iter().filter(|e| e.present).count(),
                                usize::from(copy)
                            );
                            assert!(i.layers[1].envelopes.iter().any(|e| e.present));
                        }
                        3 => {
                            assert_eq!(i.layers[0].lfos.len(), usize::from(copy));
                            assert_eq!(i.layers[1].lfos.len(), 1);
                        }
                        5 => {
                            assert_eq!(
                                i.layers[0].macro_module.controls.len(),
                                if copy { 2 } else { 0 }
                            );
                            assert_eq!(i.layers[1].macro_module.controls.len(), 2);
                            assert!(i.layers[1].macro_module.present);
                        }
                        _ => {
                            assert_eq!(i.layers[0].amplitude.present, copy);
                            assert!(i.layers[1].amplitude.present);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn component_drag_cancel_invalid_drop_and_snapshot_changes_leave_project_untouched() {
    for mode in [
        "escape",
        "outside",
        "focus",
        "modal",
        "changed",
        "main-invalid",
        "same-layer",
    ] {
        let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
        let c = InstrumentComponent::Oscillators;
        let p = source(&app, owners[0], c);
        let end = match mode {
            "outside" => egui::pos2(1.0, 1.0),
            "main-invalid" => app.instrument_ui.main_bounds.label.center(),
            "same-layer" => app.midi_editor.layer_bounds[&owners[0]].label.center(),
            _ => app.midi_editor.layer_bounds[&owners[1]].label.center(),
        };
        start(&mut app, &ctx, p);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let events = match mode {
            "escape" => vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            "focus" => vec![egui::Event::WindowFocused(false)],
            "modal" => {
                app.settings_open = true;
                vec![]
            }
            "changed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .oscillators[0]
                    .transpose = 7;
                vec![]
            }
            _ => vec![],
        };
        let original = serde_json::to_value(&app.session.project).unwrap();
        if mode == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
            frame(&mut app, &ctx, events);
        }
        frame(&mut app, &ctx, vec![event(end, false, false)]);
        assert!(app.component_drag.is_none(), "{mode}");
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{mode}"
        );
        assert!(!app.dirty, "{mode}");
    }
}
#[test]
fn shared_provider_can_move_to_main_and_copy_back() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
    app.session.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .oscillators
        .clear();
    frame(&mut app, &ctx, vec![]);
    let id = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .envelopes[0]
        .id;
    let c = InstrumentComponent::Envelope(id);
    let p = source(&app, owners[0], c);
    let end = app.instrument_ui.main_bounds.label.center();
    start(&mut app, &ctx, p);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end), event(end, false, false)],
    );
    let main = &app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .main;
    assert_eq!(main.envelopes.len(), 1);
    let mainid = main.id;
    frame(&mut app, &ctx, vec![]);
    let p = source(&app, mainid, c);
    let end = app.midi_editor.layer_bounds[&owners[1]].label.center();
    start(&mut app, &ctx, p);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(modifier(true)),
            egui::Event::PointerMoved(end),
            event(end, false, true),
        ],
    );
    let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert_eq!(i.main.envelopes.len(), 1);
    assert!(i.layers[1].envelopes.iter().any(|e| e.present));
    app.session.project.validate().unwrap();
}

#[test]
fn component_header_click_selects_its_layer_without_editing_or_requiring_a_drag() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
    app.instrument_ui.selected_layer = Some(owners[1]);
    let p = source(&app, owners[0], InstrumentComponent::Oscillators);
    super::instrument_catalog::click(&mut app, &ctx, p, false);
    assert_eq!(app.instrument_ui.selected_layer, Some(owners[0]));
    assert!(app.component_drag.is_none());
    assert!(!app.dirty);
}

#[test]
fn oscillator_strip_reorder_uses_insertion_slots_without_changing_pitch_routes() {
    for compact in [false, true] {
        let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, compact);
        let l = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        let mut r = daw_core::Modulation::new(daw_core::ModulationParameter::Pitch);
        r.target = Some(l.oscillators[0].id);
        r.amount = 12.0;
        l.oscillator_modulations.push(r);
        let original = l.clone();
        frame(&mut app, &ctx, vec![]);
        let p = source(
            &app,
            owners[0],
            InstrumentComponent::Oscillator(original.oscillators[0].id),
        );
        let end = app.instrument_ui.oscillator_slots[1].2.right_center() - egui::vec2(2.0, 0.0);
        start(&mut app, &ctx, p);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0],
            original
        );
        frame(&mut app, &ctx, vec![event(end, false, false)]);
        let l = &app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0];
        assert_eq!(
            l.oscillators,
            [
                original.oscillators[1].clone(),
                original.oscillators[0].clone()
            ]
        );
        assert_eq!(l.oscillator_modulations, original.oscillator_modulations);
        assert!(app.dirty);
    }
}

#[test]
fn shared_modulation_strips_reorder_on_amplitude_filter_drive_and_oscillator() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for kind in 0..4 {
            let (mut app, ctx, owners) = setup(theme, true);
            app.timeline_effects_visible = true;
            let l = &mut app.session.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0];
            l.envelopes.clear();
            l.lfos.clear();
            let mut routes = (0..3)
                .map(|_| {
                    daw_core::Modulation::new(match kind {
                        0 => daw_core::ModulationParameter::Amplitude,
                        1 => daw_core::ModulationParameter::Frequency,
                        2 => daw_core::ModulationParameter::Volume,
                        _ => daw_core::ModulationParameter::Pitch,
                    })
                })
                .collect::<Vec<_>>();
            if kind == 3 {
                for r in &mut routes {
                    r.target = Some(l.oscillators[0].id);
                }
            }
            match kind {
                0 => {
                    l.oscillators.clear();
                    l.amplitude.modulation_open = true;
                    l.amplitude.modulations = routes.clone();
                }
                1 | 2 => {
                    let mut e = if kind == 1 {
                        daw_core::EffectInstance::auto_filter()
                    } else {
                        daw_core::EffectInstance::drive()
                    };
                    match &mut e.unit {
                        daw_core::EffectSettings::AutoFilter(s) => {
                            s.modulation_open = true;
                            s.modulations = routes.clone();
                        }
                        daw_core::EffectSettings::Drive(s) => {
                            s.modulation_open = true;
                            s.modulations = routes.clone();
                        }
                        _ => unreachable!(),
                    }
                    app.session.project.tracks[0].effects = vec![e];
                    app.session.project.tracks[0].effects_compact = true;
                }
                _ => {
                    l.oscillator_modulation_open = true;
                    l.oscillator_modulations = routes.clone();
                }
            }
            frame(&mut app, &ctx, vec![]);
            app.dirty = false;
            let first = crate::strip_reorder::bounds(&ctx, routes[0].id);
            let last = crate::strip_reorder::bounds(&ctx, routes[2].id);
            let p = egui::pos2(first.left() + 2.0, first.center().y);
            let end = egui::pos2(last.right() - 2.0, last.center().y);
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(p), button(p, true)],
            );
            assert!(crate::strip_reorder::dragging(&ctx), "kind {kind}");
            assert!(!app.file_action_enabled(FileAction::Save));
            assert!(!app.view_switch_enabled());
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert!(!app.dirty);
            frame(&mut app, &ctx, vec![button(end, false)]);
            assert!(!crate::strip_reorder::dragging(&ctx));
            let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
            let l = i.layers.iter().find(|l| l.id == owners[0]).unwrap();
            let actual = match kind {
                0 => l.amplitude.modulations.as_slice(),
                1 | 2 => app.session.project.tracks[0].effects[0].unit.modulations(),
                _ => &l.oscillator_modulations,
            };
            assert_eq!(actual, [routes[1], routes[2], routes[0]], "kind {kind}");
            assert!(app.dirty);
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn modulation_reorder_cancels_invalid_gestures_and_keeps_no_op_drops_clean() {
    for mode in [
        "escape",
        "outside",
        "focus",
        "modal",
        "changed",
        "hidden",
        "host",
        "same-slot",
    ] {
        let (mut app, ctx, _) = setup(settings::ColorTheme::Dark, true);
        let l = &mut app.session.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        l.oscillators.clear();
        l.envelopes.clear();
        l.lfos.clear();
        l.amplitude.modulation_open = true;
        l.amplitude.modulations = (0..3)
            .map(|_| daw_core::Modulation::new(daw_core::ModulationParameter::Amplitude))
            .collect();
        let ids = l
            .amplitude
            .modulations
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>();
        frame(&mut app, &ctx, vec![]);
        app.dirty = false;
        let first = crate::strip_reorder::bounds(&ctx, ids[0]);
        let last = crate::strip_reorder::bounds(&ctx, ids[2]);
        let p = egui::pos2(first.left() + 2.0, first.center().y);
        let end = match mode {
            "outside" => egui::pos2(1.0, 1.0),
            "same-slot" => egui::pos2(first.center().x - 2.0, first.center().y),
            _ => last.right_center() - egui::vec2(2.0, 0.0),
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(p), button(p, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let mut events = vec![];
        match mode {
            "escape" => events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }),
            "modal" => app.settings_open = true,
            "changed" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .amplitude
                    .modulations[0]
                    .amount = 0.5
            }
            "hidden" => {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .amplitude
                    .modulation_open = false
            }
            "host" => app.view_mode = ViewMode::Timeline,
            _ => {}
        }
        let original = serde_json::to_value(&app.session.project).unwrap();
        if mode == "focus" {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    focused: false,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        } else {
            frame(&mut app, &ctx, events);
        }
        frame(&mut app, &ctx, vec![button(end, false)]);
        assert!(!crate::strip_reorder::dragging(&ctx), "{mode}");
        assert_eq!(
            serde_json::to_value(&app.session.project).unwrap(),
            original,
            "{mode}"
        );
        assert!(!app.dirty, "{mode}");
    }
}

fn click_component(
    app: &mut DawUi,
    ctx: &egui::Context,
    owner: Id,
    component: InstrumentComponent,
    shift: bool,
) {
    let pos = source(app, owner, component);
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    frame(
        app,
        ctx,
        vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(pos),
            button(true),
        ],
    );
    frame(app, ctx, vec![button(false)]);
}

#[test]
fn component_group_shift_selection_toggles_replaces_and_stays_in_one_lane() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for main in [false, true] {
            let (mut app, ctx, owner, last) = order_setup(theme, true, main, true);
            if !main {
                app.session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .main
                    .lfos
                    .push(daw_core::LfoInstance {
                        collapsed: true,
                        ..Default::default()
                    });
                frame(&mut app, &ctx, vec![]);
            }
            let first = app
                .instrument_ui
                .component_slots
                .iter()
                .find(|(id, c, _)| *id == owner && !c.is_source())
                .unwrap()
                .1;
            let before = app.session.project.tracks[0].instrument.clone();
            click_component(&mut app, &ctx, owner, first, false);
            click_component(&mut app, &ctx, owner, last, true);
            assert_eq!(
                app.instrument_ui.selected_components,
                HashSet::from([(owner, first), (owner, last)])
            );
            let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
            for unit in [first, last] {
                let rect = app
                    .instrument_ui
                    .component_slots
                    .iter()
                    .find(|(id, c, _)| *id == owner && *c == unit)
                    .unwrap()
                    .2;
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke == theme::selection_outline(&ctx))));
            }
            click_component(&mut app, &ctx, owner, first, true);
            assert_eq!(
                app.instrument_ui.selected_components,
                HashSet::from([(owner, last)])
            );
            click_component(&mut app, &ctx, owner, last, false);
            click_component(&mut app, &ctx, owner, first, false);
            assert_eq!(
                app.instrument_ui.selected_components,
                HashSet::from([(owner, first)])
            );
            assert_eq!(app.session.project.tracks[0].instrument, before);
            assert!(!app.dirty);
            let other = app
                .instrument_ui
                .component_slots
                .iter()
                .find(|(id, _, _)| *id != owner)
                .map(|(id, c, _)| (*id, *c))
                .unwrap();
            click_component(&mut app, &ctx, other.0, other.1, true);
            assert_eq!(
                app.instrument_ui.selected_components,
                HashSet::from([other])
            );
            assert!(!app.dirty);
        }
    }
}

#[test]
fn component_groups_reorder_in_lane_order_with_effect_previews_and_pinned_sources() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                for with_source in [false, true] {
                    if main && with_source {
                        continue;
                    }
                    for fast in [false, true] {
                        let (mut app, ctx, owner, last) = order_setup(theme, compact, main, true);
                        let order: Vec<_> = app
                            .instrument_ui
                            .component_slots
                            .iter()
                            .filter(|(id, _, _)| *id == owner)
                            .map(|(_, c, _)| *c)
                            .collect();
                        let envelope = order[usize::from(!main)];
                        let group = if with_source {
                            vec![InstrumentComponent::Oscillators, envelope]
                        } else {
                            vec![envelope, last]
                        };
                        // Select in reverse order; drag order must follow the lane.
                        click_component(&mut app, &ctx, owner, group[1], false);
                        click_component(&mut app, &ctx, owner, group[0], true);
                        let original = app.session.project.tracks[0].instrument.clone();
                        let rect = app
                            .instrument_ui
                            .component_slots
                            .iter()
                            .find(|(id, _, _)| *id == owner)
                            .unwrap()
                            .2;
                        let end = if with_source {
                            app.instrument_ui
                                .component_slots
                                .iter()
                                .rfind(|(id, _, _)| *id == owner)
                                .unwrap()
                                .2
                                .right_center()
                                + Vec2::new(4.0, 0.0)
                        } else {
                            rect.left_center() + Vec2::new(2.0, 0.0)
                        };
                        let p = source(&app, owner, group[1]);
                        start(&mut app, &ctx, p);
                        if !fast {
                            let shapes = frame_shapes(
                                &mut app,
                                &ctx,
                                vec![egui::Event::PointerMoved(end)],
                                Vec2::new(1280.0, 800.0),
                            );
                            assert_eq!(
                                app.instrument_ui.selected_components,
                                group.iter().map(|c| (owner, *c)).collect()
                            );
                            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment {stroke,..} if stroke.width == 2.0 && stroke.color == theme::accent(&ctx))));
                            assert_eq!(app.session.project.tracks[0].instrument, original);
                            assert!(!app.dirty);
                        }
                        frame(
                            &mut app,
                            &ctx,
                            vec![egui::Event::PointerMoved(end), event(end, false, false)],
                        );
                        assert!(app.error.is_none(), "{:?}", app.error);
                        assert!(app.dirty);
                        frame(&mut app, &ctx, vec![]);
                        let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                        let actual = if main {
                            i.main.visible_components()
                        } else {
                            i.layers[0].visible_components()
                        };
                        let moving: Vec<_> =
                            group.iter().copied().filter(|c| !c.is_source()).collect();
                        let mut expected: Vec<_> =
                            order.into_iter().filter(|c| !moving.contains(c)).collect();
                        let at = if with_source {
                            expected.len()
                        } else {
                            usize::from(!main)
                        };
                        expected.splice(at..at, moving);
                        assert_eq!(actual, expected);
                        assert_eq!(
                            app.instrument_ui.selected_components,
                            group.iter().map(|c| (owner, *c)).collect()
                        );
                        app.session.project.validate().unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn component_groups_copy_or_move_together_to_regular_layers_and_empty_main() {
    for main in [false, true] {
        for duplicate in [false, true] {
            for fast in [false, true] {
                let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
                let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                let a = &mut i.layers[0];
                a.envelopes[0].collapsed = true;
                a.lfos[0].collapsed = true;
                let units = [
                    InstrumentComponent::Envelope(a.envelopes[0].id),
                    InstrumentComponent::Lfo(a.lfos[0].id),
                ];
                let to = if main { i.main.id } else { owners[1] };
                frame(&mut app, &ctx, vec![]);
                click_component(&mut app, &ctx, owners[0], units[1], false);
                click_component(&mut app, &ctx, owners[0], units[0], true);
                app.dirty = false;
                let original = app.session.project.tracks[0].instrument.clone();
                let viewport = app
                    .instrument_ui
                    .component_viewports
                    .iter()
                    .find(|(id, _)| *id == to)
                    .unwrap()
                    .1;
                let end = viewport.left_center() + Vec2::new(2.0, 0.0);
                let p = source(&app, owners[0], units[1]);
                start(&mut app, &ctx, p);
                if !fast {
                    frame(
                        &mut app,
                        &ctx,
                        vec![
                            egui::Event::PointerMoved(end),
                            egui::Event::ModifiersChanged(modifier(duplicate)),
                        ],
                    );
                    assert_eq!(app.instrument_catalog_bounds.drop_target, Some(to));
                    assert_eq!(app.session.project.tracks[0].instrument, original);
                    assert!(!app.dirty);
                }
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(end),
                        egui::Event::ModifiersChanged(modifier(duplicate)),
                        event(end, false, duplicate),
                    ],
                );
                assert!(app.error.is_none(), "{:?}", app.error);
                assert!(app.dirty);
                frame(&mut app, &ctx, vec![]);
                assert_eq!(app.instrument_ui.selected_components.len(), 2);
                assert!(
                    app.instrument_ui
                        .selected_components
                        .iter()
                        .all(|(id, _)| *id == to)
                );
                let i = app.session.project.tracks[0].instrument.as_ref().unwrap();
                let order = if main {
                    i.main.visible_components()
                } else {
                    i.layers[1].visible_components()
                };
                assert!(matches!(order[0], InstrumentComponent::Envelope(_)));
                assert!(matches!(order[1], InstrumentComponent::Lfo(_)));
                if duplicate {
                    assert_eq!(i.layers[0], original.unwrap().layers[0]);
                    assert_ne!(order[0], units[0]);
                    assert_ne!(order[1], units[1]);
                } else {
                    assert!(
                        !i.layers[0]
                            .envelopes
                            .iter()
                            .any(|e| InstrumentComponent::Envelope(e.id) == units[0])
                    );
                    assert!(i.layers[0].lfos.is_empty());
                }
                app.session.project.validate().unwrap();
            }
        }
    }
}

#[test]
fn component_group_noop_cancel_invalid_target_and_unselected_anchor_keep_effect_rules() {
    for mode in [
        "same",
        "escape",
        "outside",
        "invalid-main",
        "occupied-copy",
        "unselected",
    ] {
        let (mut app, ctx, owner, last) =
            order_setup(settings::ColorTheme::Dark, true, false, true);
        let order: Vec<_> = app
            .instrument_ui
            .component_slots
            .iter()
            .filter(|(id, _, _)| *id == owner)
            .map(|(_, c, _)| *c)
            .collect();
        let units = if mode == "invalid-main" || mode == "occupied-copy" {
            vec![order[1], last]
        } else {
            vec![order[1], order[2]]
        };
        click_component(&mut app, &ctx, owner, units[1], false);
        click_component(&mut app, &ctx, owner, units[0], true);
        let original = app.session.project.tracks[0].instrument.clone();
        let first = app
            .instrument_ui
            .component_slots
            .iter()
            .find(|(id, _, _)| *id == owner)
            .unwrap()
            .2;
        let end = match mode {
            "invalid-main" => Pos2::new(
                app.instrument_ui.main_bounds.toolbar.left() - 3.0,
                app.instrument_ui.main_bounds.row.center().y,
            ),
            "outside" => Pos2::new(2.0, 2.0),
            _ => first.left_center() + Vec2::new(2.0, 0.0),
        };
        let p = source(
            &app,
            owner,
            if mode == "unselected" { last } else { units[1] },
        );
        start(&mut app, &ctx, p);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(end),
                egui::Event::ModifiersChanged(modifier(mode == "occupied-copy")),
            ],
        );
        assert_eq!(app.session.project.tracks[0].instrument, original);
        assert!(!app.dirty);
        if mode == "escape" {
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
        }
        frame(
            &mut app,
            &ctx,
            vec![event(end, false, mode == "occupied-copy")],
        );
        assert!(app.component_drag.is_none());
        assert!(app.error.is_none(), "{:?}", app.error);
        if mode == "unselected" {
            assert!(app.dirty);
            assert_eq!(
                app.instrument_ui.selected_components,
                HashSet::from([(owner, last)])
            );
            assert_eq!(
                app.session.project.tracks[0]
                    .instrument
                    .as_ref()
                    .unwrap()
                    .layers[0]
                    .visible_components()[1],
                last
            );
        } else {
            assert_eq!(app.session.project.tracks[0].instrument, original, "{mode}");
            assert!(!app.dirty, "{mode}");
        }
    }
}

fn component_delete_key(key: egui::Key, repeat: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}

#[test]
fn component_delete_removes_the_selected_group_retains_settings_and_protects_clips() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                for collapsed in [false, true] {
                    for all in [false, true] {
                        if all && !collapsed {
                            continue;
                        }
                        for key in [egui::Key::Delete, egui::Key::Backspace] {
                            let (mut app, ctx, owner, _) =
                                order_setup(theme, compact, main, collapsed);
                            let track = app.session.project.tracks[0].id;
                            let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
                            if main {
                                i.main.macro_module.add();
                                i.main.macro_module.collapsed = collapsed;
                            } else {
                                i.layers[0].macro_module.add();
                                i.layers[0].macro_module.collapsed = collapsed;
                                i.layers[0].amplitude.present = true;
                            }
                            let envelope = if main {
                                i.main.envelopes[0].id
                            } else {
                                i.layers[0].envelopes[0].id
                            };
                            let oscillator = i.layers[0].oscillators[0].id;
                            i.layers[0]
                                .oscillator_modulations
                                .push(daw_core::Modulation {
                                    target: Some(oscillator),
                                    amount: 12.0,
                                    ..daw_core::Modulation::new(
                                        daw_core::ModulationParameter::Pitch,
                                    )
                                });
                            i.layers[0]
                                .amplitude
                                .modulations
                                .push(daw_core::Modulation {
                                    source: Some(daw_core::ModulationSourceId {
                                        layer: owner,
                                        unit: envelope,
                                    }),
                                    amount: 0.6,
                                    ..daw_core::Modulation::new(
                                        daw_core::ModulationParameter::Amplitude,
                                    )
                                });
                            let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
                            app.selected_clip = Some(clip);
                            app.selected_clips.insert(clip);
                            frame(&mut app, &ctx, vec![]);
                            frame(&mut app, &ctx, vec![]);
                            let before = app.session.project.tracks[0].clone();
                            let units: Vec<_> = app
                                .instrument_ui
                                .component_slots
                                .iter()
                                .filter(|(id, c, _)| {
                                    *id == owner
                                        && (all
                                            || matches!(
                                                c,
                                                InstrumentComponent::Envelope(_)
                                                    | InstrumentComponent::Lfo(_)
                                            ))
                                })
                                .map(|(_, c, _)| *c)
                                .collect();
                            for (n, c) in units.iter().enumerate() {
                                click_component(&mut app, &ctx, owner, *c, n > 0);
                            }
                            assert_eq!(
                                app.instrument_ui.selected_components,
                                units.iter().map(|c| (owner, *c)).collect()
                            );
                            app.dirty = false;
                            frame(
                                &mut app,
                                &ctx,
                                vec![component_delete_key(key, false, egui::Modifiers::SHIFT)],
                            );
                            assert!(app.error.is_none(), "{:?}", app.error);
                            assert!(app.dirty);
                            assert!(app.instrument_ui.selected_components.is_empty());
                            let mut expected = before.instrument.clone().unwrap();
                            for c in &units {
                                match *c {
                                    InstrumentComponent::Oscillators => {
                                        expected.layers[0].oscillators.clear();
                                        expected.layers[0].oscillator_modulations.clear();
                                    }
                                    InstrumentComponent::Envelope(id) => {
                                        if main {
                                            expected
                                                .main
                                                .envelopes
                                                .iter_mut()
                                                .find(|e| e.id == id)
                                                .unwrap()
                                                .present = false;
                                        } else {
                                            expected.layers[0]
                                                .envelopes
                                                .iter_mut()
                                                .find(|e| e.id == id)
                                                .unwrap()
                                                .present = false;
                                        }
                                    }
                                    InstrumentComponent::Lfo(id) => {
                                        if main {
                                            expected
                                                .main
                                                .lfos
                                                .iter_mut()
                                                .find(|l| l.id == id)
                                                .unwrap()
                                                .present = false;
                                        } else {
                                            expected.layers[0]
                                                .lfos
                                                .iter_mut()
                                                .find(|l| l.id == id)
                                                .unwrap()
                                                .present = false;
                                        }
                                    }
                                    InstrumentComponent::Macros => {
                                        if main {
                                            expected.main.macro_module.present = false;
                                        } else {
                                            expected.layers[0].macro_module.present = false;
                                        }
                                    }
                                    InstrumentComponent::Amplitude => {
                                        expected.layers[0].amplitude.present = false
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            assert_eq!(
                                app.session.project.tracks[0].instrument,
                                Some(expected.clone())
                            );
                            assert_eq!(app.selected_track, Some(track));
                            assert_eq!(app.selected_clip, Some(clip));
                            assert_eq!(
                                serde_json::to_value(&app.session.project.tracks[0].clips).unwrap(),
                                serde_json::to_value(&before.clips).unwrap()
                            );
                            app.dirty = false;
                            for repeat in [true, true, true, false] {
                                frame(
                                    &mut app,
                                    &ctx,
                                    vec![component_delete_key(key, repeat, egui::Modifiers::NONE)],
                                );
                            }
                            assert_eq!(app.session.project.tracks[0].instrument, Some(expected));
                            assert_eq!(
                                serde_json::to_value(&app.session.project.tracks[0].clips).unwrap(),
                                serde_json::to_value(&before.clips).unwrap()
                            );
                            assert!(!app.dirty);
                            app.session.project.validate().unwrap();
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn component_delete_respects_focus_text_modifiers_modal_and_drag_guards() {
    for guard in [
        "repeat",
        "ctrl",
        "alt",
        "command",
        "outside",
        "surrender",
        "window",
        "disabled",
        "modal",
        "drag",
        "pointer",
        "view",
        "name",
    ] {
        let (mut app, ctx, owner, moving) =
            order_setup(settings::ColorTheme::Dark, true, false, true);
        let track = app.session.project.tracks[0].id;
        let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
        app.selected_clip = Some(clip);
        app.selected_clips.insert(clip);
        click_component(&mut app, &ctx, owner, moving, false);
        let original = app.session.project.tracks[0].instrument.clone();
        let clips = serde_json::to_value(&app.session.project.tracks[0].clips).unwrap();
        let mut modifiers = egui::Modifiers::NONE;
        match guard {
            "repeat" => ctx.input_mut(|input| {
                input.keys_down.insert(egui::Key::Delete);
            }),
            "ctrl" => modifiers = egui::Modifiers::CTRL,
            "alt" => modifiers = egui::Modifiers::ALT,
            "command" => modifiers = egui::Modifiers::COMMAND,
            "outside" => {
                let point = app.midi_editor.layer_bounds[&owner].label.center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(point), event(point, true, false)],
                );
                frame(&mut app, &ctx, vec![event(point, false, false)]);
                // Another pane owns its own clip shortcuts.
                app.selected_clip = None;
                app.selected_clips.clear();
            }
            "surrender" => {
                let id = ctx.memory(|m| m.focused()).unwrap();
                ctx.memory_mut(|m| m.surrender_focus(id));
                app.selected_clip = None;
                app.selected_clips.clear();
            }
            "modal" => app.unsaved_prompt = true,
            "drag" => {
                let p = source(&app, owner, moving);
                start(&mut app, &ctx, p);
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(p + Vec2::new(30.0, 0.0))],
                );
            }
            "pointer" => {
                let p = app.midi_editor.layer_bounds[&owner].oscillators[0].center();
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(p), event(p, true, false)],
                );
            }
            "view" => {
                app.set_view_mode(ViewMode::Mixer, &ctx);
                app.selected_clip = None;
                app.selected_clips.clear();
            }
            "name" => {
                app.track_name_edit = Some(TrackNameEdit {
                    track,
                    text: "MIDI 1".into(),
                    focus: false,
                })
            }
            _ => {}
        }
        app.dirty = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                focused: guard != "window",
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    component_delete_key(egui::Key::Delete, guard == "repeat", modifiers),
                ],
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(guard != "disabled", |ui| app.show(ui));
            },
        );
        output.textures_delta.clear();
        assert_eq!(
            app.session.project.tracks[0].instrument, original,
            "{guard}"
        );
        assert_eq!(
            serde_json::to_value(&app.session.project.tracks[0].clips).unwrap(),
            clips,
            "{guard}"
        );
        assert!(!app.dirty, "{guard}");
    }
}

#[test]
fn component_delete_edits_macro_name_text_without_removing_units() {
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        let (mut app, ctx, owner, _) = order_setup(settings::ColorTheme::Dark, false, true, false);
        let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
        i.main.envelopes[0].collapsed = true;
        i.main.lfos[0].collapsed = true;
        i.main.macro_module.add();
        frame(&mut app, &ctx, vec![]);
        click_component(&mut app, &ctx, owner, InstrumentComponent::Macros, false);
        super::macros::begin_macro_rename(&mut app, &ctx, "Macro 1");
        let before = app.session.project.tracks[0].instrument.clone();
        app.dirty = false;
        frame(
            &mut app,
            &ctx,
            vec![component_delete_key(key, false, egui::Modifiers::NONE)],
        );
        assert!(app.instrument_ui.macro_name_edit.is_some());
        assert_eq!(app.session.project.tracks[0].instrument, before);
        assert!(!app.dirty);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::Text("Tone".into()),
                component_delete_key(egui::Key::Enter, false, egui::Modifiers::NONE),
            ],
        );
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .macro_module
                .controls[0]
                .display_name(0),
            "Tone"
        );
    }
}

#[test]
fn component_select_all_is_lane_scoped_including_scrolled_units_and_keeps_the_project_clean() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for compact in [false, true] {
            for main in [false, true] {
                for collapsed in [false, true] {
                    let (mut app, ctx, owner, moving) =
                        order_setup(theme, compact, main, collapsed);
                    click_component(&mut app, &ctx, owner, moving, false);
                    let original = serde_json::to_value(&app.session.project).unwrap();
                    let tracks = app.selected_tracks.clone();
                    let clips = app.selected_clips.clone();
                    let expected: HashSet<_> = app
                        .instrument_ui
                        .component_slots
                        .iter()
                        .filter(|(id, _, _)| *id == owner)
                        .map(|(id, c, _)| (*id, *c))
                        .collect();
                    app.dirty = false;
                    frame(
                        &mut app,
                        &ctx,
                        vec![component_delete_key(
                            egui::Key::A,
                            false,
                            egui::Modifiers::COMMAND,
                        )],
                    );
                    assert_eq!(
                        app.instrument_ui.selected_components, expected,
                        "{main} {compact} {collapsed}"
                    );
                    assert!(app.selected_effects.is_empty());
                    assert_eq!(app.selected_tracks, tracks);
                    assert_eq!(app.selected_clips, clips);
                    assert_eq!(
                        serde_json::to_value(&app.session.project).unwrap(),
                        original
                    );
                    assert!(!app.dirty);
                    app.instrument_ui.selected_components.clear();
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::Key {
                            key: egui::Key::A,
                            physical_key: None,
                            pressed: false,
                            repeat: false,
                            modifiers: egui::Modifiers::COMMAND,
                        }],
                    );
                    frame(
                        &mut app,
                        &ctx,
                        vec![component_delete_key(
                            egui::Key::A,
                            false,
                            egui::Modifiers::COMMAND,
                        )],
                    );
                    assert_eq!(app.instrument_ui.selected_components, expected);
                    assert!(!app.dirty);
                }
            }
        }
    }
}

#[test]
fn component_select_all_keeps_macro_text_selection_in_its_editor() {
    let (mut app, ctx, owner, _) = order_setup(settings::ColorTheme::Dark, false, true, false);
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.main.envelopes[0].collapsed = true;
    i.main.lfos[0].collapsed = true;
    i.main.macro_module.add();
    frame(&mut app, &ctx, vec![]);
    click_component(&mut app, &ctx, owner, InstrumentComponent::Macros, false);
    super::macros::begin_macro_rename(&mut app, &ctx, "Macro 1");
    frame(&mut app, &ctx, vec![egui::Event::Text("Long name".into())]);
    let selected = app.instrument_ui.selected_components.clone();
    frame(
        &mut app,
        &ctx,
        vec![component_delete_key(
            egui::Key::A,
            false,
            egui::Modifiers::COMMAND,
        )],
    );
    assert_eq!(app.instrument_ui.selected_components, selected);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::Text("Tone".into()),
            component_delete_key(egui::Key::Enter, false, egui::Modifiers::NONE),
        ],
    );
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .main
            .macro_module
            .controls[0]
            .display_name(0),
        "Tone"
    );
}

#[test]
fn component_select_all_respects_an_empty_focused_lane_when_components_are_added() {
    let (mut app, ctx, owners) = setup(settings::ColorTheme::Dark, true);
    let viewport = app
        .instrument_ui
        .component_viewports
        .iter()
        .find(|(id, _)| *id == owners[1])
        .unwrap()
        .1;
    let point = viewport.center();
    super::instrument_catalog::click(&mut app, &ctx, point, false);
    frame(
        &mut app,
        &ctx,
        vec![component_delete_key(
            egui::Key::A,
            false,
            egui::Modifiers::COMMAND,
        )],
    );
    assert!(app.instrument_ui.selected_components.is_empty());
    assert!(!app.dirty);
    let i = app.session.project.tracks[0].instrument.as_mut().unwrap();
    i.layers[1].macro_module.add();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
    );
    frame(
        &mut app,
        &ctx,
        vec![component_delete_key(
            egui::Key::A,
            false,
            egui::Modifiers::COMMAND,
        )],
    );
    assert_eq!(
        app.instrument_ui.selected_components,
        HashSet::from([(owners[1], InstrumentComponent::Macros)])
    );
    assert!(!app.dirty);
}

#[test]
fn component_select_all_from_populated_lane_space_without_selecting_a_unit() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for main in [false, true] {
            for compact in [false, true] {
                for immediate in [false, true] {
                    let (mut app, ctx, owner, _) = order_setup(theme, compact, main, true);
                    let viewport = app
                        .instrument_ui
                        .component_viewports
                        .iter()
                        .find(|(id, _)| *id == owner)
                        .unwrap()
                        .1;
                    let slots: Vec<_> = app
                        .instrument_ui
                        .component_slots
                        .iter()
                        .filter(|(id, _, _)| *id == owner)
                        .copied()
                        .collect();
                    let end = slots.last().unwrap().2.right();
                    let point = Pos2::new((end + viewport.right()) * 0.5, viewport.center().y);
                    assert!(
                        viewport.contains(point)
                            && !slots.iter().any(|(_, _, r)| r.contains(point))
                    );
                    assert!(app.instrument_ui.selected_components.is_empty());
                    let original = serde_json::to_value(&app.session.project).unwrap();
                    frame(
                        &mut app,
                        &ctx,
                        vec![egui::Event::PointerMoved(point), event(point, true, false)],
                    );
                    let mut events = vec![event(point, false, false)];
                    if immediate {
                        events.push(component_delete_key(
                            egui::Key::A,
                            false,
                            egui::Modifiers::COMMAND,
                        ));
                    }
                    frame(&mut app, &ctx, events);
                    if !immediate {
                        assert!(app.instrument_ui.component_focus.owns(&ctx));
                        assert!(app.instrument_ui.selected_components.is_empty());
                        // Real applications have idle paint frames between the click and shortcut.
                        for _ in 0..3 {
                            frame(&mut app, &ctx, vec![]);
                        }
                        frame(
                            &mut app,
                            &ctx,
                            vec![component_delete_key(
                                egui::Key::A,
                                false,
                                egui::Modifiers::COMMAND,
                            )],
                        );
                    }
                    assert_eq!(
                        app.instrument_ui.selected_components,
                        slots.iter().map(|(id, c, _)| (*id, *c)).collect(),
                        "main={main}, compact={compact}, immediate={immediate}"
                    );
                    assert_eq!(
                        serde_json::to_value(&app.session.project).unwrap(),
                        original
                    );
                    assert!(!app.dirty);
                }
            }
        }
    }
}
