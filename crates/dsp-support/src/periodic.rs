//! Periodic waveforms and phase continuity.
use crate::ramps::LinearRamp64 as Ramp;

pub fn waveform(index: usize, phase: f64) -> f64 {
    let phase = phase.rem_euclid(1.0);
    match index {
        0 => (phase * std::f64::consts::TAU).sin(),
        1 => 1.0 - 4.0 * ((phase + 0.25).rem_euclid(1.0) - 0.5).abs(),
        2 => 2.0 * phase - 1.0,
        3 => 1.0 - 2.0 * phase,
        _ => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

/// Transport phase with the existing 5 ms shortest-path correction on rate/tempo changes.
#[derive(Clone)]
pub struct PhaseClock {
    sample_rate: f64,
    fade: u32,
    last_frame: Option<u64>,
    last_origin: i128,
    last_rate: f64,
    last_phase: f64,
    correction: f64,
    remaining: u32,
}
impl PhaseClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: f64::from(sample_rate),
            fade: (sample_rate / 200).max(1),
            last_frame: None,
            last_origin: 0,
            last_rate: 0.0,
            last_phase: 0.0,
            correction: 0.0,
            remaining: 0,
        }
    }
    pub fn reset(&mut self) {
        self.last_frame = None;
        self.correction = 0.0;
        self.remaining = 0;
    }
    pub fn phase_at_quarters(
        &mut self,
        frame: u64,
        origin: i128,
        quarter: f64,
        beats: f64,
        rate: f64,
    ) -> f64 {
        self.phase_target(frame, origin, rate, (quarter / beats).rem_euclid(1.0))
    }
    pub fn phase_at(&mut self, frame: u64, origin: i128, rate: f64) -> f64 {
        let elapsed = (i128::from(frame) - origin).max(0) as f64;
        let target = (elapsed * rate / self.sample_rate).rem_euclid(1.0);
        self.phase_target(frame, origin, rate, target)
    }
    fn phase_target(&mut self, frame: u64, origin: i128, rate: f64, target: f64) -> f64 {
        let continuous = self.last_origin == origin
            && self
                .last_frame
                .is_some_and(|last| frame == last.saturating_add(1) || frame == last);
        if !continuous {
            self.correction = 0.0;
            self.remaining = 0;
        } else if rate != self.last_rate {
            self.correction = (self.last_phase + self.last_rate / self.sample_rate - target + 0.5)
                .rem_euclid(1.0)
                - 0.5;
            self.remaining = self.fade;
        }
        let phase = (target + self.correction).rem_euclid(1.0);
        if self.remaining > 0 {
            self.correction -= self.correction / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.last_frame = Some(frame);
        self.last_origin = origin;
        self.last_rate = rate;
        self.last_phase = phase;
        phase
    }
}

// Exact periodic one-pole response for the five shapes; O(1), including very slow cycles.
pub fn periodic_value(waves: &[Ramp; 5], phase: f64, hz: f64, ms: f64) -> f64 {
    if ms <= 0.0 {
        return waves
            .iter()
            .enumerate()
            .map(|(i, w)| w.value() * waveform(i, phase))
            .sum();
    }
    waves
        .iter()
        .enumerate()
        .filter(|(_, w)| w.value() != 0.0)
        .map(|(i, w)| w.value() * smooth_wave(i, phase, hz, ms / 1000.0))
        .sum::<f64>()
        .clamp(-1.0, 1.0)
}
fn smooth_wave(index: usize, phase: f64, hz: f64, tau: f64) -> f64 {
    let cycle = phase.rem_euclid(1.0);
    let k = 1.0 / (hz * tau);
    match index {
        0 => {
            let w = std::f64::consts::TAU * hz * tau;
            let angle = phase * std::f64::consts::TAU;
            (angle.sin() - w * angle.cos()) / (1.0 + w * w)
        }
        1 => {
            let p = (cycle + 0.25).rem_euclid(1.0);
            let slope = 4.0 / k;
            let e = (-0.5 * k).exp();
            let corner = 1.0 - slope * (1.0 - e) / (1.0 + e);
            if p < 0.5 {
                4.0 * p - 1.0 - slope + (-corner + 1.0 + slope) * (-p * k).exp()
            } else {
                3.0 - 4.0 * p + slope + (corner - 1.0 - slope) * (-(p - 0.5) * k).exp()
            }
        }
        2 | 3 => {
            let saw = 2.0 * cycle - 1.0 - 2.0 / k + 2.0 * (-cycle * k).exp() / -(-k).exp_m1();
            if index == 2 { saw } else { -saw }
        }
        _ => {
            if cycle < 0.5 {
                1.0 - 2.0 * (-cycle * k).exp() / (1.0 + (-0.5 * k).exp())
            } else {
                -1.0 + 2.0 * (-(cycle - 0.5) * k).exp() / (1.0 + (-0.5 * k).exp())
            }
        }
    }
}

