//! Compare complete editor output against the built-in portable body, for every product.
use super::*;
use daw_drive_plugin::{Parameter, PluginState};
use egui::{Event, Rect, Vec2};
use std::sync::{Arc, atomic::Ordering};
fn context(appearance: u32, scale: f32) -> egui::Context {
    let ctx = egui::Context::default();
    daw_unit_ui::fonts::configure(&ctx);
    daw_unit_ui::theme::configure(&ctx);
    daw_unit_ui::theme::apply(
        &ctx,
        if appearance == 0 {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        },
        if appearance == 2 {
            daw_unit_ui::theme::MIST
        } else {
            daw_unit_ui::theme::LIGHT
        },
        daw_unit_ui::theme::PLUGIN_ACCENT,
    );
    ctx.set_pixels_per_point(scale);
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    ctx
}
fn paint(ctx: &egui::Context, draw: impl FnMut(&mut egui::Ui)) -> Vec<egui::ClippedPrimitive> {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(WIDTH as f32, HEIGHT as f32),
            )),
            focused: true,
            events: vec![Event::PointerGone],
            ..Default::default()
        },
        draw,
    );
    out.textures_delta.clear();
    ctx.tessellate(out.shapes, ctx.pixels_per_point())
}
#[test]
fn editor_meshes_match_builtin_body_in_every_theme_scale_and_parameter_snapshot() {
    for appearance in 0..3 {
        for scale in [1.0f32, 1.5, 2.0] {
            for variant in 0..3 {
                let mut state = PluginState::default();
                if variant != 0 {
                    for p in Parameter::ALL.into_iter().skip(1) {
                        state
                            .set(
                                p,
                                p.denormalize(if variant == 1 { 0.31 } else { 0.79 })
                                    .unwrap(),
                            )
                            .unwrap();
                    }
                }
                let model = Arc::new(Model::default());
                model.publish(state);
                model.appearance.store(appearance, Ordering::Relaxed);
                model.scale.store(scale.to_bits(), Ordering::Relaxed);
                let actual_context = context(appearance, scale);
                let mut editor = Editor::new(model.clone(), || {});
                let actual = paint(&actual_context, |ui| editor.show(ui));
                let expected_context = context(appearance, scale);
                let mut settings = state.to_settings();
                let expected = paint(&expected_context, |ui| {
                    ui.painter().rect_filled(
                        ui.max_rect(),
                        0.0,
                        daw_unit_ui::theme::unit_separator(ui.ctx()),
                    );
                    super::paint_reference(ui, &mut settings, &model);
                });
                assert_eq!(
                    actual.len(),
                    expected.len(),
                    "primitive count: appearance={appearance}, scale={scale}, snapshot={variant}"
                );
                for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
                    assert_eq!(
                        a.clip_rect, b.clip_rect,
                        "clip {i}: appearance={appearance}, scale={scale}, snapshot={variant}"
                    );
                    match (&a.primitive, &b.primitive) {
                        (egui::epaint::Primitive::Mesh(a), egui::epaint::Primitive::Mesh(b)) => {
                            assert_eq!(a.texture_id, b.texture_id, "texture {i}");
                            assert_eq!(
                                a.indices, b.indices,
                                "triangles {i}: appearance={appearance}, scale={scale}, snapshot={variant}"
                            );
                            assert_eq!(a.vertices.len(), b.vertices.len(), "vertices {i}");
                            for (n, (a, b)) in a.vertices.iter().zip(&b.vertices).enumerate() {
                                assert!(
                                    a.pos.distance(b.pos) < 0.001
                                        && a.uv.distance(b.uv) < 0.001
                                        && a.color == b.color,
                                    "vertex {i}/{n}: actual={a:?}, builtin={b:?}, appearance={appearance}, scale={scale}, snapshot={variant}"
                                );
                            }
                        }
                        _ => panic!("unexpected paint callback in unit body"),
                    }
                }
                assert!(
                    model.pop().is_none(),
                    "painting must not enqueue parameter edits"
                );
            }
        }
    }
}

