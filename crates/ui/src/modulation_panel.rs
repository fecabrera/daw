//! Shared destination-owned modulation strips and header expansion control.
use crate::{
    icons,
    knobs::{self, Knob},
    panels, rows, theme,
};
use daw_core::{InstrumentSettings, Modulation, ModulationParameter, ModulationSourceId};
use daw_unit_core::{ModulationRange, ModulationScope};
use egui::{Rect, Ui, Vec2};
pub(super) const SELECTOR_WIDTH: f32 = 65.0;
#[derive(Clone)]
pub(super) struct Source {
    pub id: ModulationSourceId,
    pub label: String,
    pub scope: ModulationScope,
    pub range: ModulationRange,
    pub behavior: daw_unit_core::ModulationBehavior,
    pub enabled: bool,
}
pub(super) fn sources(instrument: Option<&InstrumentSettings>) -> Vec<Source> {
    instrument
        .into_iter()
        .flat_map(|i| i.modulation_sources())
        .filter(|source| source.present)
        .map(|source| Source {
            id: source.id,
            label: format!("{} · {}", source.layer_name, source.display_name()),
            scope: source.scope,
            range: source.output.range,
            behavior: source.output.behavior,
            enabled: source.enabled,
        })
        .collect()
}
/// Filter through the shared graph rules before opening a provider's source picker.
pub(super) fn provider_sources(
    instrument: &InstrumentSettings,
) -> Vec<(ModulationSourceId, Vec<Source>)> {
    let all = sources(Some(instrument));
    let nodes: Vec<_> = instrument.provider_destinations().collect();
    nodes
        .iter()
        .map(|(destination, _)| {
            (
                *destination,
                all.iter()
                    .filter(|source| {
                        !daw_unit_core::modulation_would_cycle(&nodes, *destination, source.id)
                    })
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}
pub(super) fn toggle(
    ui: &mut Ui,
    header: &egui::Response,
    collapsed: bool,
    open: &mut bool,
) -> Rect {
    if collapsed {
        return Rect::NOTHING;
    }
    let rect = Rect::from_min_max(
        egui::pos2(header.rect.right() - 26.0, header.rect.top() + 1.0),
        egui::pos2(
            header.rect.right() - 1.0,
            header.rect.top() + panels::HEADER_HEIGHT - 1.0,
        ),
    );
    let hit_size = Vec2::splat(panels::HEADER_HEIGHT - 2.0);
    let hit_rect = Rect::from_center_size(rect.center(), hit_size);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(hit_rect)
            .id_salt("modulation-switch"),
    );
    let response = icons::dot_toggle(
        &mut child,
        if *open {
            "Collapse modulation controls"
        } else {
            "Expand modulation controls"
        },
        *open,
        hit_size,
        panels::UNIT_POWER_SIZE,
        theme::palette(ui.ctx()).secondary,
    );
    if ui.is_enabled() && ui.input(|input| input.focused) && response.clicked() {
        *open = !*open;
        ui.ctx().request_repaint();
    }
    rect
}
/// One parameter choice, optionally scoped to a stable repeated destination.
pub(super) struct Target {
    pub parameter: ModulationParameter,
    pub id: Option<daw_core::Id>,
    pub label: String,
}
pub(super) fn body(
    ui: &mut Ui,
    routes: &mut Vec<Modulation>,
    sources: &[Source],
    parameters: &[ModulationParameter],
    scope: ModulationScope,
    compact: bool,
) {
    let targets: Vec<_> = parameters
        .iter()
        .map(|p| Target {
            parameter: *p,
            id: None,
            label: p.label().into(),
        })
        .collect();
    targeted_body(ui, routes, sources, &targets, scope, compact);
}
pub(super) fn targeted_body(
    ui: &mut Ui,
    routes: &mut Vec<Modulation>,
    sources: &[Source],
    targets: &[Target],
    scope: ModulationScope,
    compact: bool,
) {
    targeted_body_with_sources(ui, routes, targets, scope, compact, |_| sources);
}
pub(super) fn targeted_body_with_sources<'a>(
    ui: &mut Ui,
    routes: &mut Vec<Modulation>,
    targets: &[Target],
    scope: ModulationScope,
    compact: bool,
    sources_for_target: impl Fn(Option<daw_core::Id>) -> &'a [Source],
) {
    if targets.is_empty() {
        return;
    }
    let mut remove = None;
    let mut add = false;
    let owner = crate::strip_reorder::owner(ui);
    let reordering = crate::strip_reorder::active(ui, owner);
    let original = routes.clone();
    let mut slots = Vec::new();
    let body = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for (index, route) in routes.iter_mut().enumerate() {
            let strip = ui
                .scope_builder(egui::UiBuilder::new().id_salt(route.id).sense(egui::Sense::click_and_drag()), |ui| {
                    ui.add_enabled_ui(!reordering, |ui| egui::Frame::new().inner_margin(10).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING);
                        ui.set_width(crate::midi_editor::UNIT_STRIP_WIDTH - 20.0);
                        ui.set_min_height(crate::envelope_panel::body_height(compact));
                        if !compact {
                            let sources = sources_for_target(route.target);
                            let selected = route
                                .source
                                .and_then(|id| sources.iter().find(|s| s.id == id))
                                .map_or(
                                    if route.source.is_some() {
                                        "Unavailable"
                                    } else {
                                        "None"
                                    },
                                    |s| s.label.as_str(),
                                );
                            selector_row(ui, "Source", |ui| {
                                egui::ComboBox::from_id_salt("source")
                                    .selected_text(egui::RichText::new(selected).size(panels::HEADER_FONT_SIZE))
                                    .width(SELECTOR_WIDTH)
                                    .truncate()
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut route.source, None, "None");
                                        for source in sources.iter().filter(|s| scope.accepts(s.scope)) {
                                            ui.selectable_value(
                                                &mut route.source,
                                                Some(source.id),
                                                &source.label,
                                            );
                                        }
                                    })
                                    .response
                                    .on_hover_text(if scope==ModulationScope::PerNote {"Modulation source"}else{"Shared modulation source. Per-note sources cannot feed this unit."})
                            });
                            selector_row(ui, "Destination", |ui| {
                                egui::ComboBox::from_id_salt("parameter")
                                    .selected_text(egui::RichText::new(targets.iter().find(|t| t.parameter == route.parameter && t.id == route.target).map_or("Unavailable", |t| t.label.as_str())).size(panels::HEADER_FONT_SIZE))
                                    .width(SELECTOR_WIDTH)
                                    .truncate()
                                    .show_ui(ui, |ui| {
                                        for target in targets {
                                            if ui.selectable_label(route.parameter == target.parameter && route.target == target.id, &target.label).clicked() {
                                                let target_changed = route.target != target.id;
                                                route.parameter = target.parameter;
                                                route.target = target.id;
                                                if target_changed && scope == ModulationScope::Shared && route.source.is_some_and(|source| !sources_for_target(target.id).iter().any(|s| s.id == source && scope.accepts(s.scope))) {
                                                    route.source = None;
                                                }
                                                route.amount = route.amount.clamp(-target.parameter.limit(), target.parameter.limit());
                                            }
                                        }
                                    })
                                    .response
                                    .on_hover_text("Destination parameter")
                            });
                            let spec = crate::parameter_knobs::amount(route.parameter);
                            knobs::row(ui, &mut route.amount, knobs::CHOICE_SIZE, |ui, value| {
                                ui.add(Knob::new(value, spec))
                            });
                        }
                        rows::centered(
                            ui,
                            if compact {
                                crate::envelope_panel::body_height(true)
                            } else {
                                icons::COMPACT_BUTTON_SIZE
                            },
                            |ui| {
                                if rows::power_label(
                                    ui,
                                    &format!("Mod {}", index + 1),
                                    "Toggle modulation connection",
                                    route.enabled,
                                )
                                .clicked()
                                {
                                    route.enabled = !route.enabled;
                                }
                            },
                        );
                        });
                    }))
                });
            crate::strip_reorder::arm(ui, owner, &strip.response, route.id, &original);
            slots.push((route.id, strip.response.rect));
            strip.response.context_menu(|ui| {
                if ui.add_enabled(!reordering, egui::Button::new("Remove modulation")).clicked() {
                    remove = Some(index);
                    ui.close();
                }
            });
            if index > 0 {
                panels::unit_divider(
                    ui,
                    strip.response.rect.left(),
                    strip.response.rect.y_range().into(),
                );
            }
        }
        let add_strip = ui.add_enabled_ui(!reordering, |ui| icons::circular_strip_button(
            ui,
            icons::Lucide::Plus,
            "Add modulation",
            Vec2::new(
                icons::COMPACT_BUTTON_SIZE + 20.0,
                crate::envelope_panel::body_height(compact) + 20.0,
            ),
            icons::COMPACT_BUTTON_SIZE,
        )).inner;
        if !routes.is_empty() {
            panels::unit_divider(ui, add_strip.rect.left(), add_strip.rect.y_range().into());
        }
        add = add_strip.clicked();
    });
    crate::strip_reorder::finish(
        ui,
        owner,
        routes,
        &slots,
        body.response.rect.intersect(ui.clip_rect()),
    );
    if let Some(index) = remove {
        routes.remove(index);
    }
    if add {
        let mut route = Modulation::new(targets[0].parameter);
        route.target = targets[0].id;
        routes.push(route);
    }
}

