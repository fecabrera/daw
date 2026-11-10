//! DAW shell and shared targeted modulation bindings.
use crate::{
    meters,
    modulation_panel::{self, Source, Target},
    panels,
};
use daw_core::{HarmonicLiftSettings, Id, ModulationParameter as P};
pub(super) use daw_unit_ui::harmonic_lift::Bounds;
use egui::{Ui, Vec2};
pub(super) fn show<'a>(
    ui: &mut Ui,
    s: &mut HarmonicLiftSettings,
    dimensions: (bool, f32),
    reduction: meters::GainReduction<'a>,
    sources: &[Source],
    live: impl Fn(Option<Id>) -> Option<daw_engine::ModulationSnapshot>,
    levels: (meters::OutputLevel<'_>, [meters::OutputLevel<'_>; 6]),
) -> (Bounds, egui::Response) {
    let (level, band_levels) = levels;
    let bases = [
        s.input_db, s.drive, s.push, s.limit_db, s.level_db, s.dry_wet,
    ];
    let markers = std::array::from_fn(|i| {
        modulation_panel::marker(
            &s.modulations,
            sources,
            P::HARMONIC_LIFT[i],
            bases[i],
            s.enabled,
            None,
            [live(None).map(|v| v[i]), None],
        )
    });
    let bands = std::array::from_fn(|i| {
        let band = &s.bands[i];
        let routes: Vec<_> = s
            .modulations
            .iter()
            .filter(|r| r.target == Some(band.id))
            .copied()
            .collect();
        std::array::from_fn(|j| {
            let mut marker = modulation_panel::marker(
                &routes,
                sources,
                [P::EqBandFrequency, P::EqBandGain][j],
                [band.frequency_hz, band.gain_db][j],
                s.enabled && band.enabled,
                None,
                [live(Some(band.id)).map(|v| v[j]), None],
            )?;
            let [min, max] = if j == 0 {
                daw_unit_core::HARMONIC_FREQUENCY_RANGES[i]
            } else {
                [0.0, 24.0]
            };
            marker.range = marker.range.map(|v| v.clamp(min, max));
            marker.live = marker.live.map(|v| v.map(|v| v.clamp(min, max)));
            Some(marker)
        })
    });
    let (compact, height) = dimensions;
    let mut enabled = s.enabled;
    let mut collapsed = s.collapsed;
    let mut b = Bounds::default();
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Harmonic Lift", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                b = daw_unit_ui::harmonic_lift::show(
                    ui,
                    s,
                    (compact, crate::envelope_panel::body_height(compact)),
                    daw_unit_ui::harmonic_lift::Controls {
                        markers: &markers,
                        bands: &bands,
                        reduction,
                        level,
                        band_levels,
                    },
                );
                if s.modulation_open {
                    let mut targets: Vec<_> = s
                        .bands
                        .iter()
                        .enumerate()
                        .flat_map(|(i, b)| {
                            [P::EqBandFrequency, P::EqBandGain].map(|parameter| Target {
                                parameter,
                                id: Some(b.id),
                                label: format!(
                                    "{} · {}",
                                    daw_unit_core::HARMONIC_BAND_NAMES[i],
                                    if parameter == P::EqBandGain {
                                        "Gain"
                                    } else {
                                        "Frequency"
                                    }
                                ),
                            })
                        })
                        .collect();
                    targets.extend(P::HARMONIC_LIFT.map(|parameter| Target {
                        parameter,
                        id: None,
                        label: parameter.label().into(),
                    }));
                    modulation_panel::targeted_body(
                        ui,
                        &mut s.modulations,
                        sources,
                        &targets,
                        daw_unit_core::ModulationScope::Shared,
                        compact,
                    );
                }
            });
        },
    );
    s.enabled = enabled;
    s.collapsed = collapsed;
    modulation_panel::toggle(ui, &result, collapsed, &mut s.modulation_open);
    b.header = result.header.rect;
    b.power = result.power.rect;
    (b, result.header)
}
