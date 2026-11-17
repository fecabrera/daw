//! Shared hosted-unit inventory, edits and PCM input dependencies.
use crate::*;
use daw_unit_core::{HostedSettings, HostedSource};

fn effect(e: &EffectInstance) -> Option<&HostedSettings> {
    if let EffectSettings::Hosted(s) = &e.unit {
        Some(s)
    } else {
        None
    }
}
fn effect_mut(e: &mut EffectInstance) -> Option<&mut HostedSettings> {
    if let EffectSettings::Hosted(s) = &mut e.unit {
        Some(s)
    } else {
        None
    }
}
fn instrument(i: &InstrumentSettings) -> impl Iterator<Item = &HostedSettings> {
    i.layers
        .iter()
        .flat_map(|l| {
            l.hosted
                .iter()
                .filter_map(HostedSource::settings)
                .chain(l.effects.iter().filter_map(effect))
        })
        .chain(i.buses().flat_map(|b| b.effects.iter().filter_map(effect)))
}
fn instrument_mut(i: &mut InstrumentSettings) -> impl Iterator<Item = &mut HostedSettings> {
    i.layers
        .iter_mut()
        .flat_map(|l| {
            l.hosted
                .iter_mut()
                .filter_map(HostedSource::settings_mut)
                .chain(l.effects.iter_mut().filter_map(effect_mut))
        })
        .chain(i.main.effects.iter_mut().filter_map(effect_mut))
}
impl Project {
    pub fn has_unsupported_hosted(&self) -> bool {
        self.all_effects().any(|e|matches!(&e.unit,EffectSettings::Missing{type_id,..} if type_id==daw_unit_core::HOSTED_TYPE_ID))
            || self.tracks.iter().flat_map(|t|t.instrument.iter().chain(t.beat.iter().flat_map(|b|b.subtracks.iter().map(|r|&r.instrument))))
                .flat_map(|i|&i.layers).any(|l|matches!(l.hosted,Some(daw_unit_core::HostedSource::Missing(_))))
    }

