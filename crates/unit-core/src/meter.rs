//! Lock-free effect monitoring with one shared bypass policy.
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

/// Atomic storage for a scalar or fixed-size set of readings.
pub trait MeterStorage {
    type Reading: Copy;
    const ZERO: Self::Reading;
    fn store(&self, reading: Self::Reading);
    fn initialize(&self, reading: Self::Reading) {
        self.store(reading);
    }
    fn clear(&self) {
        self.store(Self::ZERO);
    }
}

/// Nonnegative scalar monitoring with a click-resettable maximum. The current
/// cell can also serve legacy scalar consumers. Allocate handles on a control thread.
#[derive(Default)]
pub struct PeakMeter {
    current: Arc<AtomicU32>,
    peak: PeakHold,
}

/// A nonnegative, resettable maximum shared by scalar and signed band monitors.
#[derive(Default)]
pub struct PeakHold(AtomicU32);
impl PeakHold {
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn reset_peak(&self) {
        self.0.store(0, Ordering::Relaxed);
    }
    #[inline]
    fn record(&self, value: f32) {
        if value.is_finite() && value > 0.0 {
            // Positive finite float bits have the same order as their values.
            // Avoid an atomic read-modify-write for samples below the held maximum.
            // A concurrent reset can linearize after this load; the next sample
            // still records against the reset value instead of the display release.
            let bits = value.to_bits();
            if bits > self.0.load(Ordering::Relaxed) {
                self.0.fetch_max(bits, Ordering::Relaxed);
            }
        }
    }
}

impl PeakMeter {
    pub fn new(current: Arc<AtomicU32>) -> Self {
        Self {
            current,
            peak: PeakHold::default(),
        }
    }
    pub fn current(&self) -> &Arc<AtomicU32> {
        &self.current
    }
    pub fn reading(&self) -> f32 {
        f32::from_bits(self.current.load(Ordering::Relaxed))
    }
    pub fn peak(&self) -> f32 {
        self.peak.peak()
    }
    pub fn reset_peak(&self) {
        self.peak.reset_peak();
    }
    pub fn hold(&self) -> &PeakHold {
        &self.peak
    }
    fn store_current(&self, reading: f32) -> f32 {
        let value = if reading.is_finite() && reading > 0.0 {
            reading
        } else {
            0.0
        };
        self.current.store(value.to_bits(), Ordering::Relaxed);
        value
    }
}

impl MeterStorage for PeakMeter {
    type Reading = f32;
    const ZERO: f32 = 0.0;
    fn store(&self, reading: f32) {
        let value = self.store_current(reading);
        self.peak.record(value);
    }
    fn initialize(&self, reading: f32) {
        // Binding a prepared/seek-reconstructed processor is not live metering.
        self.store_current(reading);
    }
    fn clear(&self) {
        self.current.store(0, Ordering::Relaxed);
        self.reset_peak();
    }
}

/// Signed band readings with independent reduction and boost holds.
pub struct BandMeter<const N: usize> {
    current: Arc<[AtomicU32; N]>,
    reductions: [PeakHold; N],
    boosts: [PeakHold; N],
}
impl<const N: usize> Default for BandMeter<N> {
    fn default() -> Self {
        Self::new(Arc::new(std::array::from_fn(|_| AtomicU32::new(0))))
    }
}
impl<const N: usize> BandMeter<N> {
    pub fn new(current: Arc<[AtomicU32; N]>) -> Self {
        Self {
            current,
            reductions: std::array::from_fn(|_| PeakHold::default()),
            boosts: std::array::from_fn(|_| PeakHold::default()),
        }
    }
    #[inline]
    fn store_levels(&self, current: [f32; N], instant: [f32; N]) {
        self.initialize(current);
        for (i, value) in instant.into_iter().enumerate() {
            self.reductions[i].record(-value);
            self.boosts[i].record(value);
        }
    }
    pub fn current(&self) -> &Arc<[AtomicU32; N]> {
        &self.current
    }
    pub fn readings(&self) -> [f32; N] {
        std::array::from_fn(|i| f32::from_bits(self.current[i].load(Ordering::Relaxed)))
    }
    pub fn reduction_peaks(&self) -> &[PeakHold; N] {
        &self.reductions
    }
    pub fn boost_peaks(&self) -> &[PeakHold; N] {
        &self.boosts
    }
}
impl<const N: usize> MeterStorage for BandMeter<N> {
    type Reading = [f32; N];
    const ZERO: Self::Reading = [0.0; N];
    fn store(&self, reading: Self::Reading) {
        self.store_levels(reading, reading);
    }
    fn initialize(&self, reading: Self::Reading) {
        for (cell, value) in self.current.iter().zip(reading) {
            let value = if value.is_finite() { value } else { 0.0 };
            cell.store(value.to_bits(), Ordering::Relaxed);
        }
    }
    fn clear(&self) {
        self.initialize(Self::ZERO);
        for peak in self.reductions.iter().chain(&self.boosts) {
            peak.reset_peak();
        }
    }
}

