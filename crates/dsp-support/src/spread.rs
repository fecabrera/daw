//! Alternating spectral spread filter bank.
use fundsp::{
    audionode::AudioNode,
    svf::{BandpassMode, FixedSvf, HighpassMode, LowpassMode, SvfParams},
};

pub const MAX_BANDS: usize = 12;
type Bandpass = FixedSvf<f64, BandpassMode<f64>>;
#[derive(Clone)]
pub struct SpreadBank {
    bands: [Bandpass; MAX_BANDS],
    highpass: [FixedSvf<f64, HighpassMode<f64>>; 2],
    lowpass: [FixedSvf<f64, LowpassMode<f64>>; 2],
    order: u8,
    configured: [f32; 2],
    q: f64,
}
impl SpreadBank {
    pub fn invalidate_configuration(&mut self) {
        self.configured = [f32::NAN; 2];
    }

    pub fn order(&self) -> u8 {
        self.order
    }

    pub fn set_order(&mut self, order: u8) {
        self.order = order;
    }

    pub fn new(rate: u32, order: u8) -> Self {
        let params = SvfParams {
            sample_rate: rate.into(),
            cutoff: 1000.0,
            q: std::f64::consts::FRAC_1_SQRT_2,
            gain: 1.0,
        };
        Self {
            bands: std::array::from_fn(|_| FixedSvf::new(BandpassMode::new(), &params)),
            highpass: std::array::from_fn(|_| FixedSvf::new(HighpassMode::new(), &params)),
            lowpass: std::array::from_fn(|_| FixedSvf::new(LowpassMode::new(), &params)),
            order,
            configured: [f32::NAN; 2],
            q: 1.0,
        }
    }
    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
        for filter in &mut self.highpass {
            filter.reset();
        }
        for filter in &mut self.lowpass {
            filter.reset();
        }
    }
    fn configure(&mut self, frequencies: [f32; 2]) {
        if self.configured == frequencies {
            return;
        }
        self.configured = frequencies;
        let [lower, upper] = frequencies.map(f64::from);
        let ratio = (upper / lower).powf(1.0 / f64::from(self.order));
        self.q = (ratio.sqrt() / (ratio - 1.0)).clamp(0.5, 32.0);
        let mut center = lower * ratio.sqrt();
        for band in self.bands.iter_mut().take(usize::from(self.order)) {
            band.set_cutoff_q(center, self.q);
            center *= ratio;
        }
        for filter in &mut self.highpass {
            filter.set_cutoff(lower);
        }
        for filter in &mut self.lowpass {
            filter.set_cutoff(upper);
        }
    }
    pub fn sample(&mut self, input: f32, frequencies: [f32; 2], intensities: [f32; 2]) -> f64 {
        self.configure(frequencies);
        let input = self
            .highpass
            .iter_mut()
            .fold(input, |x, f| f.filter_mono(x));
        let input = self.lowpass.iter_mut().fold(input, |x, f| f.filter_mono(x));
        let mut side = 0.0;
        for (i, band) in self
            .bands
            .iter_mut()
            .take(usize::from(self.order))
            .enumerate()
        {
            let fraction = i as f64 / f64::from(self.order - 1);
            let intensity =
                f64::from(intensities[0]) + f64::from(intensities[1] - intensities[0]) * fraction;
            // FunDSP's bandpass has peak gain Q. Normalize each band to unity.
            let sample = f64::from(band.filter_mono(input)) / self.q;
            side += sample * intensity * if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        side
    }
}