    /// Parent track ownership is shared by validation, routing and preparation.
    /// Beat subtrack/layer units resolve to their parent audio bus dependency.
    pub fn hosted_units(&self) -> Vec<(Id, &HostedSettings)> {
        let mut units = Vec::new();
        for track in &self.tracks {
            units.extend(
                track
                    .effects
                    .iter()
                    .filter_map(effect)
                    .map(|s| (track.id, s)),
            );
            units.extend(
                track
                    .instrument
                    .iter()
                    .flat_map(instrument)
                    .map(|s| (track.id, s)),
            );
            if let Some(beat) = &track.beat {
                units.extend(
                    beat.output
                        .effects
                        .iter()
                        .filter_map(effect)
                        .map(|s| (track.id, s)),
                );
                units.extend(
                    beat.subtracks
                        .iter()
                        .flat_map(|row| instrument(&row.instrument))
                        .map(|s| (track.id, s)),
                );
            }
        }
        units.extend(
            self.master
                .effects
                .iter()
                .filter_map(effect)
                .map(|s| (self.project_id, s)),
        );
        units
    }
    pub fn hosted_unit(&self, id: Id) -> Option<&HostedSettings> {
        self.hosted_units()
            .into_iter()
            .map(|(_, s)| s)
            .find(|s| s.id == id)
    }
    pub(crate) fn replace_hosted(&mut self, id: Id, settings: HostedSettings) -> Result<()> {
        let units = self
            .tracks
            .iter_mut()
            .flat_map(|track| {
                track
                    .effects
                    .iter_mut()
                    .filter_map(effect_mut)
                    .chain(track.instrument.iter_mut().flat_map(instrument_mut))
                    .chain(track.beat.iter_mut().flat_map(|beat| {
                        beat.output.effects.iter_mut().filter_map(effect_mut).chain(
                            beat.subtracks
                                .iter_mut()
                                .flat_map(|row| instrument_mut(&mut row.instrument)),
                        )
                    }))
            })
            .chain(self.master.effects.iter_mut().filter_map(effect_mut));
        for current in units {
            if current.id == id {
                if settings.id != id
                    || settings.plugin.identity != current.plugin.identity
                    || settings.plugin.role != current.plugin.role
                {
                    return Err(Error(
                        "Hosted control edit cannot replace the plugin identity".into(),
                    ));
                }
                *current = settings;
                return Ok(());
            }
        }
        Err(Error("Hosted unit is no longer present".into()))
    }
    pub(crate) fn validate_hosted_units(&self) -> Result<()> {
        let mut ids: std::collections::HashSet<Id> = self
            .tracks
            .iter()
            .map(|t| t.id)
            .chain(self.all_effects().map(|e| e.id))
            .chain(std::iter::once(self.project_id))
            .collect();
        for (_, settings) in self.hosted_units() {
            if !settings.valid()
                || !ids.insert(settings.id)
                || settings.parameters.iter().any(|p| !ids.insert(p.target))
                || settings
                    .audio_inputs
                    .iter()
                    .any(|r| r.source_track == self.project_id || !ids.insert(r.id))
            {
                return Err(Error(
                    "Invalid hosted settings, PCM source, or duplicate hosted identity".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn check_hosted_track_copy(&self, tracks: &[Id]) -> Result<()> {
        if self
            .tracks
            .iter()
            .filter(|t| tracks.contains(&t.id))
            .any(|t| {
                t.effects.iter().any(|e| !e.unit.can_duplicate_hosted())
                    || t.instrument
                        .iter()
                        .chain(
                            t.beat
                                .iter()
                                .flat_map(|b| b.subtracks.iter().map(|r| &r.instrument)),
                        )
                        .any(|i| {
                            i.layers.iter().any(|l| !l.can_duplicate_hosted())
                                || i.buses()
                                    .flat_map(|b| &b.effects)
                                    .any(|e| !e.unit.can_duplicate_hosted())
                        })
                    || t.beat
                        .iter()
                        .flat_map(|b| &b.output.effects)
                        .any(|e| !e.unit.can_duplicate_hosted())
            })
        {
            return Err(Error("Plugin copy requires a current opaque state capture; a plugin without state support cannot be copied exactly".into()));
        }
        Ok(())
    }
    pub fn hosted_source_available(&self, destination: Id, source: Id) -> bool {
        self.hosted_units()
            .into_iter()
            .find(|(_, s)| s.id == destination)
            .is_some_and(|(owner, _)| self.sidechain_source_available(owner, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hosted(source: Id, enabled: bool) -> HostedSettings {
        let mut settings = HostedSettings::new(
            daw_unit_core::HostedPluginDescriptor {
                identity: daw_unit_core::PluginIdentity::Clap("org.daw.test".into()),
                name: "Owned fixture".into(),
                vendor: "DAW".into(),
                version: "1".into(),
                role: daw_unit_core::HostedRole::Effect,
            },
            vec![],
        );
        settings.audio_inputs.push(daw_unit_core::HostedAudioInput {
            id: Id::new_v4(),
            source_track: source,
            port: daw_unit_core::HostedInputId::Clap { id: 7 },
            tap: daw_unit_core::SidechainTap::PreFader,
            channels: Default::default(),
            gain_db: 0.0,
            enabled,
        });
        settings
    }
    fn install(project: &mut Project, track: Id, settings: HostedSettings) -> Result<()> {
        project.edit(Edit::SetEffects {
            track_id: track,
            effects: vec![EffectInstance {
                id: Id::new_v4(),
                unit: EffectSettings::Hosted(settings),
            }],
            compact: false,
        })
    }
    #[test]
    fn disabled_pcm_edges_reject_cycles_as_one_transaction() {
        let mut project = Project::default();
        let a = project.add_track().unwrap();
        let b = project.add_track().unwrap();
        install(&mut project, b, hosted(a, false)).unwrap();
        let before = project.clone();
        assert!(install(&mut project, a, hosted(b, false)).is_err());
        assert_eq!(project, before);
        assert!(
            project
                .sidechain_order()
                .unwrap()
                .iter()
                .position(|i| project.tracks[*i].id == a)
                .unwrap()
                < project
                    .sidechain_order()
                    .unwrap()
                    .iter()
                    .position(|i| project.tracks[*i].id == b)
                    .unwrap()
        );
    }
    #[test]
    fn control_edit_preserves_identity_and_rejects_self_sidechain() {
        let mut project = Project::default();
        let a = project.add_track().unwrap();
        let b = project.add_track().unwrap();
        let settings = hosted(a, true);
        let id = settings.id;
        install(&mut project, b, settings.clone()).unwrap();
        let before = project.clone();
        let mut invalid = settings.clone();
        invalid.plugin.identity = daw_unit_core::PluginIdentity::Clap("other".into());
        assert!(
            project
                .edit(Edit::SetHosted {
                    unit_id: id,
                    settings: Box::new(invalid)
                })
                .is_err()
        );
        let mut invalid = settings;
        invalid.audio_inputs[0].source_track = b;
        assert!(
            project
                .edit(Edit::SetHosted {
                    unit_id: id,
                    settings: Box::new(invalid)
                })
                .is_err()
        );
        assert_eq!(project, before);
    }
}
