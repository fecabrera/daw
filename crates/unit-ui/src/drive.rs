//! The Drive body shared by the DAW shell and native plugin editors.
use crate::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    parameter_knobs, rows, theme,
};
use daw_unit_core::{DriveColorShape, DriveSettings, ModulationParameter as Parameter};
use egui::{Rect, Response, Ui, Vec2};
pub const STRIP_WIDTH: f32 = crate::layout::UNIT_STRIP_WIDTH;
pub const MARGIN: i8 = crate::layout::PANEL_MARGIN;
pub const BODY_HEIGHT: f32 = crate::layout::unit_control_height();
pub const SIZE: Vec2 = Vec2::new(2.0 * STRIP_WIDTH, BODY_HEIGHT + 2.0 * MARGIN as f32);
pub struct Bounds {
    pub meter: Rect,
    pub knobs: [[Rect; 3]; 2],
    pub labels: [Rect; 2],
    /// Volume, Gain, Tone, Shape, Strength, Frequency. Absent when compact.
    pub controls: [Option<Response>; 6],
    pub rect: Rect,
}
fn shape(value: f32) -> DriveColorShape {
    DriveColorShape::ALL[value.round().clamp(0.0, 3.0) as usize]
}
fn shape_diagram(value: f32) -> Vec<Vec2> {
    let ys = match shape(value) {
        DriveColorShape::Off => [0.5; 5],
        DriveColorShape::MidScoop => [0.5, 0.7, 1.0, 0.7, 0.5],
        DriveColorShape::MidHump => [0.5, 0.3, 0.0, 0.3, 0.5],
        DriveColorShape::MidHumpBassCut => [1.0, 0.8, 0.0, 0.3, 0.5],
    };
    ys.into_iter()
        .enumerate()
        .map(|(i, y)| Vec2::new(i as f32 / 4.0 - 0.5, y - 0.5))
        .collect()
}
/// The caller owns the surrounding shell and optional modulation routing panel.
pub fn show(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    compact: bool,
    compact_height: f32,
    markers: &[Option<ModulationMarker>; 5],
    level: crate::meters::OutputLevel<'_>,
) -> Bounds {
    let mut bounds = Bounds {
        meter: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 2],
        labels: [Rect::NOTHING; 2],
        controls: std::array::from_fn(|_| None),
        rect: Rect::NOTHING,
    };
    let response = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, theme::TOOLBAR_PADDING as f32);
            for (index, name) in ["Drive", "Color"].into_iter().enumerate() {
                let strip = ui
                    .push_id(index, |ui| {
                        egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(STRIP_WIDTH - 2.0 * MARGIN as f32);
                                ui.set_min_height(if compact {
                                    compact_height
                                } else {
                                    BODY_HEIGHT
                                });
                                if !compact {
                                    for row in 0..3 {
                                        let slot = index * 3 + row;
                                        let enabled = index == 0
                                            || row == 0
                                            || settings.color_shape != DriveColorShape::Off;
                                        let response = ui
                                            .add_enabled_ui(enabled, |ui| {
                                                if slot == 3 {
                                                    knobs::row(
                                                        ui,
                                                        &mut settings.color_shape,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, value| {
                                                            let mut position = value.index() as f32;
                                                            let response = ui.add(Knob::new(
                                                                &mut position,
                                                                KnobSpec::new(
                                                                    "Color shape",
                                                                    0.0..=3.0,
                                                                    0.0,
                                                                    1.0,
                                                                )
                                                                .stepped(1.0)
                                                                .cyclic()
                                                                .diagrams(shape_diagram)
                                                                .format(|v| shape(v).name().into()),
                                                            ));
                                                            if response.changed() {
                                                                *value = shape(position);
                                                            }
                                                            response
                                                        },
                                                    )
                                                } else {
                                                    let (value, parameter, marker) = match slot {
                                                        0 => (
                                                            &mut settings.volume_db,
                                                            Parameter::Volume,
                                                            0,
                                                        ),
                                                        1 => (
                                                            &mut settings.gain,
                                                            Parameter::DriveGain,
                                                            1,
                                                        ),
                                                        2 => {
                                                            (&mut settings.tone, Parameter::Tone, 2)
                                                        }
                                                        4 => (
                                                            &mut settings.color_strength,
                                                            Parameter::ColorStrength,
                                                            3,
                                                        ),
                                                        _ => (
                                                            &mut settings.color_frequency_hz,
                                                            Parameter::ColorFrequency,
                                                            4,
                                                        ),
                                                    };
                                                    knobs::row(
                                                        ui,
                                                        value,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, value| {
                                                            ui.add(
                                                                Knob::new(
                                                                    value,
                                                                    parameter_knobs::destination(
                                                                        parameter,
                                                                    ),
                                                                )
                                                                .modulated(
                                                                    markers[marker].as_ref(),
                                                                    &mut settings.modulation_open,
                                                                ),
                                                            )
                                                        },
                                                    )
                                                }
                                            })
                                            .inner;
                                        bounds.knobs[index][row] = response.rect;
                                        bounds.controls[slot] = Some(response);
                                    }
                                }
                                rows::centered(
                                    ui,
                                    if compact {
                                        compact_height
                                    } else {
                                        crate::layout::COMPACT_BUTTON_SIZE
                                    },
                                    |ui| {
                                        bounds.labels[index] = rows::unit_label(ui, name).rect;
                                    },
                                );
                            });
                        })
                    })
                    .inner
                    .response
                    .rect;
                if index == 0 && !compact {
                    bounds.meter = crate::meters::effect_output_level(
                        ui,
                        strip,
                        MARGIN,
                        level,
                        settings.enabled,
                    );
                }
                if index > 0 {
                    divider(ui, strip.left(), strip.y_range());
                }
            }
        })
        .response;
    bounds.rect = response.rect;
    bounds
}
pub fn divider(ui: &Ui, x: f32, range: egui::Rangef) {
    ui.painter().vline(
        x,
        range,
        egui::Stroke::new(1.0, theme::unit_separator(ui.ctx())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_fits_fixed_window_and_aligns_controls_in_each_theme_and_scale() {
        for (base, palette) in [
            (egui::Theme::Dark, theme::DARK),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            for scale in [0.5, 1.0, 1.5, 2.0, 3.0, 4.0] {
                let ctx = egui::Context::default();
                crate::fonts::configure(&ctx);
                theme::configure(&ctx);
                theme::apply(&ctx, base, palette, theme::ACCENT);
                ctx.set_pixels_per_point(scale);
                let mut settings = DriveSettings::default();
                for _ in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                            ..Default::default()
                        },
                        |ui| {
                            let bounds = show(
                                ui,
                                &mut settings,
                                false,
                                0.0,
                                &std::array::from_fn(|_| None),
                                0.0.into(),
                            );
                            assert_eq!(bounds.rect.size(), SIZE);
                            for strip in 0..2 {
                                for row in 0..3 {
                                    let rect = bounds.knobs[strip][row];
                                    assert!(bounds.rect.contains_rect(rect));
                                    assert_eq!(rect.size(), Vec2::splat(knobs::CHOICE_SIZE));
                                    assert_eq!(rect.center().x, STRIP_WIDTH * (strip as f32 + 0.5));
                                    assert_eq!(rect.center().y, bounds.knobs[0][row].center().y);
                                }
                            }
                            assert!(bounds.controls[3].as_ref().unwrap().enabled());
                            assert!(!bounds.controls[4].as_ref().unwrap().enabled());
                            assert!(!bounds.controls[5].as_ref().unwrap().enabled());
                        },
                    );
                    output.textures_delta.clear();
                }
                assert_eq!(
                    (settings.color_strength, settings.color_frequency_hz),
                    (0.5, 750.0)
                );
            }
        }
    }
}
