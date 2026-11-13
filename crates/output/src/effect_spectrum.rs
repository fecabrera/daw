//! Prepared spectrum handles follow effect identity and renderer retirement.
use super::*;
pub(super) type Spectra = HashMap<Id, Arc<daw_unit_core::SpectrumMeter>>;
pub(super) fn prepare(plan: &RenderPlan, previous: &Spectra) -> Spectra {
    plan.project
        .all_effects()
        .filter(|e| matches!(e.unit, daw_core::EffectSettings::MultibandEq(_)))
        .map(|e| (e.id, previous.get(&e.id).cloned().unwrap_or_default()))
        .collect()
}
pub(super) fn bind(renderer: &mut Renderer, handles: &Spectra) {
    for (id, handle) in handles {
        renderer.set_effect_spectrum_meter(*id, handle.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_eq_scope_reuses_its_handle_and_prepared_binding_does_not_publish() {
        use daw_core::{BeatEdit, BeatSubtrack, Edit, EffectInstance, Project};
        let mut project = Project::default();
        project.add_track().unwrap();
        project.tracks[0]
            .effects
            .push(EffectInstance::multiband_equalizer());
        project.add_midi_track().unwrap();
        let instrument = project.tracks[1].instrument.as_mut().unwrap();
        instrument.layers[0]
            .effects
            .push(EffectInstance::multiband_equalizer());
        instrument
            .main
            .effects
            .push(EffectInstance::multiband_equalizer());
        let beat = project.add_beat_track().unwrap();
        let mut row = BeatSubtrack::new("Tone".into());
        row.instrument.layers[0]
            .effects
            .push(EffectInstance::multiband_equalizer());
        project
            .edit(Edit::Beat {
                track_id: beat,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: 0,
                },
            })
            .unwrap();
        project
            .master
            .effects
            .push(EffectInstance::multiband_equalizer());
        let plan = RenderPlan {
            project,
            beat_tracks: Default::default(),
            audio: Default::default(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let previous = prepare(&plan, &HashMap::new());
        let handles = prepare(&plan, &previous);
        assert_eq!(handles.len(), 5);
        let before: HashMap<_, _> = handles
            .iter()
            .map(|(id, meter)| (*id, (meter.reading(), Arc::strong_count(meter))))
            .collect();
        let mut renderer = Renderer::new(plan);
        bind(&mut renderer, &handles);
        for (id, meter) in handles {
            assert!(Arc::ptr_eq(&meter, &previous[&id]));
            assert_eq!(meter.reading(), before[&id].0);
            assert!(Arc::strong_count(&meter) > before[&id].1);
        }
    }
}
