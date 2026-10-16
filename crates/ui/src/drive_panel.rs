use crate::parameter_knobs;
use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::{DriveColorShape, DriveSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 2],
    pub labels: [Rect; 2],
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
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, compact, height, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut DriveSettings,
    compact: bool,
    height: f32,
    sources: &[crate::modulation_panel::Source],
    live: Option<[f32; 5]>,
) -> (Bounds, egui::Response) {
    let open = settings.modulation_open;
    let bases = [
        settings.volume_db,
        settings.gain,
        settings.tone,
        settings.color_strength,
        settings.color_frequency_hz,
    ];
    let markers: [Option<knobs::ModulationMarker>; 5] = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            daw_core::ModulationParameter::DRIVE[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
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
        modulation_open,
        modulations,
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
                let mut last_strip = Rect::NOTHING;
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
                                                                ui.add(
                                                                    Knob::new(
                                                                        value,
                                                                        parameter_knobs::destination(Parameter::Volume),
                                                                    )
                                                                    .modulated(
                                                                        markers[0].as_ref(),
                                                                        modulation_open,
                                                                    ),
                                                                )
                                                            },
                                                        )
                                                        .rect,
                                                        knobs::row(
                                                            ui,
                                                            gain,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                ui.add(
                                                                    Knob::new(
                                                                        value,
                                                                        parameter_knobs::destination(Parameter::DriveGain),
                                                                    )
                                                                    .modulated(
                                                                        markers[1].as_ref(),
                                                                        modulation_open,
                                                                    ),
                                                                )
                                                            },
                                                        )
                                                        .rect,
                                                        knobs::row(
                                                            ui,
                                                            tone,
                                                            knobs::CHOICE_SIZE,
                                                            |ui, value| {
                                                                ui.add(
                                                                    Knob::new(
                                                                        value,
                                                                        parameter_knobs::destination(Parameter::Tone),
                                                                    )
                                                                    .modulated(
                                                                        markers[2].as_ref(),
                                                                        modulation_open,
                                                                    ),
                                                                )
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
                                                                    ui.add(
                                                                        Knob::new(
                                                                            value,
                                                                            parameter_knobs::destination(Parameter::ColorStrength),
                                                                        )
                                                                        .modulated(
                                                                            markers[3].as_ref(),
                                                                            modulation_open,
                                                                        ),
                                                                    )
                                                                },
                                                            )
                                                            .rect;
                                                            bounds.knobs[index][2] = knobs::row(
                                                                ui,
                                                                color_frequency_hz,
                                                                knobs::CHOICE_SIZE,
                                                                |ui, value| {
                                                                    ui.add(
                                                                        Knob::new(
                                                                            value,
                                                                            parameter_knobs::destination(Parameter::ColorFrequency),
                                                                        )
                                                                        .modulated(
                                                                            markers[4].as_ref(),
                                                                            modulation_open,
                                                                        ),
                                                                    )
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
                                                    bounds.labels[index] =
                                                        rows::unit_label(ui, name).rect;
                                                },
                                            );
                                        },
                                    );
                                })
                        })
                        .inner
                        .response
                        .rect;
                    last_strip = strip;
                    if index > 0 {
                        panels::unit_divider(ui, strip.left(), strip.y_range().into());
                    }
                }
                if open {
                    panels::unit_divider(ui, last_strip.right(), last_strip.y_range().into());
                    crate::modulation_panel::body(
                        ui,
                        modulations,
                        sources,
                        <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS, <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE,
                        compact,
                    );
                }
            });
        },
    );
    crate::modulation_panel::toggle(ui, &result.header, *collapsed, modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}
