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
        let base = self.bands.each_ref().map(|b| b.current);
        if self.configuration != Some((base, offsets)) {
            self.configuration = Some((base, offsets));
            self.effective = base;
            for (band, gain, frequency, q) in [(0, 0, 1, 2), (2, 4, 5, 6)] {
                let values = &mut self.effective[band];
                values[2] = (values[2] + offsets[gain]).clamp(-24.0, 24.0);
                values[0] = (values[0] * 2_f32.powf(offsets[frequency])).clamp(20.0, 20_000.0);
                values[1] = (values[1] + offsets[q]).clamp(0.1, 10.0);
            }
            self.effective[1][2] = (self.effective[1][2] + offsets[3]).clamp(-24.0, 24.0);
            self.configure_effective();
        }
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

#[cfg(test)]
mod cache_tests {
    use super::*;
    use daw_unit_core::{EffectUnit, ProcessingContext};

    #[test]
    fn steady_parameter_cache_matches_recalculation_through_modulation_edits_and_reset() {
        for rate in [44_100, 96_000, 192_000] {
            let mut settings = EqSettings::default();
            let mut cached = Equalizer::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &settings,
            );
            let mut reference = cached.clone();
            for frame in 0..12_000 {
                if frame == 1000 || frame == 7000 {
                    settings.bands[0].frequency_hz = if frame == 1000 { 1200.0 } else { 250.0 };
                    settings.bands[1].gain_db = -9.0;
                    settings.bands[2].enabled = frame != 1000;
                    settings.enabled = frame != 7000;
                    cached.retarget(&settings);
                    reference.retarget(&settings);
                }
                if frame == 8000 {
                    cached.reset();
                    reference.reset();
                }
                let offsets = if (2500..5500).contains(&frame) {
                    let x = (frame as f32 * 0.023).sin();
                    [x * 8.0, x, x * 0.2, x * -6.0, x * 12.0, -x, x * 0.3]
                } else {
                    [0.0; 7]
                };
                let v = (frame as f32 * 0.077).sin() * 0.3;
                reference.configuration = None;
                assert_eq!(
                    cached
                        .process_modulated([v, -v * 0.7], offsets)
                        .map(f32::to_bits),
                    reference
                        .process_modulated([v, -v * 0.7], offsets)
                        .map(f32::to_bits)
                );
                assert_eq!(
                    cached.effective_parameters(),
                    reference.effective_parameters()
                );
            }
        }
    }
}
