//! Shared plugin meter storage. The portable DSP owns publication and bypass clearing.
use daw_unit_core::{BandMeter, MeterStorage, PeakHold, PeakMeter};
use std::sync::Arc;
pub struct OutputMeter {
    pub curve_band: std::sync::atomic::AtomicU32,
    pub spectrum: Arc<daw_unit_core::SpectrumMeter>,
    prepared_rate: std::sync::atomic::AtomicU32,
    telemetry_cache: daw_spectrum::telemetry::TelemetryCache,
    pub band_level_resets: std::sync::atomic::AtomicU64,
    pub band_resets: std::sync::atomic::AtomicU64,
    pub audition_allowed: Arc<std::sync::atomic::AtomicBool>,
    pub reductions: [Arc<PeakMeter>; 64],
    pub solos: [Arc<std::sync::atomic::AtomicBool>; 64],
    pub output: Arc<BandMeter<3>>,
    pub reduction: Arc<PeakMeter>,
    pub bands: Arc<BandMeter<3>>,
    pub eq_bands: [Arc<BandMeter<3>>; 64],
}
impl Default for OutputMeter {
    fn default() -> Self {
        let eq_bands: [Arc<BandMeter<3>>; 64] =
            std::array::from_fn(|_| Arc::new(BandMeter::with_curve_history()));
        Self {
            curve_band: std::sync::atomic::AtomicU32::new(64),
            spectrum: Arc::default(),
            prepared_rate: std::sync::atomic::AtomicU32::new(48000),
            telemetry_cache: Default::default(),
            band_level_resets: std::sync::atomic::AtomicU64::new(0),
            band_resets: std::sync::atomic::AtomicU64::new(0),
            audition_allowed: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            solos: std::array::from_fn(|_| Arc::default()),
            reductions: std::array::from_fn(|i| {
                eq_bands[i]
                    .curve()
                    .unwrap()
                    .curve_reduction()
                    .unwrap()
                    .clone()
            }),
            output: Arc::default(),
            reduction: Arc::default(),
            bands: Arc::default(),
            eq_bands,
        }
    }
}
impl OutputMeter {
    pub fn sample_rate_hz(&self) -> u32 {
        self.prepared_rate
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    pub(crate) fn set_prepared_rate(&self, rate: u32) {
        self.prepared_rate
            .store(rate, std::sync::atomic::Ordering::Relaxed);
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
pub const LEGACY_METER_VALUES: usize = 269;
pub const SPECTRUM_VALUES: usize = LEGACY_METER_VALUES + daw_spectrum::telemetry::VALUES;
// Send only the selected curve; inactive bands retain their own bounded histories.
pub const METER_VALUES: usize = SPECTRUM_VALUES + 4 + 2 * daw_unit_core::LEVEL_HISTORY_BINS;
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
        v[205 + i] = f64::from(m.eq_bands[i].boost_peaks()[0].peak());
        v[77 + i] = f64::from(m.reductions[i].reading());
        v[141 + i] = f64::from(m.reductions[i].peak());
        v[13 + i] = f64::from(m.eq_bands[i].readings()[0]);
    }
    let reading = m.telemetry_cache.read(&m.spectrum, m.sample_rate_hz());
    daw_spectrum::telemetry::encode(&mut v[LEGACY_METER_VALUES..SPECTRUM_VALUES], reading);
    let slot = m.curve_band.load(std::sync::atomic::Ordering::Relaxed) as usize;
    if let Some(curve) = m.eq_bands.get(slot).and_then(|m| m.curve()) {
        v[SPECTRUM_VALUES] = (slot + 1) as f64;
        v[SPECTRUM_VALUES + 1] = f64::from(curve.readings()[0]);
        if let Some(h) = curve.history().and_then(|h| h.reading()) {
            v[SPECTRUM_VALUES + 2] = h.position as f64;
            v[SPECTRUM_VALUES + 3] = h.count as f64;
            for (i, bin) in h.bins.iter().enumerate() {
                v[SPECTRUM_VALUES + 4 + 2 * i] = f64::from(bin[0]);
                v[SPECTRUM_VALUES + 5 + 2 * i] = f64::from(bin[1]);
            }
        }
    }
    v
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    if v[..LEGACY_METER_VALUES]
        .iter()
        .any(|value| !value.is_finite())
    {
        return;
    }
    let Some(reading) = daw_spectrum::telemetry::decode(&v[LEGACY_METER_VALUES..SPECTRUM_VALUES])
    else {
        return;
    };
    let tail = &v[SPECTRUM_VALUES..];
    if tail.iter().any(|v| !v.is_finite() || *v < 0.0) || tail[0].fract() != 0.0 || tail[0] > 64.0 {
        return;
    }
    if tail[0] > 0.0 {
        if tail[2].fract() != 0.0
            || tail[3].fract() != 0.0
            || tail[2] >= daw_unit_core::LEVEL_HISTORY_BINS as f32
            || tail[3] > daw_unit_core::LEVEL_HISTORY_BINS as f32
        {
            return;
        }
        let curve = m.eq_bands[tail[0] as usize - 1].curve().unwrap();
        curve.initialize([tail[1], 0.0, 0.0]);
        curve
            .history()
            .unwrap()
            .receive(daw_unit_core::LevelHistoryReading {
                position: tail[2] as usize,
                count: tail[3] as usize,
                bins: std::array::from_fn(|i| [tail[4 + 2 * i], tail[5 + 2 * i]]),
            });
    }
    m.set_prepared_rate(reading.sample_rate_hz);
    m.spectrum.store(reading);
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
        m.reductions[i].clear();
        m.reductions[i].store(v[141 + i]);
        m.reductions[i].initialize(v[77 + i]);
        m.eq_bands[i].clear_levels();
        m.eq_bands[i].store([v[205 + i], 0.0, 0.0]);
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
        self.spectrum.clear();
        self.output.clear();
        self.reduction.clear();
        self.bands.clear();
        for meter in &self.reductions {
            meter.clear();
        }
        for band in &self.eq_bands {
            band.clear();
        }
    }
}

