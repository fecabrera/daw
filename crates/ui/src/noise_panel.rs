//! Noise source controls. Generation and modulation stay in portable DSP.
use super::*;
use daw_core::{
    InstrumentComponent, InstrumentLayerSettings, ModulationParameter as P, NoiseSettings,
    NoiseType,
};
use egui::Ui;

#[cfg(test)]
#[derive(Clone)]
pub(super) struct Bounds {
    pub panel: Rect,
    pub header: Rect,
    pub power: Rect,
    pub controls: [Rect; 4],
    pub spectrum: Rect,
    pub strips: [Rect; 2],
}
impl DawUi {
    pub(super) fn noise_unit(
        &mut self,
        ui: &mut Ui,
        track: Id,
        layer: &mut InstrumentLayerSettings,
        sources: &[modulation_panel::Source],
        height: f32,
        compact: bool,
    ) -> Rect {
        let layer_id = layer.id;
        let noise = layer.noise.as_mut().unwrap();
        let mut enabled = noise.enabled;
        let mut collapsed = noise.collapsed;
        let open = noise.modulation_open;
        let live = self.output.as_ref().and_then(|o| {
            o.modulation_snapshot(daw_engine::ModulationTarget::Noise {
                track,
                layer: layer_id,
            })
        });
        let mut controls = [Rect::NOTHING; 4];
        let mut strips = [Rect::NOTHING; 2];
        let mut spectrum_rect = Rect::NOTHING;
        let result = panels::PanelStyle {
            margin: 0,
            ..Default::default()
        }
        .unit(
            ui,
            ("Noise", compact),
            height,
            &mut enabled,
            &mut collapsed,
            |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing =
                        Vec2::new(0.0, f32::from(theme::TOOLBAR_PADDING));
                    for (column, strip_rect) in strips.iter_mut().enumerate() {
                        let strip = panels::unit_strip(
                            ui,
                            envelope_panel::body_height(compact),
                            egui::UiBuilder::new()
                                .id_salt(("noise-strip", track, layer_id, column))
                                .sense(Sense::click_and_drag()),
                            |ui| {
                                ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                                if !compact {
                                    for row in 0..3 {
                                        let parameter = match (column, row) {
                                            (0, 1) => Some(P::NoiseColor),
                                            (1, 0) => Some(P::NoiseLevel),
                                            (1, 1) => Some(P::NoiseWidth),
                                            _ => None,
                                        };
                                        if let Some(parameter) = parameter {
                                            let (index, value, position) = match parameter {
                                                P::NoiseColor => (0, &mut noise.color, 1),
                                                P::NoiseWidth => (1, &mut noise.width, 3),
                                                _ => (2, &mut noise.level_db, 2),
                                            };
                                            let marker = modulation_panel::marker(
                                                &noise.modulations,
                                                sources,
                                                parameter,
                                                *value,
                                                noise.enabled,
                                                None,
                                                [
                                                    live.map(|v| v[index * 2]),
                                                    live.map(|v| v[index * 2 + 1]),
                                                ],
                                            );
                                            controls[position] = knobs::row(
                                                ui,
                                                value,
                                                knobs::CHOICE_SIZE,
                                                |ui, value| {
                                                    ui.add(
                                                        knobs::Knob::new(
                                                            value,
                                                            parameter_knobs::destination(parameter),
                                                        )
                                                        .modulated(
                                                            marker.as_ref(),
                                                            &mut noise.modulation_open,
                                                        ),
                                                    )
                                                },
                                            )
                                            .rect;
                                        } else if column == 0 && row == 0 {
                                            controls[0] =
                                                daw_unit_ui::rows::selector_row(ui, "Type", |ui| {
                                                    let kind = &mut noise.kind;
                                                    let before = *kind;
                                                    let mut response =
                                                        crate::menus::Dropdown::from_id_salt(
                                                            "noise-type",
                                                        )
                                                        .small_items()
                                                        .selected_text(
                                                            egui::RichText::new(kind.label())
                                                                .small(),
                                                        )
                                                        .width(65.0)
                                                        .show_ui(ui, |ui| {
                                                            for choice in NoiseType::ALL {
                                                                if ui
                                                                    .selectable_value(
                                                                        kind,
                                                                        choice,
                                                                        choice.label(),
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    ui.close();
                                                                }
                                                            }
                                                        })
                                                        .response
                                                        .on_hover_text("Noise type");
                                                    if before != *kind {
                                                        response.mark_changed();
                                                    }
                                                    response
                                                })
                                                .rect;
                                        } else {
                                            let rect = knobs::row(
                                                ui,
                                                &mut (),
                                                knobs::CHOICE_SIZE,
                                                |ui, _| {
                                                    ui.allocate_response(
                                                        Vec2::splat(knobs::CHOICE_SIZE),
                                                        Sense::hover(),
                                                    )
                                                },
                                            )
                                            .rect;
                                            if column == 0 {
                                                spectrum_rect = rect;
                                                spectrum(ui, rect, noise);
                                            }
                                        }
                                    }
                                }
                                rows::centered(
                                    ui,
                                    if compact {
                                        envelope_panel::body_height(true)
                                    } else {
                                        icons::COMPACT_BUTTON_SIZE
                                    },
                                    |ui| {
                                        rows::unit_label(
                                            ui,
                                            if column == 0 { "Noise" } else { "Out" },
                                        )
                                    },
                                );
                            },
                        );
                        *strip_rect = strip.response.rect;
                        let meter = meters::unit_monitor_rail(
                            strip.response.rect,
                            panels::PanelStyle::default().margin,
                        );
                        self.start_installed_component_drag(
                            ui,
                            &strip.inner.response,
                            &[
                                controls[0],
                                controls[1],
                                controls[2],
                                controls[3],
                                spectrum_rect,
                                meter,
                            ],
                            track,
                            layer_id,
                            InstrumentComponent::Noise,
                        );
                    }
                    if open {
                        modulation_panel::body(
                            ui,
                            &mut noise.modulations,
                            sources,
                            &P::NOISE,
                            daw_unit_core::ModulationScope::PerNote,
                            compact,
                        );
                    }
                });
            },
        );
        noise.enabled = enabled;
        noise.collapsed = collapsed;
        if !compact && strips[1].is_positive() {
            let target = daw_engine::SourceOutputTarget {
                track,
                layer: layer_id,
                unit: noise.id,
            };
            let reading = meters::OutputLevel::new(
                self.output
                    .as_ref()
                    .map_or(0.0, |o| o.source_output_level(target)),
                self.output
                    .as_ref()
                    .and_then(|o| o.source_output_peak(target)),
            );
            meters::effect_output_level(
                ui,
                strips[1],
                panels::PanelStyle::default().margin,
                reading,
                enabled,
            );
        }
        let toggle = modulation_panel::toggle(ui, &result, collapsed, &mut noise.modulation_open);
        self.start_installed_component_drag(
            ui,
            &result.header,
            &[result.power.rect, toggle],
            track,
            layer_id,
            InstrumentComponent::Noise,
        );
        let mut remove = false;
        let power = crate::unit_power::PowerMenu::from_ui(ui);
        crate::menus::ContextMenu::new(&result.header).show(|ui| {
            if let Some(power) = &power {
                power.show(ui);
            }
            if ui.button("Remove noise").clicked() {
                remove = true;
                ui.close();
            }
        });
        if power.as_ref().is_some_and(|p| p.action().is_some()) {
            crate::unit_power::schedule_focus(ui, &result.header);
        }
        #[cfg(test)]
        self.instrument_ui.noises.push(Bounds {
            panel: result.response.rect,
            header: result.header.rect,
            power: result.power.rect,
            controls,
            spectrum: spectrum_rect,
            strips,
        });
        if remove {
            layer.noise = None;
            layer.prune_component_order();
        }
        result.response.rect
    }
}
fn spectrum(ui: &mut Ui, rect: Rect, noise: &NoiseSettings) {
    let graph = rect.shrink2(Vec2::new(0.0, 5.0));
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter_at(graph);
    painter.rect_filled(graph, 2.0, palette.background);
    // Relative spectral balance, anchored at 1 kHz. This is not live analysis.
    let slope = match noise.kind {
        NoiseType::White => 0.0,
        NoiseType::Pink => -3.0,
        NoiseType::Brown => -6.0,
    };
    let points = (0..=64)
        .map(|i| {
            let x = i as f32 / 64.0;
            let hz = 20.0 * 1000.0_f32.powf(x);
            let db = slope * (hz / 1000.0).log2()
                + noise.color * 6.0 * ((hz / 1000.0).log2() / 3.0).tanh();
            egui::pos2(
                graph.left() + x * graph.width(),
                graph.center().y - db.clamp(-36.0, 36.0) / 72.0 * graph.height(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(
        points,
        Stroke::new(
            1.0,
            if noise.enabled {
                theme::accent(ui.ctx())
            } else {
                palette.secondary
            },
        ),
    ));
    ui.interact(rect, ui.id().with("noise-spectrum"), Sense::hover()).on_hover_text("Approximate spectral balance · 20 Hz–20 kHz · fixed relative scale. Type and base Color only; excludes Level, modulation and effects.");
}
