use crate::theme;
use egui::{Pos2, Response, Sense, Shape, Stroke, Ui, Vec2, Widget};
use std::{f32::consts::PI, ops::RangeInclusive};

pub const SIZE: f32 = 28.0;
pub const CHOICE_SIZE: f32 = 52.0;

#[derive(Clone, Copy)]
struct ParameterScale {
    position: fn(f32) -> f32,
    value: fn(f32) -> f32,
    step: fn(f32, f32) -> f32,
}

/// A parameter supplies its scale and steps; the base knob owns all interaction and painting.
#[derive(Clone, Copy)]
pub struct KnobSpec {
    name: &'static str,
    min: f32,
    max: f32,
    reset: f32,
    keyboard_step: f32,
    discrete_step: Option<f32>,
    reference_step: Option<f32>,
    reference_values: Option<&'static [f32]>,
    value_label: Option<fn(f32) -> String>,
    endpoint_labels: Option<[&'static str; 2]>,
    center: Option<f32>,
    scale: Option<ParameterScale>,
    bipolar: bool,
    cyclic: bool,
    diagram: Option<fn(f32) -> Vec<Vec2>>,
    diagram_size: Vec2,
    format: fn(f32) -> String,
}

impl KnobSpec {
    /// Use a finite increasing range, a reset inside it, and a positive keyboard step.
    pub fn new(
        name: &'static str,
        range: RangeInclusive<f32>,
        reset: f32,
        keyboard_step: f32,
    ) -> Self {
        Self {
            name,
            min: *range.start(),
            max: *range.end(),
            reset,
            keyboard_step,
            discrete_step: None,
            reference_step: None,
            reference_values: None,
            value_label: None,
            endpoint_labels: None,
            center: None,
            scale: None,
            bipolar: false,
            cyclic: false,
            diagram: None,
            diagram_size: Vec2::new(10.0, 6.0),
            format: |value| value.to_string(),
        }
    }

    /// Put this value at twelve o'clock, with a linear scale on each side.
    pub fn centered(mut self, center: f32) -> Self {
        self.center = Some(center);
        self
    }
    /// Supply a nonlinear 0–1 scale and a keyboard step in parameter units.
    /// The shared knob still owns drag sensitivity, bounds, focus, and reset.
    pub fn scale(
        mut self,
        position: fn(f32) -> f32,
        value: fn(f32) -> f32,
        step: fn(f32, f32) -> f32,
    ) -> Self {
        self.scale = Some(ParameterScale {
            position,
            value,
            step,
        });
        self
    }
    /// Quantize values while retaining fractional pointer movement during a drag.
    pub fn stepped(mut self, step: f32) -> Self {
        self.discrete_step = Some(step);
        self
    }
    /// Show reference marks at this interval without changing the parameter's input steps.
    pub fn reference_step(mut self, step: f32) -> Self {
        self.reference_step = Some(step);
        self
    }
    /// Mark specific values inside the range, independently of the parameter's input steps.
    /// These values take precedence over an evenly spaced reference interval.
    pub fn reference_values(mut self, values: &'static [f32]) -> Self {
        self.reference_values = Some(values);
        self
    }
    /// Show the current value centered above the knob using a compact formatter.
    pub fn value_label(mut self, format: fn(f32) -> String) -> Self {
        self.value_label = Some(format);
        self
    }
    /// Label both range endpoints below their marks, independently of the current value.
    pub fn endpoint_labels(mut self, min: &'static str, max: &'static str) -> Self {
        self.endpoint_labels = Some([min, max]);
        self
    }
    pub fn bipolar(mut self) -> Self {
        self.bipolar = true;
        self
    }
    /// Wrap a stepped range in both directions, including keyboard input.
    pub fn cyclic(mut self) -> Self {
        assert!(self.discrete_step.is_some(), "cyclic knobs require a step");
        self.cyclic = true;
        self
    }
    /// Draw normalized paths at the choice positions of a stepped range, instead of tick marks.
    pub fn diagrams(mut self, diagram: fn(f32) -> Vec<Vec2>) -> Self {
        self.diagram = Some(diagram);
        self
    }
    /// Set the painted size of normalized choice diagrams in logical points.
    pub fn diagram_size(mut self, size: Vec2) -> Self {
        self.diagram_size = size;
        self
    }
    pub fn format(mut self, format: fn(f32) -> String) -> Self {
        self.format = format;
        self
    }

