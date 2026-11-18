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
