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
    pub controls: [Option<Response>; 8],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::CompressorSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 8],
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
                                    let mut value = s.get(P::Threshold) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Threshold", -60.0..=0.0, -18.0, 0.01)
                                            .format(|v| {
                                                P::Threshold
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Threshold,
                                            f64::from(value).clamp(
                                                P::Threshold.info().minimum,
                                                P::Threshold.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Ratio) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Ratio", 1.0..=20.0, 4.0, 0.01)
                                            .scale(
                                                |v| (v / 1.0).ln() / (20.0_f32 / 1.0).ln(),
                                                |p| 1.0 * (20.0_f32 / 1.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::Ratio.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Ratio,
                                            f64::from(value).clamp(
                                                P::Ratio.info().minimum,
                                                P::Ratio.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
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
                                bounds.controls[7] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Comp");
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
                                    let mut value = s.get(P::Attack) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Attack", 0.1..=200.0, 10.0, 0.01)
                                            .scale(
                                                |v| (v / 0.1).ln() / (200.0_f32 / 0.1).ln(),
                                                |p| 0.1 * (200.0_f32 / 0.1).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::Attack.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Attack,
                                            f64::from(value).clamp(
                                                P::Attack.info().minimum,
                                                P::Attack.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Release) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Release", 10.0..=2000.0, 100.0, 0.01)
                                            .scale(
                                                |v| (v / 10.0).ln() / (2000.0_f32 / 10.0).ln(),
                                                |p| 10.0 * (2000.0_f32 / 10.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::Release.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Release,
                                            f64::from(value).clamp(
                                                P::Release.info().minimum,
                                                P::Release.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[3] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Knee) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Knee", 0.0..=24.0, 6.0, 0.01).format(|v| {
                                            P::Knee.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Knee,
                                            f64::from(value).clamp(
                                                P::Knee.info().minimum,
                                                P::Knee.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[4] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Response");
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
                                bounds.controls[5] = Some(r);
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
                                bounds.controls[6] = Some(r);
                                knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, _| {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                });
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
        bounds.sections[2] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
