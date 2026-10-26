use crate::theme;
use daw_core::{SILENT_GAIN_DB, linear_gain};
use egui::{Pos2, Response, Stroke, Ui, Vec2};

const HANDLE_ASPECT: f32 = 0.5;
pub const SCALE_WIDTH: f32 = 30.0;
pub use daw_unit_ui::gain::{GAIN_MARKS, MAX_GAIN_DB, format_gain};
pub fn parse_gain(text: &str) -> Option<f32> {
    daw_unit_core::gain::parse_db(text)
        .map(daw_unit_core::gain::to_db_f32)
        .filter(|db| db.is_finite() && linear_gain(*db).is_finite())
}
pub fn clamp_gain(db: f32) -> f32 {
    db.clamp(SILENT_GAIN_DB, MAX_GAIN_DB)
}

/// Position from the bottom of the handle travel, shared by the handle and scale marks.
pub fn position_for_gain(db: f32) -> f32 {
    daw_unit_core::gain::fader_position(f64::from(db)) as f32
}
pub fn gain_at_position(position: f32) -> f32 {
    daw_unit_core::gain::fader_db(f64::from(position)) as f32
}

/// Match the slider rail width when placing controls above it.
pub fn rail_width(ui: &Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body)
        .max(ui.spacing().interact_size.y)
}

/// Standard gain fader, from silence to +12 dB. Idle display preserves out-of-range values.
pub fn gain(ui: &mut Ui, value: &mut f32, height: f32) -> Response {
    let previous = *value;
    let mut response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = height;
            let (pointer_edit, pressed, shift, delta_y) = ui.input(|input| {
                (
                    input.pointer.primary_down() || input.pointer.primary_released(),
                    input.pointer.primary_pressed(),
                    input.modifiers.shift,
                    input.pointer.delta().y,
                )
            });
            let thickness = rail_width(ui);
            let travel = (height - 2.0 * thickness / 2.5 * HANDLE_ASPECT).max(1.0);
            let step = -delta_y / travel * if shift { 0.1 } else { 1.0 };
            let mut first_set = true;
            ui.allocate_ui_with_layout(
                Vec2::new(SCALE_WIDTH + thickness, height),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.allocate_space(Vec2::new(SCALE_WIDTH, height));
                    ui.add(
                        egui::Slider::from_get_set(0.0..=1.0, |new_position| {
                            if let Some(new_position) = new_position {
                                if pointer_edit && first_set {
                                    // Relative travel lets Shift change without jumping to the pointer.
                                    if pressed {
                                        if !shift {
                                            *value = gain_at_position(new_position as f32);
                                        }
                                    } else if step != 0.0 {
                                        *value = gain_at_position(position_for_gain(*value) + step);
                                    }
                                } else {
                                    *value = gain_at_position(new_position as f32);
                                }
                                first_set = false;
                            }
                            f64::from(position_for_gain(*value))
                        })
                        .vertical()
                        .smart_aim(false)
                        .clamping(egui::SliderClamping::Edits)
                        .handle_shape(egui::style::HandleShape::Rect {
                            aspect_ratio: HANDLE_ASPECT,
                        })
                        .show_value(false)
                        .trailing_fill(false),
                    )
                },
            )
            .inner
        })
        .inner;
    // Slider senses drags, so its Response does not report clicks. Read the release
    // event within this rail to keep the standard double-click reset available.
    if ui.is_enabled()
        && response.contains_pointer()
        && ui.input(|input| {
            input.focused
                && input
                    .pointer
                    .button_double_clicked(egui::PointerButton::Primary)
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| response.rect.contains(pos))
        })
    {
        *value = 0.0;
    }
    if *value != previous {
        response.mark_changed();
    }
    response
        .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), "Gain (dB)"));
    if height >= 120.0 {
        paint_scale(ui, &response, height);
    }
    response.on_hover_text(format!(
        "Gain: {} dB. Hold Shift for fine adjustment. Double-click to reset.",
        format_gain(*value)
    ))
}

