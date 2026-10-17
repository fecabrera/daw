use crate::{
    icons,
    knobs::{self, Knob},
    meters,
    midi_editor::UNIT_STRIP_WIDTH,
    panels, parameter_knobs, rows, theme,
};
use daw_core::{EqSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) struct Bounds {
    pub header: Rect,
    pub power: Rect,
    pub knobs: [[Rect; 3]; 3],
    pub bands: [Rect; 3],
    pub meters: [Rect; 3],
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut EqSettings,
    compact: bool,
    height: f32,
    levels: [f32; 3],
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .ui_stack_info(egui::UiStackInfo::default().with_tag("unit-modulation-button")),
        |ui| show_inner(ui, settings, compact, height, levels, sources, live),
    )
    .inner
}
fn show_inner(
    ui: &mut Ui,
    settings: &mut EqSettings,
    compact: bool,
    height: f32,
    levels: [f32; 3],
    sources: &[crate::modulation_panel::Source],
    live: Option<daw_engine::ModulationSnapshot>,
) -> (Bounds, egui::Response) {
    let bases = [
        settings.bands[0].gain_db,
        settings.bands[0].frequency_hz,
        settings.bands[0].q,
        settings.bands[1].gain_db,
        settings.bands[2].gain_db,
        settings.bands[2].frequency_hz,
        settings.bands[2].q,
    ];
    let markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            Parameter::EQUALIZER[i],
            bases[i],
            settings.enabled,
            None,
            [live.map(|v| v[i]), None],
        )
    });
    let panel = panels::PanelStyle::default();
    let mut bounds = Bounds {
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        knobs: [[Rect::NOTHING; 3]; 3],
        bands: [Rect::NOTHING; 3],
        meters: [Rect::NOTHING; 3],
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle { margin: 0, ..panel }.unit_in_lane(
        ui,
        ("3-band EQ", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            body(
                ui,
                settings,
                compact,
                levels,
                &mut bounds,
                &markers,
                sources,
            )
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(
        ui,
        &result.header,
        settings.collapsed,
        &mut settings.modulation_open,
    );
    bounds.header = result.header.rect;
    bounds.power = result.power.rect;
    (bounds, result.header)
}

fn body(
    ui: &mut Ui,
    settings: &mut EqSettings,
    compact: bool,
    levels: [f32; 3],
    bounds: &mut Bounds,
    markers: &[Option<knobs::ModulationMarker>; 7],
    sources: &[crate::modulation_panel::Source],
) {
    let margin = panels::PanelStyle::default().margin;
    let open = settings.modulation_open;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        let mut last_strip = Rect::NOTHING;
        for (index, name) in ["Low", "Mid", "High"].into_iter().enumerate() {
            let strip = ui.push_id(index, |ui| {
                egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(UNIT_STRIP_WIDTH - 2.0 * f32::from(margin));
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        let band = &mut settings.bands[index];
                        if !compact { bounds.knobs[index] = controls(ui, band, index, markers, &mut settings.modulation_open); }
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
            last_strip = strip;
            if !compact {
                let rail = meters::unit_monitor_rail(strip, margin);
                bounds.meters[index] = rail;
                meters::band_level_rail(ui, rail, name, levels[index]);
            }
            if index > 0 { panels::unit_divider(ui, strip.left(), strip.y_range().into()); }
        }
        if open {
            panels::unit_divider(ui, last_strip.right(), last_strip.y_range().into());
            crate::modulation_panel::body(ui, &mut settings.modulations, sources, &Parameter::EQUALIZER, <EqSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE, compact);
        }
    });
}
fn controls(
    ui: &mut Ui,
    band: &mut daw_core::EqBandSettings,
    index: usize,
    markers: &[Option<knobs::ModulationMarker>; 7],
    open: &mut bool,
) -> [Rect; 3] {
    let first = [0, 3, 4][index];
    let mut bounds = [Rect::NOTHING; 3];
    for (row, rect) in bounds.iter_mut().enumerate() {
        if index == 1 && row > 0 {
            // Mid retains the shared row/footer geometry without legacy frequency/Q controls.
            knobs::row(ui, &mut (), knobs::CHOICE_SIZE, |ui, _| {
                ui.allocate_response(Vec2::splat(knobs::CHOICE_SIZE), egui::Sense::hover())
            });
            continue;
        }
        let value = match row {
            0 => &mut band.gain_db,
            1 => &mut band.frequency_hz,
            _ => &mut band.q,
        };
        let target = first + row;
        *rect = knobs::row(ui, value, knobs::CHOICE_SIZE, |ui, v| {
            ui.add(
                Knob::new(
                    v,
                    parameter_knobs::destination(Parameter::EQUALIZER[target]),
                )
                .modulated(markers[target].as_ref(), open),
            )
        })
        .rect;
    }
    bounds
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
                                        &[],
                                        None,
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
