use crate::theme;
use egui::{Pos2, Response, Stroke, Ui, Vec2};

const HANDLE_ASPECT: f32 = 0.5;

/// Standard gain fader, in dB. Display preserves values outside the drag range.
pub fn gain(ui: &mut Ui, value: &mut f32, height: f32) -> Response {
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
            let thickness = ui
                .text_style_height(&egui::TextStyle::Body)
                .max(ui.spacing().interact_size.y);
            let travel = (height - 2.0 * thickness / 2.5 * HANDLE_ASPECT).max(1.0);
            let step = -delta_y * 72.0 / travel * if shift { 0.1 } else { 1.0 };
            let mut first_set = true;
            ui.add(
                egui::Slider::from_get_set(-60.0..=12.0, |new_value| {
                    if let Some(new_value) = new_value {
                        if pointer_edit && first_set {
                            // Relative motion keeps Shift changes and release from jumping
                            // back to the pointer's absolute rail position.
                            if pressed {
                                if !shift {
                                    *value = new_value as f32;
                                }
                            } else if step != 0.0 {
                                *value = (*value + step).clamp(-60.0, 12.0);
                            }
                        } else {
                            *value = new_value as f32;
                        }
                        first_set = false;
                    }
                    f64::from(*value)
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
        })
        .inner;
    if response.double_clicked() && *value != 0.0 {
        *value = 0.0;
        response.mark_changed();
    }
    response
        .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), "Gain (dB)"));
    if height >= 120.0 {
        let palette = theme::palette(ui.ctx());
        // egui maps values between the rectangular handle's end positions.
        let handle_inset = response.rect.width() / 2.5 * HANDLE_ASPECT;
        let rail = response.rect.shrink2(Vec2::new(0.0, handle_inset));
        for db in [-60, -48, -36, -24, -12, 0, 12] {
            let y = egui::lerp(rail.bottom()..=rail.top(), (db as f32 + 60.0) / 72.0);
            let right = response.rect.left() - 3.0;
            ui.painter().line_segment(
                [Pos2::new(right - 3.0, y), Pos2::new(right, y)],
                Stroke::new(1.0, palette.border),
            );
            ui.painter().text(
                Pos2::new(right - 5.0, y),
                egui::Align2::RIGHT_CENTER,
                db.to_string(),
                egui::FontId::proportional(8.0),
                palette.text,
            );
        }
    }
    response.on_hover_text("Gain (dB). Hold Shift for fine adjustment. Double-click to reset.")
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let y = rect.top() + inset + (rect.height() - 2.0 * inset) * (12.0 - *value) / 72.0;
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
            let ctx = egui::Context::default();
            let mut value = -12.0;
            let point = start(&ctx, &mut value, fine);
            assert!((value + 12.0).abs() < 0.001);
            let end = point - Vec2::new(0.0, 20.0);
            assert!(frame(&ctx, &mut value, vec![egui::Event::PointerMoved(end)], fine).changed());
            let first = value + 12.0;
            frame(&ctx, &mut value, vec![], fine);
            assert!((value + 12.0 - first).abs() < 0.001);
            let end = end - Vec2::new(0.0, 20.0);
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
        let ctx = egui::Context::default();
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
        frame(&ctx, &mut value, vec![], true);
        assert_eq!(value, normal);
        point.y += 10.0;
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point)],
            true,
        );
        assert!((value - normal - normal * 0.1).abs() < 0.001);
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
        assert_eq!(value, -60.0);
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
