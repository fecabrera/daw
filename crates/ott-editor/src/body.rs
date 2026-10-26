use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    7.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 4],
    pub controls: [Option<Response>; 23],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::OttSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 23],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut state = PluginState::from_settings(settings).unwrap_or_default();
    let mut bounds = Bounds {
        sections: [egui::Rect::NOTHING; 4],
        controls: std::array::from_fn(|_| None),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let strip = ui
            .push_id(0, |ui| {
                egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(2.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Frequency", 20.0..=2000.0, 250.0, 0.01)
                                            .scale(
                                                |v| (v / 20.0).ln() / (2000.0_f32 / 20.0).ln(),
                                                |p| 20.0 * (2000.0_f32 / 20.0).powf(p),
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
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowDepth) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Depth", 0.0..=1.0, 1.0, 0.01).format(
                                            |v| {
                                                P::LowDepth.format(f64::from(v)).unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowDepth,
                                            f64::from(value).clamp(
                                                P::LowDepth.info().minimum,
                                                P::LowDepth.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowInputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Input Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::LowInputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowInputDb,
                                            f64::from(value).clamp(
                                                P::LowInputDb.info().minimum,
                                                P::LowInputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[3] = Some(r);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowOutputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Output Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::LowOutputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowOutputDb,
                                            f64::from(value).clamp(
                                                P::LowOutputDb.info().minimum,
                                                P::LowOutputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[4] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowAttackMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Attack Ms", 0.1..=200.0, 10.0, 0.01)
                                            .scale(
                                                |v| (v / 0.1).ln() / (200.0_f32 / 0.1).ln(),
                                                |p| 0.1 * (200.0_f32 / 0.1).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::LowAttackMs
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowAttackMs,
                                            f64::from(value).clamp(
                                                P::LowAttackMs.info().minimum,
                                                P::LowAttackMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[5] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::LowReleaseMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Low Release Ms", 10.0..=2000.0, 100.0, 0.01)
                                            .scale(
                                                |v| (v / 10.0).ln() / (2000.0_f32 / 10.0).ln(),
                                                |p| 10.0 * (2000.0_f32 / 10.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::LowReleaseMs
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::LowReleaseMs,
                                            f64::from(value).clamp(
                                                P::LowReleaseMs.info().minimum,
                                                P::LowReleaseMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[6] = Some(r);
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
                            bounds.controls[7] = Some(r);
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
                        ui.set_width(2.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::MidDepth) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Depth", 0.0..=1.0, 1.0, 0.01).format(
                                            |v| {
                                                P::MidDepth.format(f64::from(v)).unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidDepth,
                                            f64::from(value).clamp(
                                                P::MidDepth.info().minimum,
                                                P::MidDepth.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[8] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::MidInputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Input Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::MidInputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidInputDb,
                                            f64::from(value).clamp(
                                                P::MidInputDb.info().minimum,
                                                P::MidInputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[9] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::MidOutputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Output Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::MidOutputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidOutputDb,
                                            f64::from(value).clamp(
                                                P::MidOutputDb.info().minimum,
                                                P::MidOutputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[10] = Some(r);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::MidAttackMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Attack Ms", 0.1..=200.0, 10.0, 0.01)
                                            .scale(
                                                |v| (v / 0.1).ln() / (200.0_f32 / 0.1).ln(),
                                                |p| 0.1 * (200.0_f32 / 0.1).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::MidAttackMs
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidAttackMs,
                                            f64::from(value).clamp(
                                                P::MidAttackMs.info().minimum,
                                                P::MidAttackMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[11] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::MidReleaseMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mid Release Ms", 10.0..=2000.0, 100.0, 0.01)
                                            .scale(
                                                |v| (v / 10.0).ln() / (2000.0_f32 / 10.0).ln(),
                                                |p| 10.0 * (2000.0_f32 / 10.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::MidReleaseMs
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::MidReleaseMs,
                                            f64::from(value).clamp(
                                                P::MidReleaseMs.info().minimum,
                                                P::MidReleaseMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[12] = Some(r);
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
                            bounds.controls[13] = Some(r);
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
                        ui.set_width(2.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighFrequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "High Frequency",
                                            200.0..=20000.0,
                                            2500.0,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 200.0).ln() / (20000.0_f32 / 200.0).ln(),
                                            |p| 200.0 * (20000.0_f32 / 200.0).powf(p),
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
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighDepth) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("High Depth", 0.0..=1.0, 1.0, 0.01).format(
                                            |v| {
                                                P::HighDepth
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighDepth,
                                            f64::from(value).clamp(
                                                P::HighDepth.info().minimum,
                                                P::HighDepth.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[14] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighInputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("High Input Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::HighInputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighInputDb,
                                            f64::from(value).clamp(
                                                P::HighInputDb.info().minimum,
                                                P::HighInputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[15] = Some(r);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighOutputDb) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("High Output Db", -24.0..=24.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::HighOutputDb
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighOutputDb,
                                            f64::from(value).clamp(
                                                P::HighOutputDb.info().minimum,
                                                P::HighOutputDb.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[16] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighAttackMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("High Attack Ms", 0.1..=200.0, 10.0, 0.01)
                                            .scale(
                                                |v| (v / 0.1).ln() / (200.0_f32 / 0.1).ln(),
                                                |p| 0.1 * (200.0_f32 / 0.1).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::HighAttackMs
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighAttackMs,
                                            f64::from(value).clamp(
                                                P::HighAttackMs.info().minimum,
                                                P::HighAttackMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[17] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::HighReleaseMs) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "High Release Ms",
                                            10.0..=2000.0,
                                            100.0,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 10.0).ln() / (2000.0_f32 / 10.0).ln(),
                                            |p| 10.0 * (2000.0_f32 / 10.0).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| {
                                            P::HighReleaseMs
                                                .format(f64::from(v))
                                                .unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::HighReleaseMs,
                                            f64::from(value).clamp(
                                                P::HighReleaseMs.info().minimum,
                                                P::HighReleaseMs.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[18] = Some(r);
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
                            bounds.controls[19] = Some(r);
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[2] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        let _ = strip;
        let strip = ui
            .push_id(3, |ui| {
                egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(1.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Level) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        knobs::marked_gain_spec().named("Level").format(|v| {
                                            P::Level.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Level,
                                            f64::from(value).clamp(
                                                P::Level.info().minimum,
                                                P::Level.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[20] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::DryWet) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Dry Wet", 0.0..=1.0, 1.0, 0.01).format(
                                            |v| P::DryWet.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::DryWet,
                                            f64::from(value).clamp(
                                                P::DryWet.info().minimum,
                                                P::DryWet.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[21] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Detection) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Detection", 0.0..=1.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .two_way_labels("Peak", "RMS")
                                            .format(|v| {
                                                P::Detection
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Detection,
                                            f64::from(value).clamp(
                                                P::Detection.info().minimum,
                                                P::Detection.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[22] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Out");
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[3] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