    fn normalized(self, value: f32) -> f32 {
        if let Some(scale) = self.scale {
            return (scale.position)(value).clamp(0.0, 1.0);
        }
        if self.cyclic {
            return ((value - self.min) / self.period()).rem_euclid(1.0);
        }
        match self.center {
            Some(center) if value <= center => 0.5 * (value - self.min) / (center - self.min),
            Some(center) => 0.5 + 0.5 * (value - center) / (self.max - center),
            None => (value - self.min) / (self.max - self.min),
        }
        .clamp(0.0, 1.0)
    }

    fn value(self, normalized: f32) -> f32 {
        if let Some(scale) = self.scale {
            return (scale.value)(normalized.clamp(0.0, 1.0)).clamp(self.min, self.max);
        }
        if self.cyclic {
            let step = self.discrete_step.unwrap();
            let value = (normalized.rem_euclid(1.0) * self.period() / step).round() * step;
            return self.min + value.rem_euclid(self.period());
        }
        let normalized = normalized.clamp(0.0, 1.0);
        let value = match self.center {
            Some(center) if normalized <= 0.5 => self.min + normalized * 2.0 * (center - self.min),
            Some(center) => center + (normalized - 0.5) * 2.0 * (self.max - center),
            None => self.min + normalized * (self.max - self.min),
        };
        self.discrete_step
            .map_or(value, |step| {
                self.min + ((value - self.min) / step).round() * step
            })
            .clamp(self.min, self.max)
    }

    fn period(self) -> f32 {
        self.max - self.min + self.discrete_step.unwrap()
    }

    fn drag_position(self, position: f32) -> f32 {
        if self.cyclic {
            position.rem_euclid(1.0)
        } else {
            position.clamp(0.0, 1.0)
        }
    }
}

/// Standard knob face, focus, drag accumulation, Shift sensitivity, keys, bounds/wrap, and reset.
pub struct Knob<'a> {
    value: &'a mut f32,
    spec: KnobSpec,
}
impl<'a> Knob<'a> {
    pub fn new(value: &'a mut f32, spec: KnobSpec) -> Self {
        Self { value, spec }
    }
}
impl Widget for Knob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        base_knob(ui, self.value, self.spec)
    }
}

fn gain_spec() -> KnobSpec {
    KnobSpec::new(
        "Gain",
        daw_core::SILENT_GAIN_DB..=crate::faders::MAX_GAIN_DB,
        0.0,
        0.1,
    )
    .scale(gain_position, gain_at_position, gain_step)
    .format(|value| format!("{} dB", crate::faders::format_gain(value)))
}

fn gain_position(value: f32) -> f32 {
    let position = crate::faders::position_for_gain(value);
    let unity = crate::faders::position_for_gain(0.0);
    if position <= unity {
        0.5 * position / unity
    } else {
        0.5 + 0.5 * (position - unity) / (1.0 - unity)
    }
}

fn gain_at_position(position: f32) -> f32 {
    let unity = crate::faders::position_for_gain(0.0);
    crate::faders::gain_at_position(if position <= 0.5 {
        position * 2.0 * unity
    } else {
        unity + (position - 0.5) * 2.0 * (1.0 - unity)
    })
}

fn gain_step(value: f32, delta: f32) -> f32 {
    if value < -60.0 || (value == -60.0 && delta < 0.0) {
        // Step through the amplitude tail, so keys can enter and leave exact silence.
        let position = (gain_position(value) + delta / 120.0).clamp(0.0, 1.0);
        // The log/amplitude round-trip can leave a residual at the zero endpoint.
        gain_at_position(if position <= f32::EPSILON {
            0.0
        } else {
            position
        })
    } else {
        ((value + delta) * 10.0).round() / 10.0
    }
}
fn pan_spec() -> KnobSpec {
    KnobSpec::new("Pan", -1.0..=1.0, 0.0, 0.01)
        .centered(0.0)
        .bipolar()
        .format(|value| format!("{value:.2}"))
}

