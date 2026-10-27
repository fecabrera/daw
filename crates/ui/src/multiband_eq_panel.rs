//! Variable-band EQ controls use the common unit shell, knobs, footers and routing strips.
use crate::{
    meters,
    modulation_panel::{self, Source, Target},
    panels, theme,
};
use daw_core::{Id, ModulationParameter as P, MultibandEqSettings};
use egui::{Rect, Ui, Vec2};

#[cfg(test)]
use daw_unit_ui::multiband_eq::CONTROL_SIZE;

pub(super) struct Bounds {
    #[cfg(test)]
    pub output_meter: Rect,
    pub header: Rect,
    pub power: Rect,
    pub bands: Vec<Rect>,
    pub knobs: Vec<[Rect; 4]>,
    pub band_power: Vec<Rect>,
    pub meters: Vec<Rect>,
    pub add: Rect,
    pub output: Rect,
}

pub(super) fn show(
    ui: &mut Ui,
    settings: &mut MultibandEqSettings,
    dimensions: (bool, f32),
    sources: &[Source],
    live: impl Fn(Option<Id>) -> Option<daw_engine::ModulationSnapshot>,
    levels: impl Fn(Id) -> f32,
    level: meters::OutputLevel<'_>,
) -> (Bounds, egui::Response) {
    let (compact, height) = dimensions;
    let mut bounds = Bounds {
        #[cfg(test)]
        output_meter: Rect::NOTHING,
        header: Rect::NOTHING,
        power: Rect::NOTHING,
        bands: Vec::new(),
        knobs: Vec::new(),
        band_power: Vec::new(),
        meters: Vec::new(),
        add: Rect::NOTHING,
        output: Rect::NOTHING,
    };
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Multiband EQ", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            body(ui, settings, compact, sources, &live, &levels, &mut bounds);
            if !compact {
                let rail = meters::effect_output_level(
                    ui,
                    bounds.output,
                    panels::PanelStyle::default().margin,
                    level,
                    settings.enabled,
                );
                #[cfg(test)]
                {
                    bounds.output_meter = rail;
                }
                #[cfg(not(test))]
                let _ = rail;
            }
        },
    );
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    modulation_panel::toggle(
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
    settings: &mut MultibandEqSettings,
    compact: bool,
    sources: &[Source],
    live: &impl Fn(Option<Id>) -> Option<daw_engine::ModulationSnapshot>,
    levels: &impl Fn(Id) -> f32,
    bounds: &mut Bounds,
) {
    let body_height = crate::envelope_panel::body_height(compact);
    let markers: Vec<_> = settings
        .bands
        .iter()
        .map(|band| {
            let routes: Vec<_> = settings
                .modulations
                .iter()
                .copied()
                .filter(|r| r.target == Some(band.id))
                .collect();
            let bases = [band.gain_db, band.frequency_hz, band.q];
            std::array::from_fn(|i| {
                modulation_panel::marker(
                    &routes,
                    sources,
                    P::EQ_BAND[i],
                    bases[i],
                    settings.enabled && band.enabled,
                    None,
                    [live(Some(band.id)).map(|v| v[i]), None],
                )
            })
        })
        .collect();
    let band_levels: Vec<_> = settings.bands.iter().map(|b| levels(b.id)).collect();
    let output_marker = modulation_panel::marker(
        &settings.modulations,
        sources,
        P::Level,
        settings.output_gain_db,
        settings.enabled,
        None,
        [live(None).map(|v| v[0]), None],
    );
    let mut remove = None;
    let mut add = false;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
        let shared = daw_unit_ui::multiband_eq::show(
            ui,
            settings,
            (compact, body_height),
            daw_unit_ui::multiband_eq::Controls {
                add_enabled: true,
                markers: &markers,
                levels: &band_levels,
                output_marker: output_marker.as_ref(),
                output_level: None,
            },
        );
        if let Some(index) = shared.remove {
            remove = Some(settings.bands[index].id);
        }
        if let Some(button) = shared.add {
            bounds.add = button.rect;
            add = button.clicked();
        }
        bounds.band_power = shared.power;
        bounds.meters = shared.meters;
        bounds.bands = shared.bands;
        bounds.knobs = shared.knobs;
        bounds.output = shared.output;
        let output = bounds.output;
        if settings.modulation_open {
            panels::unit_divider(ui, output.right(), output.y_range().into());
            let mut targets: Vec<_> = settings
                .bands
                .iter()
                .enumerate()
                .flat_map(|(i, b)| {
                    P::EQ_BAND.map(|parameter| Target {
                        parameter,
                        id: Some(b.id),
                        label: format!("Band {} · {}", i + 1, parameter.label()),
                    })
                })
                .collect();
            targets.push(Target {
                parameter: P::Level,
                id: None,
                label: "Output · Gain".into(),
            });
            modulation_panel::targeted_body(
                ui,
                &mut settings.modulations,
                sources,
                &targets,
                daw_unit_core::ModulationScope::Shared,
                compact,
            );
        }
    });
    if let Some(id) = remove {
        settings.remove_band(id);
    }
    if add {
        settings.bands.push(Default::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::UNIT_STRIP_WIDTH;
    fn frame(
        ctx: &egui::Context,
        s: &mut MultibandEqSettings,
        events: Vec<egui::Event>,
        compact: bool,
    ) -> Bounds {
        let mut result = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1200.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    result = Some(
                        show(
                            ui,
                            s,
                            (compact, panels::unit_height(compact)),
                            &[],
                            |_| None,
                            |_| 0.0,
                            0.0.into(),
                        )
                        .0,
                    );
                });
            },
        );
        output.textures_delta.clear();
        result.unwrap()
    }
    fn click(ctx: &egui::Context, s: &mut MultibandEqSettings, point: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                s,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                false,
            );
        }
    }
    #[test]
    fn multiband_controls_fit_standard_lane_and_add_bands_with_new_ids() {
        for compact in [false, true] {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            let mut s = MultibandEqSettings::default();
            let b = frame(&ctx, &mut s, vec![], compact);
            assert_eq!(b.bands.len(), 1);
            assert_eq!(b.bands[0].width(), UNIT_STRIP_WIDTH);
            if !compact {
                assert!(b.knobs[0].windows(2).all(|r| r[0].bottom() <= r[1].top()));
            }
            assert_eq!(b.output.width(), UNIT_STRIP_WIDTH);
            assert!((b.bands[0].bottom() - b.output.bottom()).abs() < 0.1);
            assert!(b.bands[0].right() <= b.add.left());
            assert!(b.add.right() <= b.output.left());
            assert_eq!(
                b.add.width(),
                icons::COMPACT_BUTTON_SIZE + 2.0 * f32::from(panels::PanelStyle::default().margin)
            );
            assert!((b.add.height() - b.bands[0].height()).abs() < 0.1);
            assert!(
                b.output.bottom() <= panels::unit_height(compact) + 10.0,
                "{:?}",
                b.output
            );
            if !compact {
                for rect in b.knobs[0] {
                    assert!(b.bands[0].contains_rect(rect));
                    assert_eq!(rect.width(), CONTROL_SIZE);
                    assert!((rect.center().x - b.bands[0].center().x).abs() < 0.1);
                }
                let id = s.bands[0].id;
                click(&ctx, &mut s, b.band_power[0].center());
                assert!(!s.bands[0].enabled);
                click(&ctx, &mut s, b.add.center());
                assert_eq!(s.bands.len(), 2);
                assert_ne!(s.bands[1].id, id);
            }
        }
    }
    #[test]
    fn multiband_mode_diagrams_are_finite_and_distinct() {
        for i in 0..6 {
            let path = diagram(i as f32);
            assert_eq!(path.len(), 25);
            assert!(path.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
            for j in 0..i {
                assert_ne!(path, diagram(j as f32));
            }
        }
    }
}

