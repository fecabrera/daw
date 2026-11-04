//! Stereo analyzer settings and bounded, lock-free display telemetry.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::*};
pub const LOUDNESS_TYPE_ID: &str = "daw.loudness-meter";
pub const LOUDNESS_STATE_VERSION: u32 = 1;
pub const LOUDNESS_HISTORY: usize = 180;
pub const LOUDNESS_SCALARS: usize = 14;
pub const LOUDNESS_VALUES: usize = LOUDNESS_SCALARS + 2 * LOUDNESS_HISTORY;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoudnessSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    /// Off, -14 LUFS, or -23 LUFS. A display reference only.
    pub target: u8,
    pub hq: bool,
    pub continuous: bool,
}
impl Default for LoudnessSettings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            collapsed: false,
            target: 1,
            hq: true,
            continuous: false,
        }
    }
}
impl LoudnessSettings {
    pub fn valid(&self) -> bool {
        self.version == LOUDNESS_STATE_VERSION && self.target <= 2
    }
    pub fn target_lufs(&self) -> Option<f32> {
        match self.target {
            1 => Some(-14.0),
            2 => Some(-23.0),
            _ => None,
        }
    }
}
/// M, S, I, maximum M/S/TP, LRA, low/high LRA bounds, TP, PSR, PLR,
/// elapsed seconds, incomplete flag; followed by oldest-first M and S history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoudnessReading {
    pub values: [f32; LOUDNESS_VALUES],
}
impl Default for LoudnessReading {
    fn default() -> Self {
        Self {
            values: [0.0; LOUDNESS_VALUES],
        }
    }
}
/// Workers publish without waiting; readers make at most three attempts. Controls use separate atomics.
/// Epoch checks mask queued observations and a worker publication racing with Reset/Off.
pub struct LoudnessMeter {
    sequence: AtomicU64,
    published_epoch: AtomicU64,
    epoch: AtomicU64,
    lost_epoch: AtomicU64,
    active: AtomicBool,
    hold: AtomicBool,
    values: [AtomicU32; LOUDNESS_VALUES],
}
impl Default for LoudnessMeter {
    fn default() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            published_epoch: AtomicU64::new(0),
            epoch: AtomicU64::new(1),
            lost_epoch: AtomicU64::new(0),
            active: AtomicBool::new(true),
            hold: AtomicBool::new(false),
            values: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
}
impl LoudnessMeter {
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Acquire)
    }
    pub fn mark_incomplete(&self, epoch: u64) {
        self.lost_epoch.store(epoch, Release);
    }
    pub fn reset(&self) {
        self.epoch.fetch_add(1, AcqRel);
    }
    pub fn held(&self) -> bool {
        self.hold.load(Acquire)
    }
    pub fn set_hold(&self, value: bool) {
        self.hold.store(value, Release);
    }
    pub fn set_enabled(&self, value: bool) {
        if self.active.swap(value, AcqRel) != value {
            self.hold.store(false, Release);
            self.reset();
        }
    }
    pub fn reading(&self) -> LoudnessReading {
        for _ in 0..3 {
            let epoch = self.epoch();
            let seq = self.sequence.load(Acquire);
            if seq & 1 != 0 {
                continue;
            }
            let mut values = std::array::from_fn(|i| f32::from_bits(self.values[i].load(Relaxed)));
            let published = self.published_epoch.load(Relaxed);
            std::sync::atomic::fence(Acquire);
            if seq == self.sequence.load(Acquire) && epoch == self.epoch() {
                if !self.active.load(Acquire) {
                    return LoudnessReading::default();
                }
                if published != epoch {
                    values = [0.0; LOUDNESS_VALUES];
                }
                if self.lost_epoch.load(Acquire) == epoch {
                    values[13] = 1.0;
                }
                return LoudnessReading { values };
            }
        }
        LoudnessReading::default()
    }
    /// Worker only. Epoch is captured by the audio producer for each observation.
    pub fn publish(&self, epoch: u64, reading: LoudnessReading) {
        let seq = self.sequence.load(Acquire);
        if seq & 1 != 0
            || self
                .sequence
                .compare_exchange(seq, seq.wrapping_add(1), AcqRel, Acquire)
                .is_err()
        {
            return;
        }
        for (a, v) in self.values.iter().zip(reading.values) {
            a.store(v.to_bits(), Relaxed);
        }
        self.published_epoch.store(epoch, Relaxed);
        self.sequence.fetch_add(1, Release);
    }
}
impl crate::MeterStorage for LoudnessMeter {
    type Reading = LoudnessReading;
    const ZERO: LoudnessReading = LoudnessReading {
        values: [0.0; LOUDNESS_VALUES],
    };
    fn store(&self, r: LoudnessReading) {
        self.publish(self.epoch(), r);
    }
    fn initialize(&self, _: LoudnessReading) {}
    fn clear(&self) {
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_and_bypass_mask_queued_epochs_and_incomplete_results() {
        let m = LoudnessMeter::default();
        let epoch = m.epoch();
        let mut r = LoudnessReading::default();
        r.values[0] = -23.0;
        r.values[12] = 1.0;
        m.publish(epoch, r);
        assert_eq!(m.reading(), r);
        m.reset();
        m.publish(epoch, r);
        assert_eq!(m.reading(), Default::default());
        m.publish(m.epoch(), r);
        m.mark_incomplete(m.epoch());
        assert_eq!(m.reading().values[13], 1.0);
        m.set_hold(true);
        m.set_enabled(false);
        assert!(!m.held());
        m.publish(m.epoch(), r);
        assert_eq!(m.reading(), Default::default());
        m.set_enabled(true);
        assert_eq!(m.reading(), Default::default());
    }
    #[test]
    fn settings_roundtrip_and_future_payloads_remain_missing_units() {
        let unit = crate::EffectInstance::loudness();
        let encoded = serde_json::to_value(&unit).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(encoded).unwrap(),
            unit
        );
        let mut s = LoudnessSettings {
            target: 3,
            ..Default::default()
        };
        assert!(!s.valid());
        s.target = 1;
        s.version = 2;
        let e = serde_json::json!({"type_id":LOUDNESS_TYPE_ID,"state":s});
        let unknown: crate::EffectSettings = serde_json::from_value(e).unwrap();
        assert!(matches!(unknown, crate::EffectSettings::Missing { .. }));
        assert!(unknown.valid());
    }
}
