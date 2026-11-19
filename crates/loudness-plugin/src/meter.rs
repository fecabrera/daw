//! Shared plugin meter storage. The portable DSP owns publication and bypass clearing.
use daw_unit_core::{BandMeter, MeterStorage, PeakHold, PeakMeter, PluginMeterPrefix};
use std::sync::Arc;
#[derive(Default)]
pub struct OutputMeter {
    pub output: Arc<BandMeter<3>>,
    pub loudness: Arc<daw_unit_core::LoudnessMeter>,
    pub reduction: Arc<PeakMeter>,
    pub bands: Arc<BandMeter<3>>,
}
impl OutputMeter {
    fn prefix(&self) -> PluginMeterPrefix<'_> {
        PluginMeterPrefix {
            output: &self.output,
            reduction: &self.reduction,
            bands: &self.bands,
        }
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
pub const METER_VALUES: usize = 14 + daw_unit_core::LOUDNESS_VALUES;
pub fn meter_values(m: &OutputMeter) -> [f64; METER_VALUES] {
    let mut v = [0.0; METER_VALUES];
    v[..PluginMeterPrefix::VALUES].copy_from_slice(&m.prefix().encode());
    // The existing format bridge accepts finite, non-negative scalar attributes.
    // Offset LUFS/dB values; reserve zero for negative infinity.
    for (out, value) in v[13..].iter_mut().zip(m.loudness.reading().values) {
        *out = if value.is_finite() {
            f64::from(value) + 1000.0
        } else {
            0.0
        };
    }
    v[METER_VALUES - 1] = f64::from(m.loudness.held());
    v
}
pub fn receive_meter(m: &OutputMeter, v: [f32; METER_VALUES]) {
    let reading = if v[13..13 + daw_unit_core::LOUDNESS_VALUES]
        .iter()
        .all(|v| *v == 0.0)
    {
        daw_unit_core::LoudnessReading::default()
    } else {
        daw_unit_core::LoudnessReading {
            values: std::array::from_fn(|i| {
                if v[13 + i] == 0.0 {
                    f32::NEG_INFINITY
                } else {
                    v[13 + i] - 1000.0
                }
            }),
        }
    };
    m.loudness.publish(m.loudness.epoch(), reading);
    m.loudness.set_hold(v[METER_VALUES - 1] != 0.0);
    m.prefix()
        .receive(v[..PluginMeterPrefix::VALUES].try_into().unwrap());
}
/// Bits select independent held readings; current readings remain untouched.
pub fn reset_meter(m: &OutputMeter, mask: u32) {
    if mask & 256 != 0 {
        m.loudness.reset();
    }
    if mask & 512 != 0 {
        m.loudness.set_hold(true);
    }
    if mask & 1024 != 0 {
        m.loudness.set_hold(false);
    }
    m.prefix().reset(mask);
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
    }
}

impl daw_unit_core::BandAudition for OutputMeter {}