impl MeterStorage for AtomicU32 {
    type Reading = f32;
    const ZERO: f32 = 0.0;
    fn store(&self, reading: f32) {
        self.store(reading.to_bits(), Ordering::Relaxed);
    }
}

impl<const N: usize> MeterStorage for [AtomicU32; N] {
    type Reading = [f32; N];
    const ZERO: Self::Reading = [0.0; N];
    fn store(&self, reading: Self::Reading) {
        for (cell, value) in self.iter().zip(reading) {
            MeterStorage::store(cell, value);
        }
    }
}

/// Attach on the control thread. Enable changes and publication do not allocate or
/// release the handle; the processor retains ownership until retirement.
pub struct UnitMeter<T: MeterStorage> {
    enabled: bool,
    handle: Option<Arc<T>>,
}

// DSP snapshots retain meter enable state but never share publication handles.
impl<T: MeterStorage> Clone for UnitMeter<T> {
    fn clone(&self) -> Self {
        Self::new(self.enabled)
    }
}

impl<T: MeterStorage> UnitMeter<T> {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            handle: None,
        }
    }

    pub fn attach(&mut self, handle: Arc<T>, reading: T::Reading) {
        if self.enabled {
            handle.initialize(reading);
        } else {
            handle.clear();
        }
        self.handle = Some(handle);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            if !enabled && let Some(handle) = &self.handle {
                handle.clear();
            }
        }
    }

    /// False for bypassed units and processors without a monitoring consumer.
    pub fn is_connected(&self) -> bool {
        self.enabled && self.handle.is_some()
    }

    pub fn publish(&self, reading: T::Reading) {
        if self.enabled
            && let Some(handle) = &self.handle
        {
            handle.store(reading);
        }
    }
}

impl<const N: usize> UnitMeter<BandMeter<N>> {
    #[inline]
    fn publish_levels(&self, current: [f32; N], instant: [f32; N]) {
        if self.enabled
            && let Some(handle) = &self.handle
        {
            handle.store_levels(current, instant);
        }
    }
}

/// Shared instant-attack, 300 ms-release level publication and bypass lifecycle.
/// Values are nonnegative linear amplitudes. Prepare and attach handles off the callback.
#[derive(Clone)]
pub struct LevelMonitor<const N: usize> {
    peaks: [f32; N],
    release: f32,
    meter: UnitMeter<[AtomicU32; N]>,
    peak_meter: UnitMeter<BandMeter<N>>,
}
impl<const N: usize> LevelMonitor<N> {
    pub fn new(sample_rate_hz: u32, enabled: bool) -> Self {
        Self {
            peaks: [0.0; N],
            release: (-1.0 / (0.3 * sample_rate_hz as f32)).exp(),
            meter: UnitMeter::new(enabled),
            peak_meter: UnitMeter::new(enabled),
        }
    }
    pub fn attach(&mut self, handle: Arc<[AtomicU32; N]>) {
        self.meter.attach(handle, self.peaks);
    }
    pub fn attach_peak(&mut self, handle: Arc<BandMeter<N>>) {
        self.peak_meter.attach(handle, self.peaks);
    }
    pub fn is_connected(&self) -> bool {
        self.peak_meter.is_connected() || self.meter.is_connected()
    }
    pub fn is_enabled(&self) -> bool {
        self.meter.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.is_enabled() == enabled {
            return;
        }
        self.meter.set_enabled(enabled);
        self.peak_meter.set_enabled(enabled);
        if !enabled {
            self.reset();
        }
    }
    #[inline]
    pub fn sample(&mut self, levels: [f32; N]) {
        if !self.is_connected() {
            return;
        }
        for (held, level) in self.peaks.iter_mut().zip(levels) {
            let level = if level.is_finite() {
                level.max(0.0)
            } else {
                0.0
            };
            *held = level.max(*held * self.release);
        }
        self.meter.publish(self.peaks);
        self.peak_meter.publish_levels(self.peaks, levels);
    }
    pub fn reset(&mut self) {
        self.peaks = [0.0; N];
        self.meter.publish(self.peaks);
        self.peak_meter.publish_levels(self.peaks, [0.0; N]);
    }
}

