//! Live-only effect audition bindings. No settings or export path owns these flags.
use super::*;
use std::sync::atomic::AtomicBool;
pub(super) type Solos = HashMap<ModulationTarget, Arc<AtomicBool>>;
pub(super) fn prepare(plan: &RenderPlan, previous: &Solos) -> Solos {
    plan.project
        .all_effects()
        .filter_map(|e| match &e.unit {
            daw_core::EffectSettings::Exciter(s) => Some((e.id, s)),
            _ => None,
        })
        .flat_map(|(effect, s)| {
            s.bands
                .iter()
                .map(move |b| ModulationTarget::EffectBand { effect, band: b.id })
        })
        .map(|key| (key, previous.get(&key).cloned().unwrap_or_default()))
        .collect()
}
pub(super) fn bind(renderer: &mut Renderer, solos: &Solos) {
    for (target, solo) in solos {
        if let ModulationTarget::EffectBand { effect, band } = *target {
            renderer.bind_exciter_solo(effect, band, solo.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_bindings_retain_stable_flags_and_prune_removed_bands() {
        let mut project = daw_core::Project::default();
        project.add_track().unwrap();
        let effect = daw_core::EffectInstance::exciter();
        let id = effect.id;
        let band = effect.unit.band_ids().next().unwrap();
        project.tracks[0].effects.push(effect);
        let mut plan = RenderPlan {
            project,
            audio: HashMap::new(),
            sampler_audio: Default::default(),
            stretched_audio: Default::default(),
        };
        let original = prepare(&plan, &Default::default());
        assert_eq!(original.len(), 4);
        let key = ModulationTarget::EffectBand { effect: id, band };
        original[&key].store(true, Ordering::Relaxed);
        let daw_core::EffectSettings::Exciter(s) = &mut plan.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        s.add_band();
        let next = prepare(&plan, &original);
        assert_eq!(next.len(), 5);
        assert!(Arc::ptr_eq(&original[&key], &next[&key]));
        assert!(next[&key].load(Ordering::Relaxed));
        let meters = band_meters(&plan, &HashMap::new());
        assert_eq!(meters.len(), 1, "Exciter uses one shared Out meter");
        let parameters = modulation_meters(&plan, &HashMap::new());
        assert_eq!(parameters.len(), 6);
        let mut renderer = Renderer::new(plan.clone());
        bind(&mut renderer, &next);
        bind_band_meters(&mut renderer, &meters);
        assert!(renderer.modulation_snapshot(key).is_some());
        let daw_core::EffectSettings::Exciter(s) = &mut plan.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        s.remove_band(band);
        assert!(!prepare(&plan, &next).contains_key(&key));
        plan.project.tracks[0].effects.clear();
        assert!(prepare(&plan, &next).is_empty());
    }
}