/// Smoothed stereo sweep with exact initialization at the first incoming phase.
#[derive(Clone)]
pub struct StereoSweep {
    modulation: [f64; 2],
    modulation_initialized: bool,
    modulation_alpha: f64,
}

impl StereoSweep {
    pub fn new(rate: u32) -> Self {
        Self {
            modulation: [0.0; 2],
            modulation_initialized: false,
            modulation_alpha: 1.0 - (-1.0 / (0.001 * f64::from(rate))).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.modulation_initialized = false;
    }

    pub fn modulation(&self) -> [f64; 2] {
        self.modulation
    }

    pub fn sample(
        &mut self,
        phase: f64,
        sync: f64,
        stereo: f64,
        depth: f64,
        waves: [f64; 5],
    ) -> [f64; 2] {
        let mut cutoff_offsets = [0.0; 2];
        for (ch, cutoff_offset) in cutoff_offsets.iter_mut().enumerate() {
            let channel_phase = phase + sync + if ch == 1 { stereo } else { 0.0 };
            let modulation: f64 = waves
                .iter()
                .enumerate()
                .map(|(i, weight)| waveform(i, channel_phase) * weight)
                .sum();
            if !self.modulation_initialized {
                self.modulation[ch] = modulation;
            } else {
                self.modulation[ch] += self.modulation_alpha * (modulation - self.modulation[ch]);
            }
            *cutoff_offset = depth * self.modulation[ch];
        }
        self.modulation_initialized = true;
        cutoff_offsets
    }
}

/// Phase integrated from the previous sample's rate offset.
#[derive(Clone)]
pub struct RateOffsetPhase {
    rate_phase: f64,
    rate_last: Option<(u64, i128, f64)>,
    sample_rate: f64,
}

impl RateOffsetPhase {
    pub fn last_frame(&self) -> Option<u64> {
        self.rate_last.map(|(frame, _, _)| frame)
    }

    pub fn new(rate: u32) -> Self {
        Self {
            rate_phase: 0.0,
            rate_last: None,
            sample_rate: f64::from(rate),
        }
    }

    pub fn reset(&mut self) {
        self.rate_phase = 0.0;
        self.rate_last = None;
    }

    pub fn sample(&mut self, frame: u64, origin: i128, offset: f64) -> f64 {
        // Integrate frequency offsets. Multiplying elapsed time by the current frequency
        // would create phase jumps and the wrong speed during a sweep.
        match self.rate_last {
            Some((previous, last_origin, delta))
                if last_origin == origin && frame == previous.saturating_add(1) =>
            {
                self.rate_phase = (self.rate_phase + delta / self.sample_rate).rem_euclid(1.0);
            }
            Some((previous, last_origin, _)) if previous == frame && last_origin == origin => {}
            _ => self.rate_phase = 0.0,
        }
        self.rate_last = Some((frame, origin, offset));
        self.rate_phase
    }
}