fn paint_scale(ui: &Ui, response: &Response, height: f32) {
    let palette = theme::palette(ui.ctx());
    let inset = response.rect.width() / 2.5 * HANDLE_ASPECT;
    let rail = response.rect.shrink2(Vec2::new(0.0, inset));
    let y_at = |db| egui::lerp(rail.bottom()..=rail.top(), position_for_gain(db));
    let right = response.rect.left() - 3.0;
    for db in GAIN_MARKS {
        let y = y_at(db);
        let unity = db == 0.0;
        ui.painter().line_segment(
            [
                Pos2::new(right - if unity { 8.0 } else { 6.0 }, y),
                Pos2::new(right, y),
            ],
            Stroke::new(
                if unity { 1.5 } else { 1.0 },
                if unity {
                    palette.text
                } else {
                    palette.secondary
                },
            ),
        );
        // Small windows retain the scale marks and show only the main reference labels.
        if height >= 160.0 || unity || db == SILENT_GAIN_DB {
            let label = if db == SILENT_GAIN_DB {
                "−∞".into()
            } else if db > 0.0 {
                format!("+{db:.0}")
            } else {
                format!("{db:.0}")
            };
            let label =
                ui.painter()
                    .layout_no_wrap(label, crate::fonts::numeric(8.0), palette.text);
            if db <= 0.0
                || db == MAX_GAIN_DB
                || (y_at(3.0) - y_at(0.0)).abs() >= label.size().y + 0.25
            {
                ui.painter().galley(
                    Pos2::new(right - 10.0 - label.size().x, y - label.size().y / 2.0),
                    label,
                    palette.text,
                );
            }
        }
    }
    for (low, high, step) in [(-60, -40, 5), (-40, -20, 2), (-20, 0, 1), (0, 12, 1)] {
        for db in (low..high).step_by(step as usize) {
            let db = db as f32;
            if GAIN_MARKS.contains(&db) {
                continue;
            }
            let y = y_at(db);
            // Omit subdivisions when the available travel would pack them too tightly.
            if (y - y_at(db + step as f32)).abs() >= 3.0 {
                ui.painter().line_segment(
                    [Pos2::new(right - 3.0, y), Pos2::new(right, y)],
                    Stroke::new(1.0, palette.border),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_scale_is_monotonic_reversible_and_reaches_exact_silence() {
        assert_eq!(linear_gain(gain_at_position(0.0)), 0.0);
        assert_eq!(gain_at_position(1.0), MAX_GAIN_DB);
        let mut previous = SILENT_GAIN_DB;
        for step in 0..=1000 {
            let position = step as f32 / 1000.0;
            let db = gain_at_position(position);
            assert!(db.is_finite() && db >= previous);
            assert!((position_for_gain(db) - position).abs() < 0.00001);
            if step > 0 {
                assert!(linear_gain(db) > 0.0);
            }
            previous = db;
        }
        for db in [
            -90.0, -60.0, -40.0, -30.0, -20.0, -15.0, -10.0, -6.0, -3.0, 0.0, 12.0,
        ] {
            assert!((gain_at_position(position_for_gain(db)) - db).abs() < 0.0001);
        }
        for text in ["−∞", "-∞", "-inf", " -INFINITY "] {
            assert_eq!(parse_gain(text), Some(SILENT_GAIN_DB));
        }
        assert_eq!(format_gain(SILENT_GAIN_DB), "−∞");
        assert_eq!(parse_gain("-90.0"), Some(-90.0));
        for text in ["NaN", "+inf", "inf", "∞", "-", "bad", "10000"] {
            assert_eq!(parse_gain(text), None);
        }
    }

    #[test]
    fn fader_leaves_silence_resets_to_unity_and_preserves_idle_values() {
        for initial in [SILENT_GAIN_DB, -90.0, 20.0] {
            let ctx = context();
            let mut value = initial;
            assert!(!frame(&ctx, &mut value, vec![], false).changed());
            assert_eq!(value, initial);
        }
        let ctx = context();
        let mut value = SILENT_GAIN_DB;
        let point = start(&ctx, &mut value, false);
        let end = point - Vec2::new(0.0, 5.0);
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(end)],
            false,
        );
        assert!(linear_gain(value) > 0.0 && value < -60.0);
        frame(&ctx, &mut value, vec![button(end, false)], false);
        let before = value;
        frame(&ctx, &mut value, vec![], false);
        assert_eq!(value, before);
        let ctx = context();
        value = SILENT_GAIN_DB;
        let rect = frame(&ctx, &mut value, vec![], false).rect;
        let point = Pos2::new(rect.center().x, rect.bottom());
        for _ in 0..2 {
            frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point), button(point, true)],
                false,
            );
            frame(&ctx, &mut value, vec![button(point, false)], false);
        }
        assert_eq!(value, 0.0);
    }

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        ctx
    }

    fn frame(
        ctx: &egui::Context,
        value: &mut f32,
        events: Vec<egui::Event>,
        shift: bool,
    ) -> Response {
        let mut events = events;
        events.push(egui::Event::ModifiersChanged(egui::Modifiers {
            shift,
            ..Default::default()
        }));
        let mut response = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0))),
                events,
                ..Default::default()
            },
            |ui| response = Some(gain(ui, value, 220.0)),
        );
        output.textures_delta.clear();
        response.unwrap()
    }

    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }
    }

    fn start(ctx: &egui::Context, value: &mut f32, shift: bool) -> Pos2 {
        let rect = frame(ctx, value, vec![], shift).rect;
        let inset = rect.width() / 2.5 * HANDLE_ASPECT;
        let y = egui::lerp(
            (rect.bottom() - inset)..=(rect.top() + inset),
            position_for_gain(*value),
        );
        let point = Pos2::new(rect.center().x, y);
        frame(
            ctx,
            value,
            vec![egui::Event::PointerMoved(point), button(point, true)],
            shift,
        );
        point
    }

    #[test]
    fn shift_drag_is_ten_times_finer_and_idle_or_release_do_not_change_gain() {
        let mut changes = Vec::new();
        for fine in [false, true] {
            let ctx = context();
            let mut value = -12.0;
            let point = start(&ctx, &mut value, fine);
            assert!((value + 12.0).abs() < 0.001);
            let end = point - Vec2::new(0.0, 3.0);
            assert!(frame(&ctx, &mut value, vec![egui::Event::PointerMoved(end)], fine).changed());
            let first = value + 12.0;
            frame(&ctx, &mut value, vec![], fine);
            assert!((value + 12.0 - first).abs() < 0.001);
            let end = end - Vec2::new(0.0, 3.0);
            frame(&ctx, &mut value, vec![egui::Event::PointerMoved(end)], fine);
            assert!((value + 12.0 - 2.0 * first).abs() < 0.001);
            let before_release = value;
            frame(&ctx, &mut value, vec![button(end, false)], fine);
            assert_eq!(value, before_release);
            changes.push(value + 12.0);
        }
        assert!((changes[0] / changes[1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn shift_can_change_mid_drag_without_jumps_and_gain_stays_in_range() {
        let ctx = context();
        let mut value = 0.0;
        let mut point = start(&ctx, &mut value, false);
        point.y += 10.0;
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point)],
            false,
        );
        let normal = value;
        let normal_position = position_for_gain(value);
        frame(&ctx, &mut value, vec![], true);
        assert_eq!(value, normal);
        point.y += 10.0;
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point)],
            true,
        );
        assert!(
            (position_for_gain(value)
                - normal_position
                - (normal_position - position_for_gain(0.0)) * 0.1)
                .abs()
                < 0.00001
        );
        let fine = value;
        frame(&ctx, &mut value, vec![], false);
        assert_eq!(value, fine);
        point.y += 2000.0;
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point)],
            true,
        );
        assert_eq!(value, SILENT_GAIN_DB);
        point.y -= 5000.0;
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point)],
            true,
        );
        assert_eq!(value, 12.0);
        frame(&ctx, &mut value, vec![button(point, false)], false);
        assert_eq!(value, 12.0);
    }
}

#[cfg(test)]
mod finite_gain_text_tests {
    #[test]
    fn finite_db_input_keeps_its_numeric_readout_above_the_sentinel() {
        assert_eq!(super::format_gain(-950.0), "-950.0");
        assert_eq!(super::parse_gain("-950 dB"), Some(-950.0));
        let near_silence = super::parse_gain("-999.9999999 dB").unwrap();
        assert!(near_silence > daw_core::SILENT_GAIN_DB);
        assert_ne!(super::format_gain(near_silence), "−∞");
        assert_eq!(
            super::parse_gain(&super::format_gain(near_silence)),
            Some(near_silence)
        );
        assert_eq!(super::parse_gain("−∞ dB"), Some(daw_core::SILENT_GAIN_DB));
    }
}