/// Gain has a fixed 0 dB mark at twelve o'clock; dragging covers silence to +12 dB.
pub fn gain(ui: &mut Ui, value: &mut f32) -> Response {
    ui.add(Knob::new(value, gain_spec()))
}
fn marked_gain_spec() -> KnobSpec {
    gain_spec()
        .reference_values(&crate::faders::GAIN_MARKS)
        .value_label(|value| format!("{} dB", crate::faders::format_gain(value)))
        .endpoint_labels("−∞", "+12")
}

/// Optional gain scale marks and current-value label, painted by the shared knob base.
pub fn gain_with_markers(ui: &mut Ui, value: &mut f32) -> Response {
    ui.add(Knob::new(value, marked_gain_spec()))
}

pub fn pan(ui: &mut Ui, value: &mut f32) -> Response {
    ui.add(Knob::new(value, pan_spec()))
}
pub fn pitch(ui: &mut Ui, value: &mut i8) -> Response {
    let mut scalar = f32::from(*value);
    let response = ui.add(Knob::new(
        &mut scalar,
        KnobSpec::new("Pitch", -48.0..=48.0, 0.0, 1.0)
            .stepped(1.0)
            .reference_step(12.0)
            .value_label(|value| {
                if value > 0.0 {
                    format!("+{value:.0}")
                } else {
                    format!("{value:.0}")
                }
            })
            .endpoint_labels("−48", "+48")
            .bipolar()
            .format(|value| format!("{value:.0} semitones")),
    ));
    if response.changed() {
        *value = scalar as i8;
    }
    response
}
pub fn waveform(ui: &mut Ui, value: &mut daw_core::Waveform) -> Response {
    let mut scalar = daw_core::Waveform::ALL
        .iter()
        .position(|choice| *choice == *value)
        .unwrap() as f32;
    let response = ui.add(Knob::new(&mut scalar, waveform_spec()));
    if response.changed() {
        *value = daw_core::Waveform::ALL[scalar as usize];
    }
    response
}

fn waveform_spec() -> KnobSpec {
    KnobSpec::new(
        "Waveform",
        0.0..=(daw_core::Waveform::ALL.len() - 1) as f32,
        0.0,
        1.0,
    )
    .stepped(1.0)
    .cyclic()
    .diagrams(waveform_diagram)
    .format(|value| daw_core::Waveform::ALL[value as usize].label().into())
}

fn waveform_diagram(value: f32) -> Vec<Vec2> {
    use daw_core::Waveform;
    match Waveform::ALL[value as usize] {
        Waveform::Sine => (0..=24)
            .map(|i| {
                let x = i as f32 / 24.0;
                Vec2::new(x - 0.5, -(x * 2.0 * PI).sin() * 0.5)
            })
            .collect(),
        Waveform::Saw => vec![
            Vec2::new(-0.5, 0.5),
            Vec2::new(0.0, -0.5),
            Vec2::new(0.0, 0.5),
            Vec2::new(0.5, -0.5),
        ],
        Waveform::Square => vec![
            Vec2::new(-0.5, 0.5),
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.0, -0.5),
            Vec2::new(0.0, 0.5),
            Vec2::new(0.5, 0.5),
            Vec2::new(0.5, -0.5),
        ],
        Waveform::Triangle => vec![
            Vec2::new(-0.5, 0.5),
            Vec2::new(-0.25, -0.5),
            Vec2::new(0.25, 0.5),
            Vec2::new(0.5, -0.5),
        ],
    }
}

