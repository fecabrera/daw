//! Portable sampler settings. Frame ranges refer to the imported clip's local clock.
use crate::{ClipLoop, ClipStretch, SILENT_GAIN_DB};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplerMode {
    #[default]
    Off,
    Time,
    Bars,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplerTrigger {
    #[default]
    OneShot,
    Gate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplerInput {
    pub asset_id: Uuid,
    pub name: String,
    pub source_offset_frame: u64,
    pub length_frames: u64,
    pub repeat: Option<ClipLoop>,
    pub stretch: Option<ClipStretch>,
}
impl SamplerInput {
    pub fn valid(&self) -> bool {
        !self.asset_id.is_nil()
            && self.length_frames > 0
            && self
                .source_offset_frame
                .checked_add(self.repeat.map_or(self.length_frames, |r| r.length_frames))
                .is_some()
            && self
                .repeat
                .is_none_or(|r| r.length_frames > 0 && r.phase_frame < r.length_frames)
            && self.stretch.is_none_or(ClipStretch::valid)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplerSettings {
    pub id: Uuid,
    pub version: u32,
    pub enabled: bool,
    pub collapsed: bool,
    pub input: Option<SamplerInput>,
    pub start_frame: u64,
    pub end_frame: u64,
    pub gain_db: f32,
    #[serde(default)]
    pub modulation_open: bool,
    #[serde(default)]
    pub modulations: Vec<crate::Modulation>,
    pub trigger: SamplerTrigger,
    pub mode: SamplerMode,
    pub time_ms: f64,
    /// Whole four-beat bars, at least one.
    #[serde(deserialize_with = "whole_bars")]
    pub bars: u32,
    /// MIDI note that plays the sample at its original pitch.
    pub root_note: u8,
}
// Accept numeric durations saved by the initial fractional-bar prototype.
fn whole_bars<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || value <= 0.0 || value.round() > f64::from(u32::MAX) {
        return Err(serde::de::Error::custom(
            "Sampler bars must be positive whole bars",
        ));
    }
    Ok(value.round().max(1.0) as u32)
}
impl Default for SamplerSettings {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            version: 1,
            enabled: true,
            collapsed: false,
            input: None,
            start_frame: 0,
            end_frame: 0,
            gain_db: 0.0,
            modulation_open: false,
            modulations: Vec::new(),
            trigger: SamplerTrigger::OneShot,
            mode: SamplerMode::Off,
            time_ms: 1000.0,
            bars: 1,
            root_note: 60,
        }
    }
}
impl SamplerSettings {
    /// Full MIDI range, relative to the saved root note.
    pub fn pitch_offset(&self, note: u8) -> Option<i8> {
        let offset = i16::from(note) - i16::from(self.root_note);
        (note <= 127 && self.root_note <= 127).then_some(offset as i8)
    }

    /// Initialize duration controls from output length; Bars rounds to whole four-beat bars.
    pub fn set_duration_from_frames(&mut self, frames: u64, rate: u32, tempo: f32) {
        self.time_ms = frames as f64 * 1000.0 / f64::from(rate);
        self.bars = (self.time_ms * f64::from(tempo) / 240000.0)
            .round()
            .clamp(1.0, f64::from(u32::MAX)) as u32;
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && self.version == 1
            && self.root_note <= 127
            && self.gain_db.is_finite()
            && (SILENT_GAIN_DB..=12.0).contains(&self.gain_db)
            && crate::ModulationDestinationSettings::valid_modulations(self)
            && self.time_ms.is_finite()
            && self.time_ms > 0.0
            && self.bars > 0
            && self
                .input
                .as_ref()
                .map_or(self.start_frame == 0 && self.end_frame == 0, |i| {
                    i.valid()
                        && self.start_frame < self.end_frame
                        && self.end_frame <= i.length_frames
                })
    }
    pub fn output_frames(&self, rate: u32, tempo: f32) -> Option<u64> {
        if !self.valid() || rate == 0 || !tempo.is_finite() || tempo <= 0.0 {
            return None;
        }
        self.input.as_ref()?;
        let selected = self.end_frame - self.start_frame;
        if self.mode == SamplerMode::Off {
            return Some(selected);
        }
        let frames = match self.mode {
            SamplerMode::Time => self.time_ms * f64::from(rate) / 1000.0,
            SamplerMode::Bars => {
                f64::from(self.bars) * 4.0 * 60.0 * f64::from(rate) / f64::from(tempo)
            }
            SamplerMode::Off => unreachable!(),
        }
        .round();
        if !frames.is_finite() || frames < 1.0 || frames >= u64::MAX as f64 {
            return None;
        }
        let result = frames as u64;
        ClipStretch::new(selected, result)
            .filter(|r| r.valid())
            .map(|_| result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_notes_cover_the_piano_roll_and_reject_invalid_sharps() {
        for note in 0..=127 {
            let name = crate::note_name(note);
            assert_eq!(
                crate::natural_note(&name.replace('#', ""), name.contains('#')),
                Some(note)
            );
        }
        for text in ["E4", "B3", "G9"] {
            assert_eq!(crate::natural_note(text, true), None);
        }
        for text in ["C-2", "A9", "H3", "C#4", "4", "C", "C32768"] {
            assert_eq!(crate::natural_note(text, false), None);
        }
        assert_eq!(crate::note_name(60), "C4");
    }
    #[test]
    fn duration_modes_bounds_legacy_defaults_and_source_exclusion_are_portable() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut s = SamplerSettings {
                end_frame: u64::from(rate),
                input: Some(SamplerInput {
                    asset_id: Uuid::new_v4(),
                    name: "Sample".into(),
                    source_offset_frame: 0,
                    length_frames: u64::from(rate),
                    repeat: None,
                    stretch: None,
                }),
                ..Default::default()
            };
            assert_eq!(s.output_frames(rate, 120.0), Some(u64::from(rate)));
            s.mode = SamplerMode::Time;
            s.time_ms = 2000.0;
            assert_eq!(s.output_frames(rate, 120.0), Some(u64::from(rate) * 2));
            assert_eq!(s.output_frames(rate, 60.0), Some(u64::from(rate) * 2));
            s.mode = SamplerMode::Bars;
            s.bars = 1;
            assert_eq!(s.output_frames(rate, 120.0), Some(u64::from(rate) * 2));
            assert_eq!(s.output_frames(rate, 60.0), Some(u64::from(rate) * 4));
            s.bars = 100;
            assert_eq!(s.output_frames(rate, 120.0), None);
            let mut layer = crate::InstrumentLayerSettings {
                sampler: Some(s),
                ..Default::default()
            };
            assert!(layer.valid());
            layer.oscillators.push(Default::default());
            assert!(!layer.valid());
        }
        let mut settings = SamplerSettings::default();
        settings.set_duration_from_frames(48000, 48000, 120.0);
        assert_eq!(settings.bars, 1);
        settings.set_duration_from_frames(250000, 48000, 120.0);
        assert_eq!(settings.bars, 3);
        settings.bars = 0;
        assert!(!settings.valid());
        let mut saved = serde_json::to_value(SamplerSettings::default()).unwrap();
        for (old, expected) in [(0.6411, 1), (1.49, 1), (1.5, 2), (3.0, 3)] {
            saved["bars"] = serde_json::json!(old);
            let migrated: SamplerSettings = serde_json::from_value(saved.clone()).unwrap();
            assert_eq!(migrated.bars, expected);
            assert!(serde_json::to_value(migrated).unwrap()["bars"].is_u64());
        }
        for invalid in [0.0, -1.0, f64::from(u32::MAX) + 1.0] {
            saved["bars"] = serde_json::json!(invalid);
            assert!(serde_json::from_value::<SamplerSettings>(saved.clone()).is_err());
        }
        let mut saved = serde_json::to_value(crate::InstrumentLayerSettings::default()).unwrap();
        saved.as_object_mut().unwrap().remove("sampler");
        let legacy: crate::InstrumentLayerSettings = serde_json::from_value(saved).unwrap();
        assert!(legacy.sampler.is_none() && legacy.valid());
    }
}

#[cfg(test)]
mod modulation_tests {
    use super::*;
    use crate::{Modulation, ModulationParameter as P};
    #[test]
    fn gain_routes_preserve_legacy_defaults_and_validate_typed_saved_data() {
        let base = SamplerSettings::default();
        let mut json = serde_json::to_value(&base).unwrap();
        json.as_object_mut().unwrap().remove("modulations");
        json.as_object_mut().unwrap().remove("modulation_open");
        let old: SamplerSettings = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(old, base);
        for field in ["modulations", "modulation_open"] {
            let mut invalid = json.clone();
            invalid[field] = serde_json::Value::Null;
            assert!(serde_json::from_value::<SamplerSettings>(invalid).is_err());
        }
        let mut s = base;
        let mut r = Modulation::new(P::SamplerGain);
        r.amount = -24.0;
        s.modulations.push(r);
        s.modulation_open = true;
        assert!(s.valid());
        assert_eq!(
            serde_json::from_value::<SamplerSettings>(serde_json::to_value(&s).unwrap()).unwrap(),
            s
        );
        for invalid in [
            Modulation {
                parameter: P::Pitch,
                ..r
            },
            Modulation {
                target: Some(s.id),
                ..r
            },
            Modulation { amount: 24.1, ..r },
            Modulation {
                amount: f32::NAN,
                ..r
            },
            Modulation {
                id: Uuid::nil(),
                ..r
            },
        ] {
            s.modulations[0] = invalid;
            assert!(!s.valid());
        }
        s.modulations = vec![r, r];
        assert!(!s.valid());
    }
}
