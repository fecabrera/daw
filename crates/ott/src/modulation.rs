//! OTT parameter conversion over the shared typed resolver and offset smoother.
use super::{Ott, amplitude};
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, OttSettings, SILENT_GAIN_DB,
};

impl Ott {
    /// Last processed controls in `ModulationParameter::OTT` order, in native units.
    pub fn effective_parameters(&self) -> [f32; 19] {
        let mut values = [0.0; 19];
        values[0] = self.effective[0].exp();
        values[1] = self.effective[1].exp();
        for (i, band) in self.bands.iter().enumerate() {
            let p = band.effective;
            values[2 + 5 * i..7 + 5 * i].copy_from_slice(&[
                p[0],
                p[1],
                p[2],
                p[3].exp(),
                p[4].exp(),
            ]);
        }
        values[17] = if self.effective[2] > 0.0 {
            20.0 * self.effective[2].log10()
        } else {
            SILENT_GAIN_DB
        };
        values[18] = self.effective[3];
        values
    }
    /// Frequency/time offsets use octaves; band gains and Level use dB; Depth/Wet use fractions.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 19]) -> [f32; 2] {
        let offsets = self.external.process(offsets).map(|v| v as f32);
        for parameter in &mut self.parameters {
            parameter.advance();
        }
        let mut p = self.parameters.each_ref().map(|p| p.value);
        // Preserve unmodulated crossover arithmetic. Only a moved pair needs projection.
        if offsets[0] != 0.0 || offsets[1] != 0.0 {
            let low = (p[0] + offsets[0] * std::f32::consts::LN_2)
                .exp()
                .clamp(20.0, 2000.0);
            let high = (p[1] + offsets[1] * std::f32::consts::LN_2)
                .exp()
                .clamp(200.0, 20_000.0)
                .max(low + 1.0);
            p[0] = low.ln();
            p[1] = high.ln();
        }
        if offsets[17] != 0.0 {
            p[2] = (f64::from(p[2]) * amplitude(offsets[17].clamp(-96.0, 96.0)))
                .min(amplitude(12.0)) as f32;
        }
        p[3] = (p[3] + offsets[18]).clamp(0.0, 1.0);
        self.effective = p;
        self.process_effective_sample(input, offsets)
    }
}
impl ModulationDestination for Ott {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        OttSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = OttSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(input, modulation.offsets(ModulationParameter::OTT))
    }
}
