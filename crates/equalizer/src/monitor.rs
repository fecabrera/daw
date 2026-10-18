//! Read-only analysis of the EQ output. These filters never enter the audio path.
use super::Filter;
use daw_unit_core::UnitMeter;
use fundsp::svf::SvfParams;
use std::sync::{Arc, atomic::AtomicU32};

pub(super) struct Monitor {
    // Low LP, Mid HP/LP, High HP, with independent stereo histories.
    filters: [[Filter; 2]; 4],
    boundaries: [f32; 2],
    peaks: [f32; 3],
    release: f32,
    meter: UnitMeter<[AtomicU32; 3]>,
}

impl Monitor {
    pub fn new(sample_rate_hz: u32, enabled: bool) -> Self {
        let params = SvfParams {
            sample_rate: f64::from(sample_rate_hz),
            cutoff: 1000.0,
            q: f64::from(std::f32::consts::FRAC_1_SQRT_2),
            gain: 1.0,
        };
        Self {
            filters: std::array::from_fn(|index| {
                std::array::from_fn(|_| {
                    Filter::new(if index % 2 == 0 { 2 } else { 0 }, true, &params)
                })
            }),
            boundaries: [f32::NAN; 2],
            peaks: [0.0; 3],
            release: (-1.0 / (0.3 * sample_rate_hz as f32)).exp(),
            meter: UnitMeter::new(enabled),
        }
    }

    pub fn attach(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.meter.attach(meter, self.peaks);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.meter.set_enabled(enabled);
        if !enabled {
            self.reset();
        }
    }

    fn publish(&self) {
        self.meter.publish(self.peaks);
    }

    pub fn sample(&mut self, output: [f32; 2], low: f32, high: f32) {
        if !self.meter.is_connected() {
            return;
        }
        let boundaries = [low.min(high), low.max(high)];
        if self.boundaries != boundaries {
            self.boundaries = boundaries;
            for (index, filters) in self.filters.iter_mut().enumerate() {
                let cutoff = boundaries[usize::from(index >= 2)];
                for filter in filters {
                    filter.configure([cutoff, std::f32::consts::FRAC_1_SQRT_2, 0.0, 1.0]);
                }
            }
        }
        let mut peaks = [0.0_f32; 3];
        for (ch, sample) in output.into_iter().enumerate() {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            let low = self.filters[0][ch].sample(sample);
            let middle = self.filters[1][ch].sample(sample);
            let middle = self.filters[2][ch].sample(middle);
            let high = self.filters[3][ch].sample(sample);
            for (peak, value) in peaks.iter_mut().zip([low, middle, high]) {
                if value.is_finite() {
                    *peak = peak.max(value.abs());
                }
            }
        }
        if boundaries[0] == boundaries[1] {
            peaks[1] = 0.0;
        }
        for (held, peak) in self.peaks.iter_mut().zip(peaks) {
            *held = peak.max(*held * self.release);
        }
        self.publish();
    }

    pub fn reset(&mut self) {
        for filter in self.filters.iter_mut().flatten() {
            filter.reset();
        }
        self.peaks = [0.0; 3];
        self.publish();
    }
}
