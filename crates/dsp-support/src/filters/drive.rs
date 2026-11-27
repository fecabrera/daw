//! Drive voicing and tone filter bank.
use fundsp::{audionode::AudioNode, svf::*};

type Bell = FixedSvf<f64, BellMode<f64>>;
type Bass = FixedSvf<f64, LowshelfMode<f64>>;
type Tone = FixedSvf<f64, HighshelfMode<f64>>;

#[derive(Clone)]
pub struct DriveFilters {
    // Each color branch and channel has independent history, including inactive branches.
    bells: [[Bell; 2]; 3],
    bass: [Bass; 2],
    tone_filter: [Tone; 2],
}

impl DriveFilters {
    pub fn new(rate: u32, frequency_hz: f32) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(rate),
            cutoff: f64::from(frequency_hz),
            q: 0.65,
            gain: 1.0,
        };
        Self {
            bells: std::array::from_fn(|_| {
                std::array::from_fn(|_| Bell::new(BellMode::new(), &params))
            }),
            bass: std::array::from_fn(|_| Bass::new(LowshelfMode::new(), &params)),
            tone_filter: std::array::from_fn(|_| Tone::new(HighshelfMode::new(), &params)),
        }
    }

    pub fn configure(&mut self, values: [f32; 4], previous: [f32; 4]) {
        if values[1] != previous[1] {
            let gain = f64::from(amplitude((values[1] - 0.5) * 24.0));
            for filter in &mut self.tone_filter {
                filter.set_cutoff_q_gain(2500.0, std::f64::consts::FRAC_1_SQRT_2, gain);
            }
        }
        if values[2..] != previous[2..] {
            let frequency = f64::from(values[3].exp());
            for (index, filters) in self.bells.iter_mut().enumerate() {
                let db = values[2] * if index == 0 { -12.0 } else { 12.0 };
                for filter in filters {
                    filter.set_cutoff_q_gain(frequency, 0.65, f64::from(amplitude(db)));
                }
            }
            for filter in &mut self.bass {
                filter.set_cutoff_q_gain(
                    frequency * 0.5,
                    std::f64::consts::FRAC_1_SQRT_2,
                    f64::from(amplitude(-18.0 * values[2])),
                );
            }
        }
    }

    pub fn reset(&mut self) {
        for filter in self.bells.iter_mut().flatten() {
            filter.reset();
        }
        for filter in &mut self.bass {
            filter.reset();
        }
        for filter in &mut self.tone_filter {
            filter.reset();
        }
    }

    pub fn sample(&mut self, channel: usize, dry: f32, weights: [f32; 4]) -> f32 {
        let bass = self.bass[channel].filter_mono(dry);
        let branches = [
            dry,
            self.bells[0][channel].filter_mono(dry),
            self.bells[1][channel].filter_mono(dry),
            self.bells[2][channel].filter_mono(bass),
        ];
        let color = branches
            .into_iter()
            .zip(weights)
            .map(|(sample, weight)| sample * weight)
            .sum::<f32>();
        self.tone_filter[channel].filter_mono(color)
    }
}

fn amplitude(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}
