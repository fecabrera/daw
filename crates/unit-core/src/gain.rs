//! Device-free gain scales shared by DAW controls and external host parameters.
//! Host mappings use f64 to retain finite dB values below f32 amplitude range.
//! Callers validate finite values and their parameter bounds before scale conversion.
use crate::SILENT_GAIN_DB;

pub const MAX_DB: f64 = 12.0;
const SCALE: [(f64, f64); 10] = [
    (-60.0, 0.08),
    (-40.0, 0.16),
    (-30.0, 0.28),
    (-20.0, 0.40),
    (-15.0, 0.48),
    (-10.0, 0.56),
    (-6.0, 0.64),
    (-3.0, 0.72),
    (0.0, 0.80),
    (MAX_DB, 1.0),
];

pub fn format_db(db: f64) -> String {
    if db <= f64::from(SILENT_GAIN_DB) {
        "−∞".into()
    } else if db < f64::from(SILENT_GAIN_DB) + 0.1 {
        // A one-decimal readout can round a finite value to the silence sentinel.
        db.to_string()
    } else {
        format!("{db:.1}")
    }
}

/// Parses finite dB or explicit silence. Callers enforce their own parameter bounds.
pub fn parse_db(text: &str) -> Option<f64> {
    let text = text.trim().to_lowercase();
    let text = text.strip_suffix("db").unwrap_or(&text).trim();
    if matches!(text, "−∞" | "-∞" | "-inf" | "-infinity") {
        return Some(f64::from(SILENT_GAIN_DB));
    }
    text.parse::<f64>().ok().filter(|db| db.is_finite())
}

/// Round a validated finite dB value without rounding a non-sentinel into silence.
pub fn to_db_f32(db: f64) -> f32 {
    if db > f64::from(SILENT_GAIN_DB) {
        (db as f32).max(SILENT_GAIN_DB.next_up())
    } else {
        db as f32
    }
}

pub fn fader_position(db: f64) -> f64 {
    if db <= f64::from(SILENT_GAIN_DB) {
        return 0.0;
    }
    if db < SCALE[0].0 {
        return 10.0_f64.powf((db - SCALE[0].0) / 20.0) * SCALE[0].1;
    }
    for pair in SCALE.windows(2) {
        if db <= pair[1].0 {
            return pair[0].1
                + (pair[1].1 - pair[0].1) * (db - pair[0].0) / (pair[1].0 - pair[0].0);
        }
    }
    1.0
}

pub fn fader_db(position: f64) -> f64 {
    let position = position.clamp(0.0, 1.0);
    if position == 0.0 {
        return f64::from(SILENT_GAIN_DB);
    }
    if position < SCALE[0].1 {
        // Positive host values below the supported tail stay finite, not silence.
        return (SCALE[0].0 + 20.0 * (position / SCALE[0].1).log10())
            .max(f64::from(SILENT_GAIN_DB).next_up());
    }
    for pair in SCALE.windows(2) {
        if position <= pair[1].1 {
            return pair[0].0
                + (pair[1].0 - pair[0].0) * (position - pair[0].1) / (pair[1].1 - pair[0].1);
        }
    }
    MAX_DB
}

/// The shared gain knob puts unity at twelve o'clock (normalized 0.5).
pub fn knob_position(db: f64) -> f64 {
    if db >= MAX_DB {
        return 1.0;
    }
    let position = fader_position(db);
    if position <= 0.8 {
        position * 0.625
    } else {
        0.5 + (position - 0.8) * 2.5
    }
}

pub fn knob_db(position: f64) -> f64 {
    if position >= 1.0 {
        return MAX_DB;
    }
    let position = position.clamp(0.0, 1.0);
    fader_db(if position <= 0.5 {
        position * 1.6
    } else {
        0.8 + (position - 0.5) * 0.4
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_tail_never_becomes_silence_in_host_mapping_or_text() {
        for db in [
            -999.9999, -999.0, -950.0, -900.0, -300.0, -90.0, -60.0, -1.0, 0.0, 12.0,
        ] {
            assert!(knob_position(db) > 0.0);
            assert!((knob_db(knob_position(db)) - db).abs() < 1e-10);
            assert_ne!(format_db(db), "−∞");
            assert!(parse_db(&format_db(db)).unwrap() > f64::from(SILENT_GAIN_DB));
        }
        assert_eq!(knob_db(0.0), f64::from(SILENT_GAIN_DB));
        assert_eq!(knob_position(0.0), 0.5);
        assert_eq!(knob_position(12.0), 1.0);
        assert_eq!(knob_db(1.0), 12.0);
        assert!(knob_db(1e-300) > f64::from(SILENT_GAIN_DB));
    }
}
