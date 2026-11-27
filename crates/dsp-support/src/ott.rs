//! One upward/downward compression band with its original detection and timing laws.
use fundsp::dynamics::{Meter, MeterState};
const RMS: Meter = Meter::Rms(0.01);

fn amplitude(db: f32) -> f64 {
    10_f64.powf(f64::from(db) / 20.0)
}
fn finite(value: f64) -> f32 {
    value.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
}

// Fixed 6 dB soft knees, upward 4:1 below −36 dBFS and downward 10:1 above −18 dBFS.
fn soft_over(over: f32) -> f32 {
    if over <= -3.0 {
        0.0
    } else if over < 3.0 {
        (over + 3.0).powi(2) / 12.0
    } else {
        over
    }
}
pub fn gain_curve(level_db: f32) -> f32 {
    (0.75 * soft_over(-36.0 - level_db)).min(24.0) - (0.9 * soft_over(level_db + 18.0)).min(60.0)
}
#[derive(Clone)]
pub struct OttBand {
    rms: [MeterState; 2],
    gain_db: f64,
    configured_times: [f32; 2],
    coefficients: [f64; 2],
    rate: f64,
}

impl OttBand {
    pub fn new(rate: u32) -> Self {
        Self {
            rms: std::array::from_fn(|_| {
                let mut state = MeterState::new(RMS);
                state.set_sample_rate(RMS, f64::from(rate));
                state
            }),
            gain_db: 0.0,
            configured_times: [f32::NAN; 2],
            coefficients: [0.0; 2],
            rate: f64::from(rate),
        }
    }

    pub fn reset(&mut self) {
        for state in &mut self.rms {
            state.reset(RMS);
        }
        self.gain_db = 0.0;
    }

    pub fn change_db(&self, depth: f32, enabled: f32) -> f32 {
        self.gain_db as f32 * depth * enabled
    }

    pub fn sample(&mut self, input: [f32; 2], rms_weight: f32, p: [f32; 6]) -> [f32; 2] {
        for index in 0..2 {
            let time = p[index + 3];
            if self.configured_times[index] != time {
                self.configured_times[index] = time;
                self.coefficients[index] =
                    (-1.0 / (f64::from(time).exp() * 0.001 * self.rate)).exp();
            }
        }
        let input = input.map(|x| finite(f64::from(x) * amplitude(p[1])));
        let peak = input[0].abs().max(input[1].abs());
        let mut rms: f32 = 0.0;
        for (state, sample) in self.rms.iter_mut().zip(input) {
            state.tick(RMS, sample.clamp(-1e15, 1e15));
            let level = state.level(RMS);
            if level < 1e-15 {
                state.reset(RMS);
            } else {
                rms = rms.max(level);
            }
        }
        let detected = peak * (1.0 - rms_weight) + rms * rms_weight;
        // Silence generates no audio. Remove hidden boost when the detector is silent.
        let target = if detected < 1e-15 {
            0.0
        } else {
            f64::from(gain_curve(20.0 * detected.log10()))
        };
        let relaxing = target.abs() < self.gain_db.abs() && target * self.gain_db >= 0.0;
        let coefficient = self.coefficients[usize::from(relaxing)];
        self.gain_db = target + coefficient * (self.gain_db - target);
        let gain = amplitude(self.change_db(p[0], p[5]) + p[2]);
        input.map(|x| finite(f64::from(x) * gain))
    }
}
