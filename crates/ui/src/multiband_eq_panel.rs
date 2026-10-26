//! Variable-band EQ controls use the common unit shell, knobs, footers and routing strips.
use crate::{
    icons,
    knobs::{self, Knob, KnobSpec},
    meters,
    modulation_panel::{self, Source, Target},
    panels, parameter_knobs, rows, theme,
};
use daw_core::{EqBandMode, Id, ModulationParameter as P, MultibandEqSettings};
use egui::{Rect, Ui, Vec2};

const CONTROL_SIZE: f32 = 42.0;
const CONTROL_SCALE: f32 = CONTROL_SIZE / knobs::CHOICE_SIZE;
const CONTROL_GAP: f32 = 3.0;

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
fn diagram(value: f32) -> Vec<Vec2> {
    let mode = EqBandMode::ALL[value.round() as usize];
    (0..=24)
        .map(|i| {
            let x = i as f32 / 24.0;
            let low = 1.0 / (1.0 + ((x - 0.5) * 12.0).exp());
            let peak = (-((x - 0.5) * 5.0).powi(2)).exp();
            let y = match mode {
                EqBandMode::Bell => 0.5 + 0.5 * peak,
                EqBandMode::LowShelf => 0.25 + 0.5 * low,
                EqBandMode::HighShelf => 0.75 - 0.5 * low,
                EqBandMode::LowPass => low,
                EqBandMode::HighPass => 1.0 - low,
                EqBandMode::Notch => 1.0 - peak,
            };
            Vec2::new(x - 0.5, 0.5 - y)
        })
        .collect()
}
fn remove_menu(response: &egui::Response, remove: &mut Option<Id>, id: Id) {
    response.context_menu(|ui| {
        if ui.button("Remove band").clicked() {
            *remove = Some(id);
            ui.close();
        }
    });
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
    let mut remove = None;
    let add = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
            for (index, band) in settings.bands.iter_mut().enumerate() {
                let id = band.id;
                let target_routes: Vec<_> = settings
                    .modulations
                    .iter()
                    .copied()
                    .filter(|r| r.target == Some(id))
                    .collect();
                let bases = [band.gain_db, band.frequency_hz, band.q];
                let markers: [_; 3] = std::array::from_fn(|i| {
                    modulation_panel::marker(
                        &target_routes,
                        sources,
                        P::EQ_BAND[i],
                        bases[i],
                        settings.enabled && band.enabled,
                        None,
                        [live(Some(id)).map(|v| v[i]), None],
                    )
                });
                let mut knobs = [Rect::NOTHING; 4];
                let strip = ui
                    .push_id(id, |ui| {
                        panels::unit_strip(ui, body_height, egui::UiBuilder::new(), |ui| {
                            ui.spacing_mut().item_spacing.y = CONTROL_GAP;
                            if !compact {
                                knobs = band_controls(
                                    ui,
                                    band,
                                    &markers,
                                    &mut settings.modulation_open,
                                    &mut remove,
                                );
                            }
                            rows::centered(
                                ui,
                                if compact {
                                    body_height
                                } else {
                                    icons::COMPACT_BUTTON_SIZE
                                },
                                |ui| {
                                    let response = rows::power_label(
                                        ui,
                                        &format!("Band {}", index + 1),
                                        &format!("Toggle EQ band {}", index + 1),
                                        band.enabled,
                                    );
                                    let response = response.on_hover_text(
                                        "Off bypasses this band. Right-click to remove it.",
                                    );
                                    if response.clicked() {
                                        band.enabled = !band.enabled;
                                    }
                                    bounds.band_power.push(response.rect);
                                    remove_menu(&response, &mut remove, id);
                                },
                            );
                        })
                        .response
                        .rect
                    })
                    .inner;
                if index > 0 {
                    panels::unit_divider(ui, strip.left(), strip.y_range().into());
                }
                if !compact {
                    let rail = crate::meters::unit_monitor_rail(
                        strip,
                        panels::PanelStyle::default().margin,
                    );
                    let level = crate::meters::unit_reading(settings.enabled, [levels(id)])[0];
                    crate::meters::band_level_rail(
                        ui,
                        rail,
                        &format!("Band {}", index + 1),
                        level,
                        band.enabled,
                    );
                    bounds.meters.push(rail);
                }
                bounds.bands.push(strip);
                bounds.knobs.push(knobs);
            }
            let add_control = panels::unit_add_strip(ui, body_height, "Add EQ band");
            bounds.add = add_control.inner.rect;
            let add = add_control.inner.clicked();
            let add_strip = add_control.response.rect;
            if !settings.bands.is_empty() {
                panels::unit_divider(ui, add_strip.left(), add_strip.y_range().into());
            }
            let output = panels::unit_strip(ui, body_height, egui::UiBuilder::new(), |ui| {
                if !compact {
                    let marker = modulation_panel::marker(
                        &settings.modulations,
                        sources,
                        P::Level,
                        settings.output_gain_db,
                        settings.enabled,
                        None,
                        [live(None).map(|v| v[0]), None],
                    );
                    for row in 0..3 {
                        knobs::row(
                            ui,
                            &mut settings.output_gain_db,
                            knobs::CHOICE_SIZE,
                            |ui, v| {
                                if row == 0 {
                                    ui.add(
                                        Knob::new(v, parameter_knobs::destination(P::Level))
                                            .modulated(
                                                marker.as_ref(),
                                                &mut settings.modulation_open,
                                            ),
                                    )
                                } else {
                                    ui.allocate_response(
                                        Vec2::splat(knobs::CHOICE_SIZE),
                                        egui::Sense::hover(),
                                    )
                                }
                            },
                        );
                    }
                }
                rows::centered(
                    ui,
                    if compact {
                        body_height
                    } else {
                        icons::COMPACT_BUTTON_SIZE
                    },
                    |ui| {
                        rows::unit_label(ui, "Out");
                    },
                );
            })
            .response
            .rect;
            bounds.output = output;
            panels::unit_divider(ui, output.left(), output.y_range().into());
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
            add
        })
        .inner;

    if let Some(id) = remove {
        settings.remove_band(id);
    }
    if add {
        settings.bands.push(Default::default());
    }
}

