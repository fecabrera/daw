//! Prepared source-output handles. DSP snapshots detach publication through UnitMeter.
use super::*;
pub(super) type Sources = HashMap<daw_engine::SourceOutputTarget, Arc<daw_unit_core::BandMeter<3>>>;
pub(super) fn prepare(plan: &RenderPlan, previous: &Sources) -> Sources {
    plan.project
        .tracks
        .iter()
        .flat_map(|track| {
            track.instrument.iter().flat_map(move |instrument| {
                instrument.layers.iter().filter_map(move |layer| {
                    daw_engine::SourceOutputTarget::for_layer(track.id, layer)
                })
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
    fn source_handles_follow_track_layer_and_source_identity_and_prune_removed_units() {
        let mut p = crate::tests::transport_plan();
        let track = p.project.add_midi_track().unwrap();
        let t = p.project.tracks.iter_mut().find(|t| t.id == track).unwrap();
        t.instrument = Some(daw_core::OscillatorSettings::default().into());
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
        assert!(daw_engine::SourceOutputTarget::for_layer(track, layer).is_none());
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
