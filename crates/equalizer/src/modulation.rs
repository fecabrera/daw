//! EQ owns parameter conversion and bounds; shared code resolves routes and smooths offsets.
use super::Equalizer;
use daw_unit_core::{
    EqSettings, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter, ModulationScope,
};

impl Equalizer {
    /// Last processed Low Gain/Frequency/Q, Mid Gain, and High Gain/Frequency/Q.
    /// Gains use dB, frequencies use Hz, and Q is dimensionless.
    pub fn effective_parameters(&self) -> [f32; 7] {
        let [low, mid, high] = self.effective;
        [low[2], low[0], low[1], mid[2], high[2], high[0], high[1]]
    }

    /// Signed offsets in EQUALIZER parameter order: gain dB, frequency octaves, and Q.
    /// Base settings and band-cut states are independent of modulation.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 7]) -> [f32; 2] {
        let offsets = self.external.process(offsets).map(|v| v as f32);
        for band in &mut self.bands {
            band.advance();
        }
        self.effective = std::array::from_fn(|i| self.bands[i].current);
        for (band, gain, frequency, q) in [(0, 0, 1, 2), (2, 4, 5, 6)] {
            let values = &mut self.effective[band];
            values[2] = (values[2] + offsets[gain]).clamp(-24.0, 24.0);
            values[0] = (values[0] * 2_f32.powf(offsets[frequency])).clamp(20.0, 20_000.0);
            values[1] = (values[1] + offsets[q]).clamp(0.1, 10.0);
        }
        self.effective[1][2] = (self.effective[1][2] + offsets[3]).clamp(-24.0, 24.0);
        self.process_effective_sample(input)
    }
}

impl ModulationDestination for Equalizer {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = EqSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = EqSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _context: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(input, modulation.offsets(ModulationParameter::EQUALIZER))
    }
}
