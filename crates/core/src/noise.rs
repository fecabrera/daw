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
