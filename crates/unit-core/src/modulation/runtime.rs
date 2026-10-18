use super::*;

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

/// Prepared route with a shared 5 ms amount/power ramp. Retarget by stable route ID.
pub struct ModulationRoute {
    pub settings: Modulation,
    amount: ModulationRamp,
    frames: u32,
}
impl ModulationRoute {
    pub fn new(settings: Modulation, rate: u32) -> Self {
        Self {
            settings,
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
    pub fn sample(
        &mut self,
        local: &[ModulationValue],
        shared: &[ModulationValue],
    ) -> Option<(f32, f32, bool, ModulationRange, ModulationBehavior)> {
        let amount = self.amount.next_value();
        let source = self.settings.source?;
        let (value, per_note) = if let Some(v) = local.iter().find(|v| v.source == source) {
            (v, true)
        } else {
            (shared.iter().find(|v| v.source == source)?, false)
        };
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
            ))
        }
    }
}

/// Typed, stack-owned destination inputs. Additive targets use zero as neutral;
/// Amplitude uses the note gate with no routes. Resolve once for each destination sample.
#[derive(Clone, Copy, Debug)]
pub struct ModulationInputs {
    offsets: [f32; 56],
    amplitude: f32,
}
impl ModulationInputs {
    pub fn neutral(note_gate: f32) -> Self {
        Self {
            offsets: [0.0; 56],
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
            let sample = route.sample(local, shared);
            if route.settings.target != target || !parameters.contains(&route.settings.parameter) {
                continue;
            }
            if let Some((value, amount, per_note, range, behavior)) = sample {
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
                    shared_release |= shared.iter().any(|v| {
                        Some(v.source) == route.settings.source
                            && v.owns_release(context.release_age)
                    });
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
    let local = if D::MODULATION_SCOPE.accepts(ModulationScope::PerNote) {
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
    for next in prepared.iter_mut() {
        if let Some(route) = current.iter_mut().find(|r| {
            r.settings.id == next.settings.id && r.settings.target == next.settings.target
        }) {
            route.retarget(next);
            std::mem::swap(route, next);
        }
    }
    std::mem::swap(current, prepared);
}
