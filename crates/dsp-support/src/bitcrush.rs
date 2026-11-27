//! Stereo quantization and whole-ratio sample holding.

pub fn quantize(input: f64, bits: u8) -> f64 {
    let input = input.clamp(-1.0, 1.0);
    // Symmetric endpoint-inclusive levels. Preserve digital silence explicitly:
    // an even number of equally spaced levels has no zero, including at one bit.
    if input == 0.0 {
        return 0.0;
    }
    let intervals = f64::from((1_u32 << bits) - 1);
    let magnitude = ((input.abs() + 1.0) * 0.5 * intervals).round() * 2.0 / intervals - 1.0;
    magnitude.copysign(input)
}

#[derive(Clone)]
pub struct SampleHold {
    held: [f64; 2],
    elapsed: u8,
}

impl SampleHold {
    pub fn new(elapsed: u8) -> Self {
        Self {
            held: [0.0; 2],
            elapsed,
        }
    }

    pub fn sample(&mut self, dry: [f32; 2], drive: f64, bits: u8, ratio: u8) -> [f64; 2] {
        // One capture clock preserves stereo timing. A changed ratio retains elapsed time;
        // shortening the period captures now if that time already meets the new period.
        if self.elapsed >= ratio {
            let drive = 10_f64.powf(drive / 20.0);
            self.held = dry.map(|v| quantize(f64::from(v) * drive, bits));
            self.elapsed = 0;
        }
        self.elapsed += 1;
        self.held
    }
}
