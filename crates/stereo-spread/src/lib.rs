//! Alternating spectral stereo widening. All filter storage is prepared before processing.
use daw_dsp_support::spread::SpreadBank as Bank;
use daw_unit_core::{
    EffectUnit, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter, ModulationRamp, ModulationScope, ModulationSmoother,
    PortKind, ProcessingContext, STEREO_SPREAD_TYPE_ID, StereoSpreadSettings, UnitDescriptor,
};
#[derive(Clone)]
pub struct StereoSpread {
    parameters: [ModulationRamp; 5],
    external: ModulationSmoother<4>,
    effective: [f32; 4],
    banks: [Bank; 2],
    active: usize,
    target_order: u8,
    transition_left: u32,
    fade_frames: u32,
}
impl StereoSpread {
    fn values(s: &StereoSpreadSettings) -> [f32; 5] {
        [
            s.lower_intensity,
            s.upper_intensity,
            s.lower_frequency_hz.log2(),
            s.upper_frequency_hz.log2(),
            f32::from(s.enabled),
        ]
    }
    /// Prepare with validated settings on the control thread.
    pub fn new(context: ProcessingContext, s: &StereoSpreadSettings) -> Self {
        Self {
            parameters: Self::values(s).map(ModulationRamp::new),
            external: ModulationSmoother::new(context),
            effective: [
                s.lower_intensity,
                s.upper_intensity,
                s.lower_frequency_hz,
                s.upper_frequency_hz,
            ],
            banks: std::array::from_fn(|_| Bank::new(context.sample_rate_hz, s.order)),
            active: 0,
            target_order: s.order,
            transition_left: 0,
            fade_frames: (context.sample_rate_hz / 200).max(1),
        }
    }
    pub fn retarget(&mut self, s: &StereoSpreadSettings) {
        for (ramp, target) in self.parameters.iter_mut().zip(Self::values(s)) {
            ramp.set(target, self.fade_frames);
        }
        self.target_order = s.order;
    }
    /// Intensity Lower/Upper (fractions), then actual Frequency Lower/Upper (Hz).
    pub fn effective_parameters(&self) -> [f32; 4] {
        self.effective
    }
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 4]) -> [f32; 2] {
        let p = self.parameters.each_mut().map(ModulationRamp::next_value);
        let offsets = self.external.process(offsets);
        let intensities =
            std::array::from_fn(|i| (f64::from(p[i]) + offsets[i]).clamp(-1.0, 1.0) as f32);
        let frequencies = StereoSpreadSettings::constrained_frequencies(
            (f64::from(p[2]) + offsets[2])
                .clamp(20_f64.log2(), 20_000_f64.log2())
                .exp2() as f32,
            (f64::from(p[3]) + offsets[3])
                .clamp(20_f64.log2(), 20_000_f64.log2())
                .exp2() as f32,
        );
        self.effective = [
            intensities[0],
            intensities[1],
            frequencies[0],
            frequencies[1],
        ];
        // Queue rapid Order edits until the current crossfade finishes. Both banks are fixed
        // storage; compatible updates and transitions never construct or retire an allocation.
        if self.transition_left == 0 && self.banks[self.active].order() != self.target_order {
            let incoming = &mut self.banks[1 - self.active];
            incoming.set_order(self.target_order);
            incoming.invalidate_configuration();
            incoming.reset();
            self.transition_left = self.fade_frames;
        }
        let dry = input.map(|v| if v.is_finite() { v } else { 0.0 });
        // Protect private filter intermediates at extreme finite headroom; dry is unaltered.
        let mid = ((f64::from(dry[0]) + f64::from(dry[1])) * 0.5).clamp(-1e12, 1e12) as f32;
        let mut side = self.banks[self.active].sample(mid, frequencies, intensities);
        if self.transition_left > 0 {
            let incoming = self.banks[1 - self.active].sample(mid, frequencies, intensities);
            self.transition_left -= 1;
            let blend = 1.0 - f64::from(self.transition_left) / f64::from(self.fade_frames);
            side += (incoming - side) * blend;
            if self.transition_left == 0 {
                self.active = 1 - self.active;
            }
        }
        if p[4] == 0.0 || intensities == [0.0; 2] {
            return dry;
        }
        let side = side * f64::from(p[4]);
        [f64::from(dry[0]) + side, f64::from(dry[1]) - side]
            .map(|v| v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
    }
}
impl EffectUnit for StereoSpread {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: STEREO_SPREAD_TYPE_ID,
            name: "Stereo Spread",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for p in &mut self.parameters {
            p.reset();
        }
        self.external.reset();
        for bank in &mut self.banks {
            bank.set_order(self.target_order);
            bank.invalidate_configuration();
            bank.reset();
        }
        self.active = 0;
        self.transition_left = 0;
        let frequencies = StereoSpreadSettings::constrained_frequencies(
            self.parameters[2].target().exp2(),
            self.parameters[3].target().exp2(),
        );
        self.effective = [
            self.parameters[0].target(),
            self.parameters[1].target(),
            frequencies[0],
            frequencies[1],
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 4])
    }
}
impl ModulationDestination for StereoSpread {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        StereoSpreadSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = StereoSpreadSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(ModulationParameter::STEREO_SPREAD),
        )
    }
}

#[cfg(test)]
mod tests;
