//! Limiter-owned conversions over the shared route resolver and additive smoother.
use super::{Limiter, Ramp, amplitude};
use daw_unit_core::{
    LimiterSettings, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter, ModulationScope,
};

impl Limiter {
    /// Last processed Input dB, Release ms, Lookahead ms, and Ceiling dB.
    pub fn effective_parameters(&self) -> [f32; 4] {
        let p = self.effective;
        [
            (20.0 * p[0].log10()) as f32,
            p[1] as f32,
            p[2] as f32,
            (20.0 * p[3].log10()) as f32,
        ]
    }
    /// Offsets in LIMITER order: dB, release octaves, lookahead ms, ceiling dB.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 4]) -> [f32; 2] {
        let offsets = self.external.process(offsets);
        let mut p = self.parameters.each_mut().map(Ramp::tick);
        // Keep neutral arithmetic sample-exact. Both gain targets remain finite and bounded.
        for (index, offset, max) in [(0, offsets[0], 24.0), (3, offsets[3], 0.0)] {
            if offset != 0.0 {
                p[index] = (p[index] * 10_f64.powf(offset.clamp(-96.0, 96.0) / 20.0))
                    .clamp(amplitude(-24.0), amplitude(max));
            }
        }
        if offsets[1] != 0.0 {
            p[1] = (p[1] * offsets[1].clamp(-96.0, 96.0).exp2()).clamp(1.0, 2000.0);
        }
        p[2] = (p[2] + offsets[2]).clamp(0.0, 10.0);
        self.effective = p;
        self.process_effective_sample(input, p)
    }
}
impl ModulationDestination for Limiter {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        LimiterSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = LimiterSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(input, modulation.offsets(ModulationParameter::LIMITER))
    }
}
