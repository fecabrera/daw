//! Coherent control-thread snapshots and a portable schema-2 spectrum suffix.
use daw_unit_core::MeterStorage;
pub const VALUES: usize = 527;
const SPECTRUM_FIRST: usize = 1;
const SPECTRUM_SCHEMA: usize = 513;
const SPECTRUM_VALID: usize = 514;
const SPECTRUM_GENERATION: usize = 515;
const SPECTRUM_UPPER_END: usize = 519;
const SPECTRUM_BASS_END: usize = 523;
#[derive(Default)]
pub struct TelemetryCache {
    last: daw_unit_core::SpectrumMeter,
    epoch: std::sync::atomic::AtomicU64,
    writing: std::sync::atomic::AtomicBool,
}
impl TelemetryCache {
    pub fn read(
        &self,
        source: &daw_unit_core::SpectrumMeter,
        rate: u32,
    ) -> daw_unit_core::SpectrumReading {
        use std::sync::atomic::Ordering::SeqCst;
        let empty = || daw_unit_core::SpectrumReading {
            sample_rate_hz: rate,
            ..Default::default()
        };
        let epoch = source.clear_epoch();
        if let Some(frame) = source.try_reading() {
            if frame.valid_branches == 0 {
                self.epoch.store(0, SeqCst);
            } else if self
                .writing
                .compare_exchange(false, true, SeqCst, SeqCst)
                .is_ok()
            {
                self.last.store(frame);
                self.epoch.store(epoch, SeqCst);
                self.writing.store(false, SeqCst);
            }
            return if source.clear_epoch() == epoch {
                if frame.valid_branches == 0 {
                    daw_unit_core::SpectrumReading {
                        sample_rate_hz: rate,
                        ..frame
                    }
                } else {
                    frame
                }
            } else {
                empty()
            };
        }
        if self.epoch.load(SeqCst) == epoch
            && let Some(frame) = self.last.try_reading()
            && source.clear_epoch() == epoch
        {
            return frame;
        }
        empty()
    }
}
fn put_clock(values: &mut [f64], first: usize, clock: u64) {
    for i in 0..4 {
        values[first + i] = ((clock >> (16 * i)) & 0xffff) as f64;
    }
}
fn clock(values: &[f32], first: usize) -> Option<u64> {
    let mut clock = 0;
    for i in 0..4 {
        let word = values[first + i];
        if !word.is_finite() || word.fract() != 0.0 || !(0.0..65536.0).contains(&word) {
            return None;
        }
        clock |= u64::from(word as u16) << (16 * i);
    }
    Some(clock)
}
pub fn encode(v: &mut [f64], spectrum: daw_unit_core::SpectrumReading) {
    assert_eq!(v.len(), VALUES, "prepared spectrum suffix");
    v[0] = f64::from(spectrum.sample_rate_hz);
    for i in 0..daw_unit_core::SPECTRUM_BINS {
        v[SPECTRUM_FIRST + i] = f64::from(spectrum.magnitudes[i]);
    }
    v[SPECTRUM_SCHEMA] = 2.0;
    v[SPECTRUM_VALID] = f64::from(spectrum.valid_branches);
    put_clock(v, SPECTRUM_GENERATION, spectrum.generation);
    put_clock(v, SPECTRUM_UPPER_END, spectrum.upper_end_frame);
    put_clock(v, SPECTRUM_BASS_END, spectrum.bass_end_frame);
}
pub fn decode(v: &[f32]) -> Option<daw_unit_core::SpectrumReading> {
    if v.len() != VALUES
        || v[SPECTRUM_SCHEMA] != 2.0
        || ![0.0, 1.0, 3.0].contains(&v[SPECTRUM_VALID])
        || ![44100u32, 48000, 88200, 96000, 176400, 192000]
            .iter()
            .any(|rate| *rate as f32 == v[0])
        || v.iter().any(|value| !value.is_finite())
        || v[SPECTRUM_FIRST..SPECTRUM_SCHEMA]
            .iter()
            .any(|value| *value < 0.0)
    {
        return None;
    }
    let reading = daw_unit_core::SpectrumReading {
        sample_rate_hz: v[0] as u32,
        magnitudes: std::array::from_fn(|i| v[SPECTRUM_FIRST + i]),
        valid_branches: v[SPECTRUM_VALID] as u32,
        generation: clock(v, SPECTRUM_GENERATION)?,
        upper_end_frame: clock(v, SPECTRUM_UPPER_END)?,
        bass_end_frame: clock(v, SPECTRUM_BASS_END)?,
    };
    if reading.valid_branches != 0 && (reading.generation == 0 || reading.upper_end_frame == 0) {
        return None;
    }
    if reading.valid_branches & daw_unit_core::SPECTRUM_BASS_READY != 0
        && (reading.bass_end_frame == 0 || reading.bass_end_frame > reading.upper_end_frame)
    {
        return None;
    }
    Some(reading)
}