#[cfg(test)]
mod meter_tests {
    use super::*;
    #[test]
    fn every_expanded_band_uses_the_shared_rail_without_moving_controls_and_unit_off_masks_stale_values()
     {
        for compact in [false, true] {
            for enabled in [false, true] {
                let ctx = egui::Context::default();
                theme::configure(&ctx);
                crate::fonts::configure(&ctx);
                let mut s = MultibandEqSettings {
                    enabled,
                    ..Default::default()
                };
                s.bands.push(Default::default());
                s.bands[1].enabled = false;
                let mut bounds = None;
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        bounds = Some(
                            show(
                                ui,
                                &mut s,
                                (compact, panels::unit_height(compact)),
                                &[],
                                |_| None,
                                |_| 2.0,
                                0.0.into(),
                            )
                            .0,
                        );
                    });
                });
                let bounds = bounds.unwrap();
                assert_eq!(bounds.meters.len(), if compact { 0 } else { 2 });
                for (i, rail) in bounds.meters.iter().enumerate() {
                    assert_eq!(rail.height(), 4.0);
                    assert_eq!(
                        *rail,
                        crate::meters::unit_monitor_rail(
                            bounds.bands[i],
                            panels::PanelStyle::default().margin
                        )
                    );
                    assert!(rail.bottom() < bounds.knobs[i][0].top());
                    let expected = if !enabled {
                        theme::palette(&ctx).input
                    } else if i == 1 {
                        theme::palette(&ctx).secondary
                    } else {
                        theme::palette(&ctx).error
                    };
                    assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(rect) if rect.rect==*rail && rect.fill==expected)));
                }
                output.textures_delta.clear();
            }
        }
    }
}

#[cfg(test)]
use daw_unit_ui::multiband_eq::diagram;

#[cfg(test)]
use crate::icons;
