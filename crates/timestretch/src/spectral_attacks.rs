//! Independently designed linked high-band novelty gate; no reference-engine code.
use crate::{Error, buffer};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::{f64::consts::TAU, sync::Arc};

pub(crate) struct Detector {
    fft: Arc<dyn Fft<f64>>,
    scratch: Vec<Complex<f64>>,
    spectrum: Vec<Complex<f64>>,
    power: Vec<f64>,
    previous: Vec<f64>,
    window: Vec<f64>,
    rate: u32,
    last: Option<usize>,
}
impl Detector {
    pub fn new(rate: u32) -> Result<Self, Error> {
        let width = (rate as usize * 16 / 1000)
            .next_power_of_two()
            .clamp(256, 4096);
        let fft = FftPlanner::new().plan_fft_forward(width);
        Ok(Self {
            scratch: buffer(fft.get_inplace_scratch_len(), Complex::default())?,
            spectrum: buffer(width, Complex::default())?,
            power: buffer(width / 2 + 1, 0.0)?,
            previous: buffer(width / 2 + 1, 0.0)?,
            window: (0..width)
                .map(|i| 0.5 - 0.5 * (TAU * i as f64 / width as f64).cos())
                .collect(),
            fft,
            rate,
            last: None,
        })
    }
    pub fn observe(&mut self, input: &[f32], channels: usize, center: usize) -> Option<usize> {
        let width = self.spectrum.len();
        let frames = input.len() / channels;
        // Partial windows manufacture high-frequency energy at file boundaries.
        // Energy detection still handles attacks there; spectral evidence needs
        // a complete window of actual samples.
        if center < width / 2 || center + width / 2 > frames {
            return None;
        }
        self.power.fill(0.0);
        for channel in 0..channels {
            for (i, bin) in self.spectrum.iter_mut().enumerate() {
                let frame = center as i128 + i as i128 - (width / 2) as i128;
                let sample = if frame >= 0 && frame < frames as i128 {
                    input[frame as usize * channels + channel]
                } else {
                    0.0
                };
                *bin = Complex::new(f64::from(sample) * self.window[i], 0.0);
            }
            self.fft
                .process_with_scratch(&mut self.spectrum, &mut self.scratch);
            for (power, bin) in self.power.iter_mut().zip(&self.spectrum) {
                *power += bin.norm_sqr();
            }
        }
        let low = (700 * width / self.rate as usize).max(2);
        let mut rise = 0.0;
        let mut rise_squared = 0.0;
        let mut high = 0.0;
        let mut old = 0.0;
        for bin in low..self.power.len() - 1 {
            let reference = self.previous[bin - 1]
                .max(self.previous[bin])
                .max(self.previous[bin + 1]);
            let novelty = (self.power[bin] - reference).max(0.0);
            rise += novelty;
            rise_squared += novelty * novelty;
            high += self.power[bin];
            old += self.previous[bin];
        }
        let total: f64 = self.power.iter().sum();
        let warm = self.previous.iter().any(|&p| p > 0.0);
        self.previous.copy_from_slice(&self.power);
        if !warm {
            return None;
        }
        if high <= total * 1e-5 || high <= old * 2.0 || rise <= high * 0.35 {
            return None;
        }
        // Only supplement energy attacks with broadband evidence. Harmonic
        // swells can satisfy every growth threshold while a note sustains;
        // resetting its phase there produces pitch modulation. The participation
        // ratio is the number of equally powered novelty bins, independent of
        // gain and channel polarity. Energy attacks remain eligible unchanged.
        if rise * rise < 16.0 * rise_squared {
            return None;
        }
        // Locate the first sharp rise in 1 ms difference-power blocks within
        // the centered analysis window, avoiding an FFT-window timing offset.
        let block = (self.rate as usize / 1000).max(1);
        let start = center.saturating_sub(width / 2);
        let end = (center + width / 2).min(frames);
        let mut previous = 0.0_f64;
        let mut best = 0.0_f64;
        let mut onset = start;
        for at in (start..end).step_by(block) {
            let mut energy = 0.0;
            for frame in at..(at + block).min(end) {
                for channel in 0..channels {
                    let value = f64::from(input[frame * channels + channel]);
                    let before = if frame > 0 {
                        f64::from(input[(frame - 1) * channels + channel])
                    } else {
                        0.0
                    };
                    energy += (value - before).powi(2);
                }
            }
            let delta = energy - previous;
            if delta > best {
                best = delta;
                onset = at;
            }
            previous = energy;
        }
        // An ending/cutoff can also create spectral novelty. Require sustained
        // high-frequency difference-power growth around the refined position,
        // rather than treating a single cutoff impulse as a new attack.
        let span = (self.rate as usize * 8 / 1000).max(1);
        if onset < span || onset + span > frames {
            return None;
        }
        let difference_power = |start: usize, end: usize| {
            let mut sum = 0.0;
            for frame in start..end {
                for channel in 0..channels {
                    let previous = if frame > 0 {
                        f64::from(input[(frame - 1) * channels + channel])
                    } else {
                        0.0
                    };
                    sum += (f64::from(input[frame * channels + channel]) - previous).powi(2);
                }
            }
            sum / ((end - start) * channels) as f64
        };
        if difference_power(onset + block, onset + span)
            <= 2.0 * difference_power(onset - span, onset)
        {
            return None;
        }
        if self
            .last
            .is_some_and(|last| onset < last + self.rate as usize * 30 / 1000)
        {
            return None;
        }
        self.last = Some(onset);
        Some(onset)
    }
}
