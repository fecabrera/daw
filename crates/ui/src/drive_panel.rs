use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{DriveColorShape, DriveSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 2],
    pub labels: [Rect; 2],
}
fn percentage(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}
fn percent_spec(name: &'static str, reset: f32) -> KnobSpec {
    KnobSpec::new(name, 0.0..=1.0, reset, 0.01)
        .value_label(percentage)
        .format(percentage)
        .endpoint_labels("0", "100")
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
        .map(|(index, y)| Vec2::new(index as f32 / 4.0 - 0.5, y - 0.5))
        .collect()
}
fn frequency_spec() -> KnobSpec {
    KnobSpec::new("Color frequency", 250.0..=2000.0, 750.0, 1.0)
        .scale(
            |value| (value / 250.0).log2() / 3.0,
            |position| 250.0 * 8_f32.powf(position),
            |value, delta| value * 2_f32.powf(delta / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels("250", "2k")
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    compact: bool,
    height: f32,
) -> (Bounds, egui::Response) {
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 2],
        labels: [Rect::NOTHING; 2],
    };
    // Split the saved fields so the shared shell owns power and horizontal collapse.
    let DriveSettings {
        enabled,
        collapsed,
        volume_db,
        gain,
        tone,
        color_shape,
        color_strength,
        color_frequency_hz,
        ..
    } = settings;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("Drive", compact),
        height,
        enabled,
        collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                for (index, name) in ["Drive", "Color"].into_iter().enumerate() {
                    let strip = ui
                        .push_id(index, |ui| {
                            egui::Frame::new()
                                .inner_margin(panel.margin)
                                .show(ui, |ui| {
                                    ui.with_layout(
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                            ui.set_width(
                                                UNIT_STRIP_WIDTH - 2.0 * f32::from(panel.margin),
                                            );
                                            ui.set_min_height(crate::envelope_panel::body_height(
                                                compact,
                                            ));
                                            if !compact {
                                                if index == 0 {
                                                    bounds.knobs[index] = [
                                                        knobs::row(
                                                            ui,
                                                            volume_db,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                knobs::gain_with_markers_named(
                                                                    ui,
                                                                    value,
                                                                    "Drive volume",
                                                                )
                                                            },
                                                        )
                                                        .rect,
                                                        knobs::row(
                                                            ui,
                                                            gain,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                ui.add(Knob::new(
                                                                    value,
                                                                    percent_spec("Drive gain", 0.2),
                                                                ))
                                                            },
                                                        )
                                                        .rect,
                                                        knobs::row(
                                                            ui,
                                                            tone,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                ui.add(Knob::new(
                                                                    value,
                                                                    percent_spec("Drive tone", 0.5)
                                                                        .centered(0.5)
                                                                        .bipolar()
                                                                        .endpoint_labels("−", "+"),
                                                                ))
                                                            },
                                                        )
                                                        .rect,
                                                    ];
                                                } else {
                                                    bounds.knobs[index][0] = knobs::row(
                                                        ui,
                                                        color_shape,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, value| {
                                                            let mut index = value.index() as f32;
                                                            let response = ui.add(Knob::new(
                                                                &mut index,
                                                                KnobSpec::new(
                                                                    "Color shape",
                                                                    0.0..=3.0,
                                                                    0.0,
                                                                    1.0,
                                                                )
                                                                .stepped(1.0)
                                                                .cyclic()
                                                                .diagrams(shape_diagram)
                                                                .format(|value| {
                                                                    shape(value).name().into()
                                                                }),
                                                            ));
                                                            if response.changed() {
                                                                *value = shape(index);
                                                            }
                                                            response
                                                        },
                                                    )
                                                    .rect;
                                                    ui.add_enabled_ui(
                                                        *color_shape != DriveColorShape::Off,
                                                        |ui| {
                                                            bounds.knobs[index][1] = knobs::row(
                                                                ui,
                                                                color_strength,
                                                                knobs::CHOICE_SIZE,
                                                                |ui, value| {
                                                                    ui.add(Knob::new(
                                                                        value,
                                                                        percent_spec(
                                                                            "Color strength",
                                                                            0.5,
                                                                        ),
                                                                    ))
                                                                },
                                                            )
                                                            .rect;
                                                            bounds.knobs[index][2] = knobs::row(
                                                                ui,
                                                                color_frequency_hz,
                                                                knobs::CHOICE_SIZE,
                                                                |ui, value| {
                                                                    ui.add(Knob::new(
                                                                        value,
                                                                        frequency_spec(),
                                                                    ))
                                                                },
                                                            )
                                                            .rect;
                                                        },
                                                    );
                                                }
                                            }
                                            rows::centered(
                                                ui,
                                                if compact {
                                                    crate::envelope_panel::body_height(true)
                                                } else {
                                                    icons::COMPACT_BUTTON_SIZE
                                                },
                                                |ui| {
                                                    let label = egui::RichText::new(name)
                                                        .size(panels::HEADER_FONT_SIZE)
                                                        .color(theme::palette(ui.ctx()).text);
                                                    let width = ui
                                                        .painter()
                                                        .layout_no_wrap(
                                                            name.into(),
                                                            egui::FontId::proportional(
                                                                panels::HEADER_FONT_SIZE,
                                                            ),
                                                            theme::palette(ui.ctx()).text,
                                                        )
                                                        .size()
                                                        .x;
                                                    ui.add_space(
                                                        ((ui.available_width() - width) * 0.5)
                                                            .max(0.0)
                                                            .round(),
                                                    );
                                                    bounds.labels[index] = ui
                                                        .add(
                                                            egui::Label::new(label)
                                                                .selectable(false),
                                                        )
                                                        .rect;
                                                },
                                            );
                                        },
                                    );
                                })
                        })
                        .inner
                        .response
                        .rect;
                    if index > 0 {
                        panels::unit_divider(ui, strip.left(), strip.y_range().into());
                    }
                }
            });
        },
    );
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
