//! Native shell over the shared compressor band body.
use super::*;
use crate::modulation_panel::{Source, Target};
use daw_core::{ModulationParameter as P, MultibandCompressorSettings};
pub(super) struct Bounds {
    #[cfg(test)]
    pub body: daw_unit_ui::multiband_compressor::Bounds,
    #[cfg(test)]
    pub header: Rect,
    pub power: Rect,
    pub solo_changed: bool,
}
#[allow(clippy::too_many_arguments)]
pub(super) fn show<'a>(
    ui: &mut egui::Ui,
    settings: &mut MultibandCompressorSettings,
    dimensions: (bool, f32),
    sources: &[Source],
    live: impl Fn(Option<Id>) -> Option<daw_engine::ModulationSnapshot>,
    audition: (Id, &mut HashSet<(Id, Id)>),
    level: meters::OutputLevel<'a>,
    levels: impl Fn(Id) -> meters::OutputLevel<'a>,
    reductions: impl Fn(Id) -> meters::GainReduction<'a>,
) -> (Bounds, egui::Response) {
    let (effect, solos) = audition;
    let mut selected: Vec<_> = settings
        .bands
        .iter()
        .map(|b| solos.contains(&(effect, b.id)))
        .collect();
    let markers: Vec<_> = settings
        .bands
        .iter()
        .map(|b| {
            let routes: Vec<_> = settings
                .modulations
                .iter()
                .filter(|r| r.target == Some(b.id))
                .copied()
                .collect();
            let bases = [
                b.threshold_db,
                b.ratio,
                b.attack_ms,
                b.release_ms,
                b.knee_db,
                b.level_db,
            ];
            std::array::from_fn(|i| {
                crate::modulation_panel::marker(
                    &routes,
                    sources,
                    P::COMPRESSOR[i],
                    bases[i],
                    settings.enabled && b.enabled,
                    None,
                    [live(Some(b.id)).map(|v| v[i]), None],
                )
            })
        })
        .collect();
    let band_levels: Vec<_> = settings.bands.iter().map(|b| levels(b.id)).collect();
    let band_reductions: Vec<_> = settings
        .bands
        .iter()
        .map(|b| reductions(b.id).for_unit(settings.enabled && b.enabled))
        .collect();
    let bases = [settings.level_db, settings.dry_wet];
    let output_markers = std::array::from_fn(|i| {
        crate::modulation_panel::marker(
            &settings.modulations,
            sources,
            [P::Level, P::DryWet][i],
            bases[i],
            settings.enabled,
            None,
            [live(None).map(|v| v[i]), None],
        )
    });
    let owner = ui.make_persistent_id("multiband_compressor-strip-order");
    let reordering = crate::strip_reorder::active(ui, owner);
    let original: Vec<_> = if settings.presentation.is_empty() {
        settings.bands.clone()
    } else {
        settings
            .presentation
            .iter()
            .filter_map(|id| settings.bands.iter().find(|b| b.id == *id).cloned())
            .collect()
    };
    let (compact, height) = dimensions;
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let mut body = daw_unit_ui::multiband_compressor::Bounds::default();
    // Stable pre-edit identities keep a Solo click attached to its original band.
    let ids: Vec<_> = settings.bands.iter().map(|b| b.id).collect();
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Multiband Compressor", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                body = ui
                    .add_enabled_ui(!reordering, |ui| {
                        daw_unit_ui::multiband_compressor::show(
                            ui,
                            settings,
                            (compact, crate::envelope_panel::body_height(compact)),
                            daw_unit_ui::multiband_compressor::Controls {
                                solos: &mut selected,
                                band_markers: &markers,
                                output_markers: &output_markers,
                                level,
                                levels: &band_levels,
                                reductions: &band_reductions,
                                add_enabled: true,
                            },
                        )
                    })
                    .inner;
                if settings.modulation_open {
                    let mut targets: Vec<_> = settings
                        .bands
                        .iter()
                        .enumerate()
                        .flat_map(|(i, b)| {
                            P::COMPRESSOR[..6]
                                .iter()
                                .copied()
                                .map(move |parameter| Target {
                                    parameter,
                                    id: Some(b.id),
                                    label: format!(
                                        "Band {} · {}",
                                        i + 1,
                                        if parameter == P::DryWet {
                                            "Mix"
                                        } else {
                                            parameter.label()
                                        }
                                    ),
                                })
                        })
                        .collect();
                    targets.extend([P::Level, P::DryWet].map(move |parameter| Target {
                        parameter,
                        id: None,
                        label: format!("Out · {}", parameter.label()),
                    }));
                    crate::modulation_panel::targeted_body(
                        ui,
                        &mut settings.modulations,
                        sources,
                        &targets,
                        daw_unit_core::ModulationScope::Shared,
                        compact,
                    );
                }
            });
        },
    );
    let mut slots = Vec::new();
    for (i, (band, response)) in original.iter().zip(&body.band_responses).enumerate() {
        let control = ui
            .input(|input| input.pointer.press_origin())
            .is_some_and(|p| {
                meters::unit_monitor_rail(body.bands[i], daw_unit_ui::layout::PANEL_MARGIN)
                    .contains(p)
                    || body.power[i].contains(p)
                    || body.solo[i].contains(p)
                    || body.modes[i].contains(p)
                    || body.knobs[i].iter().any(|r| r.contains(p))
            });
        if !control {
            crate::strip_reorder::arm_items(ui, owner, response, band.id, &original);
        }
        slots.push((band.id, response.rect));
    }
    let mut order = original.clone();
    if let (Some((_, first)), Some((_, last))) = (slots.first(), slots.last()) {
        crate::strip_reorder::finish_items(
            ui,
            owner,
            &mut order,
            &slots,
            first.union(*last).intersect(ui.clip_rect()),
        );
    }
    if order.iter().map(|b| b.id).ne(original.iter().map(|b| b.id))
        && order
            .iter()
            .all(|b| settings.bands.iter().any(|s| s.id == b.id))
        && order.len() == settings.bands.len()
    {
        settings.presentation = order.iter().map(|b| b.id).collect();
    }
    let mut solo_changed = false;
    for (id, selected) in ids.into_iter().zip(selected) {
        solo_changed |= if selected && enabled {
            solos.insert((effect, id))
        } else {
            solos.remove(&(effect, id))
        };
    }
    if !enabled {
        solos.retain(|(e, _)| *e != effect);
    }
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result, collapsed, &mut settings.modulation_open);
    (
        Bounds {
            #[cfg(test)]
            body,
            #[cfg(test)]
            header: result.header.rect,
            power: result.power.rect,
            solo_changed,
        },
        result.header,
    )
}
