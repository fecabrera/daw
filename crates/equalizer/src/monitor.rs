//! Read-only analysis of the EQ output. These filters never enter the audio path.
use daw_dsp_support::filters::eq3::BandAnalyzer;
use daw_unit_core::LevelMonitor;
use std::sync::{Arc, atomic::AtomicU32};

#[derive(Clone)]
pub(super) struct Monitor {
    analyzer: BandAnalyzer,
    meter: LevelMonitor<3>,
}

impl Monitor {
    pub fn new(sample_rate_hz: u32, enabled: bool) -> Self {
        Self {
            analyzer: BandAnalyzer::new(sample_rate_hz),
            meter: LevelMonitor::new(sample_rate_hz, enabled),
        }
    }

    pub fn attach(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.meter.attach(meter);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.meter.set_enabled(enabled);
        if !enabled {
            self.reset();
        }
    }

    pub fn sample(&mut self, output: [f32; 2], low: f32, high: f32) {
        if !self.meter.is_connected() {
            return;
        }
        self.meter.sample(self.analyzer.sample(output, low, high));
    }

    pub fn reset(&mut self) {
        self.analyzer.reset();
        self.meter.reset();
    }
}
