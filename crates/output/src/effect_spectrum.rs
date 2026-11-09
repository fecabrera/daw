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
