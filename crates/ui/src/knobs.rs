use crate::theme;
use egui::{Pos2, Response, Sense, Shape, Stroke, Ui, Vec2};
use std::f32::consts::PI;

pub const SIZE: f32 = 28.0;

#[derive(Clone, Copy)]
enum Parameter {
    Gain,
    Pan,
}

impl Parameter {
    fn name(self) -> &'static str {
        match self {
            Self::Gain => "Gain dB",
            Self::Pan => "Pan",
        }
    }

    fn normalized(self, value: f32) -> f32 {
        match self {
            Self::Gain if value <= 0.0 => 0.5 + value / 120.0,
            Self::Gain => 0.5 + value / 24.0,
            Self::Pan => (value + 1.0) / 2.0,
        }
        .clamp(0.0, 1.0)
    }

    fn value(self, normalized: f32) -> f32 {
        let normalized = normalized.clamp(0.0, 1.0);
        match self {
            Self::Gain if normalized <= 0.5 => (normalized - 0.5) * 120.0,
            Self::Gain => (normalized - 0.5) * 24.0,
            Self::Pan => normalized * 2.0 - 1.0,
        }
    }

    fn step(self) -> f32 {
        match self {
            Self::Gain => 0.1,
            Self::Pan => 0.01,
        }
    }
}

/// Gain has a fixed 0 dB mark at twelve o'clock; dragging covers -60 to +12 dB.
pub fn gain(ui: &mut Ui, value: &mut f32) -> Response {
    knob(ui, value, Parameter::Gain)
}

/// Pan has a fixed center mark and a bipolar value arc.
pub fn pan(ui: &mut Ui, value: &mut f32) -> Response {
    knob(ui, value, Parameter::Pan)
}

fn knob(ui: &mut Ui, value: &mut f32, parameter: Parameter) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click_and_drag());
    let previous = *value;
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if response.dragged() {
        let fine = ui.input(|i| if i.modifiers.shift { 0.1 } else { 1.0 });
        let delta = response.drag_delta();
        let normalized = parameter.normalized(*value) + (delta.x - delta.y) * fine / 150.0;
        *value = parameter.value(normalized);
    }
    if response.has_focus() && ui.is_enabled() {
        let change = ui.input(|i| {
            f32::from(i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight))
                - f32::from(
                    i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft),
                )
        });
        if change != 0.0 {
            let adjusted = (*value / parameter.step() + change).round() * parameter.step();
            *value = parameter.value(parameter.normalized(adjusted));
        }
    }
    if response.double_clicked() {
        *value = 0.0;
    }
    if *value != previous {
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), parameter.name())
    });

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();
        let radius = SIZE / 2.0 - 2.0;
        let start = 0.75 * PI;
        let sweep = 1.5 * PI;
        let normalized = parameter.normalized(*value);
        let point = |fraction: f32, radius: f32| {
            let angle = start + fraction * sweep;
            center + Vec2::angled(angle) * radius
        };
        let arc = |from: f32, to: f32, color| {
            let points: Vec<Pos2> = (0..=32)
                .map(|i| point(from + (to - from) * i as f32 / 32.0, radius))
                .collect();
            painter.add(Shape::line(points, Stroke::new(2.0, color)));
        };
        arc(0.0, 1.0, theme::palette(ui.ctx()).border);
        let origin = match parameter {
            Parameter::Gain => 0.0,
            Parameter::Pan => 0.5,
        };
        if normalized != origin {
            arc(origin, normalized, theme::accent(ui.ctx()));
        }
        painter.circle(
            center,
            radius - 3.0,
            ui.visuals().widgets.inactive.bg_fill,
            Stroke::new(
                1.0,
                if response.hovered() || response.has_focus() {
                    theme::palette(ui.ctx()).secondary
                } else {
                    theme::palette(ui.ctx()).border
                },
            ),
        );
        // The fixed mark identifies unity gain or centered pan independently of the pointer.
        painter.line_segment(
            [point(0.5, radius - 1.0), point(0.5, radius + 2.0)],
            Stroke::new(1.5, theme::palette(ui.ctx()).text),
        );
        painter.line_segment(
            [point(normalized, 2.0), point(normalized, radius - 5.0)],
            Stroke::new(1.5, theme::palette(ui.ctx()).text),
        );
    }
    let description = match parameter {
        Parameter::Gain => format!("Gain: {value:.1} dB. Top mark: 0 dB."),
        Parameter::Pan => format!("Pan: {value:.2}. Top mark: center."),
    };
    response.on_hover_text(format!(
        "{description} Drag up or right to increase. Hold Shift for fine adjustment. Use arrow keys for small steps. Double-click to reset."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        value: &mut f32,
        mut events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        events.push(egui::Event::ModifiersChanged(modifiers));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0))),
                events,
                ..Default::default()
            },
            |ui| rect = gain(ui, value).rect,
        );
        output.textures_delta.clear();
        rect
    }

    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn unity_gain_and_center_pan_share_the_top_mark() {
        for parameter in [Parameter::Gain, Parameter::Pan] {
            assert_eq!(parameter.normalized(0.0), 0.5);
            assert_eq!(parameter.value(0.5), 0.0);
        }
        assert_eq!(Parameter::Gain.value(0.0), -60.0);
        assert_eq!(Parameter::Gain.value(1.0), 12.0);
        assert_eq!(Parameter::Pan.value(0.0), -1.0);
        assert_eq!(Parameter::Pan.value(1.0), 1.0);
    }

    #[test]
    fn gain_drag_uses_incremental_motion_and_shift_reduces_sensitivity() {
        let mut results = Vec::new();
        for fine in [false, true] {
            let ctx = egui::Context::default();
            let mut value = 0.0;
            let modifiers = egui::Modifiers {
                shift: fine,
                ..Default::default()
            };
            let start = frame(&ctx, &mut value, vec![], modifiers).center();
            frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(start), pointer(start, true)],
                modifiers,
            );
            let end = start - Vec2::new(0.0, 30.0);
            frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(end)],
                modifiers,
            );
            assert!(value > 0.0);
            let dragged = value;
            frame(&ctx, &mut value, vec![], modifiers);
            assert_eq!(value, dragged);
            frame(&ctx, &mut value, vec![pointer(end, false)], modifiers);
            results.push(value);
        }
        assert!(results[1] < results[0]);
    }

    #[test]
    fn double_click_resets_gain_to_unity() {
        let ctx = egui::Context::default();
        let mut value = -6.0;
        let pos = frame(&ctx, &mut value, vec![], Default::default()).center();
        for _ in 0..2 {
            frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
                Default::default(),
            );
            frame(
                &ctx,
                &mut value,
                vec![pointer(pos, false)],
                Default::default(),
            );
        }
        assert_eq!(value, 0.0);
    }
}
