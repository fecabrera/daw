//! Scalar gain laws for unit DSP.
pub const SILENT_GAIN_DB: f32 = -1000.0;

pub fn amplitude(db: f32) -> f32 {
    if db <= SILENT_GAIN_DB {
        0.0
    } else {
        10_f32.powf(db / 20.0)
    }
}

/// Ratio between two MIDI notes, independent of their absolute tuning.
pub fn pitch_ratio(note: u8, root: u8) -> f64 {
    2_f64.powf(f64::from(i16::from(note) - i16::from(root)) / 12.0)
}

pub fn voice_gain(level: f32, mix: f32, factor: f32, gate: f32) -> f32 {
    if mix >= 1.0 {
        factor * level
    } else if mix <= 0.0 {
        gate
    } else {
        gate + (factor * level - gate) * mix
    }
}

pub fn wet_controls(b: [f32; 2], o: [f32; 2]) -> [f32; 2] {
    [
        if b[0] == 0.0 {
            SILENT_GAIN_DB
        } else {
            (20.0 * b[0].log10() + o[0]).clamp(SILENT_GAIN_DB, 12.0)
        },
        (b[1] + o[1]).clamp(0.0, 1.0),
    ]
}
