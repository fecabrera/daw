//! Shared plugin meter storage. The portable DSP owns publication and bypass clearing.
use daw_unit_core::{BandMeter, MeterStorage, PeakHold, PeakMeter};
use std::sync::Arc;
#[derive(Default)]
pub struct OutputMeter {
    rate: std::sync::atomic::AtomicU32,
    pub spectrum: Arc<daw_unit_core::SpectrumMeter>,
    telemetry_cache: daw_spectrum::telemetry::TelemetryCache,
    pub output: Arc<BandMeter<3>>,
    pub reduction: Arc<PeakMeter>,
    pub bands: Arc<BandMeter<3>>,
}
impl OutputMeter {
    pub fn sample_rate_hz(&self) -> u32 {
        let rate = self.rate.load(std::sync::atomic::Ordering::Relaxed);
        if rate == 0 { 48_000 } else { rate }
    }
    pub(crate) fn set_prepared_rate(&self, rate: u32) {
        self.rate.store(rate, std::sync::atomic::Ordering::Relaxed);
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
pub const METER_VALUES: usize = 13 + daw_spectrum::telemetry::VALUES;
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
    daw_spectrum::telemetry::encode(
        &mut v[13..],
        m.telemetry_cache.read(&m.spectrum, m.sample_rate_hz()),
    );
    v
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    if v[..13].iter().any(|v| !v.is_finite()) {
        return;
    }
    let Some(spectrum) = daw_spectrum::telemetry::decode(&v[13..]) else {
        return;
    };
    m.set_prepared_rate(spectrum.sample_rate_hz);
    m.spectrum.store(spectrum);
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
        self.spectrum.clear();
    }
}

impl daw_unit_core::BandAudition for OutputMeter {
    fn clear_spectrum(&self) {
        self.spectrum.clear();
    }
    fn clear_live_preview(&self) {
        self.bands.clear();
        self.spectrum.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectrum_roundtrip_preserves_rate_and_stale_frames_cannot_survive_clear() {
        let source = OutputMeter::default();
        let target = OutputMeter::default();
        let reading = daw_unit_core::SpectrumReading {
            sample_rate_hz: 44_100,
            valid_branches: 3,
            magnitudes: [0.25; daw_unit_core::SPECTRUM_BINS],
            upper_end_frame: 4096,
            bass_end_frame: 4096,
            ..Default::default()
        };
        source.set_prepared_rate(44_100);
        source.spectrum.store(reading);
        receive_meter(&target, meter_values(&source).map(|v| v as f32));
        assert_eq!(target.spectrum.reading(), reading);
        source.clear();
        receive_meter(&target, meter_values(&source).map(|v| v as f32));
        assert_eq!(target.spectrum.reading().valid_branches, 0);
        assert_eq!(target.sample_rate_hz(), 44_100);
    }
}