/// Keep the label and dropdown inside the same standard control row.
pub(super) fn selector_row(
    ui: &mut Ui,
    label: &str,
    selector: impl FnOnce(&mut Ui) -> egui::Response,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), knobs::CHOICE_SIZE),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_min_height(knobs::CHOICE_SIZE);
            ui.spacing_mut().item_spacing.y = f32::from(theme::TOOLBAR_PADDING) * 0.5;
            ui.label(
                egui::RichText::new(label)
                    .size(panels::HEADER_FONT_SIZE)
                    .color(theme::palette(ui.ctx()).secondary),
            );
            selector(ui);
        },
    );
}

/// Apply destination-owned limits to both the possible-range arc and its tooltip.
pub(super) fn bound_marker(
    marker: &mut knobs::ModulationMarker,
    bounds: [f32; 2],
    format: impl Fn(f32) -> String,
) {
    marker.range = marker.range.map(|v| v.clamp(bounds[0], bounds[1]));
    marker.tooltip = marker
        .tooltip
        .lines()
        .map(|line| {
            if line.starts_with("Range: ") {
                format!(
                    "Range: {} to {}",
                    format(marker.range[0]),
                    format(marker.range[1])
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
}

/// Summarize assigned routes without evaluating or advancing an envelope in the UI.
pub(super) fn marker(
    routes: &[Modulation],
    sources: &[Source],
    parameter: ModulationParameter,
    base: f32,
    enabled: bool,
    lfo: Option<(bool, f32)>,
    live: [Option<f32>; 2],
) -> Option<knobs::ModulationMarker> {
    let mut assigned = false;
    let mut active = false;
    let mut offsets = [0.0_f32; 2];
    let mut amplitude = [[0.0_f32; 2]; 3];
    let mut grouped: Vec<(
        ModulationSourceId,
        ModulationScope,
        ModulationRange,
        daw_unit_core::ModulationBehavior,
        f32,
    )> = Vec::new();
    let mut lines = vec![format!("{} modulation", parameter.label())];
    for route in routes.iter().filter(|r| r.parameter == parameter) {
        assigned = true;
        let source = route
            .source
            .and_then(|id| sources.iter().find(|s| s.id == id));
        let reason = if !enabled {
            "unit off"
        } else if !route.enabled {
            "connection off"
        } else if route.amount == 0.0 {
            "zero amount"
        } else if source.is_none() {
            "source unavailable"
        } else if !source.unwrap().enabled {
            "source off"
        } else if !matches!(
            parameter,
            ModulationParameter::Amplitude | ModulationParameter::Pitch
        ) && source.unwrap().scope == ModulationScope::PerNote
        {
            "requires shared source"
        } else {
            "active"
        };
        lines.push(format!(
            "{}: {:+.2} {} ({reason})",
            source.map_or("None / unavailable", |s| s.label.as_str()),
            route.amount,
            parameter.unit()
        ));
        if reason != "active" {
            continue;
        }
        active = true;
        let source = source.unwrap();
        if let Some((_, _, _, _, amount)) =
            grouped.iter_mut().find(|(id, _, _, _, _)| *id == source.id)
        {
            *amount += route.amount;
        } else {
            grouped.push((
                source.id,
                source.scope,
                source.range,
                source.behavior,
                route.amount,
            ));
        }
    }
    // Connections to the same source are correlated: opposite amounts cancel.
    for (_, scope, range, behavior, amount) in grouped {
        let [min, max] = range.bounds();
        if parameter == ModulationParameter::Amplitude {
            let continuous = behavior == daw_unit_core::ModulationBehavior::Continuous;
            let group = if continuous {
                2
            } else {
                usize::from(scope != ModulationScope::PerNote)
            };
            let (min, max) = if continuous { (0.0, 1.0) } else { (min, max) };
            let deltas = [(min - 1.0) * amount, (max - 1.0) * amount];
            amplitude[group][0] += deltas[0].min(deltas[1]);
            amplitude[group][1] += deltas[0].max(deltas[1]);
        } else {
            let deltas = [min * amount, max * amount];
            offsets[0] += deltas[0].min(deltas[1]);
            offsets[1] += deltas[0].max(deltas[1]);
        }
    }
    if let Some((on, depth)) = lfo.filter(|(_, depth)| *depth > 0.0) {
        assigned = true;
        let running = on && enabled;
        lines.push(format!(
            "Internal LFO: ±{depth:.2} oct ({})",
            if running { "active" } else { "off" }
        ));
        if running {
            active = true;
            offsets[0] -= depth;
            offsets[1] += depth;
        }
    }
    if !assigned {
        return None;
    }
    let range = match parameter {
        ModulationParameter::LfoRate => offsets.map(|o| (base * 2.0_f32.powf(o)).max(0.0)),
        ModulationParameter::Pitch => offsets.map(|o| base + o.clamp(-96.0, 96.0)),
        ModulationParameter::Frequency => {
            offsets.map(|o| (base * 2.0_f32.powf(o)).clamp(20.0, 20_000.0))
        }
        ModulationParameter::ColorFrequency => {
            offsets.map(|o| (base * 2.0_f32.powf(o)).clamp(250.0, 2000.0))
        }
        ModulationParameter::Resonance => offsets.map(|o| (base + o).clamp(0.1, 10.0)),
        ModulationParameter::Level => {
            offsets.map(|o| (base + o).clamp(daw_core::SILENT_GAIN_DB, 12.0))
        }
        ModulationParameter::Volume => {
            if base <= daw_core::SILENT_GAIN_DB {
                [daw_core::SILENT_GAIN_DB; 2]
            } else {
                offsets.map(|o| (base + o.clamp(-96.0, 96.0)).clamp(daw_core::SILENT_GAIN_DB, 12.0))
            }
        }
        ModulationParameter::MacroValue
        | ModulationParameter::LfoDepth
        | ModulationParameter::DryWet
        | ModulationParameter::DriveGain
        | ModulationParameter::Tone
        | ModulationParameter::ColorStrength => offsets.map(|o| (base + o).clamp(0.0, 1.0)),
        ModulationParameter::Amplitude => {
            let factors = amplitude
                .map(|[min, max]| [(1.0 + min).clamp(0.0, 16.0), (1.0 + max).clamp(0.0, 16.0)]);
            std::array::from_fn(|i| {
                let factor = factors[0][i] * factors[1][i] * factors[2][i];
                if factor > 0.0 && base > daw_core::SILENT_GAIN_DB {
                    base + 20.0 * factor.log10()
                } else {
                    daw_core::SILENT_GAIN_DB
                }
            })
        }
    };
    lines.push("Outer arc: possible held-note range, limited to the knob scale. Pointer and readout: base value.".into());
    if parameter == ModulationParameter::Amplitude {
        lines.push("Moving dot: highest active voice gain, including note gate and bypass smoothing; voice sum excluded.".into());
    } else if parameter == ModulationParameter::Pitch {
        lines.push(
            "Moving dots: lowest and highest active voice transpose, excluding MIDI note pitch."
                .into(),
        );
    } else if parameter == ModulationParameter::Frequency {
        lines.push("Moving dot: processed value; Frequency shows both stereo channels.".into());
    } else {
        lines.push("Moving dot: processed value.".into());
    }
    let format_value = |value: f32| match parameter {
        ModulationParameter::Pitch => format!("{value:+.2} semitones"),
        ModulationParameter::Frequency | ModulationParameter::ColorFrequency => {
            knobs::format_frequency(value)
        }
        ModulationParameter::LfoRate => format!("{value:.2} Hz"),
        ModulationParameter::Resonance => format!("Q {value:.2}"),
        ModulationParameter::Amplitude
        | ModulationParameter::Level
        | ModulationParameter::Volume => {
            if value <= daw_core::SILENT_GAIN_DB {
                "−∞ dB".into()
            } else {
                format!("{value:.1} dB")
            }
        }
        ModulationParameter::MacroValue
        | ModulationParameter::LfoDepth
        | ModulationParameter::DryWet
        | ModulationParameter::DriveGain
        | ModulationParameter::Tone
        | ModulationParameter::ColorStrength => format!("{:.0}%", value * 100.0),
    };
    lines.push(format!(
        "Range: {} to {}",
        format_value(range[0]),
        format_value(range[1])
    ));
    if let Some(value) = live[0] {
        lines.push(format!(
            "Current: {}{}",
            format_value(value),
            live[1].map_or(String::new(), |right| format!(" / {}", format_value(right)))
        ));
    } else {
        lines.push("Live values appear during playback.".into());
    }
    lines.push("Click to open modulation controls.".into());
    Some(knobs::ModulationMarker {
        active,
        range,
        live,
        tooltip: lines.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::EnvelopeMode;
    fn source(mode: EnvelopeMode) -> Source {
        Source {
            id: ModulationSourceId {
                layer: daw_core::Id::new_v4(),
                unit: daw_core::Id::new_v4(),
            },
            label: "Layer 1 · Envelope 1".into(),
            scope: mode.into(),
            range: ModulationRange::Unipolar,
            behavior: daw_unit_core::ModulationBehavior::Contour,
            enabled: true,
        }
    }
    fn route(source: &Source, parameter: ModulationParameter, amount: f32) -> Modulation {
        let mut route = Modulation::new(parameter);
        route.source = Some(source.id);
        route.amount = amount;
        route
    }
    #[test]
    fn continuous_amplitude_ranges_multiply_contours_and_normalize_polarity() {
        let contour = source(EnvelopeMode::Legato);
        let mut lfo = source(EnvelopeMode::Legato);
        lfo.behavior = daw_unit_core::ModulationBehavior::Continuous;
        for polarity in [ModulationRange::Unipolar, ModulationRange::Bipolar] {
            lfo.range = polarity;
            let routes = [
                route(&contour, ModulationParameter::Amplitude, 0.5),
                route(&lfo, ModulationParameter::Amplitude, 0.5),
            ];
            let marker = marker(
                &routes,
                &[contour.clone(), lfo.clone()],
                ModulationParameter::Amplitude,
                0.0,
                true,
                None,
                [None; 2],
            )
            .unwrap();
            let range = marker.range;
            assert!((range[0] - 20.0 * 0.25_f32.log10()).abs() < 0.0001);
            assert!(range[1].abs() < 0.0001);
        }
    }
    #[test]
    fn markers_read_bipolar_range_from_provider_metadata() {
        let mut sources = [source(EnvelopeMode::Legato)];
        sources[0].range = ModulationRange::Bipolar;
        let routes = [route(&sources[0], ModulationParameter::Frequency, -2.0)];
        assert_eq!(
            marker(
                &routes,
                &sources,
                ModulationParameter::Frequency,
                1000.0,
                true,
                None,
                [None; 2]
            )
            .unwrap()
            .range,
            [250.0, 4000.0]
        );
    }
    #[test]
    fn marker_ranges_combine_sources_cancel_correlated_routes_and_clamp_bounds() {
        use ModulationParameter::*;
        let sources = [
            source(EnvelopeMode::Legato),
            source(EnvelopeMode::Retrigger),
        ];
        let routes = [
            route(&sources[0], Frequency, -2.0),
            route(&sources[1], Frequency, 1.0),
        ];
        let m = marker(
            &routes,
            &sources,
            Frequency,
            1000.0,
            true,
            Some((true, 1.0)),
            [Some(900.0), Some(1100.0)],
        )
        .unwrap();
        assert_eq!(m.range, [125.0, 4000.0]);
        assert!(m.active && m.tooltip.contains("-2.00 oct") && m.tooltip.contains("Internal LFO"));
        assert_eq!(m.live, [Some(900.0), Some(1100.0)]);
        let cancelled = [
            route(&sources[0], Frequency, -2.0),
            route(&sources[0], Frequency, 2.0),
        ];
        assert_eq!(
            marker(
                &cancelled, &sources, Frequency, 1000.0, true, None, [None; 2]
            )
            .unwrap()
            .range,
            [1000.0; 2]
        );
        for (parameter, base, amount, expected) in [
            (Frequency, 1000.0, 6.0, [1000.0, 20000.0]),
            (Resonance, 2.0, -10.0, [0.1, 2.0]),
            (Level, 0.0, 24.0, [0.0, 12.0]),
            (DryWet, 0.7, 1.0, [0.7, 1.0]),
        ] {
            assert_eq!(
                marker(
                    &[route(&sources[0], parameter, amount)],
                    &sources,
                    parameter,
                    base,
                    true,
                    None,
                    [None; 2]
                )
                .unwrap()
                .range,
                expected
            );
        }
    }
    #[test]
    fn marker_status_and_tooltip_distinguish_inactive_assignments() {
        let mut sources = [source(EnvelopeMode::Legato)];
        let mut connection = route(&sources[0], ModulationParameter::Frequency, 1.0);
        for reason in [
            "connection off",
            "source off",
            "zero amount",
            "source unavailable",
            "unit off",
        ] {
            let mut routes = [connection];
            sources[0].enabled = true;
            match reason {
                "connection off" => routes[0].enabled = false,
                "source off" => sources[0].enabled = false,
                "zero amount" => routes[0].amount = 0.0,
                "source unavailable" => routes[0].source = None,
                _ => {}
            }
            let m = marker(
                &routes,
                &sources,
                ModulationParameter::Frequency,
                1000.0,
                reason != "unit off",
                None,
                [None; 2],
            )
            .unwrap();
            assert!(!m.active);
            assert!(m.tooltip.contains(reason), "{}", m.tooltip);
            assert_eq!(m.range, [1000.0; 2]);
        }
        assert!(
            marker(
                &[],
                &sources,
                ModulationParameter::Frequency,
                1000.0,
                true,
                None,
                [None; 2]
            )
            .is_none()
        );
        connection.parameter = ModulationParameter::Amplitude;
        let local = source(EnvelopeMode::PerNote);
        let routes = [
            connection,
            route(&local, ModulationParameter::Amplitude, -0.5),
        ];
        let m = marker(
            &routes,
            &[sources[0].clone(), local],
            ModulationParameter::Amplitude,
            0.0,
            true,
            None,
            [None; 2],
        )
        .unwrap();
        assert_eq!(m.range[0], daw_core::SILENT_GAIN_DB);
        assert!((m.range[1] - 20.0 * 1.5_f32.log10()).abs() < 1e-6);
    }
    #[test]
    fn drive_modulation_ranges_respect_volume_silence_aggregate_limits_and_color_frequency() {
        use ModulationParameter::*;
        let source = source(EnvelopeMode::Legato);
        let sources = [source.clone()];
        for (p, base, amount, expected) in [
            (Volume, 0.0, -24.0, [-24.0, 0.0]),
            (
                Volume,
                daw_core::SILENT_GAIN_DB,
                24.0,
                [daw_core::SILENT_GAIN_DB; 2],
            ),
            (DriveGain, 0.2, 1.0, [0.2, 1.0]),
            (Tone, 0.5, -1.0, [0.0, 0.5]),
            (ColorStrength, 0.5, 1.0, [0.5, 1.0]),
            (ColorFrequency, 750.0, 3.0, [750.0, 2000.0]),
        ] {
            let marker = marker(
                &[route(&source, p, amount)],
                &sources,
                p,
                base,
                true,
                None,
                [None; 2],
            )
            .unwrap();
            assert_eq!(marker.range, expected);
        }
        let routes = (0..5)
            .map(|_| route(&source, Volume, -24.0))
            .collect::<Vec<_>>();
        assert_eq!(
            marker(&routes, &sources, Volume, 0.0, true, None, [None; 2])
                .unwrap()
                .range,
            [-96.0, 0.0]
        );
    }
}
