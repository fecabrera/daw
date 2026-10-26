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
    pub controls: [Option<Response>; 5],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::FilterSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 5],
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
                                    let mut value = s.get(P::Shape) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Shape", 0.0..=3.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .cyclic()
                                            .diagrams(filter_diagram)
                                            .format(|v| {
                                                P::Shape.format(f64::from(v)).unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Shape,
                                            f64::from(value).clamp(
                                                P::Shape.info().minimum,
                                                P::Shape.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[0] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Frequency) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Frequency", 20.0..=20000.0, 1000.0, 0.01)
                                            .scale(
                                                |v| (v / 20.0).ln() / (20000.0_f32 / 20.0).ln(),
                                                |p| 20.0 * (20000.0_f32 / 20.0).powf(p),
                                                |v, d| v * 2.0_f32.powf(d / 24.0),
                                            )
                                            .format(|v| {
                                                P::Frequency
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Frequency,
                                            f64::from(value).clamp(
                                                P::Frequency.info().minimum,
                                                P::Frequency.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Q) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new(
                                            "Q",
                                            0.1..=10.0,
                                            std::f32::consts::FRAC_1_SQRT_2,
                                            0.01,
                                        )
                                        .scale(
                                            |v| (v / 0.1).ln() / (10.0_f32 / 0.1).ln(),
                                            |p| 0.1 * (10.0_f32 / 0.1).powf(p),
                                            |v, d| v * 2.0_f32.powf(d / 24.0),
                                        )
                                        .format(|v| P::Q.format(f64::from(v)).unwrap_or_default()),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Q,
                                            f64::from(value)
                                                .clamp(P::Q.info().minimum, P::Q.info().maximum),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Filter");
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
                                bounds.controls[3] = Some(r);
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
                                bounds.controls[4] = Some(r);
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
        bounds.sections[1] = strip;
        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
fn filter_diagram(value: f32) -> Vec<Vec2> {
    let shape = daw_unit_core::FilterShape::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let band = (-((x - 0.5) * 5.0).powi(2)).exp();
            let level = match shape {
                daw_unit_core::FilterShape::LowPass => low,
                daw_unit_core::FilterShape::HighPass => 1.0 - low,
                daw_unit_core::FilterShape::BandPass => band,
                daw_unit_core::FilterShape::Notch => 1.0 - band,
            };
            Vec2::new(x - 0.5, 0.5 - level)
        })
        .collect()
}
