//! Fixed, automatable band slots composed from shared unit controls.
use daw_drive_plugin::{Parameter as P, PluginState};
use daw_unit_core::{EqBandMode, MultibandEqSettings};
use daw_unit_ui::{
    knobs::{self, Knob, KnobSpec, ModulationMarker},
    layout::*,
    meters, rows,
};
use egui::{Rect, Response, Ui, Vec2};
pub const SIZE: Vec2 = Vec2::new(
    9.0 * UNIT_STRIP_WIDTH,
    daw_unit_ui::layout::unit_control_height() + 2.0 * PANEL_MARGIN as f32,
);
pub struct Bounds {
    pub sections: [Rect; 65],
    pub controls: [Option<Response>; 321],
}
fn parameter(index: usize, field: usize) -> P {
    P::ALL[1 + 5 * index + field]
}
fn control(ui: &mut Ui, state: &mut PluginState, p: P) -> Response {
    let info = p.info();
    let mut value = state.get(p) as f32;
    let mut spec = KnobSpec::new(
        info.name,
        info.minimum as f32..=info.maximum as f32,
        info.default as f32,
        if info.steps > 0 { 1.0 } else { 0.01 },
    );
    if info.steps > 0 {
        spec = spec
            .stepped(1.0)
            .cyclic()
            .diagrams(diagram)
            .format(|v| EqBandMode::ALL[v as usize].label().to_owned());
    } else if p.unit() == "Hz" {
        spec = spec
            .scale(
                |v| (v / 20.0).ln() / 1000_f32.ln(),
                |t| 20.0 * 1000_f32.powf(t),
                |v, d| v * 2_f32.powf(d / 24.0),
            )
            .format(knobs::format_frequency);
    } else if p.unit() == "Q" {
        spec = spec.scale(
            |v| (v / 0.1).ln() / 100_f32.ln(),
            |t| 0.1 * 100_f32.powf(t),
            |v, d| v * 2_f32.powf(d / 24.0),
        );
    } else {
        spec = spec
            .centered(0.0)
            .bipolar()
            .reference_step(6.0)
            .format(|v| format!("{v:.1} dB"));
    }
    let response = ui.add(Knob::new(&mut value, spec));
    if response.changed() {
        let _ = state.set(p, f64::from(value).clamp(info.minimum, info.maximum));
    }
    response
}
fn diagram(value: f32) -> Vec<Vec2> {
    let mode = EqBandMode::ALL[value as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let bell = (-((x - 0.5) * 5.0).powi(2)).exp();
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let y = match mode {
                EqBandMode::Bell => 0.5 - 0.4 * bell,
                EqBandMode::LowShelf => 0.5 - 0.4 * low,
                EqBandMode::HighShelf => 0.5 - 0.4 * (1.0 - low),
                EqBandMode::LowPass => 1.0 - low,
                EqBandMode::HighPass => low,
                EqBandMode::Notch => bell,
            };
            Vec2::new(x - 0.5, y - 0.5)
        })
        .collect()
}
pub fn show(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    _: bool,
    _: f32,
    _: &[Option<ModulationMarker>; 321],
    level: meters::OutputLevel<'_>,
) -> Bounds {
    let mut state = PluginState::from_settings(settings).unwrap_or_default();
    let mut bounds = Bounds {
        sections: [Rect::NOTHING; 65],
        controls: std::array::from_fn(|_| None),
    };
    egui::ScrollArea::horizontal()
        .id_salt("bands")
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for band in 0..64 {
                    let strip = ui
                        .push_id(band, |ui| {
                            egui::Frame::new()
                                .inner_margin(PANEL_MARGIN)
                                .show(ui, |ui| {
                                    ui.set_width(
                                        2.0 * UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN),
                                    );
                                    ui.horizontal_top(|ui| {
                                        for column in 0..2 {
                                            ui.vertical(|ui| {
                                                ui.set_width(
                                                    UNIT_STRIP_WIDTH
                                                        - 2.0 * f32::from(PANEL_MARGIN),
                                                );
                                                for row in 0..3 {
                                                    let field = 1 + column * 3 + row;
                                                    let r = knobs::row(
                                                        ui,
                                                        &mut state,
                                                        knobs::CHOICE_SIZE,
                                                        |ui, state| {
                                                            if field <= 4 {
                                                                control(
                                                                    ui,
                                                                    state,
                                                                    parameter(band, field),
                                                                )
                                                            } else {
                                                                ui.allocate_response(
                                                                    Vec2::splat(knobs::CHOICE_SIZE),
                                                                    egui::Sense::hover(),
                                                                )
                                                            }
                                                        },
                                                    );
                                                    if field <= 4 {
                                                        bounds.controls[parameter(band, field)
                                                            as usize
                                                            - 101] = Some(r);
                                                    }
                                                }
                                            });
                                        }
                                    });
                                    rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                                        let p = parameter(band, 0);
                                        let mut response = rows::power_label(
                                            ui,
                                            &format!("Band {}", band + 1),
                                            "Band power",
                                            state.get(p) != 0.0,
                                        );
                                        if response.clicked() {
                                            let _ = state.set(p, f64::from(state.get(p) == 0.0));
                                            response.mark_changed();
                                        }
                                        bounds.controls[p as usize - 101] = Some(response);
                                    });
                                })
                        })
                        .inner
                        .response
                        .rect;
                    bounds.sections[band] = strip;
                    if band > 0 {
                        daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
                    }
                }
                let strip = egui::Frame::new()
                    .inner_margin(PANEL_MARGIN)
                    .show(ui, |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(PANEL_MARGIN));
                        for row in 0..3 {
                            let r = knobs::row(ui, &mut state, knobs::CHOICE_SIZE, |ui, s| {
                                if row == 0 {
                                    let mut value = s.get(P::Level) as f32;
                                    let r =
                                        ui.add(Knob::new(&mut value, knobs::marked_gain_spec()));
                                    if r.changed() {
                                        let _ = s.set(P::Level, f64::from(value));
                                    }
                                    r
                                } else {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                }
                            });
                            if row == 0 {
                                bounds.controls[320] = Some(r);
                            }
                        }
                        rows::centered(ui, COMPACT_BUTTON_SIZE, |ui| {
                            rows::unit_label(ui, "Output");
                        });
                    })
                    .response
                    .rect;
                bounds.sections[64] = strip;
                daw_unit_ui::drive::divider(ui, strip.left(), strip.y_range());
                meters::effect_output_level(ui, strip, PANEL_MARGIN, level, settings.enabled);
            })
        });
    *settings = state.to_settings();
    bounds
}
