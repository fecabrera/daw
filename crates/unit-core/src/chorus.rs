use serde::{Deserialize, Serialize};
pub const CHORUS_TYPE_ID: &str = "daw.chorus";
pub const CHORUS_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChorusSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub delay_ms: f32,
    pub depth: f32,
    pub feedback: f32,
    pub rate_hz: f32,
    /// Missing fields retain the original free-running Hz behavior.
    #[serde(default = "hz_mode")]
    pub rate_mode: crate::LfoRateMode,
    #[serde(default)]
    pub note_rate: crate::NoteRate,
    pub stereo_phase_degrees: f32,
    pub voices: u8,
    pub width: f32,
    pub level_db: f32,
    pub dry_wet: f32,
}
fn hz_mode() -> crate::LfoRateMode {
    crate::LfoRateMode::Hz
}
impl Default for ChorusSettings {
    fn default() -> Self {
        Self {
            version: CHORUS_STATE_VERSION,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            delay_ms: 15.0,
            depth: 0.25,
            feedback: 0.0,
            rate_hz: 0.8,
            rate_mode: hz_mode(),
            note_rate: crate::NoteRate::default(),
            stereo_phase_degrees: 90.0,
            voices: 2,
            width: 1.0,
            level_db: 0.0,
            dry_wet: 0.35,
        }
    }
}
impl ChorusSettings {
    /// Continuous values in the shared CHORUS destination order.
    pub fn values(&self) -> [f32; 8] {
        [
            self.delay_ms,
            self.depth,
            self.feedback,
            self.rate_hz,
            self.stereo_phase_degrees,
            self.width,
            self.level_db,
            self.dry_wet,
        ]
    }
    pub fn base_rate_hz(&self, tempo_bpm: f32) -> f64 {
        match self.rate_mode {
            crate::LfoRateMode::Hz => f64::from(self.rate_hz),
            crate::LfoRateMode::Note => f64::from(tempo_bpm) / (60.0 * self.note_rate.beats()),
        }
    }
    pub fn rate_bounds(&self, tempo_bpm: f32) -> [f64; 2] {
        match self.rate_mode {
            crate::LfoRateMode::Hz => [0.05, 10.0],
            crate::LfoRateMode::Note => [crate::NoteRate(1), crate::NoteRate(26)]
                .map(|r| f64::from(tempo_bpm) / (60.0 * r.beats())),
        }
    }
    pub fn valid(&self) -> bool {
        let bounds = [
            (5.0, 30.0),
            (0.0, 1.0),
            (-0.8, 0.8),
            (0.05, 10.0),
            (-180.0, 180.0),
            (0.0, 2.0),
            (crate::SILENT_GAIN_DB, 12.0),
            (0.0, 1.0),
        ];
        self.version == CHORUS_STATE_VERSION
            && self.note_rate.0 < crate::NoteRate::COUNT
            && (1..=4).contains(&self.voices)
            && self
                .values()
                .into_iter()
                .zip(bounds)
                .all(|(v, (lo, hi))| v.is_finite() && (lo..=hi).contains(&v))
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chorus_bounds_roundtrip_and_future_state_retention() {
        let effect = crate::EffectInstance::chorus();
        assert!(effect.unit.valid());
        let saved = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(saved).unwrap(),
            effect
        );
        let defaults = serde_json::to_value(ChorusSettings::default()).unwrap();
        for (name, lo, hi) in [
            ("delay_ms", 5.0, 30.0),
            ("depth", 0.0, 1.0),
            ("feedback", -0.8, 0.8),
            ("rate_hz", 0.05, 10.0),
            ("stereo_phase_degrees", -180.0, 180.0),
            ("width", 0.0, 2.0),
            ("level_db", crate::SILENT_GAIN_DB, 12.0),
            ("dry_wet", 0.0, 1.0),
        ] {
            for (v, valid) in [(lo, true), (hi, true), (lo - 1.0, false), (hi + 1.0, false)] {
                let mut payload = defaults.clone();
                payload[name] = serde_json::json!(v);
                assert_eq!(
                    serde_json::from_value::<ChorusSettings>(payload)
                        .unwrap()
                        .valid(),
                    valid,
                    "{name}: {v}"
                );
            }
        }
        for voices in [0, 5, 255] {
            assert!(
                !ChorusSettings {
                    voices,
                    ..Default::default()
                }
                .valid()
            );
        }
        let mut payload = defaults.clone();
        payload["voices"] = serde_json::json!(1.5);
        assert!(serde_json::from_value::<ChorusSettings>(payload).is_err());
        let mut saved = serde_json::to_value(&effect.unit).unwrap();
        saved["state"]["version"] = serde_json::json!(99);
        let loaded: crate::EffectSettings = serde_json::from_value(saved.clone()).unwrap();
        assert!(matches!(loaded, crate::EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(loaded).unwrap(), saved);
        assert!(
            !ChorusSettings {
                rate_hz: f32::NAN,
                ..Default::default()
            }
            .valid()
        );
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    #[test]
    fn legacy_hz_and_synced_note_settings_are_valid_and_round_trip() {
        let mut legacy = serde_json::to_value(ChorusSettings::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("rate_mode");
        legacy.as_object_mut().unwrap().remove("note_rate");
        let loaded: ChorusSettings = serde_json::from_value(legacy).unwrap();
        assert_eq!(loaded, ChorusSettings::default());
        for note in 0..crate::NoteRate::COUNT {
            let settings = ChorusSettings {
                rate_mode: crate::LfoRateMode::Note,
                note_rate: crate::NoteRate(note),
                ..loaded.clone()
            };
            assert!(settings.valid());
            assert_eq!(
                serde_json::from_str::<ChorusSettings>(&serde_json::to_string(&settings).unwrap())
                    .unwrap(),
                settings
            );
        }
        assert!(
            !ChorusSettings {
                note_rate: crate::NoteRate(27),
                ..loaded
            }
            .valid()
        );
    }
}