impl daw_unit_core::BandAudition for OutputMeter {
    fn curve_band(&self) -> Option<&std::sync::atomic::AtomicU32> {
        Some(&self.curve_band)
    }
    fn clear_live_preview(&self) {
        for reduction in &self.reductions {
            reduction.initialize(0.0);
        }
        for band in &self.eq_bands {
            if let Some(curve) = band.curve() {
                curve.clear_levels();
            }
        }
    }
    fn clear_spectrum(&self) {
        self.spectrum.clear();
    }
    fn band_level_reset_requests(&self) -> Option<&std::sync::atomic::AtomicU64> {
        Some(&self.band_level_resets)
    }
    fn reset_band_level_peak(&self, index: usize) -> bool {
        if let Some(m) = self.eq_bands.get(index) {
            m.boost_peaks()[0].reset_peak();
            true
        } else {
            false
        }
    }
    fn band_reset_requests(&self) -> Option<&std::sync::atomic::AtomicU64> {
        Some(&self.band_resets)
    }
    fn reset_band_peak(&self, index: usize) -> bool {
        if let Some(m) = self.reductions.get(index) {
            m.reset_peak();
            true
        } else {
            false
        }
    }
    fn set_audition_allowed(&self, allowed: bool) {
        self.audition_allowed
            .store(allowed, std::sync::atomic::Ordering::Relaxed);
    }
    fn band_solos(&self) -> &[Arc<std::sync::atomic::AtomicBool>] {
        &self.solos
    }
}

#[cfg(test)]
mod curve_tests {
    use super::*;
    use daw_unit_core::{BandAudition, LevelHistoryReading};
    use std::sync::atomic::Ordering;
    #[test]
    fn selected_curve_transport_preserves_other_histories_and_rejects_partial_metadata() {
        let sender = OutputMeter::default();
        let receiver = OutputMeter::default();
        sender.curve_band.store(7, Ordering::Relaxed);
        let curve = sender.eq_bands[7].curve().unwrap();
        curve.initialize([0.3, 0.0, 0.0]);
        let h = LevelHistoryReading {
            position: 2,
            count: 2,
            bins: std::array::from_fn(|_| [0.6, 4.0]),
        };
        curve.history().unwrap().receive(h);
        let values = meter_values(&sender).map(|v| v as f32);
        receive_meter(&receiver, values);
        assert_eq!(
            receiver.eq_bands[7]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading(),
            Some(h)
        );
        assert_eq!(receiver.eq_bands[7].curve().unwrap().readings()[0], 0.3);
        assert_eq!(
            receiver.eq_bands[0]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading()
                .unwrap()
                .count,
            0
        );
        let mut invalid = values;
        invalid[SPECTRUM_VALUES + 2] = 0.5;
        receive_meter(&receiver, invalid);
        assert_eq!(
            receiver.eq_bands[7]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading(),
            Some(h)
        );
        receiver.clear_live_preview();
        assert_eq!(receiver.eq_bands[7].curve().unwrap().readings()[0], 0.0);
        assert_eq!(
            receiver.eq_bands[7]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading(),
            Some(h)
        );
        receiver.clear();
        assert_eq!(
            receiver.eq_bands[7]
                .curve()
                .unwrap()
                .history()
                .unwrap()
                .reading()
                .unwrap()
                .count,
            0
        );
    }
}
