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
    pub controls: [Option<Response>; 10],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::EqSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 10],
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
                                    let mut value = s.get(P::LowGain) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Gain", -24.0..=24.0, 0.0, 0.01).format(
                                            |v| P::LowGain.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowGain,
                                            f64::from(value).clamp(
                                                P::LowGain.info().minimum,
                                                P::LowGain.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Frequency", 20.0..=20000.0, 250.0, 0.01)
                                            .scale(
                                                |v| (v / 20.0).ln() / (20000.0_f32 / 20.0).ln(),
                                                |p| 20.0 * (20000.0_f32 / 20.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::LowFrequency
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowFrequency,
                                            f64::from(value).clamp(
                                                P::LowFrequency.info().minimum,
                                                P::LowFrequency.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowQ) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "Low Q",
                                            0.1..=10.0,
                                            std::f32::consts::FRAC_1_SQRT_2,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 0.1).ln() / (10.0_f32 / 0.1).ln(),
                                            |p| 0.1 * (10.0_f32 / 0.1).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::LowQ.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowQ,
                                            f64::from(value).clamp(
                                                P::LowQ.info().minimum,
                                                P::LowQ.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            let mut r = rows::power_label(
                                ui,
                                "Low",
                                "Low power",
                                state.get(P::LowEnabled) != 0.0,
                            );
                            if r.clicked() {
                                let _ = state
                                    .set(P::LowEnabled, f64::from(state.get(P::LowEnabled) == 0.0));
                                r.mark_changed();
                            }
                            bounds.controls[3] = Some(r);
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
                                    let mut value = s.get(P::MidGain) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Gain", -24.0..=24.0, 0.0, 0.01).format(
                                            |v| P::MidGain.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidGain,
                                            f64::from(value).clamp(
                                                P::MidGain.info().minimum,
                                                P::MidGain.info().maximum,
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
                            let mut r = rows::power_label(
                                ui,
                                "Mid",
                                "Mid power",
                                state.get(P::MidEnabled) != 0.0,
                            );
                            if r.clicked() {
                                let _ = state
                                    .set(P::MidEnabled, f64::from(state.get(P::MidEnabled) == 0.0));
                                r.mark_changed();
                            }
                            bounds.controls[5] = Some(r);
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
                                    let mut value = s.get(P::HighGain) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("High Gain", -24.0..=24.0, 0.0, 0.01).format(
                                            |v| {
                                                P::HighGain.format(f64::from(v)).unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighGain,
                                            f64::from(value).clamp(
                                                P::HighGain.info().minimum,
                                                P::HighGain.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[6] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "High Frequency",
                                            20.0..=20000.0,
                                            2500.0,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 20.0).ln() / (20000.0_f32 / 20.0).ln(),
                                            |p| 20.0 * (20000.0_f32 / 20.0).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::HighFrequency
                                                .format(f64::from(v))
                                                .unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighFrequency,
                                            f64::from(value).clamp(
                                                P::HighFrequency.info().minimum,
                                                P::HighFrequency.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[7] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighQ) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "High Q",
                                            0.1..=10.0,
                                            std::f32::consts::FRAC_1_SQRT_2,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 0.1).ln() / (10.0_f32 / 0.1).ln(),
                                            |p| 0.1 * (10.0_f32 / 0.1).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::HighQ.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighQ,
                                            f64::from(value).clamp(
                                                P::HighQ.info().minimum,
                                                P::HighQ.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[8] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            let mut r = rows::power_label(
                                ui,
                                "High",
                                "High power",
                                state.get(P::HighEnabled) != 0.0,
                            );
                            if r.clicked() {
                                let _ = state.set(
                                    P::HighEnabled,
                                    f64::from(state.get(P::HighEnabled) == 0.0),
                                );
                                r.mark_changed();
                            }
                            bounds.controls[9] = Some(r);
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
