//! Stereo direction and width control with explicit M/S input decoding.
use daw_dsp_support::{imaging, ramps::ExactRamp64 as Ramp};
use daw_unit_core::{
    DIRECTION_MIXER_TYPE_ID, DirectionMixerInput, DirectionMixerSettings, EffectUnit,
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, ModulationSmoother, PortKind, ProcessingContext,
    UnitDescriptor,
};

#[derive(Clone)]
pub struct DirectionMixer {
    // Degrees, spread control value, M/S input blend, whole-unit wet blend.
    parameters: [Ramp; 4],
    fade_frames: u32,
    external: ModulationSmoother<2>,
    effective: [f32; 2],
}
impl DirectionMixer {
    /// Prepare with validated settings on the control thread.
    pub fn new(context: ProcessingContext, settings: &DirectionMixerSettings) -> Self {
        Self {
            parameters: Self::values(settings).map(Ramp::new),
            fade_frames: (context.sample_rate_hz / 200).max(1),
            external: ModulationSmoother::new(context),
            effective: [settings.direction_degrees, settings.spread],
        }
    }
    fn values(settings: &DirectionMixerSettings) -> [f64; 4] {
        [
            f64::from(settings.direction_degrees),
            f64::from(settings.spread),
            f64::from(settings.input == DirectionMixerInput::MidSide),
            f64::from(settings.enabled),
        ]
    }
    /// Compatible updates preserve modulation history and unrelated in-flight ramps.
    pub fn retarget(&mut self, settings: &DirectionMixerSettings) {
        for (ramp, target) in self.parameters.iter_mut().zip(Self::values(settings)) {
            ramp.retarget(target, self.fade_frames);
        }
    }
    pub fn effective_parameters(&self) -> [f32; 2] {
        self.effective
    }
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 2]) -> [f32; 2] {
        let offsets = self.external.process(offsets);
        let [direction, spread, mode, wet] = self.parameters.each_mut().map(Ramp::tick);
        let degrees = (direction + offsets[0]).clamp(-180.0, 180.0);
        let spread = (spread + offsets[1]).clamp(0.0, 2.0);
        self.effective = [degrees as f32, spread as f32];
        let dry = input.map(|v| if v.is_finite() { v } else { 0.0 });
        // Retain exact unity and dry samples, including subnormal and headroom values.
        if wet == 0.0 || (mode == 0.0 && degrees == 0.0 && spread == 1.0) {
            return dry;
        }
        imaging::direction(dry, degrees, spread, mode, wet)
    }
}
impl EffectUnit for DirectionMixer {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: DIRECTION_MIXER_TYPE_ID,
            name: "Direction Mixer",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        for parameter in &mut self.parameters {
            parameter.reset();
        }
        self.external.reset();
        self.effective = [
            self.parameters[0].value() as f32,
            self.parameters[1].value() as f32,
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 2])
    }
}
impl ModulationDestination for DirectionMixer {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        DirectionMixerSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = DirectionMixerSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(ModulationParameter::DIRECTION_MIXER),
        )
    }
}

#[cfg(test)]
mod tests;
