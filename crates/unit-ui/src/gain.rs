use daw_unit_core::SILENT_GAIN_DB;
pub const MAX_GAIN_DB: f32 = 12.0;
pub const GAIN_MARKS: [f32; 13] = [
    SILENT_GAIN_DB,
    -40.0,
    -30.0,
    -20.0,
    -15.0,
    -10.0,
    -6.0,
    -3.0,
    0.0,
    3.0,
    6.0,
    9.0,
    MAX_GAIN_DB,
];

pub fn format_gain(db: f32) -> String {
    daw_unit_core::gain::format_db(f64::from(db))
}

pub(crate) fn linear_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
