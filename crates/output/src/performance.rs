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
    cache_memory: [AtomicU64; 8],
    activation_buckets: [AtomicU64; 32],
    max_activation_ns: AtomicU64,
}
#[derive(Clone, Debug)]
pub struct PerformanceSnapshot {
    pub callbacks: u64,
    pub max_request_to_activation_ns: u64,
    /// Log2 microsecond buckets for completed requests, including worker and delivery time.
    pub activation_duration_buckets: [u64; 32],
    /// Worker-owned base/model/schedule/DSP estimates and optional cache storage.
    pub cache_memory: [u64; 8],
    /// All live output renderer owners, including queued and retired instances.
    pub output_renderer_bytes: u64,
    pub over_budget: u64,
    pub max_callback_ns: u64,
    pub max_commands_ns: u64,
    pub max_prepared_wait_ns: u64,
    pub device_failures: u64,
    /// Log2 nanosecond buckets, saturated at the final bucket. Cumulative until output closes.
    pub duration_buckets: [u64; 32],
}
static OUTPUT_RENDERER_BYTES: AtomicU64 = AtomicU64::new(0);
pub(super) struct RendererMemoryLease(u64);
impl RendererMemoryLease {
    pub fn try_new(bytes: u64) -> Result<Self, String> {
        const LIMIT: u64 = 512 * 1024 * 1024;
        OUTPUT_RENDERER_BYTES.fetch_update(Relaxed, Relaxed, |used| {
            used.checked_add(bytes).filter(|total| *total <= LIMIT)
        }).map_err(|_| "Audio preparation exceeds the 512 MiB output-state budget. Current playback is retained; reduce instrument voices or effects, or retry after pending work finishes.".to_owned())?;
        Ok(Self(bytes))
    }
}
impl Drop for RendererMemoryLease {
    fn drop(&mut self) {
        OUTPUT_RENDERER_BYTES.fetch_sub(self.0, Relaxed);
    }
}
impl Performance {
    pub(super) fn overruns(&self) -> u64 {
        self.over_budget.load(Relaxed)
    }
    pub(super) fn activation(&self, ns: u64) {
        self.max_activation_ns.fetch_max(ns, Relaxed);
        self.activation_buckets[ns.div_ceil(1000).max(1).ilog2().min(31) as usize]
            .fetch_add(1, Relaxed);
    }

    pub(super) fn cache(&self, memory: daw_engine::TransportCacheMemory) {
        let values = [
            memory.renderer.model_bytes,
            memory.renderer.schedule_bytes,
            memory.renderer.processor_bytes,
            memory.checkpoint_bytes,
            memory.warming_bytes,
            memory.checkpoint_count as u64,
            memory.covered_frame,
            memory.replayed_frames,
        ];
        for (slot, value) in self.cache_memory.iter().zip(values) {
            slot.store(value, Relaxed);
        }
    }

    pub fn snapshot(&self) -> PerformanceSnapshot {
        PerformanceSnapshot {
            callbacks: self.callbacks.load(Relaxed),
            max_request_to_activation_ns: self.max_activation_ns.load(Relaxed),
            activation_duration_buckets: std::array::from_fn(|i| {
                self.activation_buckets[i].load(Relaxed)
            }),
            cache_memory: std::array::from_fn(|i| self.cache_memory[i].load(Relaxed)),
            output_renderer_bytes: OUTPUT_RENDERER_BYTES.load(Relaxed),
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
        stats.activation(400_000);
        stats.activation(20_000_000);
        assert!(RendererMemoryLease::try_new(u64::MAX).is_err());
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.callbacks, 2);
        assert_eq!(snapshot.max_request_to_activation_ns, 20_000_000);
        assert_eq!(snapshot.activation_duration_buckets.iter().sum::<u64>(), 2);
        assert_eq!(snapshot.over_budget, 1);
        assert_eq!(snapshot.max_callback_ns, 200_000);
        assert_eq!(snapshot.max_commands_ns, 12_000);
        assert_eq!(snapshot.max_prepared_wait_ns, 99_000);
        assert_eq!(snapshot.device_failures, 1);
        assert_eq!(snapshot.duration_buckets.iter().sum::<u64>(), 2);
    }
}
