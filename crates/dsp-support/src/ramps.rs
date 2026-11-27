//! Scalar DSP ramps with explicit arithmetic and endpoint policies.

/// Arithmetic ramp for controls maintained in double precision.
#[derive(Clone, Copy)]
pub struct LinearRamp64 {
    value: f64,
    target: f64,
    remaining: u32,
}
impl LinearRamp64 {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    pub fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    pub fn next_value(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.value
    }
    pub fn weights<const N: usize>(active: usize) -> [Self; N] {
        std::array::from_fn(|i| Self::new(f64::from(i == active)))
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }

    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn target(&self) -> f64 {
        self.target
    }
}

/// Single-precision ramp that assigns the exact target on its final step.
#[derive(Clone)]
pub struct ExactRamp32 {
    value: f32,
    target: f32,
    remaining: u32,
}
impl ExactRamp32 {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    pub fn retarget(&mut self, target: f32, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    /// Restart an ordered set of ramps on one clock, even if a target stayed the same.
    pub fn restart(&mut self, frames: u32) {
        self.remaining = frames;
    }
    pub fn advance(&mut self) {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / self.remaining as f32;
            self.remaining -= 1;
            if self.remaining == 0 {
                self.value = self.target;
            }
        }
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }

    pub fn value(&self) -> f32 {
        self.value
    }
    pub fn remaining_frames(&self) -> u32 {
        self.remaining
    }

    pub fn target(&self) -> f32 {
        self.target
    }
}

#[derive(Clone, Copy)]
/// Shared finite linear ramp for control values and contribution weights.
pub struct ModulationRamp {
    value: f32,
    target: f32,
    left: u32,
}
impl ModulationRamp {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            left: 0,
        }
    }
    pub fn set(&mut self, value: f32, frames: u32) {
        if self.target != value {
            self.target = value;
            self.left = frames;
        }
    }
    pub fn next_value(&mut self) -> f32 {
        advance_linear_f32(&mut self.value, self.target, &mut self.left);
        self.value
    }
    pub fn target(&self) -> f32 {
        self.target
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.left = 0;
    }
}

/// Shared 1 ms smoothing for additive parameter offsets. Nonfinite inputs become neutral.
/// f64 state preserves the existing filter and drive response at every project rate.
#[derive(Clone)]
pub struct OffsetSmoother<const N: usize> {
    values: [f64; N],
    alpha: f64,
}
impl<const N: usize> OffsetSmoother<N> {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            values: [0.0; N],
            alpha: 1.0 - (-1.0 / (0.001 * f64::from(sample_rate))).exp(),
        }
    }
    pub fn reset(&mut self) {
        self.values = [0.0; N];
    }
    pub fn process(&mut self, offsets: [f32; N]) -> [f64; N] {
        for (value, target) in self.values.iter_mut().zip(offsets) {
            let target = if target.is_finite() {
                f64::from(target)
            } else {
                0.0
            };
            *value += self.alpha * (target - *value);
        }
        self.values
    }
}

/// Double-precision ramp with exact terminal value.
#[derive(Clone, Copy)]
pub struct ExactRamp64 {
    value: f64,
    target: f64,
    remaining: u32,
}
impl ExactRamp64 {
    pub fn remaining_frames(&self) -> u32 {
        self.remaining
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    pub fn retarget(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    pub fn tick(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
            if self.remaining == 0 {
                self.value = self.target;
            }
        }
        self.value
    }
    pub fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}

/// Advance a single-precision linear ramp without forcing a terminal assignment.
pub fn advance_linear_f32(value: &mut f32, target: f32, remaining: &mut u32) {
    if *remaining > 0 {
        *value += (target - *value) / *remaining as f32;
        *remaining -= 1;
    }
}