fn band_controls(
    ui: &mut Ui,
    band: &mut daw_core::MultibandEqBand,
    markers: &[Option<knobs::ModulationMarker>; 3],
    open: &mut bool,
    remove: &mut Option<Id>,
) -> [Rect; 4] {
    std::array::from_fn(|row| {
        let response = knobs::row(ui, band, CONTROL_SIZE, |ui, band| {
            if row == 0 {
                return crate::lfo_controls::choice(
                    ui,
                    &mut band.mode,
                    &EqBandMode::ALL,
                    KnobSpec::new("Band mode", 0.0..=5.0, 0.0, 1.0)
                        .stepped(1.0)
                        .cyclic()
                        .diagrams(diagram)
                        .display_scale(CONTROL_SCALE)
                        .format(|v| EqBandMode::ALL[v.round() as usize].label().into()),
                );
            }
            let i = match row {
                1 => 1,
                2 => 0,
                _ => 2,
            };
            let enabled = i != 0 || band.mode.uses_gain();
            let value = match i {
                0 => &mut band.gain_db,
                1 => &mut band.frequency_hz,
                _ => &mut band.q,
            };
            ui.add_enabled_ui(enabled, |ui| {
                ui.add(
                    Knob::new(
                        value,
                        parameter_knobs::destination(P::EQ_BAND[i]).display_scale(CONTROL_SCALE),
                    )
                    .modulated(markers[i].as_ref(), open),
                )
            })
            .inner
        });
        remove_menu(&response, remove, band.id);
        response.rect
    })
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
