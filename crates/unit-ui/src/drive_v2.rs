//! Drive v2 body shared by the DAW and native plugin editors.
use crate::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    parameter_knobs, rows, theme,
};
use daw_unit_core::{DriveV2Clip, DriveV2ColorShape, DriveV2Settings, ModulationParameter as P};
use egui::{Rect, Response, Ui, Vec2};
pub const UNIT_STRIP_WIDTH: f32 = crate::layout::UNIT_STRIP_WIDTH;
pub const MARGIN: i8 = crate::layout::PANEL_MARGIN;
pub const BODY_HEIGHT: f32 = crate::layout::unit_control_height();
pub const SIZE: Vec2 = Vec2::new(3.0 * UNIT_STRIP_WIDTH, BODY_HEIGHT + 2.0 * MARGIN as f32);
pub struct Bounds {
    pub sections: [Rect; 3],
    pub knobs: [[Rect; 3]; 3],
    pub color_power: Rect,
    pub clip: Rect,
    /// Level, Gain, Tone, Shape, Strength, Frequency, Input, Mix, Clip, Color power.
    pub controls: [Option<Response>; 10],
    pub rect: Rect,
}
fn diagram(value: f32) -> Vec<Vec2> {
    let ys = match value.round() as usize {
        0 => [0.5, 0.7, 1.0, 0.7, 0.5],
        1 => [0.5, 0.3, 0.0, 0.3, 0.5],
        _ => [1.0, 0.8, 0.0, 0.3, 0.5],
    };
    ys.into_iter()
        .enumerate()
        .map(|(i, y)| Vec2::new(i as f32 / 4.0 - 0.5, y - 0.5))
        .collect()
}

