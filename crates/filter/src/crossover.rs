//! Shared stereo fourth-order crossover; backend nodes remain private.
use fundsp::{
    audionode::AudioNode,
    svf::{FixedSvf, HighpassMode, LowpassMode, SvfParams},
};
/// Two cascaded Butterworth sections on each branch: fourth-order Linkwitz–Riley.
#[derive(Clone)]
pub struct LinkwitzRileySplit {
    low: [[FixedSvf<f64, LowpassMode<f64>>; 2]; 2],
    high: [[FixedSvf<f64, HighpassMode<f64>>; 2]; 2],
    frequency: f32,
}
impl LinkwitzRileySplit {
    /// Current boundary in Hz, after the caller's control smoothing.
    pub fn frequency_hz(&self) -> f32 {
        self.frequency
    }

    pub fn new(rate: u32, frequency: f32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(rate),
            cutoff: f64::from(frequency),
            q: std::f64::consts::FRAC_1_SQRT_2,
            gain: 1.0,
        };
        Self {
            low: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(LowpassMode::new(), &params))
            }),
            high: std::array::from_fn(|_| {
                std::array::from_fn(|_| FixedSvf::new(HighpassMode::new(), &params))
            }),
            frequency,
        }
    }
    pub fn configure(&mut self, frequency: f32) {
        if self.frequency != frequency {
            self.frequency = frequency;
            for channel in &mut self.low {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
            for channel in &mut self.high {
                for node in channel {
                    node.set_cutoff(f64::from(frequency));
                }
            }
        }
    }
    pub fn sample(&mut self, input: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let low = std::array::from_fn(|ch| {
            self.low[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        let high = std::array::from_fn(|ch| {
            self.high[ch]
                .iter_mut()
                .fold(input[ch], |x, node| node.filter_mono(x))
        });
        (low, high)
    }
    pub fn reset(&mut self) {
        for channel in &mut self.low {
            for node in channel {
                node.reset();
            }
        }
        for channel in &mut self.high {
            for node in channel {
                node.reset();
            }
        }
    }
}
