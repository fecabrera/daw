//! Compressor-only extension. Other products keep the shared thirteen-value prefix.
#[path = "meter.rs"]
mod common;
use daw_unit_core::{BandMeter, LEVEL_HISTORY_BINS, LevelHistoryReading, MeterStorage};
use std::sync::Arc;
pub struct OutputMeter {
    common: common::OutputMeter,
}
impl Default for OutputMeter {
    fn default() -> Self {
        Self {
            common: common::OutputMeter {
                bands: Arc::new(BandMeter::with_history()),
                ..Default::default()
            },
        }
    }
}
impl std::ops::Deref for OutputMeter {
    type Target = common::OutputMeter;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
pub const METER_VALUES: usize = 15 + 2 * LEVEL_HISTORY_BINS;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    let mut values = [0.0; METER_VALUES];
    values[..common::METER_VALUES].copy_from_slice(&common::meter_values(&m.common));
    if let Some(history) = m.bands.history().and_then(|h| h.reading()) {
        values[13] = history.position as f64;
        values[14] = history.count as f64;
        for (i, bin) in history.bins.iter().enumerate() {
            values[15 + 2 * i] = f64::from(bin[0]);
            values[16 + 2 * i] = f64::from(bin[1]);
        }
    }
    values
}
pub fn receive_meter(m: &OutputMeter, values: [f32; METER_VALUES]) {
    common::receive_meter(&m.common, std::array::from_fn(|i| values[i]));
    if let Some(history) = m.bands.history() {
        let valid = values[13].is_finite()
            && values[13] >= 0.0
            && values[13].fract() == 0.0
            && values[14].is_finite()
            && values[14] >= 0.0
            && values[14].fract() == 0.0;
        history.receive(LevelHistoryReading {
            position: if valid {
                values[13] as usize
            } else {
                usize::MAX
            },
            count: values[14] as usize,
            bins: std::array::from_fn(|i| [values[15 + 2 * i], values[16 + 2 * i]]),
        });
    }
}
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    common::reset_meter(&m.common, mask);
}
impl MeterStorage for OutputMeter {
    type Reading = [f32; 3];
    const ZERO: Self::Reading = [0.0; 3];
    fn store(&self, value: Self::Reading) {
        self.common.store(value);
    }
    fn initialize(&self, value: Self::Reading) {
        self.common.initialize(value);
    }
    fn clear(&self) {
        self.common.clear();
    }
}
impl daw_unit_core::BandAudition for OutputMeter {
    fn clear_live_preview(&self) {
        self.bands.clear_levels();
    }
}
