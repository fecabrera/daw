//! Audio-clocked four-second history. One serialized producer; bounded atomic snapshots.
use crate::{MeterStorage, UnitMeter};
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};
pub const LEVEL_HISTORY_SECONDS: usize = 4;
pub const LEVEL_HISTORY_BINS: usize = LEVEL_HISTORY_SECONDS * 50;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelHistoryReading {
    /// Next bin to replace; the oldest bin when the ring is full.
    pub position: usize,
    pub count: usize,
    /// Linked input sample peak (linear), and maximum applied reduction (dB).
    pub bins: [[f32; 2]; LEVEL_HISTORY_BINS],
}
impl Default for LevelHistoryReading {
    fn default() -> Self {
        Self {
            position: 0,
            count: 0,
            bins: [[0.0; 2]; LEVEL_HISTORY_BINS],
        }
    }
}
pub struct LevelHistory {
    version: AtomicU64,
    position: AtomicU32,
    count: AtomicU32,
    bins: [[AtomicU32; 2]; LEVEL_HISTORY_BINS],
}
impl Default for LevelHistory {
    fn default() -> Self {
        Self {
            version: AtomicU64::new(0),
            position: AtomicU32::new(0),
            count: AtomicU32::new(0),
            bins: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU32::new(0))),
        }
    }
}
impl LevelHistory {
    /// No waiting for the audio producer. All payload cells share one total order
    /// with the version observations; a partial write cannot become a valid frame.
    pub fn reading(&self) -> Option<LevelHistoryReading> {
        for _ in 0..2 {
            let version = self.version.load(Ordering::SeqCst);
            if version & 1 != 0 {
                continue;
            }
            let reading = LevelHistoryReading {
                position: self.position.load(Ordering::SeqCst) as usize,
                count: self.count.load(Ordering::SeqCst) as usize,
                bins: std::array::from_fn(|i| {
                    self.bins[i]
                        .each_ref()
                        .map(|c| f32::from_bits(c.load(Ordering::SeqCst)))
                }),
            };
            if version == self.version.load(Ordering::SeqCst) {
                return Some(reading);
            }
        }
        None
    }
    /// Control-thread receipt of a complete plugin telemetry frame.
    pub fn receive(&self, reading: LevelHistoryReading) {
        if reading.position >= LEVEL_HISTORY_BINS
            || reading.count > LEVEL_HISTORY_BINS
            || reading
                .bins
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || *v < 0.0)
        {
            self.clear();
            return;
        }
        self.version.fetch_add(1, Ordering::SeqCst);
        for (cells, values) in self.bins.iter().zip(reading.bins) {
            for (cell, value) in cells.iter().zip(values) {
                cell.store(value.to_bits(), Ordering::SeqCst);
            }
        }
        self.position
            .store(reading.position as u32, Ordering::SeqCst);
        self.count.store(reading.count as u32, Ordering::SeqCst);
        self.version.fetch_add(1, Ordering::SeqCst);
    }
}
impl MeterStorage for LevelHistory {
    type Reading = [f32; 2];
    const ZERO: Self::Reading = [0.0; 2];
    fn store(&self, value: Self::Reading) {
        self.version.fetch_add(1, Ordering::SeqCst);
        let index = self.position.load(Ordering::SeqCst) as usize;
        for (cell, value) in self.bins[index].iter().zip(value) {
            let value = if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            };
            cell.store(value.to_bits(), Ordering::SeqCst);
        }
        self.position
            .store(((index + 1) % LEVEL_HISTORY_BINS) as u32, Ordering::SeqCst);
        self.count.store(
            (self.count.load(Ordering::SeqCst) + 1).min(LEVEL_HISTORY_BINS as u32),
            Ordering::SeqCst,
        );
        self.version.fetch_add(1, Ordering::SeqCst);
    }
    fn clear(&self) {
        self.receive(LevelHistoryReading::default());
    }
}
#[derive(Clone)]
pub struct LevelHistoryWriter {
    meter: UnitMeter<LevelHistory>,
    period: u32,
    frames: u32,
    peaks: [f32; 2],
    pending: bool,
}
impl LevelHistoryWriter {
    pub fn new(rate: u32, enabled: bool) -> Self {
        Self {
            meter: UnitMeter::new(enabled),
            period: (rate / 50).max(1),
            frames: 0,
            peaks: [0.0; 2],
            pending: false,
        }
    }
    pub fn attach_prepared(&mut self, handle: Arc<LevelHistory>) {
        self.meter.attach_prepared(handle);
        self.pending = true;
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.meter.set_enabled(enabled);
        if !enabled {
            self.frames = 0;
            self.peaks = [0.0; 2];
        }
    }
    pub fn reset(&mut self) {
        self.frames = 0;
        self.peaks = [0.0; 2];
        self.meter.clear();
    }
    pub fn sample(&mut self, input: f32, reduction: f32) {
        if self.pending {
            self.pending = false;
            if !self.meter.is_enabled() {
                self.meter.clear();
            }
        }
        if !self.meter.is_connected() {
            return;
        }
        for (peak, value) in self.peaks.iter_mut().zip([input, reduction]) {
            if value.is_finite() {
                *peak = peak.max(value);
            }
        }
        self.frames += 1;
        if self.frames == self.period {
            self.meter.publish(self.peaks);
            self.frames = 0;
            self.peaks = [0.0; 2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_second_history_is_audio_clocked_at_every_project_rate_and_wraps_in_place() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let handle = Arc::new(LevelHistory::default());
            let mut writer = LevelHistoryWriter::new(rate, true);
            writer.attach_prepared(handle.clone());
            // A transient early in each bucket must survive until publication.
            for i in 0..rate * 4 {
                writer.sample(if i % (rate / 50) == 0 { 0.75 } else { 0.01 }, 6.0);
            }
            let full = handle.reading().unwrap();
            assert_eq!((full.count, full.position), (LEVEL_HISTORY_BINS, 0));
            assert!(full.bins.iter().all(|b| *b == [0.75, 6.0]));
            for _ in 0..rate / 50 {
                writer.sample(0.25, 3.0);
            }
            let next = handle.reading().unwrap();
            assert_eq!((next.count, next.position), (LEVEL_HISTORY_BINS, 1));
            assert_eq!(next.bins[0], [0.25, 3.0]);
            assert_eq!(next.bins[1..], full.bins[1..]);
        }
    }
    #[test]
    fn preparation_and_clones_do_not_publish_and_bypass_and_reset_clear() {
        let handle = Arc::new(LevelHistory::default());
        let mut writer = LevelHistoryWriter::new(48000, true);
        writer.attach_prepared(handle.clone());
        for _ in 0..960 {
            writer.sample(0.5, 9.0);
        }
        let before = handle.reading().unwrap();
        let mut clone = writer.clone();
        for _ in 0..960 {
            clone.sample(1.0, 20.0);
        }
        assert_eq!(handle.reading().unwrap(), before);
        let mut prepared = LevelHistoryWriter::new(48000, false);
        prepared.attach_prepared(handle.clone());
        assert_eq!(handle.reading().unwrap(), before);
        writer.set_enabled(false);
        assert_eq!(handle.reading().unwrap().count, 0);
        for _ in 0..960 {
            writer.sample(1.0, 20.0);
        }
        assert_eq!(handle.reading().unwrap().count, 0);
        writer.set_enabled(true);
        for _ in 0..960 {
            writer.sample(f32::NAN, f32::INFINITY);
        }
        assert_eq!(handle.reading().unwrap().bins[0], [0.0; 2]);
        writer.reset();
        assert_eq!(handle.reading().unwrap(), LevelHistoryReading::default());
    }
    #[test]
    fn snapshots_never_mix_bin_pairs_under_concurrent_publication() {
        let handle = Arc::new(LevelHistory::default());
        let producer = handle.clone();
        let thread = std::thread::spawn(move || {
            for i in 1..20000 {
                producer.store([i as f32, 2.0 * i as f32]);
            }
        });
        for _ in 0..500 {
            if let Some(reading) = handle.reading() {
                assert!(reading.position < LEVEL_HISTORY_BINS);
                for bin in reading.bins {
                    assert_eq!(bin[1], bin[0] * 2.0);
                }
            }
        }
        thread.join().unwrap();
    }
}
