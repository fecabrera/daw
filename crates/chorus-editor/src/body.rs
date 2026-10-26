use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    4.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 3],
    pub controls: [Option<Response>; 11],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::ChorusSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 11],
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
                                    let mut value = s.get(P::Delay) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Delay", 5.0..=30.0, 15.0, 0.01).format(
                                            |v| P::Delay.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Delay,
                                            f64::from(value).clamp(
                                                P::Delay.info().minimum,
                                                P::Delay.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Depth) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Depth", 0.0..=1.0, 0.25, 0.01).format(|v| {
                                            P::Depth.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Depth,
                                            f64::from(value).clamp(
                                                P::Depth.info().minimum,
                                                P::Depth.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Feedback) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Feedback", -0.8..=0.8, 0.0, 0.01).format(
                                            |v| {
                                                P::Feedback.format(f64::from(v)).unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Feedback,
                                            f64::from(value).clamp(
                                                P::Feedback.info().minimum,
                                                P::Feedback.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Chorus");
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
                                    let mut value = s.get(P::RateMode) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Rate Mode", 0.0..=1.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .two_way_labels("Hz", "Note")
                                            .format(|v| {
                                                P::RateMode.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::RateMode,
                                            f64::from(value).clamp(
                                                P::RateMode.info().minimum,
                                                P::RateMode.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[9] = Some(r);
                                if state.get(P::RateMode) == 0.0 {
                                    let r =
                                        knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                            let mut value = s.get(P::RateHz) as f32;
                                            let r = ui.add(Knob::new(
                                                &mut value,
                                                KnobSpec::new("Rate Hz", 0.05..=10.0, 0.8, 0.01)
                                                    .scale(
                                                        |v| {
                                                            (v / 0.05).ln() / (10.0_f32 / 0.05).ln()
                                                        },
                                                        |p| 0.05 * (10.0_f32 / 0.05).powf(p),
                                                        |v, d| v * 2.0_f32.powf(d / 24.0),
                                                    )
                                                    .format(|v| {
                                                        P::RateHz
                                                            .format(f64::from(v))
                                                            .unwrap_or_default()
                                                    }),
                                            ));
                                            if r.changed() {
                                                let _ = s.set(
                                                    P::RateHz,
                                                    f64::from(value).clamp(
                                                        P::RateHz.info().minimum,
                                                        P::RateHz.info().maximum,
                                                    ),
                                                );
                                            }
                                            r
                                        });
                                    bounds.controls[3] = Some(r);
                                } else if state.get(P::RateMode) == 1.0 {
                                    let r =
                                        knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                            let mut value = s.get(P::NoteRate) as f32;
                                            let r = ui.add(Knob::new(
                                                &mut value,
                                                KnobSpec::new("Note Rate", 0.0..=26.0, 12.0, 1.0)
                                                    .stepped(1.0)
                                                    .format(|v| {
                                                        P::NoteRate
                                                            .format(f64::from(v))
                                                            .unwrap_or_default()
                                                    }),
                                            ));
                                            if r.changed() {
                                                let _ = s.set(
                                                    P::NoteRate,
                                                    f64::from(value).clamp(
                                                        P::NoteRate.info().minimum,
                                                        P::NoteRate.info().maximum,
                                                    ),
                                                );
                                            }
                                            r
                                        });
                                    bounds.controls[10] = Some(r);
                                }
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::StereoPhase) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Stereo Phase", -180.0..=180.0, 90.0, 0.01)
                                            .format(|v| {
                                                P::StereoPhase
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::StereoPhase,
                                            f64::from(value).clamp(
                                                P::StereoPhase.info().minimum,
                                                P::StereoPhase.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[4] = Some(r);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Voices) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Voices", 1.0..=4.0, 2.0, 1.0)
                                            .stepped(1.0)
                                            .format(|v| {
                                                P::Voices.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Voices,
                                            f64::from(value).clamp(
                                                P::Voices.info().minimum,
                                                P::Voices.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[8] = Some(r);
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
                            rows::unit_label(ui, "Voices");
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
                                    let mut value = s.get(P::Width) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Width", 0.0..=2.0, 1.0, 0.01).format(|v| {
                                            P::Width.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Width,
                                            f64::from(value).clamp(
                                                P::Width.info().minimum,
                                                P::Width.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[5] = Some(r);
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
                                bounds.controls[6] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::DryWet) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Dry Wet", 0.0..=1.0, 0.35, 0.01).format(
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
                                bounds.controls[7] = Some(r);
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
