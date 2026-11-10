//! Preserve the common telemetry prefix and append six branch levels and holds.
#[path = "../../compressor-plugin/src/meter.rs"]
mod common;
use daw_unit_core::{BandMeter, MeterStorage};
use std::sync::Arc;

#[derive(Default)]
pub struct OutputMeter {
    common: common::OutputMeter,
    pub branches: [Arc<BandMeter<3>>; 6],
}
impl std::ops::Deref for OutputMeter {
    type Target = common::OutputMeter;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
pub const METER_VALUES: usize = common::METER_VALUES + 12;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    let mut values = [0.; METER_VALUES];
    values[..common::METER_VALUES].copy_from_slice(&common::meter_values(&m.common));
    for (i, band) in m.branches.iter().enumerate() {
        values[common::METER_VALUES + i * 2] = f64::from(band.readings()[0]);
        values[common::METER_VALUES + i * 2 + 1] = f64::from(band.boost_peaks()[0].peak());
    }
    values
}
pub fn receive_meter(m: &OutputMeter, values: [f32; METER_VALUES]) {
    common::receive_meter(&m.common, std::array::from_fn(|i| values[i]));
    for (i, band) in m.branches.iter().enumerate() {
        band.clear();
        band.store([values[common::METER_VALUES + i * 2 + 1], 0., 0.]);
        band.initialize([values[common::METER_VALUES + i * 2], 0., 0.]);
    }
}
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    common::reset_meter(&m.common, mask);
    for (i, band) in m.branches.iter().enumerate() {
        if mask & (32 << i) != 0 {
            band.boost_peaks()[0].reset_peak();
        }
    }
}
impl MeterStorage for OutputMeter {
    type Reading = [f32; 3];
    const ZERO: Self::Reading = [0.; 3];
    fn store(&self, v: Self::Reading) {
        self.common.store(v);
    }
    fn initialize(&self, v: Self::Reading) {
        self.common.initialize(v);
    }
    fn clear(&self) {
        self.common.clear();
        for band in &self.branches {
            band.clear();
        }
    }
}
impl daw_unit_core::BandAudition for OutputMeter {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branch_telemetry_roundtrips_and_resets_one_hold_without_touching_currents() {
        let source = OutputMeter::default();
        for (i, band) in source.branches.iter().enumerate() {
            band.store([i as f32 + 1., 0., 0.]);
            band.initialize([0.25, 0., 0.]);
        }
        let values = meter_values(&source);
        assert_eq!(METER_VALUES, 25);
        let received = OutputMeter::default();
        receive_meter(&received, values.map(|v| v as f32));
        assert_eq!(meter_values(&received), values);
        reset_meter(&received, 32 << 4);
        for (i, band) in received.branches.iter().enumerate() {
            assert_eq!(band.readings()[0], 0.25);
            assert_eq!(
                band.boost_peaks()[0].peak(),
                if i == 4 { 0. } else { i as f32 + 1. }
            );
        }
        received.clear();
        assert_eq!(meter_values(&received), [0.; METER_VALUES]);
    }
}
