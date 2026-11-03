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
            for variant in 0..5 {
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
                if variant == 3 {
                    state.set(Parameter::Bypass, 1.0).unwrap();
                }
                let model = Arc::new(Model::default());
                let mut reading = daw_unit_core::LoudnessReading::default();
                reading.values[..14].copy_from_slice(&[
                    -18.0,
                    -20.0,
                    -19.0,
                    -10.0,
                    -15.0,
                    -1.0,
                    8.0,
                    -26.0,
                    -18.0,
                    -2.0,
                    18.0,
                    18.0,
                    180.0,
                    if variant == 4 { 1.0 } else { 0.0 },
                ]);
                for (i, value) in reading.values[14..].iter_mut().enumerate() {
                    *value = -24.0 + (i as f32 * 0.1).sin() * 8.0;
                }
                model
                    .output_meter
                    .loudness
                    .publish(model.output_meter.loudness.epoch(), reading);
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
                        daw_unit_ui::theme::palette(ui.ctx()).panel,
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
fn hold_and_reset_buttons_send_analysis_commands_without_host_parameter_edits() {
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
    fn click(ctx: &egui::Context, editor: &mut Editor, pos: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                editor,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    let model = Arc::new(Model::default());
    let before = model.current();
    let epoch = model.output_meter.loudness.epoch();
    let probe = context(0, 1.0);
    let mut settings = before.unwrap().to_settings();
    let mut hold = Rect::NOTHING;
    let mut reset = Rect::NOTHING;
    paint(&probe, |ui| {
        let b = daw_unit_ui::loudness::show(
            ui,
            &mut settings,
            false,
            daw_unit_ui::layout::unit_control_height(),
            Some(&model.output_meter.loudness),
            0.0.into(),
        );
        hold = b.hold;
        reset = b.reset;
    });
    let ctx = context(0, 1.0);
    let mut editor = Editor::new(model.clone(), || {});
    frame(&ctx, &mut editor, vec![]);
    click(&ctx, &mut editor, hold.center());
    assert!(model.output_meter.loudness.held());
    assert_eq!(model.meter_reset.swap(0, Ordering::AcqRel), 512);
    click(&ctx, &mut editor, reset.center());
    assert_ne!(model.output_meter.loudness.epoch(), epoch);
    assert_eq!(model.meter_reset.swap(0, Ordering::AcqRel), 256);
    assert_eq!(model.current(), before);
    assert!(model.pop().is_none());
}

#[test]
fn all_strip_footers_share_the_standard_bottom_baseline() {
    for appearance in 0..3 {
        for scale in [1.0, 1.5, 2.0] {
            let ctx = context(appearance, scale);
            let mut settings = daw_unit_core::LoudnessSettings::default();
            paint(&ctx, |ui| {
                let b = daw_unit_ui::loudness::show(
                    ui,
                    &mut settings,
                    false,
                    daw_unit_ui::layout::unit_control_height(),
                    None,
                    0.0.into(),
                );
                for footer in b.footers {
                    assert!(
                        (footer.center().y - b.footers[0].center().y).abs() <= 0.5,
                        "unequal footer positions at scale {scale}: {:?}",
                        b.footers
                    );
                    assert!(
                        (footer.bottom()
                            - (HEIGHT as f32 - daw_unit_ui::layout::PANEL_MARGIN as f32))
                            .abs()
                            <= 0.5,
                        "footer must sit at the common bottom margin: {footer:?}"
                    );
                }
            });
        }
    }
}
