/// Gain-dependent Soft, Hard and Tube curves normalized at full-scale input.
#[derive(Clone, Copy)]
pub struct NormalizedClipper {
    drive: f32,
    soft_scale: f32,
}

impl NormalizedClipper {
    /// Prepare the clipper from a nonnegative drive factor. Near zero it passes audio through.
    pub fn new(drive: f32) -> Self {
        let soft_scale = if drive < 1e-4 {
            1.0
        } else {
            drive.tanh().recip()
        };
        Self { drive, soft_scale }
    }

    /// Blend with Soft as the residual weight during mode transitions.
    #[inline]
    pub fn process(self, input: f32, hard_weight: f32, tube_weight: f32) -> f32 {
        if self.drive < 1e-4 {
            return input;
        }
        let driven = input * self.drive;
        let soft = driven.tanh() * self.soft_scale;
        let mut output = soft;
        if hard_weight > 0.0 {
            let hard = driven.clamp(-1.0, 1.0) / self.drive.min(1.0);
            output += (hard - soft) * hard_weight;
        }
        if tube_weight > 0.0 {
            output += (self.tube(input, driven) - soft) * tube_weight;
        }
        output
    }

    fn tube(self, input: f32, driven: f32) -> f32 {
        // Constant-folded and runtime tanh can differ. Keep digital silence exact.
        if input == 0.0 {
            return 0.0;
        }
        let bias = 0.3_f32;
        let baseline = bias.tanh();
        let positive = (self.drive + bias).tanh() - baseline;
        let negative = baseline - (bias - self.drive).tanh();
        ((driven + bias).tanh() - baseline) / positive.max(negative)
    }
}
