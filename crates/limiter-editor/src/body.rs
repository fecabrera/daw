use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    2.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 2],
    pub controls: [Option<Response>; 6],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::LimiterSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 6],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut state = PluginState::from_settings(settings).unwrap_or_default();
    let mut bounds = Bounds {
        sections: [egui::Rect::NOTHING; 2],
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
                                    let mut value = s.get(P::Input) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Input", -24.0..=24.0, 0.0, 0.01).format(
                                            |v| P::Input.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Input,
                                            f64::from(value).clamp(
                                                P::Input.info().minimum,
                                                P::Input.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Release) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Release", 1.0..=2000.0, 100.0, 0.01)
                                            .scale(
                                                |v| (v / 1.0).ln() / (2000.0_f32 / 1.0).ln(),
                                                |p| 1.0 * (2000.0_f32 / 1.0).powf(p),
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
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Lookahead) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Lookahead", 0.0..=10.0, 1.0, 0.01).format(
                                            |v| {
                                                P::Lookahead
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            },
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Lookahead,
                                            f64::from(value).clamp(
                                                P::Lookahead.info().minimum,
                                                P::Lookahead.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Limit");
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
                                    let mut value = s.get(P::Ceiling) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Ceiling", -24.0..=0.0, -1.0, 0.01).format(
                                            |v| P::Ceiling.format(f64::from(v)).unwrap_or_default(),
                                        ),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Ceiling,
                                            f64::from(value).clamp(
                                                P::Ceiling.info().minimum,
                                                P::Ceiling.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[3] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Mode) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Mode", 0.0..=1.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .two_way_labels("Peak", "True Peak")
                                            .format(|v| {
                                                P::Mode.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Mode,
                                            f64::from(value).clamp(
                                                P::Mode.info().minimum,
                                                P::Mode.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[4] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Routing) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Routing", 0.0..=1.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .two_way_labels("L/R", "M/S")
                                            .format(|v| {
                                                P::Routing.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Routing,
                                            f64::from(value).clamp(
                                                P::Routing.info().minimum,
                                                P::Routing.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[5] = Some(r);
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
        bounds.sections[1] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
