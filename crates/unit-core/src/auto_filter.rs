use serde::{Deserialize, Serialize};

pub const AUTO_FILTER_TYPE_ID: &str = "daw.auto-filter";
pub const AUTO_FILTER_STATE_VERSION: u32 = 1;

macro_rules! choices {
    ($name:ident, $default:ident, $($variant:ident => $label:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
        impl Default for $name { fn default() -> Self { Self::$default } }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub fn index(self) -> usize { Self::ALL.iter().position(|value| *value == self).unwrap() }
            pub fn name(self) -> &'static str { match self { $(Self::$variant => $label),+ } }
        }
    };
}
choices!(FilterShape, LowPass, LowPass => "Low-pass", HighPass => "High-pass", BandPass => "Band-pass", Notch => "Notch");
choices!(LfoWave, Sine, Sine => "Sine", Triangle => "Triangle", SawUp => "Saw up", SawDown => "Saw down", Square => "Square");
choices!(LfoRateMode, Note, Hz => "Hz", Note => "Note");

/// Index selects 4 bars, 2 bars, 1 bar, 1/2, 1/4, 1/8, 1/16, 1/32, or 1/64.
/// Each duration has straight, dotted, and triplet forms, in that order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteRate(pub u8);
impl Default for NoteRate {
    fn default() -> Self {
        Self(12)
    }
}
impl NoteRate {
    pub const COUNT: u8 = 27;
    pub fn beats(self) -> f64 {
        let straight = 16.0 / 2_f64.powi(i32::from(self.0 / 3));
        straight
            * match self.0 % 3 {
                1 => 1.5,
                2 => 2.0 / 3.0,
                _ => 1.0,
            }
    }
    pub fn label(self) -> String {
        let base = [
            "4 bars", "2 bars", "1 bar", "1/2", "1/4", "1/8", "1/16", "1/32", "1/64",
        ][(self.0 / 3).min(8) as usize];
        format!(
            "{base}{}",
            match self.0 % 3 {
                1 => " D",
                2 => " T",
                _ => "",
            }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoFilterSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    /// Saved visibility of this destination's modulation controls.
    #[serde(default)]
    pub modulation_open: bool,
    pub shape: FilterShape,
    pub frequency_hz: f32,
    pub q: f32,
    pub lfo_enabled: bool,
    pub wave: LfoWave,
    pub rate_mode: LfoRateMode,
    /// Both rate values are retained when the mode changes.
    pub rate_hz: f32,
    pub note_rate: NoteRate,
    /// Bipolar sweep depth: frequency * 2^(strength * LFO).
    pub strength_octaves: f32,
    pub sync_phase_degrees: f32,
    /// Signed right-channel offset relative to the left channel.
    pub stereo_phase_degrees: f32,
    pub level_db: f32,
    pub dry_wet: f32,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
}
impl Default for AutoFilterSettings {
    fn default() -> Self {
        Self {
            version: AUTO_FILTER_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            shape: FilterShape::LowPass,
            frequency_hz: 1000.0,
            q: std::f32::consts::FRAC_1_SQRT_2,
            lfo_enabled: true,
            wave: LfoWave::Sine,
            rate_mode: LfoRateMode::Note,
            rate_hz: 2.0,
            note_rate: NoteRate::default(),
            strength_octaves: 1.0,
            sync_phase_degrees: 0.0,
            stereo_phase_degrees: 0.0,
            level_db: 0.0,
            dry_wet: 1.0,
            modulations: Vec::new(),
        }
    }
}
impl AutoFilterSettings {
    pub fn valid(&self) -> bool {
        fn bounded(value: f32, min: f32, max: f32) -> bool {
            value.is_finite() && (min..=max).contains(&value)
        }
        self.version == AUTO_FILTER_STATE_VERSION
            && bounded(self.frequency_hz, 20.0, 20_000.0)
            && bounded(self.q, 0.1, 10.0)
            && bounded(self.rate_hz, 0.01, 20.0)
            && self.note_rate.0 < NoteRate::COUNT
            && bounded(self.strength_octaves, 0.0, 6.0)
            && bounded(self.sync_phase_degrees, -180.0, 180.0)
            && bounded(self.stereo_phase_degrees, -180.0, 180.0)
            && bounded(self.level_db, crate::SILENT_GAIN_DB, 12.0)
            && bounded(self.dry_wet, 0.0, 1.0)
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}