impl LevelMonitor<3> {
    /// Standard effect-output reading: linked stereo absolute peak in slot zero.
    /// Remaining slots stay zero for the existing prepared band-meter transport.
    #[inline]
    pub fn sample_output(&mut self, output: [f32; 2]) {
        self.sample([output[0].abs().max(output[1].abs()), 0.0, 0.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_bands_hold_both_directions_and_reset_independently_through_shared_lifecycle() {
        let handle = Arc::new(BandMeter::<3>::default());
        let mut meter = UnitMeter::new(true);
        meter.attach(handle.clone(), [-30.0, 2.0, -40.0]);
        let peaks = || handle.reduction_peaks().each_ref().map(|p| p.peak());
        let boosts = || handle.boost_peaks().each_ref().map(|p| p.peak());
        assert_eq!(peaks(), [0.0; 3]);
        assert_eq!(boosts(), [0.0; 3]);
        meter.publish([-12.0, 24.0, -48.0]);
        meter.publish([6.0, -3.0, -2.0]);
        assert_eq!(handle.readings(), [6.0, -3.0, -2.0]);
        assert_eq!(peaks(), [12.0, 3.0, 48.0]);
        assert_eq!(boosts(), [6.0, 24.0, 0.0]);
        handle.reduction_peaks()[1].reset_peak();
        assert_eq!(peaks(), [12.0, 0.0, 48.0]);
        assert_eq!(boosts(), [6.0, 24.0, 0.0]);
        handle.boost_peaks()[1].reset_peak();
        assert_eq!(boosts(), [6.0, 0.0, 0.0]);
        assert_eq!(handle.readings(), [6.0, -3.0, -2.0]);
        meter.clone().publish([-60.0; 3]);
        meter.clone().publish([60.0; 3]);
        meter.attach(handle.clone(), [-60.0; 3]);
        meter.attach(handle.clone(), [60.0; 3]);
        assert_eq!(peaks(), [12.0, 0.0, 48.0]);
        assert_eq!(boosts(), [6.0, 0.0, 0.0]);
        meter.publish([f32::NAN, f32::INFINITY, -0.0]);
        assert_eq!(handle.readings(), [0.0; 3]);
        assert_eq!(peaks(), [12.0, 0.0, 48.0]);
        assert_eq!(boosts(), [6.0, 0.0, 0.0]);
        meter.set_enabled(false);
        assert_eq!(peaks(), [0.0; 3]);
        assert_eq!(boosts(), [0.0; 3]);
        meter.publish([-50.0; 3]);
        meter.publish([50.0; 3]);
        assert_eq!(handle.readings(), [0.0; 3]);
        assert_eq!(peaks(), [0.0; 3]);
        assert_eq!(boosts(), [0.0; 3]);
        meter.set_enabled(true);
        meter.publish([-2.0, 10.0, -4.0]);
        assert_eq!(peaks(), [2.0, 0.0, 4.0]);
        assert_eq!(boosts(), [0.0, 10.0, 0.0]);
        meter.publish([0.0; 3]);
        assert_eq!(peaks(), [2.0, 0.0, 4.0]);
        assert_eq!(boosts(), [0.0, 10.0, 0.0]);
    }

    #[test]
    fn held_peak_retains_short_readings_resets_independently_and_follows_bypass() {
        let handle = Arc::new(PeakMeter::default());
        let mut meter = UnitMeter::new(true);
        meter.attach(handle.clone(), 2.0);
        assert_eq!(handle.peak(), 0.0);
        meter.publish(18.0);
        meter.publish(3.0);
        assert_eq!(handle.reading(), 3.0);
        assert_eq!(handle.peak(), 18.0);
        handle.reset_peak();
        assert_eq!(handle.reading(), 3.0);
        assert_eq!(handle.peak(), 0.0);
        meter.publish(4.0);
        meter.publish(0.0);
        assert_eq!(handle.peak(), 4.0);
        let snapshot = meter.clone();
        snapshot.publish(50.0);
        assert_eq!(handle.peak(), 4.0);
        meter.attach(handle.clone(), 30.0);
        assert_eq!(handle.peak(), 4.0);
        meter.set_enabled(false);
        assert_eq!(handle.reading(), 0.0);
        assert_eq!(handle.peak(), 0.0);
        meter.publish(50.0);
        assert_eq!(handle.peak(), 0.0);
        meter.set_enabled(true);
        meter.publish(6.0);
        for value in [f32::NAN, f32::INFINITY, -5.0, -0.0] {
            meter.publish(value);
            assert_eq!(handle.reading(), 0.0);
            assert_eq!(handle.peak(), 6.0);
        }
        meter.publish(7.0);
        assert_eq!(handle.peak(), 7.0);
    }

    #[test]
    fn processor_snapshots_detach_meter_handles() {
        let original = Arc::new(AtomicU32::new(0));
        let mut meter = UnitMeter::new(true);
        meter.attach(original.clone(), 1.0);
        let mut snapshot = meter.clone();
        assert!(!snapshot.is_connected());
        snapshot.publish(2.0);
        assert_eq!(f32::from_bits(original.load(Ordering::Relaxed)), 1.0);
        let separate = Arc::new(AtomicU32::new(0));
        snapshot.attach(separate.clone(), 3.0);
        assert_eq!(f32::from_bits(separate.load(Ordering::Relaxed)), 3.0);
        assert_eq!(Arc::strong_count(&original), 2);
        meter.set_enabled(false);
        let mut disabled = meter.clone();
        disabled.attach(separate.clone(), 4.0);
        assert_eq!(separate.load(Ordering::Relaxed), 0);
    }

    fn check<T: MeterStorage>(handle: Arc<T>, reading: T::Reading, cells: &[&AtomicU32]) {
        let owners = Arc::strong_count(&handle);
        let mut meter = UnitMeter::new(false);
        meter.attach(handle.clone(), reading);
        assert!(!meter.is_connected());
        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
        meter.set_enabled(true);
        meter.publish(reading);
        assert!(meter.is_connected());
        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) != 0));
        meter.set_enabled(false);
        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
        // Sentinel values prove disabled publication does not write, including reset.
        for cell in cells {
            cell.store(123, Ordering::Relaxed);
        }
        meter.publish(T::ZERO);
        meter.set_enabled(false);
        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 123));
        assert_eq!(Arc::strong_count(&handle), owners + 1);
        meter.set_enabled(true);
        meter.publish(T::ZERO);
        assert!(cells.iter().all(|cell| cell.load(Ordering::Relaxed) == 0));
    }

    #[test]
    fn bypass_clears_and_suspends_scalar_and_band_publication_without_dropping_handles() {
        let scalar = Arc::new(AtomicU32::new(42));
        check(scalar.clone(), 3.0, &[scalar.as_ref()]);
        let bands = Arc::new(std::array::from_fn::<_, 3, _>(|_| AtomicU32::new(42)));
        check(bands.clone(), [1.0, -2.0, 3.0], &bands.each_ref());
    }
}

