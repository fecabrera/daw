//! Preallocated callback diagnostics. Budget overruns are not device xrun counts.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Default)]
pub struct Performance {
    callbacks: AtomicU64,
    over_budget: AtomicU64,
    max_callback_ns: AtomicU64,
    max_commands_ns: AtomicU64,
    max_prepared_wait_ns: AtomicU64,
    device_failures: AtomicU64,
    buckets: [AtomicU64; 32],
}
#[derive(Clone, Debug)]
pub struct PerformanceSnapshot {
    pub callbacks: u64,
    pub over_budget: u64,
    pub max_callback_ns: u64,
    pub max_commands_ns: u64,
    pub max_prepared_wait_ns: u64,
    pub device_failures: u64,
    /// Log2 nanosecond buckets, saturated at the final bucket. Cumulative until output closes.
    pub duration_buckets: [u64; 32],
}
impl Performance {
    pub fn snapshot(&self) -> PerformanceSnapshot {
        PerformanceSnapshot {
            callbacks: self.callbacks.load(Relaxed),
            over_budget: self.over_budget.load(Relaxed),
            max_callback_ns: self.max_callback_ns.load(Relaxed),
            max_commands_ns: self.max_commands_ns.load(Relaxed),
            max_prepared_wait_ns: self.max_prepared_wait_ns.load(Relaxed),
            device_failures: self.device_failures.load(Relaxed),
            duration_buckets: std::array::from_fn(|i| self.buckets[i].load(Relaxed)),
        }
    }
    pub(super) fn callback(&self, ns: u64, frames: usize, device_rate: u32) {
        self.callbacks.fetch_add(1, Relaxed);
        self.max_callback_ns.fetch_max(ns, Relaxed);
        let budget = (frames as u128 * 1_000_000_000 / u128::from(device_rate.max(1)))
            .min(u128::from(u64::MAX)) as u64;
        if ns > budget {
            self.over_budget.fetch_add(1, Relaxed);
        }
        self.buckets[ns.max(1).ilog2().min(31) as usize].fetch_add(1, Relaxed);
    }
    pub(super) fn commands(&self, ns: u64) {
        self.max_commands_ns.fetch_max(ns, Relaxed);
    }
    pub(super) fn prepared_wait(&self, ns: u64) {
        self.max_prepared_wait_ns.fetch_max(ns, Relaxed);
    }
    pub(super) fn device_failure(&self) {
        self.device_failures.fetch_add(1, Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_use_actual_device_frames_and_rate() {
        let stats = Performance::default();
        stats.callback(200_000, 32, 192_000);
        stats.callback(200_000, 512, 48_000);
        stats.commands(12_000);
        stats.prepared_wait(99_000);
        stats.device_failure();
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.callbacks, 2);
        assert_eq!(snapshot.over_budget, 1);
        assert_eq!(snapshot.max_callback_ns, 200_000);
        assert_eq!(snapshot.max_commands_ns, 12_000);
        assert_eq!(snapshot.max_prepared_wait_ns, 99_000);
        assert_eq!(snapshot.device_failures, 1);
        assert_eq!(snapshot.duration_buckets.iter().sum::<u64>(), 2);
    }
}
