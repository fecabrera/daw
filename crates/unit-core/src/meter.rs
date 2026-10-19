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
        handle.store(if self.enabled { reading } else { T::ZERO });
        self.handle = Some(handle);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            if !enabled && let Some(handle) = &self.handle {
                handle.store(T::ZERO);
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

#[cfg(test)]
mod tests {
    use super::*;

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
