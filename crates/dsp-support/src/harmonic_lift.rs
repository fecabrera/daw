//! Harmonic Lift branch filters, shaping and linked maximization.
use crate::filters::svf::{HighShelf, StateVariableFilter as Svf};
use crate::ramps::ModulationRamp as Ramp;

fn finite(v: f64) -> f32 {
    if v.is_finite() {
        v.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
    } else {
        0.
    }
}
fn gain(db: f32) -> f64 {
    if db <= crate::gain::SILENT_GAIN_DB {
        0.
    } else {
        10_f64.powf(f64::from(db) / 20.)
    }
}
#[derive(Clone)]
pub struct HarmonicLiftKernel {
    pre: [[Svf; 2]; 6],
    post: [[Svf; 2]; 6],
    emphasis: [HighShelf; 2],
    deemphasis: [HighShelf; 2],
    envelope: f64,
    coefficient: f64,
    rate: f64,
    reduction: f32,
    branch_peaks: [f32; 6],
    maximizer: Ramp,
    fade: u32,
}
impl HarmonicLiftKernel {
    pub fn move_branch(&mut self, from: usize, to: usize) {
        self.pre.swap(from, to);
        self.post.swap(from, to);
        self.branch_peaks.swap(from, to);
    }

    pub fn branch_peaks(&self) -> [f32; 6] {
        self.branch_peaks
    }

    pub fn reduction(&self) -> f32 {
        self.reduction
    }

    pub fn new(rate: u32, active: bool) -> Self {
        let fade = (rate / 200).max(1);
        let rate = f64::from(rate);
        Self {
            pre: std::array::from_fn(|_| std::array::from_fn(|_| Svf::default())),
            post: std::array::from_fn(|_| std::array::from_fn(|_| Svf::default())),
            emphasis: std::array::from_fn(|_| HighShelf::new(rate, 6300., 0.1, 3.)),
            deemphasis: std::array::from_fn(|_| HighShelf::new(rate, 6300., 0.1, -3.)),
            envelope: 0.,
            coefficient: (-std::f64::consts::TAU * 10. / rate).exp(),
            rate,
            reduction: 0.,
            branch_peaks: [0.; 6],
            maximizer: Ramp::new(f32::from(active)),
            fade,
        }
    }
    pub fn sample(
        &mut self,
        input: [f64; 2],
        bands: [[f32; 3]; 6],
        drive: f32,
        push: f32,
        limit: f32,
    ) -> [f64; 2] {
        let mut wet = input;
        self.branch_peaks.fill(0.);
        for (i, b) in bands.iter().enumerate() {
            for ch in 0..2 {
                let v = if i == 5 {
                    self.pre[i][ch].high(input[ch], f64::from(b[0]), 0.58, self.rate)
                } else {
                    self.pre[i][ch].band(input[ch], f64::from(b[0]), 0.58, self.rate)
                };
                let shaped = if v > 0. {
                    v.tanh()
                } else {
                    v / (1. + v.abs().powf(0.4466)).powf(2.23914)
                };
                let branch =
                    self.post[i][ch].high(shaped, 10., std::f64::consts::FRAC_1_SQRT_2, self.rate);
                let contribution = f64::from(b[2]) * gain(b[1]) * branch;
                wet[ch] += contribution;
                self.branch_peaks[i] = self.branch_peaks[i].max(finite(contribution).abs());
            }
        }
        for (ch, v) in wet.iter_mut().enumerate() {
            let emphasized = self.emphasis[ch].sample(*v);
            let k = 3. * f64::from(drive);
            let shaped = (1. + k) * emphasized / (1. + k * emphasized.abs());
            let processed = self.deemphasis[ch].sample(shaped);
            if drive > 0. {
                *v = processed;
            }
        }
        let peak = wet[0].abs().max(wet[1].abs());
        self.envelope = (1. - self.coefficient) * peak + self.coefficient * self.envelope;
        let threshold = 10_f64.powf(-1.5 * f64::from(push));
        let reduction = (threshold / self.envelope.max(0.).sqrt().max(peak).max(1e-30)).min(1.);
        // Zero Push bypasses maximization. Ramp that discrete boundary as well
        // as Push itself, so hot input cannot step when reduction first engages.
        self.maximizer.set(f32::from(push > 0.0), self.fade);
        let active = f64::from(self.maximizer.next_value());
        self.reduction = (-20.0 * ((1.0 - active) + active * reduction).max(1e-30).log10()) as f32;
        let multiplier = gain(limit) * ((1.0 - active) + active * reduction / threshold);
        wet.map(|v| v * multiplier)
    }
    pub fn reset(&mut self, active: bool) {
        self.maximizer = Ramp::new(f32::from(active));
        for filter in self.pre.iter_mut().chain(&mut self.post).flatten() {
            filter.reset();
        }
        for f in self.emphasis.iter_mut().chain(&mut self.deemphasis) {
            f.reset();
        }
        self.envelope = 0.;
        self.reduction = 0.;
        self.branch_peaks.fill(0.);
    }
}