fn time_position(value: f32) -> f32 {
    value.ln_1p() / daw_core::MAX_ENVELOPE_TIME_MS.ln_1p()
}
fn time_at_position(position: f32) -> f32 {
    (position * daw_core::MAX_ENVELOPE_TIME_MS.ln_1p()).exp_m1()
}
pub fn format_time(value: f32) -> String {
    if value < 1000.0 {
        format!("{value:.1} ms")
    } else {
        format!("{:.2} s", value / 1000.0)
    }
}
fn format_percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

pub fn envelope_time(ui: &mut Ui, value: &mut f32, name: &'static str, reset: f32) -> Response {
    ui.add(Knob::new(
        value,
        KnobSpec::new(name, 0.0..=daw_core::MAX_ENVELOPE_TIME_MS, reset, 1.0)
            .scale(time_position, time_at_position, |value, delta| {
                value + delta
            })
            .value_label(format_time)
            .endpoint_labels("0 ms", "10 s")
            .format(format_time),
    ))
}
pub fn sustain(ui: &mut Ui, value: &mut f32) -> Response {
    ui.add(Knob::new(
        value,
        KnobSpec::new("Sustain level", 0.0..=1.0, 0.8, 0.01)
            .value_label(format_percent)
            .endpoint_labels("0%", "100%")
            .format(format_percent),
    ))
}
pub fn curve_amount(ui: &mut Ui, value: &mut f32) -> Response {
    ui.add(Knob::new(
        value,
        KnobSpec::new("Curve amount", 0.0..=1.0, 0.5, 0.01)
            .value_label(format_percent)
            .endpoint_labels("0%", "100%")
            .format(format_percent),
    ))
}
pub fn curve_shape(
    ui: &mut Ui,
    value: &mut daw_core::CurveShape,
    reset: daw_core::CurveShape,
    falling: bool,
) -> Response {
    let mut scalar = daw_core::CurveShape::ALL
        .iter()
        .position(|shape| shape == value)
        .unwrap() as f32;
    let reset = daw_core::CurveShape::ALL
        .iter()
        .position(|shape| *shape == reset)
        .unwrap() as f32;
    let response = ui.add(Knob::new(
        &mut scalar,
        KnobSpec::new("Curve shape", 0.0..=3.0, reset, 1.0)
            .stepped(1.0)
            .cyclic()
            .diagrams(if falling {
                falling_curve_diagram
            } else {
                rising_curve_diagram
            })
            .diagram_size(Vec2::splat(10.0))
            .format(|value| daw_core::CurveShape::ALL[value as usize].label().into()),
    ));
    if response.changed() {
        *value = daw_core::CurveShape::ALL[scalar as usize];
    }
    response
}
fn curve_diagram(value: f32, falling: bool) -> Vec<Vec2> {
    let shape = daw_core::CurveShape::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let progress = shape.progress(t, 1.0, falling);
            Vec2::new(
                t - 0.5,
                if falling {
                    progress - 0.5
                } else {
                    0.5 - progress
                },
            )
        })
        .collect()
}
fn rising_curve_diagram(value: f32) -> Vec<Vec2> {
    curve_diagram(value, false)
}
fn falling_curve_diagram(value: f32) -> Vec<Vec2> {
    curve_diagram(value, true)
}

/// Common compact unit row with an aligned standard knob.
pub fn row<T>(
    ui: &mut Ui,
    value: &mut T,
    height: f32,
    knob: impl FnOnce(&mut Ui, &mut T) -> Response,
) -> Response {
    crate::rows::centered(ui, height, |ui| {
        ui.add_space(((ui.available_width() - CHOICE_SIZE) * 0.5).max(0.0));
        ui.allocate_ui_with_layout(
            Vec2::new(CHOICE_SIZE, height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.set_min_width(CHOICE_SIZE);
                knob(ui, value)
            },
        )
        .inner
    })
    .inner
}

