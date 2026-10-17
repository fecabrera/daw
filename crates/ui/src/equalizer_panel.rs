use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, rows, theme,
};
use daw_core::EqSettings;
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 3],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}

fn gain_spec() -> KnobSpec {
    KnobSpec::new("EQ gain", -24.0..=24.0, 0.0, 0.1)
        .centered(0.0)
        .bipolar()
        .reference_step(6.0)
        .value_label(|value| format!("{value:.1} dB"))
        .endpoint_labels("−24", "+24")
        .format(|value| format!("{value:.1} dB"))
}
fn frequency_spec(reset: f32) -> KnobSpec {
    KnobSpec::new("EQ frequency", 20.0..=20_000.0, reset, 1.0)
        .scale(
            |value| (value / 20.0).log10() / 3.0,
            |position| 20.0 * 1000_f32.powf(position),
            |value, delta| value * 2_f32.powf(delta / 24.0),
        )
        .value_label(knobs::format_frequency)
        .format(knobs::format_frequency)
        .endpoint_labels("20", "20k")
}
fn q_spec() -> KnobSpec {
    KnobSpec::new("EQ Q", 0.1..=10.0, std::f32::consts::FRAC_1_SQRT_2, 0.01)
        .scale(
            |value| (value / 0.1).log10() / 2.0,
            |position| 0.1 * 100_f32.powf(position),
            |value, delta| value * 2_f32.powf(delta / 24.0),
        )
        .value_label(|value| format!("Q {value:.2}"))
        .format(|value| format!("Q {value:.2}"))
        .endpoint_labels("0.1", "10")
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut EqSettings,
    compact: bool,
    height: f32,
    levels: [f32; 3],
) -> (Bounds, egui::Response) {
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 3],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("3-band EQ", compact),
        height,
        &mut settings.enabled,
        &mut settings.collapsed,
        |ui| {
            body(
                ui,
                &mut settings.bands,
                panel.margin,
                compact,
                levels,
                &mut bounds,
            )
        },
    );
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}

fn body(
    ui: &mut Ui,
    bands: &mut [daw_core::EqBandSettings; 3],
    margin: i8,
    compact: bool,
    levels: [f32; 3],
    bounds: &mut Bounds,
) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        for (index, name) in ["Low", "Mid", "High"].into_iter().enumerate() {
            let strip = ui.push_id(index, |ui| {
                egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(margin));
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        let band = &mut bands[index];
                        if !compact { bounds.knobs[index] = controls(ui, band, EqSettings::default().bands[index].frequency_hz, index == 1); }
                        rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                            let response = rows::power_label(ui, name, &format!("Toggle {name} band cut"), band.enabled);
                            let response = response.on_hover_text(if index == 1 {
                                "Mid shelf uses Low/High frequency boundaries and Q slopes. Off cuts this middle range; saved gain is retained.".into()
                            } else {
                                format!("{name}: off cuts this range using frequency and Q; saved gain is retained.")
                            });
                            bounds.bands[index] = response.rect;
                            if response.clicked() { band.enabled = !band.enabled; }
                        });
                    });
                })
            }).inner.response.rect;
            if !compact {
                let rail = meters::unit_monitor_rail(strip, margin);
                bounds.meters[index] = rail;
                meters::band_level_rail(ui, rail, name, levels[index]);
            }
            if index > 0 { panels::unit_divider(ui, strip.left(), strip.y_range().into()); }
        }
    });
}
fn controls(
    ui: &mut Ui,
    band: &mut daw_core::EqBandSettings,
    frequency_reset: f32,
    middle: bool,
) -> [Rect; 3] {
    if middle {
        let gain = knobs::row(ui, &mut band.gain_db, knobs::CHOICE_SIZE, |ui, value| {
            ui.add(Knob::new(value, gain_spec()))
        })
        .rect;
        // Keep footer alignment with the outer strips without exposing unused controls.
        for _ in 0..2 {
            knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| {
                ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover())
            });
        }
        return [gain, Rect::NOTHING, Rect::NOTHING];
    }
    [
        knobs::row(ui, &mut band.gain_db, knobs::CHOICE_SIZE, |ui, value| {
            ui.add(Knob::new(value, gain_spec()))
        })
        .rect,
        knobs::row(
            ui,
            &mut band.frequency_hz,
            knobs::CHOICE_SIZE,
            |ui, value| ui.add(Knob::new(value, frequency_spec(frequency_reset))),
        )
        .rect,
        knobs::row(ui, &mut band.q, knobs::CHOICE_SIZE, |ui, value| {
            ui.add(Knob::new(value, q_spec()))
        })
        .rect,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_rails_fit_each_strip_and_leave_controls_and_saved_settings_unchanged() {
        for color_theme in [
            crate::settings::ColorTheme::Dark,
            crate::settings::ColorTheme::Light,
        ] {
            for compact in [false, true] {
                for collapsed in [false, true] {
                    let ctx = egui::Context::default();
                    theme::configure(&ctx);
                    crate::fonts::configure(&ctx);
                    theme::apply(
                        &ctx,
                        crate::settings::AppSettings {
                            theme: color_theme,
                            ..Default::default()
                        },
                    );
                    let mut settings = EqSettings {
                        collapsed,
                        ..Default::default()
                    };
                    let original = settings.clone();
                    let mut bounds = None;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(800.0, 600.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default().show(ui, |ui| {
                                bounds = Some(
                                    show(
                                        ui,
                                        &mut settings,
                                        compact,
                                        if compact { 68.0 } else { 244.0 },
                                        [0.0, 10_f32.powf(-30.0 / 20.0), 2.0],
                                    )
                                    .0,
                                );
                            });
                        },
                    );
                    output.textures_delta.clear();
                    let bounds = bounds.unwrap();
                    assert_eq!(settings, original);
                    for index in 0..3 {
                        let rail = bounds.meters[index];
                        if compact || collapsed {
                            assert!(!rail.is_positive());
                        } else {
                            assert_eq!(rail.height(), 4.0);
                            assert_eq!(
                                rail.width(),
                                UNIT_STRIP_WIDTH
                                    - 2.0 * f32::from(panels::PanelStyle::default().margin)
                            );
                            assert!(rail.top() >= bounds.header.bottom());
                            assert!(rail.bottom() < bounds.knobs[index][0].top());
                            assert!(
                                (rail.center().x - bounds.knobs[index][0].center().x).abs() < 0.01
                            );
                            let expected = [0.0, rail.width() * 0.5, rail.width()][index];
                            let color = if index == 2 {
                                theme::palette(&ctx).error
                            } else {
                                theme::accent(&ctx)
                            };
                            let fills: Vec<_> = output
                                .shapes
                                .iter()
                                .filter_map(|shape| match &shape.shape {
                                    egui::Shape::Rect(rect)
                                        if rect.rect.min == rail.min && rect.fill == color =>
                                    {
                                        Some(rect.rect)
                                    }
                                    _ => None,
                                })
                                .collect();
                            if index == 0 {
                                assert!(fills.is_empty());
                            } else {
                                assert_eq!(fills.len(), 1);
                                assert!((fills[0].width() - expected).abs() < 0.01);
                            }
                        }
                    }
                }
            }
        }
    }
}
