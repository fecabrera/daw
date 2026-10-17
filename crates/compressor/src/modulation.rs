//! Unit-owned parameter conversion over the shared route resolver and offset smoother.
use super::{Compressor, amplitude};
use daw_unit_core::{
    CompressorSettings, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter, ModulationScope,
};

fn ratio(slope: f32) -> f32 {
    (1.0 - slope).recip().clamp(1.0, 20.0)
}
impl Compressor {
    /// Last processed Threshold dBFS, Ratio, Attack/Release ms, Knee dB, Level dB, Dry/Wet.
    pub fn effective_parameters(&self) -> [f32; 7] {
        let p = self.effective;
        [
            p[0],
            ratio(p[1]),
            p[3].exp(),
            p[4].exp(),
            p[2],
            if p[6] > 0.0 {
                20.0 * p[6].log10()
            } else {
                daw_unit_core::SILENT_GAIN_DB
            },
            p[7],
        ]
    }
    /// Offsets use COMPRESSOR order: dB, ratio, time octaves, dB, dB, normalized wet offset.
    /// Base ramps and detector/envelope state remain independent of modulation.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 7]) -> [f32; 2] {
        let offsets = self.external.process(offsets).map(|v| v as f32);
        for parameter in &mut self.parameters {
            parameter.advance();
        }
        let mut p = self.parameters.each_ref().map(|p| p.value);
        p[0] = (p[0] + offsets[0]).clamp(-60.0, 0.0);
        // Preserve the legacy slope arithmetic exactly when the ratio offset is neutral.
        if offsets[1] != 0.0 {
            p[1] = 1.0 - (ratio(p[1]) + offsets[1]).clamp(1.0, 20.0).recip();
        }
        p[3] = (p[3] + offsets[2] * std::f32::consts::LN_2).clamp(0.1_f32.ln(), 200_f32.ln());
        p[4] = (p[4] + offsets[3] * std::f32::consts::LN_2).clamp(10_f32.ln(), 2000_f32.ln());
        p[2] = (p[2] + offsets[4]).clamp(0.0, 24.0);
        if offsets[5] != 0.0 {
            p[6] = (p[6] * amplitude(offsets[5].clamp(-96.0, 96.0))).min(amplitude(12.0));
        }
        p[7] = (p[7] + offsets[6]).clamp(0.0, 1.0);
        self.effective = p;
        self.configure([p[3], p[4]]);
        self.process_effective_sample(input)
    }
}
impl ModulationDestination for Compressor {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        CompressorSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = CompressorSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(input, modulation.offsets(ModulationParameter::COMPRESSOR))
    }
}