fn base_knob(ui: &mut Ui, value: &mut f32, spec: KnobSpec) -> Response {
    let size =
        if spec.diagram.is_some() || spec.value_label.is_some() || spec.endpoint_labels.is_some() {
            CHOICE_SIZE
        } else {
            SIZE
        };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let previous = *value;
    let drag_id = response.id.with("knob-drag");
    if ui.is_enabled() && ui.input(|input| input.focused) {
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.drag_started() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(drag_id, spec.normalized(*value)));
        }
        if response.dragged() {
            let fine = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
            let delta = response.drag_delta();
            let normalized = ui.ctx().data_mut(|data| {
                let current = data
                    .get_temp::<f32>(drag_id)
                    .unwrap_or_else(|| spec.normalized(*value));
                let next = spec.drag_position(current + (delta.x - delta.y) * fine / 150.0);
                // Keep fractional movement between frames so stepped knobs respond to slow drags.
                data.insert_temp(drag_id, next);
                next
            });
            *value = spec.value(normalized);
        }
        if response.has_focus() {
            let change = ui.input(|input| {
                f32::from(
                    input.key_pressed(egui::Key::ArrowUp)
                        || input.key_pressed(egui::Key::ArrowRight),
                ) - f32::from(
                    input.key_pressed(egui::Key::ArrowDown)
                        || input.key_pressed(egui::Key::ArrowLeft),
                )
            });
            if change != 0.0 {
                let adjusted = spec.scale.map_or_else(
                    || (*value / spec.keyboard_step + change).round() * spec.keyboard_step,
                    |scale| (scale.step)(*value, change * spec.keyboard_step),
                );
                *value = spec.value(spec.normalized(adjusted));
                ui.ctx()
                    .data_mut(|data| data.insert_temp(drag_id, spec.normalized(*value)));
            }
        }
        if response.double_clicked() {
            *value = spec.reset;
            ui.ctx().data_mut(|data| data.remove::<f32>(drag_id));
        }
    }
    if response.drag_stopped() || !ui.input(|input| input.pointer.primary_down() && input.focused) {
        ui.ctx().data_mut(|data| data.remove::<f32>(drag_id));
    }
    if *value != previous {
        response.mark_changed();
    }
    response
        .widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), spec.name));

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center = rect.center();
        let radius = SIZE / 2.0 - 2.0;
        let start = if spec.cyclic { -0.5 * PI } else { 0.75 * PI };
        let sweep = if spec.cyclic { 2.0 * PI } else { 1.5 * PI };
        let normalized = spec.normalized(*value);
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
        let origin = if spec.bipolar {
            spec.normalized(spec.reset)
        } else {
            0.0
        };
        if !spec.cyclic && normalized != origin {
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
        let mark = |fraction, subdued| {
            painter.line_segment(
                [
                    point(fraction, if subdued { radius } else { radius - 1.0 }),
                    point(fraction, radius + 2.0),
                ],
                Stroke::new(
                    if subdued { 1.0 } else { 1.5 },
                    if subdued {
                        theme::palette(ui.ctx()).secondary
                    } else {
                        theme::palette(ui.ctx()).text
                    },
                ),
            )
        };
        if let Some(diagram) = spec.diagram {
            let step = spec.discrete_step.unwrap();
            for index in 0..=((spec.max - spec.min) / step).round() as u32 {
                let choice = spec.min + index as f32 * step;
                let center = point(spec.normalized(choice), 20.0);
                let points = diagram(choice)
                    .into_iter()
                    .map(|p| center + p * spec.diagram_size)
                    .collect();
                let color = if choice == *value {
                    theme::accent(ui.ctx())
                } else {
                    theme::palette(ui.ctx()).secondary
                };
                painter.add(Shape::line(points, Stroke::new(1.25, color)));
            }
        } else if let Some(values) = spec.reference_values {
            for &value in values {
                if value.is_finite() && (spec.min..=spec.max).contains(&value) {
                    mark(spec.normalized(value), value != spec.reset);
                }
            }
        } else if let Some(step) = spec
            .reference_step
            .or(spec.discrete_step)
            .filter(|step| (spec.max - spec.min) / step <= 15.0)
        {
            for index in 0..=((spec.max - spec.min) / step).round() as u32 {
                let value = spec.min + index as f32 * step;
                mark(
                    spec.normalized(value),
                    spec.reference_step.is_some() && value != spec.reset,
                );
            }
        } else {
            mark(spec.normalized(spec.reset), false);
        }
        if let Some(format) = spec.value_label {
            painter.text(
                center - Vec2::new(0.0, radius + 3.0),
                egui::Align2::CENTER_BOTTOM,
                format(*value),
                crate::fonts::numeric(9.0),
                theme::palette(ui.ctx()).text,
            );
        }
        if let Some(labels) = spec.endpoint_labels {
            for (value, label) in [(spec.min, labels[0]), (spec.max, labels[1])] {
                painter.text(
                    point(spec.normalized(value), radius + 5.0),
                    egui::Align2::CENTER_TOP,
                    label,
                    crate::fonts::numeric(9.0),
                    theme::palette(ui.ctx()).text,
                );
            }
        }
        painter.line_segment(
            [point(normalized, 2.0), point(normalized, radius - 5.0)],
            Stroke::new(1.5, theme::palette(ui.ctx()).text),
        );
    }
    response.on_hover_text(format!("{}: {}. Drag up or right to increase. Hold Shift for fine adjustment. Use arrow keys for small steps. Double-click to reset.", spec.name, (spec.format)(*value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        value: &mut f32,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::Rect {
        spec_frame(ctx, value, events, modifiers, gain_spec(), true)
    }

    fn spec_frame(
        ctx: &egui::Context,
        value: &mut f32,
        mut events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
        spec: KnobSpec,
        enabled: bool,
    ) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        events.push(egui::Event::ModifiersChanged(modifiers));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.0))),
                events,
                ..Default::default()
            },
            |ui| rect = ui.add_enabled(enabled, Knob::new(value, spec)).rect,
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
        for parameter in [gain_spec(), pan_spec()] {
            assert_eq!(parameter.normalized(0.0), 0.5);
            assert_eq!(parameter.value(0.5), 0.0);
        }
        assert_eq!(gain_spec().value(0.0), daw_core::SILENT_GAIN_DB);
        assert_eq!(gain_spec().value(1.0), 12.0);
        assert_eq!(pan_spec().value(0.0), -1.0);
        assert_eq!(pan_spec().value(1.0), 1.0);
    }

    #[test]
    fn optional_gain_markers_preserve_the_scale_and_drag_adjustment() {
        let mut changes = Vec::new();
        for spec in [gain_spec(), marked_gain_spec()] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            let mut value = -12.0;
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(Default::default(), |ui| {
                rect = ui.add(Knob::new(&mut value, spec)).rect;
            });
            let marks: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::LineSegment { points, stroke }
                        if ((points[1] - rect.center()).length() - 14.0).abs() < 0.01 =>
                    {
                        Some((points, stroke))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                marks.len(),
                if spec.reference_values.is_some() {
                    13
                } else {
                    1
                }
            );
            let unity: Vec<_> = marks
                .iter()
                .filter(|(points, _)| {
                    (points[1] - (rect.center() - Vec2::new(0.0, 14.0))).length() < 0.01
                })
                .collect();
            assert_eq!(unity.len(), 1);
            assert_eq!(unity[0].1.width, 1.5);
            assert_eq!(value, -12.0);
            let point = rect.center();
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point), pointer(point, true)],
                Default::default(),
                spec,
                true,
            );
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point - Vec2::new(0.0, 10.0))],
                Default::default(),
                spec,
                true,
            );
            changes.push(value);
            assert_eq!(spec.value(0.0), daw_core::SILENT_GAIN_DB);
            assert_eq!(spec.value(0.5), 0.0);
            assert_eq!(spec.value(1.0), 12.0);
            output.textures_delta.clear();
        }
        assert!(changes[0] > -12.0);
        assert_eq!(changes[0], changes[1]);
    }

    #[test]
    fn gain_knob_can_enter_and_leave_exact_silence_with_drag_and_keys() {
        let spec = gain_spec();
        let ctx = egui::Context::default();
        let mut value = daw_core::SILENT_GAIN_DB;
        let point = frame(&ctx, &mut value, vec![], Default::default()).center();
        assert_eq!(daw_core::linear_gain(value), 0.0);
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
            Default::default(),
        );
        frame(
            &ctx,
            &mut value,
            vec![pointer(point, false)],
            Default::default(),
        );
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(
            &ctx,
            &mut value,
            vec![key(egui::Key::ArrowUp)],
            Default::default(),
        );
        assert!(value < -60.0 && daw_core::linear_gain(value) > 0.0);
        frame(
            &ctx,
            &mut value,
            vec![key(egui::Key::ArrowDown)],
            Default::default(),
        );
        assert_eq!(daw_core::linear_gain(value), 0.0);
        let ctx = egui::Context::default();
        let point = frame(&ctx, &mut value, vec![], Default::default()).center();
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
            Default::default(),
        );
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point - Vec2::new(0.0, 10.0))],
            Default::default(),
        );
        assert!(
            value < 0.0 && daw_core::linear_gain(value) > 0.0,
            "gain={value}"
        );
        frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point + Vec2::new(0.0, 200.0))],
            Default::default(),
        );
        assert_eq!(value, daw_core::SILENT_GAIN_DB);
        let mut previous = -1.0;
        for index in 0..=1000 {
            let position = index as f32 / 1000.0;
            let gain = spec.value(position);
            assert!(daw_core::linear_gain(gain) >= previous);
            assert!((spec.normalized(gain) - position).abs() < 0.00001);
            previous = daw_core::linear_gain(gain);
        }
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
    #[test]
    fn stepped_knobs_accumulate_slow_motion_and_shift_can_change_without_a_jump() {
        let ctx = egui::Context::default();
        let spec = KnobSpec::new("Waveform", 0.0..=3.0, 0.0, 1.0).stepped(1.0);
        let mut value = 0.0;
        let start = spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
            Default::default(),
            spec,
            true,
        );
        for distance in 1..=20 {
            let point = start - Vec2::new(0.0, distance as f32 * 2.0);
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point)],
                Default::default(),
                spec,
                true,
            );
            assert_eq!(value.fract(), 0.0);
        }
        assert_eq!(
            value, 1.0,
            "small per-frame drags must accumulate across a detent"
        );
        let fine = egui::Modifiers::SHIFT;
        spec_frame(&ctx, &mut value, vec![], fine, spec, true);
        assert_eq!(value, 1.0);
        let end = start - Vec2::new(0.0, 70.0);
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(end)],
            fine,
            spec,
            true,
        );
        assert_eq!(value, 1.0);
        spec_frame(
            &ctx,
            &mut value,
            vec![pointer(end, false)],
            fine,
            spec,
            true,
        );
        spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true);
        assert_eq!(value, 1.0);
    }

    #[test]
    fn stepped_keyboard_values_clamp_reset_and_ignore_disabled_input() {
        for spec in [
            KnobSpec::new("Waveform", 0.0..=3.0, 0.0, 1.0).stepped(1.0),
            KnobSpec::new("Pitch", -48.0..=48.0, 0.0, 1.0)
                .stepped(1.0)
                .bipolar(),
        ] {
            let ctx = egui::Context::default();
            let mut value = spec.max;
            let point =
                spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point), pointer(point, true)],
                Default::default(),
                spec,
                true,
            );
            spec_frame(
                &ctx,
                &mut value,
                vec![pointer(point, false)],
                Default::default(),
                spec,
                true,
            );
            let key = |key, pressed| egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            };
            spec_frame(
                &ctx,
                &mut value,
                vec![key(egui::Key::ArrowUp, true)],
                Default::default(),
                spec,
                true,
            );
            assert_eq!(value, spec.max);
            spec_frame(
                &ctx,
                &mut value,
                vec![
                    key(egui::Key::ArrowUp, false),
                    key(egui::Key::ArrowDown, true),
                ],
                Default::default(),
                spec,
                true,
            );
            assert_eq!(value, spec.max - 1.0);
            spec_frame(
                &ctx,
                &mut value,
                vec![key(egui::Key::ArrowDown, false)],
                Default::default(),
                spec,
                true,
            );
            spec_frame(
                &ctx,
                &mut value,
                vec![key(egui::Key::ArrowDown, true)],
                Default::default(),
                spec,
                false,
            );
            assert_eq!(value, spec.max - 1.0);
            let ctx = egui::Context::default();
            let point =
                spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
            for _ in 0..2 {
                spec_frame(
                    &ctx,
                    &mut value,
                    vec![egui::Event::PointerMoved(point), pointer(point, true)],
                    Default::default(),
                    spec,
                    true,
                );
                spec_frame(
                    &ctx,
                    &mut value,
                    vec![pointer(point, false)],
                    Default::default(),
                    spec,
                    true,
                );
            }
            assert_eq!(value, spec.reset);
        }
    }

    #[test]
    fn waveform_drag_wraps_for_multiple_turns_in_both_directions() {
        let ctx = egui::Context::default();
        let spec = waveform_spec();
        let mut value = 0.0;
        let point = spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
            Default::default(),
            spec,
            true,
        );
        for index in (1_i32..=16).chain((-8..16).rev()) {
            let pos = point - Vec2::new(0.0, index as f32 * 37.5);
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(pos)],
                Default::default(),
                spec,
                true,
            );
            assert_eq!(value, index.rem_euclid(4) as f32);
        }
        let pos = point + Vec2::new(0.0, 300.0);
        spec_frame(&ctx, &mut value, vec![], egui::Modifiers::SHIFT, spec, true);
        assert_eq!(value, 0.0, "changing Shift must not jump to another choice");
        let end = pos - Vec2::new(0.0, 375.0);
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(end)],
            egui::Modifiers::SHIFT,
            spec,
            true,
        );
        assert_eq!(value, 1.0);
        spec_frame(
            &ctx,
            &mut value,
            vec![pointer(end, false)],
            egui::Modifiers::SHIFT,
            spec,
            true,
        );
        spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true);
        assert_eq!(value, 1.0, "release must preserve the selected waveform");
    }

    #[test]
    fn waveform_arrow_keys_wrap_and_disabled_input_cannot_change_the_choice() {
        let ctx = egui::Context::default();
        let spec = waveform_spec();
        let mut value = 0.0;
        let point = spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
        spec_frame(
            &ctx,
            &mut value,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
            Default::default(),
            spec,
            true,
        );
        spec_frame(
            &ctx,
            &mut value,
            vec![pointer(point, false)],
            Default::default(),
            spec,
            true,
        );
        let key = |key, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        for (key_code, direction) in [(egui::Key::ArrowLeft, -1_i32), (egui::Key::ArrowRight, 1)] {
            let initial = value as i32;
            for index in 1..=9 {
                spec_frame(
                    &ctx,
                    &mut value,
                    vec![key(key_code, true)],
                    Default::default(),
                    spec,
                    true,
                );
                assert_eq!(value, (initial + direction * index).rem_euclid(4) as f32);
                spec_frame(
                    &ctx,
                    &mut value,
                    vec![key(key_code, false)],
                    Default::default(),
                    spec,
                    true,
                );
            }
        }
        let previous = value;
        spec_frame(
            &ctx,
            &mut value,
            vec![key(egui::Key::ArrowLeft, true)],
            Default::default(),
            spec,
            false,
        );
        assert_eq!(value, previous);
        let ctx = egui::Context::default();
        value = 3.0;
        let point = spec_frame(&ctx, &mut value, vec![], Default::default(), spec, true).center();
        for _ in 0..2 {
            spec_frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerMoved(point), pointer(point, true)],
                Default::default(),
                spec,
                true,
            );
            spec_frame(
                &ctx,
                &mut value,
                vec![pointer(point, false)],
                Default::default(),
                spec,
                true,
            );
        }
        assert_eq!(value, 0.0);
    }
}