#[test]
fn builtin_control_gestures_emit_only_their_bound_host_parameter() {
    fn event(pos: egui::Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        out.textures_delta.clear();
    }
    for sync in [false, true] {
        let mut state = PluginState::default();
        state.set(Parameter::Sync, f64::from(sync)).unwrap();
        let probe = context(0, 1.0);
        let mut settings = state.to_settings();
        let mut controls = vec![];
        paint(&probe, |ui| {
            controls = super::probe_controls(ui, &mut settings)
        });
        for (parameter, rect) in controls {
            if parameter.info().steps > 0 && !parameter.info().key.ends_with("_enabled") {
                continue;
            }
            let screen =
                Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32));
            if !screen.contains_rect(rect) {
                continue;
            }
            // Dropdown selectors use menu selection, not the knob drag gesture below.
            if (daw_drive_plugin::IDENTITY.product == "Saturator"
                && matches!(parameter.info().key, "shape" | "clip_mode"))
                || (daw_drive_plugin::IDENTITY.product == "Drive v2"
                    && parameter.info().key == "clip")
                || (daw_drive_plugin::IDENTITY.product == "Harmonic Lift"
                    && parameter.info().key == "quality")
            {
                continue;
            }
            let model = Arc::new(Model::default());
            model.publish(state);
            let ctx = context(0, 1.0);
            let mut editor = Editor::new(model.clone(), || {});
            frame(&ctx, &mut editor, vec![]);
            let start = rect.center();
            frame(
                &ctx,
                &mut editor,
                vec![Event::PointerMoved(start), event(start, true)],
            );
            if parameter.info().scale == daw_drive_plugin::Scale::Boolean
                || parameter.info().key == "sync"
            {
                frame(&ctx, &mut editor, vec![event(start, false)]);
            } else {
                let direction = if parameter.normalize(state.get(parameter)).unwrap() > 0.95 {
                    1.0
                } else {
                    -1.0
                };
                let end = start + egui::vec2(0.0, direction * 120.0);
                for fraction in [0.2, 0.5, 1.0] {
                    frame(
                        &ctx,
                        &mut editor,
                        vec![Event::PointerMoved(start + (end - start) * fraction)],
                    );
                }
                frame(&ctx, &mut editor, vec![event(end, false)]);
            }
            let mut began = false;
            let mut ended = false;
            let mut values = 0;
            let mut changed = false;
            while let Some(edit) = model.pop() {
                match edit {
                    Edit::Presentation(_) => panic!("parameter gestures must not reorder strips"),
                    Edit::Begin(p) => {
                        assert_eq!(p, parameter, "wrong gesture target");
                        began = true;
                    }
                    Edit::Value(p, value) => {
                        assert_eq!(p, parameter, "wrong value target");
                        assert!(began && !ended);
                        changed |= value != state.get(p);
                        values += 1;
                    }
                    Edit::End(p) => {
                        assert_eq!(p, parameter, "wrong end target");
                        assert!(began);
                        ended = true;
                    }
                }
            }
            assert!(
                began && ended && values > 0 && changed,
                "{parameter:?}: missing balanced UI edit"
            );
        }
    }
}

#[test]
fn new_plugin_defaults_match_builtin_defaults() {
    super::assert_builtin_defaults();
}

#[test]
fn dropdowns_publish_balanced_gestures_for_mode_note_and_warm() {
    fn frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        events: Vec<Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        out.textures_delta.clear();
        out.shapes
    }
    fn button(pos: egui::Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }
    }
    fn label(shape: &egui::Shape, name: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == name => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| label(s, name)),
            _ => None,
        }
    }
    for (parameter, slot, text, value) in [
        (Parameter::Sync, 2, "Sync", 1.0),
        (Parameter::Note, 3, "1/8T", 5.0),
        (Parameter::Warm, 14, "On", 1.0),
    ] {
        let mut state = PluginState::default();
        if parameter == Parameter::Note {
            state.set(Parameter::Sync, 1.0).unwrap();
        }
        let mut rect = Rect::NOTHING;
        paint(&context(0, 1.0), |ui| {
            rect = body::render(
                ui,
                &mut state.to_settings(),
                0.0.into(),
                0.0.into(),
                [0.0; 3].into(),
            )
            .controls[slot]
                .as_ref()
                .unwrap()
                .rect;
        });
        let model = Arc::new(Model::default());
        model.publish(state);
        let ctx = context(0, 1.0);
        let mut editor = Editor::new(model.clone(), || {});
        frame(&ctx, &mut editor, vec![]);
        let pos = rect.center();
        frame(
            &ctx,
            &mut editor,
            vec![Event::PointerMoved(pos), button(pos, true)],
        );
        frame(&ctx, &mut editor, vec![button(pos, false)]);
        let shapes = frame(&ctx, &mut editor, vec![]);
        let target = shapes
            .iter()
            .find_map(|s| label(&s.shape, text))
            .unwrap_or_else(|| panic!("missing {parameter:?} dropdown option {text}"));
        frame(
            &ctx,
            &mut editor,
            vec![Event::PointerMoved(target), button(target, true)],
        );
        frame(&ctx, &mut editor, vec![button(target, false)]);
        let mut edits = vec![];
        while let Some(edit) = model.pop() {
            edits.push(edit);
        }
        assert_eq!(
            edits,
            vec![
                Edit::Begin(parameter),
                Edit::Value(parameter, value),
                Edit::End(parameter)
            ]
        );
    }
}
