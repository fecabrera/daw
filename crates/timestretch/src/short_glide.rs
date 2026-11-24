use crate::{Error, buffer, spectral::wrap};
use rustfft::{FftPlanner, num_complex::Complex};
use std::f64::consts::TAU;

// Short-window frequency measurements are used only as phase-synthesis
// targets. Long analysis magnitudes and the synthesis overlap are retained.
pub(super) struct ShortGlide {
    fft: std::sync::Arc<dyn rustfft::Fft<f64>>,
    spectra: Vec<Complex<f64>>,
    scratch: Vec<Complex<f64>>,
    power: Vec<f64>,
    window: Vec<f64>,
    estimates: Vec<(f64, f64)>,
    channels: usize,
    rate: f64,
}
impl ShortGlide {
    pub(super) const WIDTH: usize = 2048;
    const LAG: i64 = 256;

    pub(super) fn new(
        planner: &mut FftPlanner<f64>,
        channels: usize,
        sample_rate: u32,
    ) -> Result<Self, Error> {
        let fft = planner.plan_fft_forward(Self::WIDTH);
        let scratch = buffer(fft.get_inplace_scratch_len(), Complex::default())?;
        let mut estimates = Vec::new();
        estimates
            .try_reserve_exact(Self::WIDTH / 2)
            .map_err(|_| Error::ResourceLimit)?;
        Ok(Self {
            fft,
            spectra: buffer(3 * channels * Self::WIDTH, Complex::default())?,
            scratch,
            power: buffer(Self::WIDTH / 2, 0.0)?,
            window: (0..Self::WIDTH)
                .map(|i| 0.5 - 0.5 * (TAU * i as f64 / Self::WIDTH as f64).cos())
                .collect(),
            estimates,
            channels,
            rate: f64::from(sample_rate),
        })
    }

    pub(super) fn estimate(&mut self, input: &[f32], center: i64) -> &[(f64, f64)] {
        let frames = (input.len() / self.channels) as i64;
        let block = self.channels * Self::WIDTH;
        let width = Self::WIDTH as i64;
        for (frame, offset) in [-Self::LAG, 0, Self::LAG].into_iter().enumerate() {
            for ch in 0..self.channels {
                let start = frame * block + ch * Self::WIDTH;
                let spectrum = &mut self.spectra[start..start + Self::WIDTH];
                for (i, val) in spectrum.iter_mut().enumerate() {
                    let position = center + offset + i as i64 - width / 2;
                    let sample = if (0..frames).contains(&position) {
                        f64::from(input[position as usize * self.channels + ch])
                    } else {
                        0.0
                    };
                    *val = Complex::new(sample * self.window[i], 0.0);
                }
                self.fft.process_with_scratch(spectrum, &mut self.scratch);
            }
        }
        for (bin, value) in self.power.iter_mut().enumerate() {
            *value = (0..self.channels)
                .map(|ch| self.spectra[block + ch * Self::WIDTH + bin].norm_sqr())
                .sum();
        }
        let max_power = self.power.iter().copied().fold(0.0, f64::max);
        self.estimates.clear();
        let bin_width = TAU / Self::WIDTH as f64;
        let delta = Self::LAG as f64;
        let scale = self.rate / TAU;
        for bin in 4..self.power.len() - 4 {
            let expected = bin_width * bin as f64;
            if expected * scale > 5000.0 {
                break;
            }
            let power = self.power[bin];
            if power < max_power * 0.0025
                || power <= self.power[bin - 1]
                || power < self.power[bin + 1]
            {
                continue;
            }
            let central = self.power[bin - 1..=bin + 1].iter().sum::<f64>();
            let nearby = self.power[bin - 3..=bin + 3].iter().sum::<f64>();
            if central < nearby * 0.85 {
                continue;
            }
            let mut before = Complex::<f64>::default();
            let mut after = Complex::<f64>::default();
            for ch in 0..self.channels {
                let offset = ch * Self::WIDTH + bin;
                let prev = self.spectra[offset];
                let current = self.spectra[block + offset];
                let next = self.spectra[2 * block + offset];
                before += current * prev.conj();
                after += next * current.conj();
            }
            let low = wrap(before.arg() - expected * delta) / delta;
            let high = wrap(after.arg() - expected * delta) / delta;
            if low.abs().max(high.abs()) > 0.85 * bin_width {
                continue;
            }
            let slope = (high - low).abs() * scale * self.rate / delta;
            if !(20.0..=1400.0).contains(&slope) {
                continue;
            }
            let strength = ((slope - 20.0) / 60.0).clamp(0.0, 1.0);
            let weight = strength * strength * (3.0 - 2.0 * strength);
            self.estimates.push((expected + (low + high) * 0.5, weight));
        }
        &self.estimates
    }
}
