use serde::{Deserialize, Serialize};

pub const SHIMMER_REVERB_TYPE_ID: &str = "daw.shimmer-reverb";
pub const SHIMMER_REVERB_STATE_VERSION: u32 = 1;

/// Thirty-one intervals: quarter-semitone steps near zero, integer semitones outside ±1 st.
pub fn shimmer_pitch(index: i8) -> f32 {
    let n = f32::from(index.clamp(-15, 15));
    n.signum() * (n.abs() * 0.25).max(n.abs() - 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_bounds_future_payload_and_discrete_intervals() {
        let effect = crate::EffectInstance::shimmer_reverb();
        let value = serde_json::to_value(&effect).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::EffectInstance>(value).unwrap(),
            effect
        );
        assert_eq!(
            (-15..=15).map(shimmer_pitch).collect::<Vec<_>>(),
            vec![
                -12.0, -11.0, -10.0, -9.0, -8.0, -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, -0.75,
                -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0,
                10.0, 11.0, 12.0
            ]
        );
        let mut future = serde_json::to_value(&effect.unit).unwrap();
        future["state"]["version"] = 99.into();
        let loaded: crate::EffectSettings = serde_json::from_value(future.clone()).unwrap();
        assert!(matches!(loaded, crate::EffectSettings::Missing { .. }));
        assert_eq!(serde_json::to_value(loaded).unwrap(), future);
        for (key, bad) in [
            ("size", -0.1),
            ("predelay_ms", 1000.1),
            ("note", 16.0),
            ("pitch_1", 16.0),
            ("pitch_2", -16.0),
            ("level_db", 12.1),
        ] {
            let mut v = serde_json::to_value(ShimmerReverbSettings::default()).unwrap();
            v[key] = serde_json::json!(bad);
            assert!(
                serde_json::from_value::<ShimmerReverbSettings>(v).map_or(true, |s| !s.valid())
            );
        }
        let mut s = ShimmerReverbSettings {
            size: f32::NAN,
            ..Default::default()
        };
        assert!(!s.valid());
        s.size = 0.5;
        s.modulations
            .push(crate::Modulation::new(crate::ModulationParameter::Pitch));
        assert!(!s.valid());
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShimmerReverbSettings {
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub size: f32,
    pub predelay_ms: f32,
    pub sync: bool,
    /// Index in NOTE_BEATS. Retained independently from free-time pre-delay.
    pub note: u8,
    pub vowel: f32,
    pub vowel_amount: f32,
    pub pitch_1: i8,
    pub amount_1: f32,
    pub pitch_2: i8,
    pub amount_2: f32,
    pub rate: f32,
    pub depth: f32,
    pub low_damp: f32,
    pub high_damp: f32,
    pub warm: bool,
    pub level_db: f32,
    pub dry_wet: f32,
}
impl Default for ShimmerReverbSettings {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            collapsed: false,
            modulation_open: false,
            modulations: Vec::new(),
            size: 0.75,
            predelay_ms: 0.0,
            sync: false,
            note: 6,
            vowel: 0.0,
            vowel_amount: 0.0,
            pitch_1: 0,
            amount_1: 0.0,
            pitch_2: 0,
            amount_2: 0.0,
            rate: 0.390625,
            depth: 0.429688,
            low_damp: 0.0,
            high_damp: 0.0,
            warm: false,
            level_db: 0.0,
            dry_wet: 0.5,
        }
    }
}
impl ShimmerReverbSettings {
    pub const MAX_TAIL_SECONDS: u32 = 120;
    pub const MAX_PREDELAY_MS: f32 = 8000.0;
    pub const NOTE_BEATS: [f32; 16] = [
        0.0625,
        0.125,
        1.0 / 6.0,
        0.1875,
        0.25,
        1.0 / 3.0,
        0.375,
        0.5,
        2.0 / 3.0,
        0.75,
        1.0,
        4.0 / 3.0,
        1.5,
        2.0,
        8.0 / 3.0,
        3.0,
    ];
    pub const NOTE_LABELS: [&'static str; 16] = [
        "1/64", "1/32", "1/16T", "1/32D", "1/16", "1/8T", "1/16D", "1/8", "1/4T", "1/8D", "1/4",
        "1/2T", "1/4D", "1/2", "1/1T", "1/2D",
    ];
    /// Shared destination order. Pitch, mode, note and Warm remain discrete.
    pub fn values(&self) -> [f32; 12] {
        [
            self.size,
            self.predelay_ms,
            self.vowel,
            self.vowel_amount,
            self.amount_1,
            self.amount_2,
            self.rate,
            self.depth,
            self.low_damp,
            self.high_damp,
            self.level_db,
            self.dry_wet,
        ]
    }
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.note < 16
            && (-15..=15).contains(&self.pitch_1)
            && (-15..=15).contains(&self.pitch_2)
            && self.values().into_iter().enumerate().all(|(i, v)| {
                let (lo, hi) = match i {
                    1 => (0.0, 1000.0),
                    10 => (crate::SILENT_GAIN_DB, 12.0),
                    _ => (0.0, 1.0),
                };
                v.is_finite() && (lo..=hi).contains(&v)
            })
            && crate::ModulationDestinationSettings::valid_modulations(self)
    }
    pub fn tail_frames(&self, rate: u32) -> u64 {
        if self.enabled {
            u64::from(rate) * u64::from(Self::MAX_TAIL_SECONDS)
        } else {
            0
        }
    }
}
