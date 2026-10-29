//! Stable, worker-prepared stereo handles for regular instrument layers and Output.
use super::*;

pub(super) type Layers = HashMap<(Id, Id), Arc<StereoMeter>>;

pub(super) fn prepare(plan: &RenderPlan, previous: &Layers) -> Layers {
    plan.project
        .tracks
        .iter()
        .flat_map(|track| {
            track.instrument.iter().flat_map(move |instrument| {
                instrument
                    .layers
                    .iter()
                    .map(|layer| layer.id)
                    .chain(std::iter::once(instrument.main.id))
                    .map(move |layer| {
                        let key = (track.id, layer);
                        (key, previous.get(&key).cloned().unwrap_or_default())
                    })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_follow_scoped_identity_reorder_removal_and_shared_stereo_reset() {
        let mut plan = crate::tests::transport_plan();
        let track = plan.project.add_midi_track().unwrap();
        let instrument = plan
            .project
            .tracks
            .last_mut()
            .unwrap()
            .instrument
            .as_mut()
            .unwrap();
        instrument
            .layers
            .push(daw_core::InstrumentLayerSettings::default());
        let layer = instrument.layers[0].id;
        let peer = instrument.layers[1].id;
        let output = instrument.main.id;
        let meters = prepare(&plan, &Default::default());
        assert_eq!(meters.len(), 3);
        meters[&(track, layer)].publish_peaks([0.5, 1.25]);
        assert_eq!(
            meters[&(track, layer)].levels(),
            [(0.5, true), (1.25, true)]
        );
        assert_eq!(meters[&(track, peer)].levels(), [(0.0, false); 2]);
        assert_eq!(meters[&(track, output)].held_peak(), 0.0);
        plan.project
            .tracks
            .last_mut()
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers
            .swap(0, 1);
        let updated = prepare(&plan, &meters);
        assert!(Arc::ptr_eq(
            &updated[&(track, layer)],
            &meters[&(track, layer)]
        ));
        updated[&(track, layer)].reset();
        assert_eq!(updated[&(track, layer)].held_peak(), 0.0);
        assert!(!updated[&(track, layer)].is_clipped());
        // Layer identities are scoped to their track, including legacy/shared input IDs.
        let copy = plan.project.add_midi_track().unwrap();
        plan.project
            .tracks
            .last_mut()
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers[0]
            .id = layer;
        let updated = prepare(&plan, &updated);
        assert!(!Arc::ptr_eq(
            &updated[&(track, layer)],
            &updated[&(copy, layer)]
        ));
        plan.project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .instrument
            .as_mut()
            .unwrap()
            .layers
            .retain(|l| l.id != layer);
        let updated = prepare(&plan, &updated);
        assert!(!updated.contains_key(&(track, layer)));
        assert!(updated.contains_key(&(track, peer)));
        assert!(updated.contains_key(&(copy, layer)));
    }

    #[test]
    fn prepared_publication_uses_rendered_layer_and_output_peaks_and_clears_silent_blocks() {
        use daw_core::{Edit, MidiNote, MidiPattern};
        let mut plan = crate::tests::transport_plan();
        let track = plan.project.add_midi_track().unwrap();
        let instrument = plan
            .project
            .tracks
            .last_mut()
            .unwrap()
            .instrument
            .as_mut()
            .unwrap();
        *instrument = daw_core::OscillatorSettings::default().into();
        instrument.layers[0].gain_db = 12.0;
        instrument.layers[0].pan = -1.0;
        let layer = instrument.layers[0].id;
        let output = instrument.main.id;
        let clip = plan.project.add_midi_pattern(track, 0).unwrap();
        plan.project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![MidiNote {
                        id: Id::new_v4(),
                        pitch: 60,
                        velocity: 127,
                        start_tick: 0,
                        length_ticks: 3840,
                    }],
                },
            })
            .unwrap();
        let meters = prepare(&plan, &Default::default());
        let tracks = track_meters(&plan, &Default::default());
        let mut renderer = PreparedRenderer::new(plan, &tracks);
        renderer.layer_meters = meters.clone();
        renderer.play();
        for _ in 0..4096 {
            renderer.next_sample();
        }
        renderer.publish_track_peaks();
        assert!(meters[&(track, layer)].levels()[0].0 > 0.0);
        assert_eq!(meters[&(track, layer)].levels()[1].0, 0.0);
        assert_eq!(
            meters[&(track, layer)].levels(),
            meters[&(track, output)].levels()
        );
        let held = meters[&(track, layer)].held_peak();
        renderer.reset_track_peaks();
        renderer.publish_track_peaks();
        assert_eq!(meters[&(track, layer)].levels().map(|v| v.0), [0.0; 2]);
        assert_eq!(meters[&(track, layer)].held_peak(), held);
    }
}
