//! Host identity validation for procedural Noise sources.
use crate::*;
impl Project {
    pub fn noises(&self) -> impl Iterator<Item = &NoiseSettings> {
        self.instruments()
            .flat_map(|i| &i.layers)
            .filter_map(|l| l.noise.as_ref())
    }
    pub(crate) fn validate_noises(&self, track: &Track) -> Result<()> {
        for noise in track
            .instrument
            .iter()
            .chain(
                track
                    .beat
                    .iter()
                    .flat_map(|b| b.subtracks.iter().map(|s| &s.instrument)),
            )
            .flat_map(|i| &i.layers)
            .filter_map(|l| l.noise.as_ref())
        {
            if self.noises().filter(|n| n.id == noise.id).count() != 1
                || self.assets.iter().any(|a| a.id == noise.id)
                || self.samplers().any(|s| s.id == noise.id)
            {
                return Err(Error("Noise identity must be unique".into()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn track_and_group_copies_refresh_noise_and_owned_routes_only() {
        for grouped in [false, true] {
            let mut project = Project::default();
            let track = project.add_midi_track().unwrap();
            let instrument = project.tracks[0].instrument.as_mut().unwrap();
            let layer = &mut instrument.layers[0];
            layer.envelopes.push(Default::default());
            let provider = ModulationSourceId {
                layer: layer.id,
                unit: layer.envelopes[0].id,
            };
            let mut route = Modulation::new(ModulationParameter::NoiseLevel);
            route.source = Some(provider);
            let noise = NoiseSettings {
                enabled: false,
                collapsed: true,
                color: 0.4,
                width: 0.8,
                modulations: vec![route],
                ..Default::default()
            };
            layer.noise = Some(noise.clone());
            project.validate().unwrap();
            let owner = if grouped {
                let id = Id::new_v4();
                project
                    .edit(Edit::GroupTracks {
                        track_ids: vec![track],
                        group_id: id,
                        name: "Group".into(),
                    })
                    .unwrap();
                id
            } else {
                track
            };
            let original = project
                .tracks
                .iter()
                .find(|t| t.id == track)
                .unwrap()
                .clone();
            let copied_owner = Id::new_v4();
            project
                .edit(Edit::DuplicateTrack {
                    track_id: owner,
                    new_track_id: copied_owner,
                })
                .unwrap();
            let copy = project
                .tracks
                .iter()
                .find(|t| {
                    if grouped {
                        t.group_id == Some(copied_owner)
                    } else {
                        t.id == copied_owner
                    }
                })
                .unwrap();
            let copied = copy.instrument.as_ref().unwrap().layers[0]
                .noise
                .as_ref()
                .unwrap();
            assert_ne!(copied.id, noise.id);
            assert_ne!(copied.modulations[0].id, noise.modulations[0].id);
            assert_eq!(copied.modulations[0].source, Some(provider));
            let mut normalized = copied.clone();
            normalized.id = noise.id;
            normalized.modulations[0].id = noise.modulations[0].id;
            assert_eq!(normalized, noise);
            assert!(copy.clips.is_empty());
            assert_eq!(
                serde_json::to_value(project.tracks.iter().find(|t| t.id == track).unwrap())
                    .unwrap(),
                serde_json::to_value(&original).unwrap()
            );
            project.validate().unwrap();
        }
    }
    #[test]
    fn beat_copy_keeps_fresh_noise_and_route_identities() {
        let mut project = Project::default();
        let track = project.add_beat_track().unwrap();
        let mut row = BeatSubtrack::new("Noise".into());
        let noise = NoiseSettings {
            modulations: vec![Modulation::new(ModulationParameter::NoiseLevel)],
            ..Default::default()
        };
        row.instrument.layers[0].noise = Some(noise.clone());
        project
            .edit(Edit::Beat {
                track_id: track,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: 0,
                },
            })
            .unwrap();
        let copy = Id::new_v4();
        project
            .edit(Edit::DuplicateTrack {
                track_id: track,
                new_track_id: copy,
            })
            .unwrap();
        let copied = project
            .tracks
            .iter()
            .find(|t| t.id == copy)
            .unwrap()
            .beat
            .as_ref()
            .unwrap()
            .subtracks[0]
            .instrument
            .layers[0]
            .noise
            .as_ref()
            .unwrap();
        assert_ne!(copied.id, noise.id);
        assert_ne!(copied.modulations[0].id, noise.modulations[0].id);
        project.validate().unwrap();
    }
    #[test]
    fn noise_transfers_reject_source_conflicts_and_copy_fresh_identities() {
        let mut project = Project::default();
        let track = project.add_midi_track().unwrap();
        let i = project.tracks[0].instrument.as_mut().unwrap();
        i.layers[0].noise = Some(Default::default());
        i.layers.push(InstrumentLayerSettings::default());
        i.layers.push(InstrumentLayerSettings {
            oscillators: vec![Default::default()],
            ..Default::default()
        });
        let (from, to, occupied, main) =
            (i.layers[0].id, i.layers[1].id, i.layers[2].id, i.main.id);
        let id = i.layers[0].noise.as_ref().unwrap().id;
        for destination in [occupied, main, from] {
            assert!(!project.can_transfer_component(
                track,
                from,
                destination,
                InstrumentComponent::Noise,
                true
            ));
        }
        let edit = project
            .component_group_transfer(track, from, to, &[InstrumentComponent::Noise], 1, true)
            .unwrap()
            .edit;
        project.edit(edit).unwrap();
        let i = project.tracks[0].instrument.as_ref().unwrap();
        assert_ne!(i.layers[1].noise.as_ref().unwrap().id, id);
        let saved = serde_json::to_value(&project).unwrap();
        let mut invalid = i.clone();
        invalid.layers[0].sampler = Some(Default::default());
        assert!(
            project
                .edit(Edit::SetInstrument {
                    track_id: track,
                    instrument: Some(invalid)
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), saved);
    }
}
