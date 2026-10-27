//! Shared plugin meter storage. The portable DSP owns publication and bypass clearing.
use daw_unit_core::{BandMeter, MeterStorage, PeakHold, PeakMeter};
use std::sync::Arc;
pub struct OutputMeter {
    pub output: Arc<BandMeter<3>>,
    pub reduction: Arc<PeakMeter>,
    pub bands: Arc<BandMeter<3>>,
    pub eq_bands: [Arc<BandMeter<3>>; 64],
}
impl Default for OutputMeter {
    fn default() -> Self {
        Self {
            output: Arc::default(),
            reduction: Arc::default(),
            bands: Arc::default(),
            eq_bands: std::array::from_fn(|_| Arc::default()),
        }
    }
}
impl OutputMeter {
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
pub const METER_VALUES: usize = 77;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    let mut v = [0.0; METER_VALUES];
    v[0] = f64::from(m.readings()[0]);
    v[1] = f64::from(m.boost_peaks()[0].peak());
    v[2] = f64::from(m.reduction.reading());
    v[3] = f64::from(m.reduction.peak());
    for i in 0..3 {
        v[4 + i] = f64::from(m.bands.readings()[i]);
        v[7 + i] = f64::from(m.bands.reduction_peaks()[i].peak());
        v[10 + i] = f64::from(m.bands.boost_peaks()[i].peak());
    }
    for i in 0..64 {
        v[13 + i] = f64::from(m.eq_bands[i].readings()[0]);
    }
    v
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    m.output.clear();
    m.output.store([v[1], 0.0, 0.0]);
    m.output.initialize([v[0], 0.0, 0.0]);
    m.reduction.clear();
    m.reduction.store(v[3]);
    m.reduction.initialize(v[2]);
    m.bands.clear();
    m.bands.store(std::array::from_fn(|i| -v[7 + i]));
    m.bands.store(std::array::from_fn(|i| v[10 + i]));
    m.bands.initialize(std::array::from_fn(|i| v[4 + i]));
    for i in 0..64 {
        m.eq_bands[i].initialize([v[13 + i], 0.0, 0.0]);
    }
}
/// Bits select independent held readings; current readings remain untouched.
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    if mask & 1 != 0 {
        m.output.boost_peaks()[0].reset_peak();
    }
    if mask & 2 != 0 {
        m.reduction.reset_peak();
    }
    for i in 0..3 {
        if mask & (4 << i) != 0 {
            m.bands.reduction_peaks()[i].reset_peak();
            m.bands.boost_peaks()[i].reset_peak();
        }
    }
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
        for band in &self.eq_bands {
            band.clear();
        }
    }
}