pub fn show(
    ui: &mut Ui,
    s: &mut DriveV2Settings,
    compact: bool,
    compact_height: f32,
    markers: &[Option<ModulationMarker>; 7],
) -> Bounds {
    let mut bounds = Bounds {
        sections: [Rect::NOTHING; 3],
        knobs: [[Rect::NOTHING; 3]; 3],
        color_power: Rect::NOTHING,
        clip: Rect::NOTHING,
        controls: std::array::from_fn(|_| None),
        rect: Rect::NOTHING,
    };
    let DriveV2Settings {
        input_gain_db,
        gain,
        tone,
        color_enabled,
        color_shape,
        color_strength,
        color_frequency_hz,
        level_db,
        dry_wet,
        clip,
        modulation_open,
        ..
    } = s;
    let response = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));

        for (column, name) in ["Drive", "Color", "Output"].into_iter().enumerate() {
            let strip = ui
                .push_id(column, |ui| {
                    egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(MARGIN));
                            ui.set_min_height(if compact { compact_height } else { BODY_HEIGHT });
                            if !compact {
                                for row in 0..3 {
                                    let response = if column == 1 && row == 0 {
                                        ui.add_enabled_ui(*color_enabled, |ui| {
                                            knobs::row(
                                                ui,
                                                color_shape,
                                                knobs::CHOICE_SIZE,
                                                |ui, value| {
                                                    let mut index = value.index() as f32;
                                                    let response = ui.add(Knob::new(
                                                        &mut index,
                                                        KnobSpec::new(
                                                            "Color shape",
                                                            0.0..=2.0,
                                                            0.0,
                                                            1.0,
                                                        )
                                                        .stepped(1.0)
                                                        .cyclic()
                                                        .diagrams(diagram)
                                                        .format(|v| {
                                                            DriveV2ColorShape::ALL
                                                                [v.round().clamp(0.0, 2.0) as usize]
                                                                .name()
                                                                .into()
                                                        }),
                                                    ));
                                                    if response.changed() {
                                                        *value =
                                                            DriveV2ColorShape::ALL[index as usize];
                                                    }
                                                    response
                                                },
                                            )
                                        })
                                        .inner
                                    } else if column == 0 && row == 0 {
                                        knobs::row(ui, clip, knobs::CHOICE_SIZE, |ui, value| {
                                            let mut mode = f32::from(*value == DriveV2Clip::Hard);
                                            let response = ui.add(Knob::new(
                                                &mut mode,
                                                KnobSpec::new("Clip mode", 0.0..=1.0, 0.0, 1.0)
                                                    .two_way_labels("Soft", "Hard")
                                                    .format(|v| {
                                                        if v < 0.5 {
                                                            "Soft".into()
                                                        } else {
                                                            "Hard".into()
                                                        }
                                                    }),
                                            ));
                                            if response.changed() {
                                                *value = if mode < 0.5 {
                                                    DriveV2Clip::Soft
                                                } else {
                                                    DriveV2Clip::Hard
                                                };
                                            }
                                            response
                                        })
                                    } else {
                                        let (value, i): (&mut f32, usize) = match (column, row) {
                                            (2, 0) => (&mut *input_gain_db, 0),
                                            (0, 1) => (&mut *gain, 1),
                                            (0, 2) => (&mut *tone, 2),
                                            (1, 1) => (&mut *color_strength, 3),
                                            (1, 2) => (&mut *color_frequency_hz, 4),
                                            (2, 1) => (&mut *level_db, 5),
                                            (2, 2) => (&mut *dry_wet, 6),
                                            _ => unreachable!(),
                                        };
                                        ui.add_enabled_ui(column != 1 || *color_enabled, |ui| {
                                            knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, v| {
                                                let spec =
                                                    parameter_knobs::destination(P::DRIVE_V2[i])
                                                        .named(
                                                            [
                                                                "Input gain",
                                                                "Drive gain",
                                                                "Drive tone",
                                                                "Color strength",
                                                                "Color frequency",
                                                                "Output level",
                                                                "Dry/Wet",
                                                            ][i],
                                                        );
                                                ui.add(Knob::new(v, spec).modulated(
                                                    markers[i].as_ref(),
                                                    modulation_open,
                                                ))
                                            })
                                        })
                                        .inner
                                    };
                                    bounds.knobs[column][row] = response.rect;
                                    if column == 0 && row == 0 {
                                        bounds.clip = response.rect;
                                    }
                                    let slot = match (column, row) {
                                        (0, 0) => 8,
                                        (0, 1) => 1,
                                        (0, 2) => 2,
                                        (1, 0) => 3,
                                        (1, 1) => 4,
                                        (1, 2) => 5,
                                        (2, 0) => 6,
                                        (2, 1) => 0,
                                        (2, 2) => 7,
                                        _ => unreachable!(),
                                    };
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
                                    if column == 1 {
                                        let mut r = rows::power_label(
                                            ui,
                                            "Color",
                                            "Toggle Color",
                                            *color_enabled,
                                        );
                                        bounds.color_power = r.rect;
                                        if r.clicked() {
                                            *color_enabled = !*color_enabled;
                                            r.mark_changed();
                                        }
                                        bounds.controls[9] = Some(r);
                                    } else {
                                        rows::unit_label(ui, name);
                                    }
                                },
                            );
                        });
                    })
                })
                .inner
                .response
                .rect;
            bounds.sections[column] = strip;
            if column > 0 {
                crate::drive::divider(ui, strip.left(), strip.y_range());
            }
        }
    });
    bounds.rect = response.response.rect;
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn three_strips_fit_the_fixed_editor_and_color_power_controls_enablement() {
        for scale in [1.0, 1.5, 2.0] {
            for color_enabled in [false, true] {
                let ctx = egui::Context::default();
                crate::fonts::configure(&ctx);
                theme::configure(&ctx);
                ctx.set_pixels_per_point(scale);
                let mut settings = DriveV2Settings {
                    color_enabled,
                    ..Default::default()
                };
                for _ in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                            ..Default::default()
                        },
                        |ui| {
                            let b = show(
                                ui,
                                &mut settings,
                                false,
                                0.0,
                                &std::array::from_fn(|_| None),
                            );
                            assert_eq!(b.rect.size(), SIZE);
                            for column in 0..3 {
                                for row in 0..3 {
                                    let r = b.knobs[column][row];
                                    assert!(b.rect.contains_rect(r));
                                    assert_eq!(r.size(), Vec2::splat(knobs::CHOICE_SIZE));
                                    assert_eq!(
                                        r.center().x,
                                        UNIT_STRIP_WIDTH * (column as f32 + 0.5)
                                    );
                                    assert_eq!(r.center().y, b.knobs[0][row].center().y);
                                }
                            }
                            for slot in 3..=5 {
                                assert_eq!(
                                    b.controls[slot].as_ref().unwrap().enabled(),
                                    color_enabled
                                );
                            }
                            assert!(b.controls[9].as_ref().unwrap().enabled());
                        },
                    );
                    output.textures_delta.clear();
                }
            }
        }
    }
}
