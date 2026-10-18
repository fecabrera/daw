use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{CompressorDetection, ModulationParameter as Parameter, OttSettings};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub controls: [[[Rect; 3]; 2]; 4],
    pub sections: [Rect; 4],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}
/// Fixed band/control identity; an empty Mid frequency row has no destination.
pub(super) fn parameter_index(section: usize, column: usize, row: usize) -> Option<usize> {
    match (section, column, row) {
        (0, 0, 0) => Some(0),
        (2, 0, 0) => Some(1),
        (band, 1, 0) if band < 3 => Some(2 + 5 * band),
        (band, column, 1) if band < 3 => Some(3 + 5 * band + column),
        (band, column, 2) if band < 3 => Some(5 + 5 * band + column),
        (3, 0, 0) => Some(17),
        (3, 0, 1) => Some(18),
        _ => None,
    }
}
fn row(
    ui: &mut Ui,
    settings: &mut OttSettings,
    section: usize,
    column: usize,
    row: usize,
    markers: &[Option<knobs::ModulationMarker>; 19],
) -> Rect {
    knobs::row(ui, settings, knobs::CHOICE_SIZE, |ui, s| {
        if let Some(index) = parameter_index(section, column, row) {
            let value = match index {
                0 => &mut s.low_frequency_hz,
                1 => &mut s.high_frequency_hz,
                17 => &mut s.level_db,
                18 => &mut s.dry_wet,
                _ => {
                    let band = &mut s.bands[(index - 2) / 5];
                    match (index - 2) % 5 {
                        0 => &mut band.depth,
                        1 => &mut band.input_db,
                        2 => &mut band.output_db,
                        3 => &mut band.attack_ms,
                        4 => &mut band.release_ms,
                        _ => unreachable!(),
                    }
                }
            };
            let response = ui.add(
                Knob::new(
                    value,
                    if index == 17 {
                        parameter_knobs::destination(Parameter::Level).named("OTT output level")
                    } else {
                        parameter_knobs::destination(Parameter::OTT[index])
                    },
                )
                .modulated(markers[index].as_ref(), &mut s.modulation_open),
            );
            if response.changed() {
                if index == 0 {
                    s.low_frequency_hz = s.low_frequency_hz.min(s.high_frequency_hz - 1.0);
                }
                if index == 1 {
                    s.high_frequency_hz = s.high_frequency_hz.max(s.low_frequency_hz + 1.0);
                }
            }
            return response;
        }
        if section < 3 {
            return ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover());
        }
        let mut position = f32::from(s.detection == CompressorDetection::Rms);
        let response = ui.add(Knob::new(
            &mut position,
            KnobSpec::new("Detection mode", 0.0..=1.0, 0.0, 1.0)
                .two_way_labels("Peak", "RMS")
                .format(|v| {
                    if v == 0.0 {
                        "Peak".into()
                    } else {
                        "RMS".into()
                    }
                }),
        ));
        if response.changed() {
            s.detection = if position == 0.0 {
                CompressorDetection::Peak
            } else {
                CompressorDetection::Rms
            };
        }
        response
    })
    .rect
}
pub(super) fn show(
    ui: &mut Ui,
    settings: &mut OttSettings,
    compact: bool,
    height: f32,
    gains: [f32; 3],
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, compact, height, gains, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut OttSettings,
    compact: bool,
    height: f32,
    gains: [f32; 3],
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let gains = meters::unit_reading(settings.enabled, gains);
    let mut bases = [0.0; 19];
    bases[0] = settings.low_frequency_hz;
    bases[1] = settings.high_frequency_hz;
    for (i, b) in settings.bands.iter().enumerate() {
        bases[2 + 5 * i..7 + 5 * i].copy_from_slice(&[
            b.depth,
            b.input_db,
            b.output_db,
            b.attack_ms,
            b.release_ms,
        ]);
    }
    bases[17] = settings.level_db;
    bases[18] = settings.dry_wet;
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::OTT[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let open = settings.modulation_open;
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        controls: [[[Rect::NOTHING; 3]; 2]; 4],
        sections: [Rect::NOTHING; 4],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(ui, ("OTT", compact), height, &mut enabled, &mut collapsed, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for (section, name) in ["Low", "Mid", "High", "Out"].into_iter().enumerate() {
                let rect = ui.push_id(section, |ui| egui::Frame::new().inner_margin(panel.margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let column_width = knobs::CHOICE_SIZE;
                        let padding = (UNIT_STRIP_WIDTH - column_width) * 0.5;
                        let width = if section < 3 { 2.0 * column_width + 3.0 * padding } else { UNIT_STRIP_WIDTH };
                        ui.set_width(width - 2.0 * f32::from(panel.margin));
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        if !compact {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = padding;
                                ui.add_space(padding - f32::from(panel.margin));
                                for column in 0..if section < 3 { 2 } else { 1 } {
                                    ui.push_id(column, |ui| ui.allocate_ui_with_layout(Vec2::new(column_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                        ui.set_width(column_width);
                                        for index in 0..3 { bounds.controls[section][column][index] = row(ui, settings, section, column, index, &markers); }
                                    }));
                                }
                            });
                        }
                        rows::centered(ui, if compact { crate::envelope_panel::body_height(true) } else { icons::COMPACT_BUTTON_SIZE }, |ui| {
                            if section < 3 {
                                let response = rows::power_label(ui, name, "Toggle band compression", settings.bands[section].enabled).on_hover_text("Off bypasses compression; band input/output gains remain active.");
                                bounds.bands[section] = response.rect;
                                if response.clicked() { settings.bands[section].enabled = !settings.bands[section].enabled; }
                            } else { rows::unit_label(ui, name); }
                        });
                    });
                })).inner.response.rect;
                bounds.sections[section] = rect;
                if section > 0 { panels::unit_divider(ui, rect.left(), rect.y_range().into()); }
                if !compact && section < 3 {
                    let rail = meters::unit_monitor_rail(rect, panel.margin);
                    bounds.meters[section] = rail;
                    meters::gain_change(ui, rail, name, gains[section], settings.bands[section].enabled);
                }
            }
            if open {
                let last = bounds.sections[3];
                panels::unit_divider(ui, last.right(), last.y_range().into());
                crate::modulation_panel::body(ui, &mut settings.modulations, sources,
                    <OttSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS,
                    <OttSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
            }
        });
    });
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_meter_colors_follow_each_band_power_without_changing_readings_or_settings() {
        for color_theme in [
            crate::settings::ColorTheme::Dark,
            crate::settings::ColorTheme::Light,
        ] {
            for enabled in [false, true] {
                for disabled_band in 0..=3 {
                    for gain in [-12.0, 0.0, 12.0] {
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
                        let mut settings = OttSettings {
                            enabled,
                            ..Default::default()
                        };
                        for (index, band) in settings.bands.iter_mut().enumerate() {
                            band.enabled = index != disabled_band;
                        }
                        let saved = settings.clone();
                        let mut bounds = None;
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::new(1000.0, 400.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                egui::CentralPanel::default().show(ui, |ui| {
                                    bounds = Some(
                                        show(ui, &mut settings, false, 244.0, [gain; 3], &[], None)
                                            .0,
                                    );
                                });
                            },
                        );
                        output.textures_delta.clear();
                        assert_eq!(settings, saved);
                        let palette = theme::palette(&ctx);
                        for (index, rail) in bounds.unwrap().meters.into_iter().enumerate() {
                            let fills: Vec<_> = output
                                .shapes
                                .iter()
                                .filter_map(|shape| match &shape.shape {
                                    egui::Shape::Rect(rect)
                                        if rail.contains_rect(rect.rect)
                                            && rect.rect.width() > 0.0
                                            && [
                                                theme::accent(&ctx),
                                                palette.meter,
                                                palette.secondary,
                                            ]
                                            .contains(&rect.fill) =>
                                    {
                                        Some(rect)
                                    }
                                    _ => None,
                                })
                                .collect();
                            if !enabled || gain == 0.0 {
                                assert!(fills.is_empty());
                            } else {
                                assert_eq!(fills.len(), 1);
                                let expected = if index == disabled_band {
                                    palette.secondary
                                } else if gain > 0.0 {
                                    palette.meter
                                } else {
                                    theme::accent(&ctx)
                                };
                                assert_eq!(fills[0].fill, expected);
                                assert!((fills[0].rect.width() - rail.width() * 0.25).abs() < 0.01);
                                assert_eq!(fills[0].rect.center().x < rail.center().x, gain < 0.0);
                            }
                        }
                    }
                }
            }
        }
    }
}
