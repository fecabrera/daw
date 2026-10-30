//! Native shell and modulation bindings over the shared exciter body.
use super::*;
use crate::modulation_panel::{Source, Target};
use daw_core::{ExciterSettings, ModulationParameter as P};
#[derive(Default)]
pub(super) struct Audition {
    pub solos: HashSet<(Id, Id)>,
    project: Option<Id>,
}
pub(super) struct Bounds {
    #[cfg(test)]
    pub header: Rect,
    pub power: Rect,
    pub solo_changed: bool,
    #[cfg(test)]
    pub body: daw_unit_ui::exciter::Bounds,
}
impl DawUi {
    pub(super) fn sync_exciter_audition(&mut self) {
        if self.exciter_audition.project != Some(self.session.project.project_id) {
            self.exciter_audition.solos.clear();
            self.exciter_audition.project = Some(self.session.project.project_id);
        }
        self.exciter_audition.solos.retain(|(effect, band)| {
            self.session
                .project
                .all_effects()
                .any(|e| e.id == *effect && e.unit.band_ids().any(|b| b == *band))
        });
        if let Some(output) = &self.output {
            output.set_exciter_solos(&self.exciter_audition.solos);
        }
    }
}
pub(super) fn show(
    ui: &mut egui::Ui,
    settings: &mut ExciterSettings,
    dimensions: (bool, f32),
    sources: &[Source],
    live: impl Fn(Option<Id>) -> Option<daw_engine::ModulationSnapshot>,
    audition: (Id, &mut HashSet<(Id, Id)>),
    level: meters::OutputLevel<'_>,
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
            let bases = [b.drive_db, b.level_db, b.mix];
            std::array::from_fn(|i| {
                crate::modulation_panel::marker(
                    &routes,
                    sources,
                    P::SATURATOR[i],
                    bases[i],
                    settings.enabled && b.enabled,
                    None,
                    [live(Some(b.id)).map(|v| v[i]), None],
                )
            })
        })
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
    let (compact, height) = dimensions;
    let mut enabled = settings.enabled;
    let mut collapsed = settings.collapsed;
    let mut body = daw_unit_ui::exciter::Bounds::default();
    // Stable pre-edit identities keep a Solo click attached to its original band.
    let ids: Vec<_> = settings.bands.iter().map(|b| b.id).collect();
    let result = panels::PanelStyle {
        margin: 0,
        ..Default::default()
    }
    .unit_in_lane(
        ui,
        ("Multiband Exciter", compact),
        height,
        &mut enabled,
        &mut collapsed,
        |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                body = daw_unit_ui::exciter::show(
                    ui,
                    settings,
                    (compact, crate::envelope_panel::body_height(compact)),
                    daw_unit_ui::exciter::Controls {
                        solos: &mut selected,
                        band_markers: &markers,
                        output_markers: &output_markers,
                        level,
                    },
                );
                if settings.modulation_open {
                    panels::unit_divider(ui, body.output.right(), body.output.y_range().into());
                    let mut targets: Vec<_> = settings
                        .bands
                        .iter()
                        .enumerate()
                        .flat_map(|(i, b)| {
                            P::SATURATOR.map(|parameter| Target {
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
                    targets.extend([P::Level, P::DryWet].map(|parameter| Target {
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
    let mut solo_changed = false;
    for (id, selected) in ids.into_iter().zip(selected) {
        solo_changed |= if selected {
            solos.insert((effect, id))
        } else {
            solos.remove(&(effect, id))
        };
    }
    settings.enabled = enabled;
    settings.collapsed = collapsed;
    crate::modulation_panel::toggle(ui, &result.header, collapsed, &mut settings.modulation_open);
    (
        Bounds {
            #[cfg(test)]
            header: result.header.rect,
            power: result.power.rect,
            solo_changed,
            #[cfg(test)]
            body,
        },
        result.header,
    )
}
