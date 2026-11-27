use super::ModulationRamp as Ramp;
use super::*;
use crate::{ModulationParameter as P, ProcessingContext};
use daw_dsp_support::gain::{amplitude as gain, wet_controls};

#[derive(Clone)]
pub struct WetOutput {
    base: [Ramp; 2],
    external: ModulationSmoother<2>,
    effective: [f32; 2],
}
impl WetOutput {
    pub fn new(context: ProcessingContext, level_db: f32, mix: f32) -> Self {
        Self {
            base: [gain(level_db), mix].map(Ramp::new),
            external: ModulationSmoother::new(context),
            effective: [level_db, mix],
        }
    }

    pub fn retarget(&mut self, level_db: f32, mix: f32, frames: u32) {
        self.set_targets([gain(level_db), mix], frames);
    }

    fn set_targets(&mut self, targets: [f32; 2], frames: u32) {
        for (ramp, value) in self.base.iter_mut().zip(targets) {
            ramp.set(value, frames);
        }
    }

    pub fn inherit(&mut self, previous: &mut Self, frames: u32) {
        let targets = self.base.map(|ramp| ramp.target());
        std::mem::swap(self, previous);
        self.set_targets(targets, frames);
    }

    pub fn effective(&self) -> [f32; 2] {
        self.effective
    }

    pub fn reset(&mut self) {
        for ramp in &mut self.base {
            ramp.reset();
        }
        self.external.reset();
    }
}

impl ModulationDestination for WetOutput {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = &[P::Level, P::DryWet];
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        let b = self.base.each_mut().map(Ramp::next_value);
        let o = self
            .external
            .process(m.offsets([P::Level, P::DryWet]))
            .map(|v| v as f32);
        self.effective = wet_controls(b, o);
        input
    }
}
