use super::*;
use std::hash::BuildHasher;
static ROUTE_HASH: std::sync::OnceLock<std::collections::hash_map::RandomState> =
    std::sync::OnceLock::new();
fn route_bucket(route: &ModulationRoute, count: usize) -> usize {
    (ROUTE_HASH
        .get()
        .expect("prepared route hash seed")
        .hash_one((route.settings.id, route.settings.target))
        % count as u64) as usize
}

#[derive(Clone, Copy)]
/// Shared finite linear ramp for control values and contribution weights.
pub struct ModulationRamp {
    value: f32,
    target: f32,
    left: u32,
}
impl ModulationRamp {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            left: 0,
        }
    }
    pub fn set(&mut self, value: f32, frames: u32) {
        if self.target != value {
            self.target = value;
            self.left = frames;
        }
    }
    pub fn next_value(&mut self) -> f32 {
        if self.left > 0 {
            self.value += (self.target - self.value) / self.left as f32;
            self.left -= 1;
        }
        self.value
    }
    pub fn target(&self) -> f32 {
        self.target
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.left = 0;
    }
}

#[derive(Clone, Copy)]
struct SourceBinding {
    source: ModulationSourceId,
    local_generation: u64,
    shared_generation: u64,
    local_slot: Option<usize>,
    shared_slot: Option<usize>,
}

/// Prepared route with a shared 5 ms amount/power ramp. Retarget by stable route ID.
pub struct ModulationRoute {
    pub settings: Modulation,
    amount: ModulationRamp,
    frames: u32,
    binding: Option<SourceBinding>,
    local_slot: Option<usize>,
    shared_slot: Option<usize>,
    // Intrusive scratch table for allocation-free history matching. Never read while sampling.
    match_head: usize,
    match_next: usize,
}
impl ModulationRoute {
    pub fn new(settings: Modulation, rate: u32) -> Self {
        ROUTE_HASH.get_or_init(std::collections::hash_map::RandomState::new);
        Self {
            match_head: usize::MAX,
            match_next: usize::MAX,
            settings,
            binding: None,
            local_slot: None,
            shared_slot: None,
            amount: ModulationRamp::new(if settings.enabled && settings.valid() {
                settings.amount
            } else {
                0.0
            }),
            frames: (rate / 200).max(1),
        }
    }
    pub fn reset(&mut self) {
        self.amount.reset();
    }
    pub fn retarget(&mut self, next: &Self) {
        self.settings = next.settings;
        self.amount.set(next.amount.target, self.frames);
    }
    fn value<'a>(
        &mut self,
        local: &'a [ModulationValue],
        shared: &'a [ModulationValue],
        context: ModulationContext,
    ) -> Option<(&'a ModulationValue, bool)> {
        let source = self.settings.source?;
        if let Some(shared_index) = context.shared_sources
            && (context.local_sources.is_some() || local.is_empty())
        {
            let local_generation = context.local_sources.map_or(0, |i| i.generation);
            if self.binding.is_none_or(|b| {
                (b.source, b.local_generation, b.shared_generation)
                    != (source, local_generation, shared_index.generation)
            }) {
                self.binding = Some(SourceBinding {
                    source,
                    local_generation,
                    shared_generation: shared_index.generation,
                    local_slot: context.local_sources.and_then(|i| i.position(source)),
                    shared_slot: shared_index.position(source),
                });
            }
            let SourceBinding {
                local_slot,
                shared_slot,
                ..
            } = self.binding?;
            if let Some(value) = local_slot
                .and_then(|i| local.get(i))
                .filter(|v| v.source == source)
            {
                return Some((value, true));
            }
            return shared_slot
                .and_then(|i| shared.get(i))
                .filter(|v| v.source == source)
                .map(|v| (v, false));
        }
        // Slice-only clients remain supported. Validate cached hits and retain local
        // precedence even when callers replace or reorder their publication buffers.
        if self
            .local_slot
            .and_then(|i| local.get(i))
            .is_none_or(|v| v.source != source)
        {
            self.local_slot = local.iter().position(|v| v.source == source);
        }
        if let Some(i) = self.local_slot {
            return Some((&local[i], true));
        }
        if self
            .shared_slot
            .and_then(|i| shared.get(i))
            .is_none_or(|v| v.source != source)
        {
            self.shared_slot = shared.iter().position(|v| v.source == source);
        }
        self.shared_slot.map(|i| (&shared[i], false))
    }
    pub fn sample(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
    ) -> Option<(f32, f32, bool, ModulationRange, ModulationBehavior)> {
        self.sample_context(local, shared, ModulationContext::default())
            .map(|(v, a, p, r, b, _)| (v, a, p, r, b))
    }
    fn sample_context(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> Option<(f32, f32, bool, ModulationRange, ModulationBehavior, bool)> {
        let amount = self.amount.next_value();
        let (value, per_note) = self.value(local, shared, context)?;
        if !amount.is_finite()
            || amount == 0.0
            || !value.value.is_finite()
            || !value.weight.is_finite()
            || value.weight <= 0.0
        {
            None
        } else {
            Some((
                value.value,
                amount * value.weight.min(1.0),
                per_note,
                value.range,
                value.behavior,
                value.owns_release(context.release_age),
            ))
        }
    }
}

