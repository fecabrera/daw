//! Worker-compiled effect transactions. No source schedule or unrelated DSP is rebuilt.
use super::{RenderPlan, Renderer, effects::EffectChain};
use daw_core::{EffectInstance, EffectSettings, Id, Project};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Owner {
    Master,
    Track(usize),
    Layer(usize, usize),
}

struct ChainUpdate {
    owner: Owner,
    chain: EffectChain,
    settings: Vec<EffectInstance>,
    slots: Vec<Option<(Owner, usize, bool)>>,
    direct_slots: Option<Vec<usize>>,
}

/// Conservative reasons for retaining exact renderer preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum EffectUpdateFallback {
    Arrangement = 0,
    Latency = 1,
    Sidechain = 2,
    PublicationBudget = 3,
}

/// An owned transaction: consumed storage also receives retired processors and metadata.
/// Construct on a worker, apply at a processing boundary, destroy off the audio thread.
pub struct EffectUpdate {
    plan: RenderPlan,
    chains: Vec<ChainUpdate>,
    effect_tracks: HashMap<Id, Option<usize>>,
    gain_targets: Vec<[f32; 4]>,
    end_frame: u64,
    prepared_processors: usize,
    fingerprint: u64,
}

pub(super) fn fingerprint(plan: &RenderPlan) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    // Layer row order is presentation only; hash each chain under its stable owner.
    let mut effect_groups = Vec::new();
    for track in &plan.project.tracks {
        effect_groups.push(((Some(track.id), None), track.effects.as_slice()));
        if let Some(instrument) = &track.instrument {
            effect_groups.push((
                (Some(track.id), Some(instrument.main.id)),
                instrument.main.effects.as_slice(),
            ));
            let mut layers: Vec<_> = instrument.layers.iter().collect();
            layers.sort_by_key(|l| l.id);
            for layer in layers {
                effect_groups.push(((Some(track.id), Some(layer.id)), layer.effects.as_slice()));
            }
        }
    }
    effect_groups.push(((None, None), plan.project.master.effects.as_slice()));
    for (owner, effects) in effect_groups {
        owner.hash(&mut hash);
        effects.len().hash(&mut hash);
        for effect in effects {
            effect.id.hash(&mut hash);
            match &effect.unit {
                daw_core::EffectSettings::HarmonicLift(_) => {
                    daw_unit_core::HARMONIC_LIFT_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::MultibandCompressor(_) => {
                    daw_unit_core::MULTIBAND_COMPRESSOR_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Exciter(_) => {
                    daw_unit_core::EXCITER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::MultibandEq(_) => {
                    daw_unit_core::MULTIBAND_EQ_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Loudness(_) => {
                    daw_unit_core::LOUDNESS_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Eq3(_) => daw_unit_core::EQ_TYPE_ID.hash(&mut hash),
                daw_core::EffectSettings::DriveV2(_) => {
                    daw_unit_core::DRIVE_V2_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Drive(_) => daw_unit_core::DRIVE_TYPE_ID.hash(&mut hash),
                daw_core::EffectSettings::AutoFilter(_) => {
                    daw_unit_core::AUTO_FILTER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Filter(_) => {
                    daw_unit_core::FILTER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Compressor(_) => {
                    daw_unit_core::COMPRESSOR_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Ott(_) => daw_unit_core::OTT_TYPE_ID.hash(&mut hash),
                daw_core::EffectSettings::Limiter(_) => {
                    daw_unit_core::LIMITER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::StereoSpread(_) => {
                    daw_unit_core::STEREO_SPREAD_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Saturator(_) => {
                    daw_unit_core::SATURATOR_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Bitcrusher(_) => {
                    daw_unit_core::BITCRUSHER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Chorus(_) => {
                    daw_unit_core::CHORUS_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::AmbientReverb(_) => {
                    daw_unit_core::AMBIENT_REVERB_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::ShimmerReverb(_) => {
                    daw_unit_core::SHIMMER_REVERB_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::PlateReverb(_) => {
                    daw_unit_core::PLATE_REVERB_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::DirectionMixer(_) => {
                    daw_unit_core::DIRECTION_MIXER_TYPE_ID.hash(&mut hash)
                }
                daw_core::EffectSettings::Missing { type_id, .. } => type_id.hash(&mut hash),
            }
        }
    }
    hash.finish()
}

fn owners(project: &Project) -> Vec<(Owner, &[EffectInstance])> {
    let mut out = vec![(Owner::Master, project.master.effects.as_slice())];
    for (t, track) in project.tracks.iter().enumerate() {
        out.push((Owner::Track(t), &track.effects));
        if let Some(instrument) = &track.instrument {
            // MidiTrackRuntime stores buses in canonical ID order, independently of
            // the saved row order. Use that same order for every prepared slot reference.
            let mut layers: Vec<_> = instrument.layers.iter().collect();
            layers.sort_by_key(|layer| layer.id);
            for (l, layer) in layers.into_iter().enumerate() {
                out.push((Owner::Layer(t, l), &layer.effects));
            }
            out.push((
                Owner::Layer(t, instrument.layers.len()),
                &instrument.main.effects,
            ));
        }
    }
    out
}

fn clear_effects(project: &mut Project) {
    project.master.effects.clear();
    project.master.effects_compact = false;
    for track in &mut project.tracks {
        track.effects.clear();
        track.effects_compact = false;
        if let Some(instrument) = &mut track.instrument {
            for layer in &mut instrument.layers {
                layer.effects.clear();
                layer
                    .component_order
                    .retain(|c| !matches!(c, daw_core::InstrumentComponent::Effect(_)));
            }
            instrument.main.effects.clear();
            instrument
                .main
                .component_order
                .retain(|c| !matches!(c, daw_core::InstrumentComponent::Effect(_)));
        }
    }
}

fn route_topology(a: &EffectSettings, b: &EffectSettings) -> bool {
    a.modulations().len() == b.modulations().len()
        && a.modulations().iter().zip(b.modulations()).all(|(a, b)| {
            let mut a = *a;
            a.amount = b.amount;
            a.enabled = b.enabled;
            a == *b
        })
}

// Presentation flags never retarget DSP. This comparison runs only on workers.
fn controls_changed(a: &EffectSettings, b: &EffectSettings) -> bool {
    macro_rules! compare {
        ($a:expr, $b:expr) => {{
            let mut a = $a.clone();
            a.collapsed = $b.collapsed;
            a.modulation_open = $b.modulation_open;
            a != *$b
        }};
    }
    match (a, b) {
        (EffectSettings::Eq3(a), EffectSettings::Eq3(b)) => compare!(a, b),
        (EffectSettings::Drive(a), EffectSettings::Drive(b)) => compare!(a, b),
        (EffectSettings::DriveV2(a), EffectSettings::DriveV2(b)) => compare!(a, b),
        (EffectSettings::AutoFilter(a), EffectSettings::AutoFilter(b)) => compare!(a, b),
        (EffectSettings::Filter(a), EffectSettings::Filter(b)) => compare!(a, b),
        (EffectSettings::Compressor(a), EffectSettings::Compressor(b)) => compare!(a, b),
        (EffectSettings::Ott(a), EffectSettings::Ott(b)) => compare!(a, b),
        (EffectSettings::Limiter(a), EffectSettings::Limiter(b)) => compare!(a, b),
        (EffectSettings::DirectionMixer(a), EffectSettings::DirectionMixer(b)) => compare!(a, b),
        (EffectSettings::StereoSpread(a), EffectSettings::StereoSpread(b)) => compare!(a, b),
        (EffectSettings::Saturator(a), EffectSettings::Saturator(b)) => compare!(a, b),
        (EffectSettings::Bitcrusher(a), EffectSettings::Bitcrusher(b)) => compare!(a, b),
        (EffectSettings::Chorus(a), EffectSettings::Chorus(b)) => compare!(a, b),
        (EffectSettings::AmbientReverb(a), EffectSettings::AmbientReverb(b)) => compare!(a, b),
        (EffectSettings::ShimmerReverb(a), EffectSettings::ShimmerReverb(b)) => compare!(a, b),
        (EffectSettings::PlateReverb(a), EffectSettings::PlateReverb(b)) => compare!(a, b),
        (EffectSettings::HarmonicLift(a), EffectSettings::HarmonicLift(b)) => compare!(a, b),
        (EffectSettings::MultibandCompressor(a), EffectSettings::MultibandCompressor(b)) => {
            compare!(a, b)
        }
        (EffectSettings::Exciter(a), EffectSettings::Exciter(b)) => compare!(a, b),
        (EffectSettings::MultibandEq(a), EffectSettings::MultibandEq(b)) => compare!(a, b),
        (EffectSettings::Loudness(a), EffectSettings::Loudness(b)) => {
            let mut a = a.clone();
            a.collapsed = b.collapsed;
            a != *b
        }
        (EffectSettings::Missing { .. }, EffectSettings::Missing { .. }) => false,
        _ => true,
    }
}

fn reusable(a: &EffectSettings, b: &EffectSettings) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
        && a.band_ids().eq(b.band_ids())
        && route_topology(a, b)
}

impl EffectUpdate {
    /// Return None before processor construction when non-effect content, timing or
    /// control dependencies require the existing exact renderer reconstruction.
    pub fn prepare(previous: &RenderPlan, desired: RenderPlan) -> Option<Self> {
        Self::prepare_with_reason(previous, desired).ok()
    }
    pub fn prepare_with_reason(
        previous: &RenderPlan,
        desired: RenderPlan,
    ) -> Result<Self, EffectUpdateFallback> {
        let previous = previous.clone().prepare_beats();
        let desired = desired.prepare_beats();
        let mut a = previous.clone();
        let mut b = desired.clone();
        clear_effects(&mut a.project);
        clear_effects(&mut b.project);
        if !a.same_processing_plan(&b) {
            return Err(EffectUpdateFallback::Arrangement);
        }
        let old = owners(&previous.project);
        let next = owners(&desired.project);
        let rate = desired.project.sample_rate_hz;
        if old.len() != next.len()
            || old.iter().zip(&next).any(|((o, a), (n, b))| {
                o != n
                    || super::latency::chain_frames(a, rate)
                        != super::latency::chain_frames(b, rate)
            })
        {
            return Err(EffectUpdateFallback::Latency);
        }
        // Per-owner delays above preserve layer distribution and every bus/send delay.
        // Sidechain destination stages must also match; graph equality alone is insufficient.
        let mut old_specs = super::audio_sidechain::specs(&previous.project);
        let mut next_specs = super::audio_sidechain::specs(&desired.project);
        old_specs.sort_by_key(|s| s.route.id);
        next_specs.sort_by_key(|s| s.route.id);
        if old_specs.len() != next_specs.len()
            || old_specs.iter().zip(&next_specs).any(|(a, b)| {
                let mut route = a.route;
                route.amount = b.route.amount;
                route.enabled = b.route.enabled;
                a.destination != b.destination || a.stage != b.stage || route != b.route
            })
        {
            return Err(EffectUpdateFallback::Sidechain);
        }
        // Match globally so atomic cross-owner moves retain the same processor.
        let by_id: HashMap<_, _> = old
            .iter()
            .flat_map(|(owner, effects)| {
                effects
                    .iter()
                    .enumerate()
                    .map(move |(slot, e)| (e.id, (*owner, slot, e)))
            })
            .collect();
        let structural_work: usize = old
            .iter()
            .zip(&next)
            .filter(|((_, before), (_, after))| {
                before != after
                    && !(before.len() == after.len()
                        && before
                            .iter()
                            .zip(*after)
                            .all(|(a, b)| a.id == b.id && reusable(&a.unit, &b.unit)))
            })
            .map(|(_, (_, after))| after.len())
            .sum();
        // Bound publication work before constructing any candidate processors.
        if structural_work > 4096 {
            return Err(EffectUpdateFallback::PublicationBudget);
        }
        let mut chains = Vec::new();
        let mut prepared_processors = 0;
        for ((_, before), (owner, after)) in old.iter().zip(&next) {
            if before == after {
                continue;
            }
            let reuse: HashSet<_> = after
                .iter()
                .filter(|e| {
                    by_id
                        .get(&e.id)
                        .is_some_and(|(_, _, old)| reusable(&old.unit, &e.unit))
                })
                .map(|e| e.id)
                .collect();
            prepared_processors += after
                .iter()
                .filter(|e| {
                    !reuse.contains(&e.id) && !matches!(e.unit, EffectSettings::Missing { .. })
                })
                .count();
            let direct_slots = (before.len() == after.len()
                && before
                    .iter()
                    .zip(*after)
                    .all(|(a, b)| a.id == b.id && reuse.contains(&b.id)))
            .then(|| {
                before
                    .iter()
                    .zip(*after)
                    .enumerate()
                    .filter_map(|(i, (a, b))| controls_changed(&a.unit, &b.unit).then_some(i))
                    .collect()
            });
            let slots = after
                .iter()
                .map(|e| {
                    by_id.get(&e.id).map(|(owner, slot, before)| {
                        (*owner, *slot, controls_changed(&before.unit, &e.unit))
                    })
                })
                .collect();
            chains.push(ChainUpdate {
                owner: *owner,
                chain: if direct_slots.is_some() {
                    EffectChain::chain(None, &[], rate, desired.project.buffer_size_frames)
                } else {
                    EffectChain::prepared(after, rate, desired.project.buffer_size_frames, &reuse)
                },
                settings: after.to_vec(),
                slots,
                direct_slots,
            });
        }
        let effect_tracks = next
            .iter()
            .flat_map(|(owner, effects)| {
                let track = match owner {
                    Owner::Master => None,
                    Owner::Track(t) | Owner::Layer(t, _) => Some(*t),
                };
                effects.iter().map(move |e| (e.id, track))
            })
            .collect();
        let audible = desired.project.track_mix_audible();
        let gain_targets = desired
            .project
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let [mono, stereo] = super::track_processing::track_pan_coefficients(t);
                let gate = if audible[i] {
                    daw_core::linear_gain(t.gain_db)
                } else {
                    0.0
                };
                [
                    mono[0] * gate,
                    mono[1] * gate,
                    stereo[0] * gate,
                    stereo[1] * gate,
                ]
            })
            .collect();
        let end_frame = desired.project.end().max(
            desired
                .beat_tracks
                .values()
                .map(|b| b.logical_end_frame)
                .max()
                .unwrap_or(0),
        );
        Ok(Self {
            fingerprint: fingerprint(&desired),
            plan: desired,
            chains,
            effect_tracks,
            gain_targets,
            end_frame,
            prepared_processors,
        })
    }

    /// Apply eligible controls to the active topology while a structural candidate is
    /// pending. New/removed units and incompatible controls wait for the complete edit.
    pub fn prepare_controls(previous: &RenderPlan, desired: &RenderPlan) -> Option<Self> {
        let mut hybrid = previous.clone().prepare_beats();
        let desired = desired.clone().prepare_beats();
        let mut a = hybrid.clone();
        let mut b = desired.clone();
        clear_effects(&mut a.project);
        clear_effects(&mut b.project);
        if !a.same_processing_plan(&b) {
            return None;
        }
        let settings: HashMap<_, _> = desired
            .project
            .all_effects()
            .map(|e| (e.id, &e.unit))
            .collect();
        let mut changed = false;
        let mut update = |effects: &mut Vec<EffectInstance>| {
            for effect in effects {
                if let Some(next) = settings.get(&effect.id)
                    && effect.unit != **next
                    && reusable(&effect.unit, next)
                {
                    effect.unit = (*next).clone();
                    changed = true;
                }
            }
        };
        update(&mut hybrid.project.master.effects);
        for track in &mut hybrid.project.tracks {
            update(&mut track.effects);
            if let Some(instrument) = &mut track.instrument {
                for layer in &mut instrument.layers {
                    update(&mut layer.effects);
                }
                update(&mut instrument.main.effects);
            }
        }
        changed.then(|| Self::prepare(previous, hybrid)).flatten()
    }

    /// Number of new/incompatible DSP processors actually constructed by this transaction.
    pub fn prepared_processors(&self) -> usize {
        self.prepared_processors
    }

    /// New metadata before publication; old metadata afterwards. Borrow only off audio.
    pub fn plan(&self) -> &RenderPlan {
        &self.plan
    }

    /// Bind worker-prepared telemetry. Reused processors keep their existing handles.
    pub fn bind_meters(
        &mut self,
        reductions: &HashMap<Id, std::sync::Arc<daw_unit_core::PeakMeter>>,
        outputs: &HashMap<Id, std::sync::Arc<daw_unit_core::BandMeter<3>>>,
        bands: &HashMap<super::ModulationTarget, std::sync::Arc<daw_unit_core::BandMeter<3>>>,
        loudness: &HashMap<Id, std::sync::Arc<daw_unit_core::LoudnessMeter>>,
        spectra: &HashMap<Id, std::sync::Arc<daw_unit_core::SpectrumMeter>>,
        solos: &HashMap<super::ModulationTarget, std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) {
        for c in &mut self.chains {
            for (id, meter) in reductions {
                c.chain.set_meter(*id, meter);
            }
            for (id, meter) in outputs {
                c.chain.set_output_meter(*id, meter);
            }
            for (id, meter) in loudness {
                c.chain.set_loudness_meter(*id, meter);
            }
            for (id, meter) in spectra {
                c.chain.set_spectrum_meter(*id, meter);
            }
            for (target, meter) in bands {
                match *target {
                    super::ModulationTarget::Effect(id) => c.chain.set_band_meter(id, meter),
                    super::ModulationTarget::EffectBand { effect, band } => {
                        c.chain.set_band_level_meter(effect, band, meter)
                    }
                    _ => {}
                }
            }
            for (target, flag) in solos {
                if let super::ModulationTarget::EffectBand { effect, band } = *target {
                    c.chain.bind_exciter_solo(effect, band, flag);
                }
            }
        }
    }

    pub fn heap_bytes(&self) -> u64 {
        self.plan.model_bytes()
            + daw_unit_core::memory::vec_bytes(&self.chains)
            + self
                .chains
                .iter()
                .map(|c| {
                    c.chain.heap_bytes()
                        + daw_unit_core::memory::effect_settings_bytes(&c.settings)
                        + daw_unit_core::memory::vec_bytes(&c.slots)
                        + c.direct_slots
                            .as_ref()
                            .map_or(0, daw_unit_core::memory::vec_bytes)
                })
                .sum::<u64>()
            + daw_unit_core::memory::map_bytes(&self.effect_tracks)
            + daw_unit_core::memory::vec_bytes(&self.gain_targets)
    }
}

impl Renderer {
    fn effect_chain_mut(&mut self, owner: Owner) -> &mut EffectChain {
        match owner {
            Owner::Master => &mut self.master_effects,
            Owner::Track(t) => &mut self.effects[t],
            Owner::Layer(t, l) => {
                &mut self.midi[t]
                    .as_mut()
                    .expect("prepared instrument")
                    .layer_effects[l]
                    .chain
            }
        }
    }

    /// Apply only after the host verifies the transaction's base processing revision.
    /// All destinations are allocated and all references validated by prepare().
    /// Old owners stay in the transaction until the host retires it off audio.
    pub fn apply_effect_update(&mut self, update: &mut EffectUpdate) {
        for chain in &mut update.chains {
            if let Some(slots) = &chain.direct_slots {
                let current = self.effect_chain_mut(chain.owner);
                for &i in slots {
                    if let Some(unit) = &mut current.units[i].1 {
                        unit.retarget(&chain.settings[i].unit);
                    }
                    let enabled = match &chain.settings[i].unit {
                        EffectSettings::Eq3(s) => s.enabled,
                        EffectSettings::DirectionMixer(s) => s.enabled,
                        EffectSettings::StereoSpread(s) => s.enabled,
                        _ => continue,
                    };
                    if let Some(tap) = &mut current.output_taps[i] {
                        tap.set_enabled(enabled);
                    }
                }
                continue;
            }
            for (i, source) in chain.slots.iter().enumerate() {
                let Some((owner, slot, changed)) = source else {
                    continue;
                };
                let previous = self.effect_chain_mut(*owner);
                if chain.chain.units[i].1.is_none() {
                    chain.chain.units[i].1 = previous.units[*slot].1.take();
                    if *changed && let Some(unit) = &mut chain.chain.units[i].1 {
                        unit.retarget(&chain.settings[i].unit);
                    }
                } else if let (Some(next), Some(old)) =
                    (&mut chain.chain.units[i].1, &mut previous.units[*slot].1)
                {
                    EffectChain::inherit_unit(next, old, &chain.settings[i].unit);
                }
                if let (Some(old), Some(next)) = (
                    &mut previous.output_taps[*slot],
                    &mut chain.chain.output_taps[i],
                ) {
                    old.retarget(next);
                    std::mem::swap(old, next);
                }
            }
        }
        for chain in &mut update.chains {
            if chain.direct_slots.is_none() {
                let previous = self.effect_chain_mut(chain.owner);
                chain.chain.mono = previous.mono;
                std::mem::swap(previous, &mut chain.chain);
            }
            if let Owner::Layer(t, l) = chain.owner {
                std::mem::swap(
                    &mut self.midi[t].as_mut().unwrap().layer_effects[l].settings,
                    &mut chain.settings,
                );
            }
        }
        if self.effect_fingerprint != update.fingerprint {
            self.transition_from = self.previous;
            self.transition = (self.plan.project.sample_rate_hz / 200).max(1);
            self.effect_fingerprint = update.fingerprint;
        }
        std::mem::swap(&mut self.plan, &mut update.plan);
        std::mem::swap(&mut self.effect_tracks, &mut update.effect_tracks);
        self.end_frame = update.end_frame;
        for (i, runtime) in self.midi.iter_mut().enumerate() {
            if let Some(runtime) = runtime {
                runtime.refresh_effect_metadata(&self.plan.project.tracks[i]);
            }
        }
        for (gain, target) in self.gains.iter_mut().zip(&update.gain_targets) {
            if gain.target != *target {
                gain.target = *target;
                gain.remaining = (self.plan.project.sample_rate_hz / 200).max(1);
            }
        }
    }
}
