//! Prepared source-output handles. DSP snapshots detach publication through UnitMeter.
use super::*;
pub(super) type Sources = HashMap<daw_engine::SourceOutputTarget, Arc<daw_unit_core::BandMeter<3>>>;
pub(super) fn prepare(plan: &RenderPlan, previous: &Sources) -> Sources {
    plan.project
        .tracks
        .iter()
        .flat_map(|track| {
            track.instrument.iter().map(move |i| (track.id, i)).chain(
                track
                    .beat
                    .iter()
                    .flat_map(|beat| beat.subtracks.iter().map(|row| (row.id, &row.instrument))),
            )
        })
        .flat_map(|(track, instrument)| {
            instrument.layers.iter().flat_map(move |layer| {
                daw_engine::SourceOutputTarget::for_layer(track, layer)
                    .into_iter()
                    .chain(layer.amplitude.present.then_some((
                        daw_engine::SourceOutputTarget::amplitude(track, layer.id),
                        layer.amplitude.enabled,
                    )))
            })
        })
        .map(|(key, _)| (key, previous.get(&key).cloned().unwrap_or_default()))
        .collect()
}
pub(super) fn bind(renderer: &mut Renderer, meters: &Sources) {
    for (target, meter) in meters {
        renderer.set_source_output_peak_meter(*target, meter.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn beat_audio_unit_handles_match_the_compiled_subtrack_scope() {
        let mut p = crate::tests::transport_plan();
        let track = p.project.add_beat_track().unwrap();
        let beat = p
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .beat
            .as_mut()
            .unwrap();
        let row = beat.add_subtrack().unwrap();
        let layer = &mut beat
            .subtracks
            .iter_mut()
            .find(|s| s.id == row)
            .unwrap()
            .instrument
            .layers[0];
        layer.oscillators = vec![Default::default()];
        layer.amplitude.present = true;
        let source = daw_engine::SourceOutputTarget::for_layer(row, layer)
            .unwrap()
            .0;
        let amplitude = daw_engine::SourceOutputTarget::amplitude(row, layer.id);
        let logical = prepare(&p, &Default::default());
        assert!(logical.contains_key(&source));
        assert!(logical.contains_key(&amplitude));
        let compiled = prepare(&p.prepare_beats(), &logical);
        assert_eq!(compiled.len(), logical.len());
        for (key, meter) in logical {
            assert!(Arc::ptr_eq(&meter, &compiled[&key]));
        }
    }
    #[test]
    fn amplitude_handles_are_distinct_retained_and_pruned_independently() {
        let mut p = crate::tests::transport_plan();
        let track = p.project.add_midi_track().unwrap();
        let t = p.project.tracks.iter_mut().find(|t| t.id == track).unwrap();
        t.instrument = Some(daw_core::OscillatorSettings::default().into());
        let layer = &mut t.instrument.as_mut().unwrap().layers[0];
        layer.amplitude.present = true;
        let source = daw_engine::SourceOutputTarget::for_layer(track, layer)
            .unwrap()
            .0;
        let amplitude = daw_engine::SourceOutputTarget::amplitude(track, layer.id);
        let first = prepare(&p, &Default::default());
        assert_eq!(first.len(), 2);
        assert!(!Arc::ptr_eq(&first[&source], &first[&amplitude]));
        let next = prepare(&p, &first);
        assert!(Arc::ptr_eq(&first[&amplitude], &next[&amplitude]));
        p.project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .amplitude
            .present = false;
        let removed = prepare(&p, &next);
        assert_eq!(removed.len(), 1);
        assert!(Arc::ptr_eq(&first[&source], &removed[&source]));
        assert!(!removed.contains_key(&amplitude));
    }
    #[test]
    fn source_handles_follow_track_layer_and_source_identity_and_prune_removed_units() {
        let mut p = crate::tests::transport_plan();
        let track = p.project.add_midi_track().unwrap();
        let t = p.project.tracks.iter_mut().find(|t| t.id == track).unwrap();
        t.instrument = Some(daw_core::OscillatorSettings::default().into());
        t.instrument.as_mut().unwrap().layers[0].amplitude.present = false;
        let key = daw_engine::SourceOutputTarget::for_layer(
            track,
            &t.instrument.as_ref().unwrap().layers[0],
        )
        .unwrap()
        .0;
        let original = prepare(&p, &Default::default());
        assert_eq!(original.len(), 1);
        let next = prepare(&p, &original);
        assert!(Arc::ptr_eq(&original[&key], &next[&key]));
        let copy = p.project.add_midi_track().unwrap();
        let t = p.project.tracks.iter_mut().find(|t| t.id == copy).unwrap();
        t.instrument = Some(daw_core::OscillatorSettings::default().into());
        t.instrument.as_mut().unwrap().layers[0].id = key.layer;
        t.instrument.as_mut().unwrap().layers[0].amplitude.present = false;
        let keys = prepare(&p, &next);
        assert_eq!(keys.len(), 2);
        let other = daw_engine::SourceOutputTarget { track: copy, ..key };
        assert!(!Arc::ptr_eq(&keys[&key], &keys[&other]));
        let layer = &mut p
            .project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.oscillators.clear();
        layer.sampler = Some(Default::default());
        assert_eq!(
            daw_engine::SourceOutputTarget::for_layer(track, layer)
                .unwrap()
                .0
                .unit,
            layer.sampler.as_ref().unwrap().id
        );
        let source = daw_core::SamplerSettings::granular();
        let unit = source.id;
        layer.sampler = Some(source);
        let updated = prepare(&p, &keys);
        assert!(!updated.contains_key(&key));
        let replacement = daw_engine::SourceOutputTarget { unit, ..key };
        assert!(updated.contains_key(&replacement));
        assert!(Arc::ptr_eq(&updated[&other], &keys[&other]));
        p.project.tracks.retain(|t| t.id != track);
        assert!(!prepare(&p, &updated).contains_key(&replacement));
    }
}
