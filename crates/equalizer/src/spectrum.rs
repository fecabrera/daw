//! Prepared full-range, overlapped analysis; no audio-path changes or callback allocation.
mod fft;
mod mapping;
use daw_unit_core::{
    SPECTRUM_BINS, SPECTRUM_UPPER_READY, SpectrumMeter, SpectrumReading, UnitMeter,
};
use std::sync::Arc;
type Complex = [f32; 2];
const WORK_PER_FRAME: usize = 80;
#[derive(Clone)]
pub(crate) struct Spectrum {
    meter: UnitMeter<SpectrumMeter>,
    reading: SpectrumReading,
    branch: fft::Branch,
    frames: u64,
    last_publication: u64,
    active: bool,
    #[cfg(test)]
    max_work: usize,
    #[cfg(test)]
    overruns: usize,
}
impl Spectrum {
    pub fn new(rate: u32, enabled: bool) -> Self {
        let scale = (rate / 44_100).max(1).next_power_of_two() as usize;
        let n = 4096 * scale;
        Self {
            meter: UnitMeter::new(enabled),
            reading: SpectrumReading {
                sample_rate_hz: rate,
                ..Default::default()
            },
            branch: fft::Branch::new(n, rate as f32, n / 4, n, 20_000.0, 26, 2400),
            frames: 0,
            last_publication: 0,
            active: false,
            #[cfg(test)]
            max_work: 0,
            #[cfg(test)]
            overruns: 0,
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        self.branch.heap_bytes()
    }
    pub fn is_bound(&self) -> bool {
        self.meter.spectrum_bound()
    }
    pub fn is_enabled(&self) -> bool {
        self.meter.is_enabled()
    }
    pub fn attach(&mut self, meter: Arc<SpectrumMeter>) {
        self.meter.attach_prepared(meter);
        self.active = false;
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.meter.is_enabled() == enabled {
            return;
        }
        if self.active {
            self.meter.set_enabled(enabled);
        } else {
            self.meter.set_prepared_enabled(enabled);
        }
        if !enabled {
            self.reset_state();
        }
    }
    pub fn retarget(&mut self, prepared: &mut Self) {
        self.meter.retarget_spectrum(&mut prepared.meter);
        self.active = true;
        if !self.meter.is_enabled() {
            self.reset_state();
        }
    }
    fn reset_state(&mut self) {
        self.branch.reset();
        self.frames = 0;
        self.last_publication = 0;
        self.reading.magnitudes.fill(0.0);
        self.reading.valid_branches = 0;
        self.reading.upper_end_frame = 0;
        self.reading.bass_end_frame = 0;
    }
    pub fn reset(&mut self) {
        self.reset_state();
        if self.active {
            self.meter.clear();
        }
    }
    pub fn sample(&mut self, input: Complex) {
        if !self.active {
            self.active = true;
            self.meter.activate_spectrum(self.reading.sample_rate_hz);
        }
        if !self.meter.is_connected() {
            return;
        }
        // Service oldest capture samples before the ring ingests/overwrites one.
        // Eight copies finish the full window in half a hop.
        let mut work = 0;
        for _ in 0..8 {
            work += usize::from(self.branch.copy());
        }
        self.frames += 1;
        let input = input.map(|v| {
            if v.is_finite() {
                v.clamp(-1e6, 1e6)
            } else {
                0.0
            }
        });
        if !self.branch.feed(input, self.frames) {
            #[cfg(test)]
            {
                self.overruns += 1;
            }
            self.reset(); // Bounded logical invalidation; never clear FFT-sized storage.
            return;
        }
        while work < WORK_PER_FRAME {
            if !self.branch.step() {
                break;
            }
            work += 1;
        }
        #[cfg(test)]
        {
            self.max_work = self.max_work.max(work);
        }
        if self.branch.valid
            && !self.branch.pending
            && self.branch.end_frame != self.last_publication
        {
            let elapsed = self.branch.end_frame - self.last_publication;
            let release = (-(elapsed as f32)
                / self.reading.sample_rate_hz as f32
                / daw_unit_core::LEVEL_METER_RELEASE_SECONDS)
                .exp();
            for i in 0..SPECTRUM_BINS {
                let amplitude = self.branch.powers[i].max(0.0).sqrt();
                self.reading.magnitudes[i] = amplitude.max(self.reading.magnitudes[i] * release);
            }
            // Preserve the schema-2 full-range branch flag/clock. Legacy bass
            // fields stay empty; there is no second analysis branch to await.
            self.reading.valid_branches = SPECTRUM_UPPER_READY;
            self.reading.upper_end_frame = self.branch.end_frame;
            self.reading.bass_end_frame = 0;
            self.meter.publish_spectrum(self.reading);
            self.last_publication = self.branch.end_frame;
        }
    }
}
#[cfg(test)]
mod tests;
