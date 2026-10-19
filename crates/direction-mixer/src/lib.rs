//! Stereo direction and width control with explicit M/S input decoding.
use daw_unit_core::{
    DIRECTION_MIXER_TYPE_ID, DirectionMixerInput, DirectionMixerSettings, EffectUnit,
    ModulationContext, ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationParameter, ModulationScope, ModulationSmoother, PortKind, ProcessingContext,
    UnitDescriptor,
};

#[derive(Clone, Copy)]
struct Ramp {
    value: f64,
    target: f64,
    remaining: u32,
}
impl Ramp {
    fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn tick(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
            if self.remaining == 0 {
                self.value = self.target;
            }
        }
        self.value
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}

#[derive(Clone)]
pub struct DirectionMixer {
    // Degrees, spread control value, M/S input blend, whole-unit wet blend.
    parameters: [Ramp; 4],
    fade_frames: u32,
    external: ModulationSmoother<2>,
    effective: [f32; 2],
}
impl DirectionMixer {
    fn lr_mid_gain(spread: f64) -> f64 {
        // Reference L/R Mid gains at Spread 1, 1.25, 1.5, 1.75, and 2.
        // Interpolate continuously; do not quantize the shared control or offsets.
        const GAINS: [f64; 5] = [1.0, 0.818, 0.65, 0.482, 0.3];
        let position = (spread - 1.0).max(0.0) * 4.0;
        let index = (position as usize).min(GAINS.len() - 2);
        GAINS[index] + (GAINS[index + 1] - GAINS[index]) * (position - index as f64)
    }
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
        let direction = (direction + offsets[0]).clamp(-180.0, 180.0);
        let spread = (spread + offsets[1]).clamp(0.0, 2.0);
        self.effective = [direction as f32, spread as f32];
        let dry = input.map(|v| if v.is_finite() { v } else { 0.0 });
        // Retain exact unity and dry samples, including subnormal and headroom values.
        if wet == 0.0 || (mode == 0.0 && direction == 0.0 && spread == 1.0) {
            return dry;
        }
        let [left, right] = dry.map(f64::from);
        let mid_lr = (left + right) * 0.5;
        let side_lr = (left - right) * 0.5;
        // L/R narrows by reducing Side below unity, then widens by reducing Mid.
        // The upper segments follow the measured reference gains.
        // M/S retains direct Side-level control. Blend complete interpretations
        // through the existing input-mode ramp to avoid a discontinuous switch.
        let mid_lr = mid_lr * Self::lr_mid_gain(spread);
        let side_lr = side_lr * spread.min(1.0);
        let mid = mid_lr + (left - mid_lr) * mode;
        let side = side_lr + (right * spread - side_lr) * mode;
        // Positive angles pan the center right, matching clockwise knob movement.
        // ±180 exchanges sides without polarity inversion. Exact cardinal coefficients
        // keep swaps and mono endpoints exact.
        let (sin, cos) = match direction {
            0.0 => (0.0, 1.0),
            90.0 => (1.0, 0.0),
            -90.0 => (-1.0, 0.0),
            -180.0 | 180.0 => (0.0, -1.0),
            _ => direction.to_radians().sin_cos(),
        };
        let output = [
            mid * (1.0 - sin).sqrt() + side * cos,
            mid * (1.0 + sin).sqrt() - side * cos,
        ];
        std::array::from_fn(|ch| {
            (f64::from(dry[ch]) + (output[ch] - f64::from(dry[ch])) * wet)
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        })
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
            self.parameters[0].value as f32,
            self.parameters[1].value as f32,
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
