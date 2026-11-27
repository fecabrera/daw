//! Live meter binding and telemetry for the portable spectrum analyzer.
pub mod telemetry;
use daw_dsp_support::analysis::{
    release_spectrum,
    spectrum::{SpectrumAnalyzer as Analysis, Update},
};
use daw_unit_core::{SPECTRUM_UPPER_READY, SpectrumMeter, SpectrumReading, UnitMeter};
use std::sync::Arc;
#[derive(Clone)]
pub struct SpectrumAnalyzer {
    meter: UnitMeter<SpectrumMeter>,
    reading: SpectrumReading,
    analyzer: Analysis,
    active: bool,
    #[cfg(test)]
    overruns: usize,
}
impl SpectrumAnalyzer {
    pub fn new(rate: u32, enabled: bool) -> Self {
        Self {
            meter: UnitMeter::new(enabled),
            reading: SpectrumReading {
                sample_rate_hz: rate,
                ..Default::default()
            },
            analyzer: Analysis::new(rate),
            active: false,
            #[cfg(test)]
            overruns: 0,
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        self.analyzer.heap_bytes()
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
        self.analyzer.reset();
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
    pub fn sample(&mut self, input: [f32; 2]) {
        if !self.active {
            self.active = true;
            self.meter.activate_spectrum(self.reading.sample_rate_hz);
        }
        if !self.meter.is_connected() {
            return;
        }
        match self.analyzer.sample(input) {
            Update::Pending => {}
            Update::Overrun => {
                #[cfg(test)]
                {
                    self.overruns += 1;
                }
                self.reset();
            }
            Update::Ready { powers, end_frame } => {
                let elapsed = end_frame - self.reading.upper_end_frame;
                release_spectrum(
                    &mut self.reading.magnitudes,
                    powers,
                    elapsed,
                    self.reading.sample_rate_hz,
                );
                // Preserve the schema-2 full-range flag and clock.
                self.reading.valid_branches = SPECTRUM_UPPER_READY;
                self.reading.upper_end_frame = end_frame;
                self.reading.bass_end_frame = 0;
                self.meter.publish_spectrum(self.reading);
            }
        }
    }
}
#[cfg(test)]
mod tests;