/// Typed, stack-owned destination inputs. Additive targets use zero as neutral;
/// Amplitude uses the note gate with no routes. Resolve once for each destination sample.
#[derive(Clone, Copy, Debug)]
pub struct ModulationInputs {
    offsets: [f32; 65],
    amplitude: f32,
}
impl ModulationInputs {
    pub fn neutral(note_gate: f32) -> Self {
        Self {
            offsets: [0.0; 65],
            amplitude: note_gate,
        }
    }
    pub fn offset(&self, parameter: ModulationParameter) -> f32 {
        parameter.offset_index().map_or(0.0, |i| self.offsets[i])
    }
    pub fn offsets<const N: usize>(&self, parameters: [ModulationParameter; N]) -> [f32; N] {
        parameters.map(|p| self.offset(p))
    }
    pub fn amplitude(&self) -> f32 {
        self.amplitude
    }
    pub fn resolve(
        routes: &mut [ModulationRoute],
        parameters: &[ModulationParameter],
        local: &[ModulationValue],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> Self {
        Self::resolve_target(routes, parameters, None, local, shared, context)
    }
    pub fn resolve_target(
        routes: &mut [ModulationRoute],
        parameters: &[ModulationParameter],
        target: Option<ModulationTargetId>,
        local: &[ModulationValue],
        shared: &[ModulationValue],
        context: ModulationContext,
    ) -> Self {
        let gate = context.note_gate;
        let mut inputs = Self::neutral(gate);
        let mut per_note_delta = [0.0; 2];
        let mut shared_delta = [0.0; 2];
        let mut shared_release = false;
        let mut direct = None;
        let mut effective = 0;
        let mut continuous = 0.0;
        for route in routes {
            // Advance every route ramp once even if a caller supplies an unsupported target.
            let sample = route.sample_context(local, shared, context);
            if route.settings.target != target || !parameters.contains(&route.settings.parameter) {
                continue;
            }
            if let Some((value, amount, per_note, range, behavior, owns_release)) = sample {
                if let Some(index) = route.settings.parameter.offset_index() {
                    inputs.offsets[index] += value * amount;
                    continue;
                }
                if behavior == ModulationBehavior::Continuous {
                    // Gain modulation uses a normalized cycle; additive destinations retain polarity.
                    let normalized = match range {
                        ModulationRange::Unipolar => value,
                        ModulationRange::Bipolar => (value + 1.0) * 0.5,
                    };
                    continuous += amount * (normalized - 1.0);
                    continue;
                }
                let deltas = if per_note {
                    &mut per_note_delta
                } else {
                    &mut shared_delta
                };
                deltas[0] += amount * (value - gate);
                deltas[1] += amount * (value - 1.0);
                if !per_note && value > 0.0 {
                    shared_release |= owns_release;
                }
                effective += 1;
                direct = Some((value, amount, per_note));
            }
        }
        inputs.amplitude = if let Some((value, 1.0, true)) = direct.filter(|_| effective == 1) {
            // Preserve legacy full-depth ADSR arithmetic exactly.
            value.clamp(0.0, 16.0)
        } else {
            // Only final released voices carry a shared tail; earlier voices must not revive.
            let (shaped, shared_factor) = if shared_release {
                (1.0 + per_note_delta[1], gate + shared_delta[0])
            } else {
                (gate + per_note_delta[0], 1.0 + shared_delta[1])
            };
            shaped.clamp(0.0, 16.0) * shared_factor.clamp(0.0, 16.0)
        };
        if continuous != 0.0 {
            inputs.amplitude *= (1.0 + continuous).clamp(0.0, 16.0);
        }
        inputs
    }
}
impl ModulationParameter {
    fn offset_index(self) -> Option<usize> {
        match self {
            Self::EnvelopeAttack => Some(56),
            Self::EnvelopeDecay => Some(57),
            Self::EnvelopeSustain => Some(58),
            Self::EnvelopeRelease => Some(59),
            Self::EnvelopeAttackCurve => Some(60),
            Self::EnvelopeDecayCurve => Some(61),
            Self::EnvelopeReleaseCurve => Some(62),
            Self::EnvelopeAttackNote => Some(63),
            Self::EnvelopeDecayNote => Some(64),
            Self::OttLowFrequency => Some(30),
            Self::OttHighFrequency => Some(31),
            Self::OttLowDepth => Some(32),
            Self::OttLowInput => Some(33),
            Self::OttLowOutput => Some(34),
            Self::OttLowAttack => Some(35),
            Self::OttLowRelease => Some(36),
            Self::OttMidDepth => Some(37),
            Self::OttMidInput => Some(38),
            Self::OttMidOutput => Some(39),
            Self::OttMidAttack => Some(40),
            Self::OttMidRelease => Some(41),
            Self::OttHighDepth => Some(42),
            Self::OttHighInput => Some(43),
            Self::OttHighOutput => Some(44),
            Self::OttHighAttack => Some(45),
            Self::OttHighRelease => Some(46),

            Self::SamplerGain => Some(25),
            Self::DirectionMixerDirection => Some(47),
            Self::DirectionMixerSpread => Some(48),
            Self::BitcrusherDrive => Some(53),
            Self::BitcrusherResolution => Some(54),
            Self::BitcrusherDownsampling => Some(55),
            Self::StereoSpreadLowerIntensity => Some(49),
            Self::StereoSpreadUpperIntensity => Some(50),
            Self::StereoSpreadLowerFrequency => Some(51),
            Self::StereoSpreadUpperFrequency => Some(52),
            Self::LimiterInput => Some(26),
            Self::LimiterRelease => Some(27),
            Self::LimiterLookahead => Some(28),
            Self::LimiterCeiling => Some(29),
            Self::LfoDepth => Some(10),
            Self::LfoRate => Some(11),
            Self::MacroValue => Some(12),
            Self::CompressorThreshold => Some(20),
            Self::CompressorRatio => Some(21),
            Self::CompressorAttack => Some(22),
            Self::CompressorRelease => Some(23),
            Self::CompressorKnee => Some(24),
            Self::EqLowGain => Some(13),
            Self::EqLowFrequency => Some(14),
            Self::EqLowQ => Some(15),
            Self::EqMidGain => Some(16),
            Self::EqHighGain => Some(17),
            Self::EqHighFrequency => Some(18),
            Self::EqHighQ => Some(19),
            Self::Amplitude => None,
            Self::Pitch => Some(9),
            Self::Frequency => Some(0),
            Self::Resonance => Some(1),
            Self::Level => Some(2),
            Self::DryWet => Some(3),
            Self::Volume => Some(4),
            Self::DriveGain => Some(5),
            Self::Tone => Some(6),
            Self::ColorStrength => Some(7),
            Self::ColorFrequency => Some(8),
        }
    }
}

/// Shared 1 ms smoothing for additive parameter offsets. Nonfinite inputs become neutral.
/// f64 state preserves the existing filter and drive response at every project rate.
pub struct ModulationSmoother<const N: usize> {
    values: [f64; N],
    alpha: f64,
}
impl<const N: usize> ModulationSmoother<N> {
    pub fn new(context: crate::ProcessingContext) -> Self {
        Self {
            values: [0.0; N],
            alpha: 1.0 - (-1.0 / (0.001 * f64::from(context.sample_rate_hz))).exp(),
        }
    }
    pub fn reset(&mut self) {
        self.values = [0.0; N];
    }
    pub fn process(&mut self, offsets: [f32; N]) -> [f64; N] {
        for (value, target) in self.values.iter_mut().zip(offsets) {
            let target = if target.is_finite() {
                f64::from(target)
            } else {
                0.0
            };
            *value += self.alpha * (target - *value);
        }
        self.values
    }
}

/// Standard entry point for all modulated units: resolve cached controls, then process audio.
/// Prepare routes and source buffers before calling; this function performs no allocation.
pub fn process_modulation<D: ModulationDestination>(
    destination: &mut D,
    routes: &mut [ModulationRoute],
    input: D::Audio,
    local: &[ModulationValue],
    shared: &[ModulationValue],
    context: ModulationContext,
) -> D::Audio {
    let local = if destination
        .modulation_input_scope()
        .accepts(ModulationScope::PerNote)
    {
        local
    } else {
        &[]
    };
    let inputs = ModulationInputs::resolve_target(
        routes,
        D::MODULATION_PARAMETERS,
        destination.modulation_target(),
        local,
        shared,
        context,
    );
    destination.process_with_modulation(input, &inputs, context)
}

/// Prepare one repeated destination on the control thread. Each route ramp advances once per sample.
pub fn prepare_target_routes(
    routes: &[Modulation],
    target: ModulationTargetId,
    rate: u32,
) -> Vec<ModulationRoute> {
    routes
        .iter()
        .filter(|r| r.target == Some(target))
        .map(|r| ModulationRoute::new(*r, rate))
        .collect()
}
/// Move compatible route history into prepared storage without allocation or destruction.
pub fn retarget_modulation_routes(
    current: &mut Vec<ModulationRoute>,
    prepared: &mut Vec<ModulationRoute>,
) {
    if current.len() <= 16 {
        // Avoid table setup for the common small connection strip.
        for next in prepared.iter_mut() {
            if let Some(route) = current.iter_mut().find(|r| {
                (r.settings.id, r.settings.target) == (next.settings.id, next.settings.target)
            }) {
                route.retarget(next);
                std::mem::swap(route, next);
            }
        }
    } else {
        let count = current.len();
        for route in current.iter_mut() {
            route.match_head = usize::MAX;
        }
        for i in 0..count {
            let bucket = route_bucket(&current[i], count);
            current[i].match_next = current[bucket].match_head;
            current[bucket].match_head = i;
        }
        for next in prepared.iter_mut() {
            let mut index = current[route_bucket(next, count)].match_head;
            while index != usize::MAX {
                let route = &mut current[index];
                if (route.settings.id, route.settings.target)
                    == (next.settings.id, next.settings.target)
                {
                    route.retarget(next);
                    let (head, link) = (route.match_head, route.match_next);
                    std::mem::swap(route, next);
                    // A bucket can occupy the matched slot. Preserve its links after the swap.
                    current[index].match_head = head;
                    current[index].match_next = link;
                    break;
                }
                index = route.match_next;
            }
        }
    }
    std::mem::swap(current, prepared);
}

#[cfg(test)]
mod matching_tests {
    use super::*;
    #[test]
    fn history_table_handles_collisions_removals_and_incoming_order() {
        let source = ModulationSourceId {
            layer: uuid::Uuid::new_v4(),
            unit: uuid::Uuid::new_v4(),
        };
        let mut current = Vec::new();
        // Force a collision chain independently of the process-randomized hash seed.
        while current.len() < 32 {
            let mut settings = Modulation::new(ModulationParameter::Level);
            settings.source = Some(source);
            settings.amount = 0.2 + current.len() as f32 * 0.01;
            let route = ModulationRoute::new(settings, 48_000);
            if route_bucket(&route, 32) == 0 {
                current.push(route);
            }
        }
        let expected: Vec<_> = current
            .iter()
            .rev()
            .step_by(2)
            .map(|r| (r.settings.id, r.settings.amount))
            .collect();
        let mut prepared: Vec<_> = current
            .iter()
            .rev()
            .step_by(2)
            .map(|r| {
                let mut settings = r.settings;
                settings.amount = 0.8;
                ModulationRoute::new(settings, 48_000)
            })
            .collect();
        let missing = Modulation::new(ModulationParameter::Level);
        prepared.push(ModulationRoute::new(missing, 48_000));
        retarget_modulation_routes(&mut current, &mut prepared);
        let value = ModulationValue {
            source,
            value: 0.5,
            weight: 1.0,
            range: ModulationRange::Unipolar,
            behavior: ModulationBehavior::Continuous,
            release_age: None,
        };
        for (route, (id, previous)) in current.iter_mut().zip(expected) {
            assert_eq!(route.settings.id, id);
            let amount = route.sample(&[], &[value]).unwrap().1;
            assert!((amount - (previous + (0.8 - previous) / 240.0)).abs() < 1e-6);
        }
        assert_eq!(current.last().unwrap().settings.id, missing.id);
    }
}
