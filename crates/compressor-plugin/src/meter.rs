//! Shared plugin meter storage. The portable DSP owns publication and bypass clearing.
use daw_unit_core::{BandMeter, MeterStorage, PeakHold, PeakMeter, PluginMeterPrefix};
use std::sync::Arc;
#[derive(Default)]
pub struct OutputMeter {
    pub output: Arc<BandMeter<3>>,
    pub reduction: Arc<PeakMeter>,
    pub bands: Arc<BandMeter<3>>,
}
impl OutputMeter {
    fn prefix(&self) -> PluginMeterPrefix<'_> {
        PluginMeterPrefix {
            output: &self.output,
            reduction: &self.reduction,
            bands: &self.bands,
        }
    }

    pub fn current(&self) -> &Arc<[std::sync::atomic::AtomicU32; 3]> {
        self.output.current()
    }
    pub fn readings(&self) -> [f32; 3] {
        self.output.readings()
    }
    pub fn boost_peaks(&self) -> &[PeakHold; 3] {
        self.output.boost_peaks()
    }
}
pub const METER_VALUES: usize = 13;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    m.prefix().encode()
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    m.prefix().receive(&v);
}
/// Bits select independent held readings; current readings remain untouched.
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    m.prefix().reset(mask);
}

impl MeterStorage for OutputMeter {
    type Reading = [f32; 3];
    const ZERO: [f32; 3] = [0.0; 3];
    fn store(&self, v: [f32; 3]) {
        self.output.store(v);
    }
    fn initialize(&self, v: [f32; 3]) {
        self.output.initialize(v);
    }
    fn clear(&self) {
        self.output.clear();
        self.reduction.clear();
        self.bands.clear();
    }
}

impl daw_unit_core::BandAudition for OutputMeter {
    fn clear_live_preview(&self) {
        self.bands.clear();
    }
}
