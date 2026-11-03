//! Procedural stereo noise. All state is prepared, fixed-size and deterministic.
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, ModulationSmoother, NoiseSettings, NoiseType, PortKind,
    ProcessingContext, StereoSourceUnit, UnitDescriptor,
};
const BANDS: usize = 11;
#[derive(Clone)]
struct Generator {
    random: u64,
    filters: [f64; BANDS],
    tone: f64,
}
impl Generator {
    fn new(seed: u64) -> Self {
        Self {
            random: seed.max(1),
            filters: [0.0; BANDS],
            tone: 0.0,
        }
    }
    fn white(&mut self) -> f64 {
        self.random ^= self.random >> 12;
        self.random ^= self.random << 25;
        self.random ^= self.random >> 27;
        let bits = self.random.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (bits >> 11) as f64 * (2.0 / ((1_u64 << 53) as f64)) - 1.0
    }
    fn sample(&mut self, coefficients: &[f64; BANDS], pink: &[f64; BANDS], brown: f64) -> [f64; 3] {
        let white = self.white();
        let mut p = 0.0;
        for (i, state) in self.filters.iter_mut().enumerate() {
            *state = white + coefficients[i] * (*state - white);
            p += *state * pink[i];
        }
        [white, p, self.filters[0] * brown]
    }
}
#[derive(Clone)]
pub struct Noise {
    id: u128,
    generators: [Generator; 2],
    coefficients: [f64; BANDS],
    pink: [f64; BANDS],
    brown: f64,
    tone_coefficient: f64,
    weights: [f64; 3],
    target_weights: [f64; 3],
    type_remaining: u32,
    controls: [f64; 3],
    targets: [f64; 3],
    remaining: u32,
    fade: u32,
    offsets: ModulationSmoother<3>,
    effective: [f32; 3],
    last_color: f64,
    last_width: f64,
    color_gains: [f64; 2],
    width_mix: [f64; 2],
}
impl Noise {
    pub fn new(settings: &NoiseSettings, context: ProcessingContext) -> Self {
        let rate = f64::from(context.sample_rate_hz.max(1));
        let coefficients = std::array::from_fn(|i| {
            if i == BANDS - 1 {
                0.0
            } else {
                (-std::f64::consts::TAU * (20.0 * 2.0_f64.powi(i as i32)).min(rate * 0.45) / rate)
                    .exp()
            }
        });
        // Parallel low-pass sections with octave-spaced corners approximate a 1/f spectrum.
        // Compute exact stationary covariance for white input to match neutral RMS.
        let mut pink: [f64; BANDS] = std::array::from_fn(|i| 2.0_f64.powf(-(i as f64) * 0.5));
        let mut variance = 0.0;
        for i in 0..BANDS {
            for j in 0..BANDS {
                variance += pink[i] * pink[j] * (1.0 - coefficients[i]) * (1.0 - coefficients[j])
                    / (1.0 - coefficients[i] * coefficients[j]);
            }
        }
        for weight in &mut pink {
            *weight /= variance.sqrt();
        }
        let brown = ((1.0 + coefficients[0]) / (1.0 - coefficients[0])).sqrt();
        let controls = targets(settings);
        let weights = weights(settings.kind);
        let id = settings.id.as_u128();
        Self {
            id,
            generators: [
                Generator::new(mix(id)),
                Generator::new(mix(id ^ 0xa076_1d64_78bd_642f)),
            ],
            coefficients,
            pink,
            brown,
            tone_coefficient: (-std::f64::consts::TAU * 1000.0_f64.min(rate * 0.45) / rate).exp(),
            controls,
            targets: controls,
            weights,
            target_weights: weights,
            type_remaining: 0,
            remaining: 0,
            fade: (context.sample_rate_hz / 200).max(1),
            offsets: ModulationSmoother::new(context),
            effective: [
                controls[0] as f32,
                controls[1] as f32,
                if controls[2] > 0.0 {
                    (20.0 * controls[2].log10()) as f32
                } else {
                    daw_unit_core::SILENT_GAIN_DB
                },
            ],
            last_color: f64::NAN,
            last_width: f64::NAN,
            color_gains: [1.0; 2],
            width_mix: [1.0, 0.0],
        }
    }
    /// Restart from stable note and source identities; no global random generator is shared.
    pub fn trigger(&mut self, note: u128) {
        let seed = self.id ^ note;
        self.generators = [
            Generator::new(mix(seed)),
            Generator::new(mix(seed ^ 0xa076_1d64_78bd_642f)),
        ];
        self.controls = self.targets;
        self.weights = self.target_weights;
        self.remaining = 0;
        self.type_remaining = 0;
        self.offsets.reset();
    }
    /// Compatible control changes retain random, filter and modulation history.
    pub fn retarget(&mut self, next: &Self) {
        self.targets = next.targets;
        self.target_weights = next.target_weights;
        self.remaining = self.fade;
        self.type_remaining = self.fade;
        // Never replace/drop route storage or other heap owners on the callback.
    }
    pub fn compatible(&self, next: &Self) -> bool {
        self.id == next.id
    }
    pub fn values(&self) -> [f32; 3] {
        self.effective
    }
    fn render(&mut self, offsets: [f64; 3]) -> [f32; 2] {
        if self.remaining > 0 {
            for i in 0..3 {
                self.controls[i] +=
                    (self.targets[i] - self.controls[i]) / f64::from(self.remaining);
            }
            self.remaining -= 1;
        }
        if self.type_remaining > 0 {
            for i in 0..3 {
                self.weights[i] +=
                    (self.target_weights[i] - self.weights[i]) / f64::from(self.type_remaining);
            }
            self.type_remaining -= 1;
        }
        let color = (self.controls[0] + offsets[0]).clamp(-1.0, 1.0);
        let width = (self.controls[1] + offsets[1]).clamp(0.0, 1.0);
        let gain = if self.controls[2] == 0.0 {
            0.0
        } else {
            (self.controls[2]
                * if offsets[2] == 0.0 {
                    1.0
                } else {
                    10.0_f64.powf(offsets[2].clamp(-96.0, 96.0) / 20.0)
                })
            .min(10.0_f64.powf(12.0 / 20.0))
        };
        self.effective = [
            color as f32,
            width as f32,
            if gain > 0.0 {
                (20.0 * gain.log10()) as f32
            } else {
                daw_unit_core::SILENT_GAIN_DB
            },
        ];
        if color != self.last_color {
            self.color_gains = [
                10.0_f64.powf(-color * 6.0 / 20.0),
                10.0_f64.powf(color * 6.0 / 20.0),
            ];
            self.last_color = color;
        }
        if width != self.last_width {
            let angle = width * std::f64::consts::FRAC_PI_2;
            self.width_mix = [angle.cos(), angle.sin()];
            self.last_width = width;
        }
        let [low, high] = self.color_gains;
        let samples: [f64; 2] = std::array::from_fn(|ch| {
            let generator = &mut self.generators[ch];
            let kinds = generator.sample(&self.coefficients, &self.pink, self.brown);
            let input: f64 = kinds
                .into_iter()
                .zip(self.weights)
                .map(|(s, w)| s * w)
                .sum();
            generator.tone = input + self.tone_coefficient * (generator.tone - input);
            (generator.tone * low + (input - generator.tone) * high) * gain
        });
        [
            samples[0] as f32,
            (samples[0] * self.width_mix[0] + samples[1] * self.width_mix[1]) as f32,
        ]
    }
}
fn targets(s: &NoiseSettings) -> [f64; 3] {
    [
        if s.color.is_finite() {
            f64::from(s.color.clamp(-1.0, 1.0))
        } else {
            0.0
        },
        if s.width.is_finite() {
            f64::from(s.width.clamp(0.0, 1.0))
        } else {
            0.0
        },
        if !s.enabled || !s.level_db.is_finite() || s.level_db <= daw_unit_core::SILENT_GAIN_DB {
            0.0
        } else {
            10.0_f64.powf(f64::from(s.level_db.min(12.0)) / 20.0)
        },
    ]
}
fn weights(kind: NoiseType) -> [f64; 3] {
    match kind {
        NoiseType::White => [1.0, 0.0, 0.0],
        NoiseType::Pink => [0.0, 1.0, 0.0],
        NoiseType::Brown => [0.0, 0.0, 1.0],
    }
}
fn mix(seed: u128) -> u64 {
    let mut x = seed as u64 ^ (seed >> 64) as u64;
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (x ^ (x >> 31)).max(1)
}
impl ModulationDestination for Noise {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        NoiseSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn process_with_modulation(
        &mut self,
        _: Self::Audio,
        inputs: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        let offsets = self
            .offsets
            .process(inputs.offsets(ModulationParameter::NOISE));
        self.render(offsets)
    }
}
impl StereoSourceUnit for Noise {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: "daw.noise",
            name: "Noise",
            is_source: true,
            inputs: &[PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.trigger(0);
    }
    fn next_sample(&mut self) -> [f32; 2] {
        let offsets = self.offsets.process([0.0; 3]);
        self.render(offsets)
    }
}

#[cfg(test)]
mod tests;
