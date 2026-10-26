use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    3.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 3],
    pub controls: [Option<Response>; 5],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::StereoSpreadSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 5],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut state = PluginState::from_settings(settings).unwrap_or_default();
    let mut bounds = Bounds {
        sections: [egui::Rect::NOTHING; 3],
        controls: std::array::from_fn(|_| None),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let strip = ui
            .push_id(0, |ui| {
                egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(1.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowerIntensity) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Lower Intensity", -1.0..=1.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::LowerIntensity
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowerIntensity,
                                            f64::from(value).clamp(
                                                P::LowerIntensity.info().minimum,
                                                P::LowerIntensity.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowerFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "Lower Frequency",
                                            20.0..=20000.0,
                                            20.0,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 20.0).ln() / (20000.0_f32 / 20.0).ln(),
                                            |p| 20.0 * (20000.0_f32 / 20.0).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::LowerFrequency
                                                .format(f64::from(v))
                                                .unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowerFrequency,
                                            f64::from(value).clamp(
                                                P::LowerFrequency.info().minimum,
                                                P::LowerFrequency.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
                                knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, _| {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                });
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Lower");
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[0] = strip;
        let _ = strip;
        let strip = ui
            .push_id(1, |ui| {
                egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(1.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::UpperIntensity) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Upper Intensity", -1.0..=1.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::UpperIntensity
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::UpperIntensity,
                                            f64::from(value).clamp(
                                                P::UpperIntensity.info().minimum,
                                                P::UpperIntensity.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::UpperFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "Upper Frequency",
                                            20.0..=20000.0,
                                            20000.0,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 20.0).ln() / (20000.0_f32 / 20.0).ln(),
                                            |p| 20.0 * (20000.0_f32 / 20.0).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::UpperFrequency
                                                .format(f64::from(v))
                                                .unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::UpperFrequency,
                                            f64::from(value).clamp(
                                                P::UpperFrequency.info().minimum,
                                                P::UpperFrequency.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[3] = Some(r);
                                knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, _| {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                });
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Upper");
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[1] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        let _ = strip;
        let strip = ui
            .push_id(2, |ui| {
                egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(1.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Order) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Order", 2.0..=12.0, 8.0, 1.0)
                                            .stepped(1.0)
                                            .format(|v| {
                                                P::Order.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Order,
                                            f64::from(value).clamp(
                                                P::Order.info().minimum,
                                                P::Order.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[4] = Some(r);
                                knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, _| {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                });
                                knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, _| {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                });
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Spread");
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[2] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