#[cfg(test)]
mod level_monitor_tests {
    use super::*;
    #[test]
    fn shared_levels_use_rate_independent_decay_clear_bypass_and_detach_clones() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            let handle = Arc::new([AtomicU32::new(0)]);
            let read = || f32::from_bits(handle[0].load(Ordering::Relaxed));
            let mut monitor = LevelMonitor::<1>::new(rate, true);
            monitor.attach(handle.clone());
            monitor.sample([2.0]);
            assert_eq!(read(), 2.0);
            monitor.clone().sample([8.0]);
            assert_eq!(read(), 2.0);
            for _ in 0..rate * 3 / 10 {
                monitor.sample([0.0]);
            }
            assert!((read() - 2.0 / std::f32::consts::E).abs() < 0.001);
            monitor.set_enabled(false);
            assert_eq!(read(), 0.0);
            monitor.sample([2.0]);
            assert_eq!(read(), 0.0);
            monitor.set_enabled(true);
            monitor.sample([0.5]);
            assert_eq!(read(), 0.5);
            monitor.reset();
            monitor.sample([f32::NAN]);
            assert_eq!(read(), 0.0);
        }
    }
}

#[cfg(test)]
mod output_peak_tests {
    use super::*;
    #[test]
    fn output_peaks_capture_samples_without_reholding_decaying_values_after_reset() {
        let meter = Arc::new(BandMeter::<3>::default());
        let mut monitor = LevelMonitor::new(48000, true);
        monitor.attach_peak(meter.clone());
        monitor.sample_output([0.25, -2.0]);
        monitor.sample_output([0.0; 2]);
        assert_eq!(meter.boost_peaks()[0].peak(), 2.0);
        assert!(meter.readings()[0] > 1.0);
        meter.boost_peaks()[0].reset_peak();
        monitor.sample_output([0.0; 2]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        assert!(
            meter.readings()[0] > 1.0,
            "display release must not repopulate the held peak"
        );
        monitor.sample_output([0.125, -0.5]);
        assert_eq!(meter.boost_peaks()[0].peak(), 0.5);
        let fresh = Arc::new(BandMeter::<3>::default());
        monitor.attach_peak(fresh.clone());
        assert_eq!(
            fresh.boost_peaks()[0].peak(),
            0.0,
            "binding is not live peak capture"
        );
        assert!(fresh.readings()[0] > 0.0);
        let mut snapshot = monitor.clone();
        snapshot.sample_output([4.0; 2]);
        assert_eq!(fresh.boost_peaks()[0].peak(), 0.0);
        monitor.set_enabled(false);
        assert_eq!(fresh.readings(), [0.0; 3]);
        assert_eq!(fresh.boost_peaks()[0].peak(), 0.0);
        monitor.sample_output([4.0; 2]);
        assert_eq!(fresh.readings(), [0.0; 3]);
        assert_eq!(Arc::strong_count(&fresh), 2);
        monitor.set_enabled(true);
        monitor.sample_output([0.2, -0.4]);
        assert_eq!(fresh.boost_peaks()[0].peak(), 0.4);
    }
}
