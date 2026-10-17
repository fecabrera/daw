use crate::*;

impl From<&SamplerInput> for Clip {
    fn from(input: &SamplerInput) -> Self {
        Self {
            id: input.asset_id,
            asset_id: input.asset_id,
            name: input.name.clone(),
            color: None,
            start_frame: 0,
            source_offset_frame: input.source_offset_frame,
            length_frames: input.length_frames,
            repeat: input.repeat,
            stretch: input.stretch,
        }
    }
}
impl From<&Clip> for SamplerInput {
    fn from(clip: &Clip) -> Self {
        Self {
            asset_id: clip.asset_id,
            name: clip.name.clone(),
            source_offset_frame: clip.source_offset_frame,
            length_frames: clip.length_frames,
            repeat: clip.repeat,
            stretch: clip.stretch,
        }
    }
}
impl Project {
    pub fn samplers(&self) -> impl Iterator<Item = &SamplerSettings> {
        self.tracks
            .iter()
            .filter_map(|t| t.instrument.as_ref())
            .flat_map(|i| &i.layers)
            .filter_map(|l| l.sampler.as_ref())
    }
    pub(crate) fn validate_samplers(&self, track: &Track) -> Result<()> {
        for s in track
            .instrument
            .iter()
            .flat_map(|i| &i.layers)
            .filter_map(|l| l.sampler.as_ref())
        {
            if self.samplers().filter(|other| other.id == s.id).count() != 1
                || self.assets.iter().any(|a| a.id == s.id)
            {
                return Err(Error("Sampler identity must be unique".into()));
            }
            if let Some(input) = &s.input {
                self.validate_clip(&Clip::from(input))?;
                if self
                    .assets
                    .iter()
                    .find(|a| a.id == input.asset_id)
                    .unwrap()
                    .midi_pattern
                    .is_some()
                {
                    return Err(Error("Sampler requires an audio source".into()));
                }
                if s.output_frames(self.sample_rate_hz, self.tempo_bpm)
                    .is_none()
                {
                    return Err(Error(
                        "Sampler duration must be between 1/8 and 8 times the selected region"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampler_transfers_and_source_conflicts_are_atomic() {
        let mut p = Project::default();
        let track = p.add_midi_track().unwrap();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        i.layers[0].sampler = Some(Default::default());
        i.layers.push(InstrumentLayerSettings::default());
        i.layers.push(InstrumentLayerSettings {
            oscillators: vec![Default::default()],
            ..Default::default()
        });
        let (from, to, conflict, main) =
            (i.layers[0].id, i.layers[1].id, i.layers[2].id, i.main.id);
        let original_id = i.layers[0].sampler.as_ref().unwrap().id;
        for destination in [conflict, main, from] {
            assert!(!p.can_transfer_component(
                track,
                from,
                destination,
                InstrumentComponent::Sampler,
                true
            ));
        }
        let copied = p
            .component_group_transfer(track, from, to, &[InstrumentComponent::Sampler], 1, true)
            .unwrap();
        p.edit(copied.edit).unwrap();
        let i = p.tracks[0].instrument.as_mut().unwrap();
        assert_ne!(i.layers[1].sampler.as_ref().unwrap().id, original_id);
        assert_eq!(
            i.layers[1].visible_components()[0],
            InstrumentComponent::Sampler
        );
        assert!(!p.can_transfer_component(
            track,
            conflict,
            to,
            InstrumentComponent::Oscillators,
            false
        ));
        let saved = serde_json::to_value(&p).unwrap();
        let mut invalid = p.tracks[0].instrument.clone().unwrap();
        invalid.layers[1].oscillators.push(Default::default());
        assert!(
            p.edit(Edit::SetInstrument {
                track_id: track,
                instrument: Some(invalid)
            })
            .is_err()
        );
        assert_eq!(serde_json::to_value(&p).unwrap(), saved);
        let mut removed = p.tracks[0].instrument.clone().unwrap();
        removed.layers[1].component_order = vec![InstrumentComponent::Sampler];
        assert_eq!(
            removed.layers[1].remove_components(&[InstrumentComponent::Sampler]),
            Some(true)
        );
        assert!(removed.valid());
        assert!(removed.layers[1].component_order.is_empty());
        let id = Id::new_v4();
        p.edit(Edit::DuplicateTrack {
            track_id: track,
            new_track_id: id,
        })
        .unwrap();
        p.validate().unwrap();
        assert_ne!(
            p.tracks[1].instrument.as_ref().unwrap().layers[0]
                .sampler
                .as_ref()
                .unwrap()
                .id,
            original_id
        );
    }
}
