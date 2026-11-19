use super::ModulationRamp as Ramp;
use super::*;
use crate::{ModulationParameter as P, ProcessingContext};

fn gain(db: f32) -> f32 {
    if db <= crate::SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(db / 20.0)
    }
}
fn db(v: f32) -> f32 {
    if v == 0.0 {
        crate::SILENT_GAIN_DB
    } else {
        20.0 * v.log10()
    }
}

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
        self.effective = [
            if b[0] == 0.0 {
                crate::SILENT_GAIN_DB
            } else {
                (db(b[0]) + o[0]).clamp(crate::SILENT_GAIN_DB, 12.0)
            },
            (b[1] + o[1]).clamp(0.0, 1.0),
        ];
        input
    }
}
