//! Procedural stereo noise. All state is prepared, fixed-size and deterministic.
use daw_dsp_support::noise::NoiseGenerator;
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, ModulationSmoother, NoiseSettings, NoiseType, PortKind,
    ProcessingContext, StereoSourceUnit, UnitDescriptor,
};
#[derive(Clone)]
pub struct Noise {
    id: u128,
    generator: NoiseGenerator,
    offsets: ModulationSmoother<3>,
}

impl Noise {
    pub fn new(settings: &NoiseSettings, context: ProcessingContext) -> Self {
        let id = settings.id.as_u128();
        Self {
            id,
            generator: NoiseGenerator::new(
                context.sample_rate_hz,
                id,
                targets(settings),
                weights(settings.kind),
            ),
            offsets: ModulationSmoother::new(context),
        }
    }

    pub fn trigger(&mut self, note: u128) {
        self.generator.trigger(self.id ^ note);
        self.offsets.reset();
    }

    pub fn retarget(&mut self, next: &Self) {
        self.generator.retarget(&next.generator);
    }

    pub fn compatible(&self, next: &Self) -> bool {
        self.id == next.id
    }

    pub fn values(&self) -> [f32; 3] {
        self.generator.values()
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
        self.generator.render(offsets)
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
        self.generator.render(offsets)
    }
}

#[cfg(test)]
mod tests;
