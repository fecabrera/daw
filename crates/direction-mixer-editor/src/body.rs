use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    1.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [egui::Rect; 1],
    pub controls: [Option<Response>; 3],
}
pub fn show(
    ui: &mut Ui,
    settings: &mut daw_unit_core::DirectionMixerSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 3],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut state = PluginState::from_settings(settings).unwrap_or_default();
    let mut bounds = Bounds {
        sections: [egui::Rect::NOTHING; 1],
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
                                        KnobSpec::new("Input", 0.0..=1.0, 0.0, 1.0)
                                            .stepped(1.0)
                                            .two_way_labels("L/R", "M/S")
                                            .format(|v| {
                                                P::Input.format(f64::from(v)).unwrap_or_default()
                                            }),
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
                                    let mut value = s.get(P::Direction) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Direction", -180.0..=180.0, 0.0, 0.01)
                                            .format(|v| {
                                                P::Direction
                                                    .format(f64::from(v))
                                                    .unwrap_or_default()
                                            }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Direction,
                                            f64::from(value).clamp(
                                                P::Direction.info().minimum,
                                                P::Direction.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[1] = Some(r);
                                let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                    let mut value = s.get(P::Spread) as f32;
                                    let r = ui.add(Knob::new(
                                        &mut value,
                                        KnobSpec::new("Spread", 0.0..=2.0, 1.0, 0.01).format(|v| {
                                            P::Spread.format(f64::from(v)).unwrap_or_default()
                                        }),
                                    ));
                                    if r.changed() {
                                        let _ = s.set(
                                            P::Spread,
                                            f64::from(value).clamp(
                                                P::Spread.info().minimum,
                                                P::Spread.info().maximum,
                                            ),
                                        );
                                    }
                                    r
                                });
                                bounds.controls[2] = Some(r);
                            });
                        });
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Stereo");
                        });
                    })
            })
            .inner
            .response
            .rect;
        bounds.sections[0] = strip;
        meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
    });
    *settings = state.to_settings();
    bounds
}
