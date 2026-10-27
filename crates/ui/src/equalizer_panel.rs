#[cfg(test)]
use crate::panels::UNIT_STRIP_WIDTH;
use crate::{knobs, meters, panels, theme};
use daw_core::{EqSettings, ModulationParameter as Parameter};
use egui::{Rect, Ui, Vec2};

pub(super) use daw_unit_ui::equalizer::Bounds;

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
    let levels = meters::unit_reading(settings.enabled, levels);
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
        sections: [Rect::NOTHING; 3],
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
    let open = settings.modulation_open;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        let shared = daw_unit_ui::equalizer::show(
            ui,
            settings,
            compact,
            crate::envelope_panel::body_height(compact),
            levels,
            markers,
        );
        bounds.knobs = shared.knobs;
        bounds.bands = shared.bands;
        bounds.meters = shared.meters;
        bounds.sections = shared.sections;
        let last_strip = bounds.sections[2];
        if open {
            panels::unit_divider(ui, last_strip.right(), last_strip.y_range().into());
            crate::modulation_panel::body(
                ui,
                &mut settings.modulations,
                sources,
                &Parameter::EQUALIZER,
                <EqSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE,
                compact,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_rails_follow_band_power_and_leave_layout_and_saved_settings_unchanged() {
        for color_theme in [
            crate::settings::ColorTheme::Dark,
            crate::settings::ColorTheme::Light,
        ] {
            for (enabled, compact) in [(true, false), (false, false), (true, true), (false, true)] {
                for collapsed in [false, true] {
                    for disabled_band in 0..=3 {
                        for peak in [0.0, 10_f32.powf(-30.0 / 20.0), 2.0] {
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
                                enabled,
                                collapsed,
                                ..Default::default()
                            };
                            for (index, band) in settings.bands.iter_mut().enumerate() {
                                band.enabled = index != disabled_band;
                            }
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
                                                [peak; 3],
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
                                        (rail.center().x - bounds.knobs[index][0].center().x).abs()
                                            < 0.01
                                    );
                                    let expected = rail.width()
                                        * if peak == 0.0 || !enabled {
                                            0.0
                                        } else if peak > 1.0 {
                                            1.0
                                        } else {
                                            0.5
                                        };
                                    let color = if !settings.bands[index].enabled {
                                        theme::palette(&ctx).secondary
                                    } else if peak > 1.0 {
                                        theme::palette(&ctx).error
                                    } else {
                                        theme::accent(&ctx)
                                    };
                                    let fills: Vec<_> = output
                                        .shapes
                                        .iter()
                                        .filter_map(|shape| match &shape.shape {
                                            egui::Shape::Rect(rect)
                                                if rect.rect.min == rail.min
                                                    && rect.fill == color =>
                                            {
                                                Some(rect.rect)
                                            }
                                            _ => None,
                                        })
                                        .collect();
                                    if peak == 0.0 || !enabled {
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
    }
}
