//! Saved per-track piano-roll constraints. These do not transform existing notes.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiScale {
    Chromatic,
    #[default]
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
}
impl MidiScale {
    pub const ALL: [Self; 13] = [
        Self::Chromatic,
        Self::Major,
        Self::Minor,
        Self::HarmonicMinor,
        Self::MelodicMinor,
        Self::Dorian,
        Self::Phrygian,
        Self::Lydian,
        Self::Mixolydian,
        Self::Locrian,
        Self::MajorPentatonic,
        Self::MinorPentatonic,
        Self::Blues,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Chromatic => "Chromatic",
            Self::Major => "Major",
            Self::Minor => "Natural minor",
            Self::HarmonicMinor => "Harmonic minor",
            Self::MelodicMinor => "Melodic minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::Lydian => "Lydian",
            Self::Mixolydian => "Mixolydian",
            Self::Locrian => "Locrian",
            Self::MajorPentatonic => "Major pentatonic",
            Self::MinorPentatonic => "Minor pentatonic",
            Self::Blues => "Blues",
        }
    }
    fn intervals(self) -> &'static [u8] {
        match self {
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiOptions {
    pub root: Option<u8>,
    pub scale: MidiScale,
    pub snap: bool,
}
impl MidiOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(self) -> Result<()> {
        if self.root.is_some_and(|root| root >= 12) {
            return Err(Error(
                "MIDI scale root must be Off or a pitch class from 0 to 11".into(),
            ));
        }
        Ok(())
    }

    pub fn contains_pitch(self, pitch: u8) -> bool {
        self.root.is_none_or(|root| {
            self.scale
                .intervals()
                .contains(&((pitch % 12 + 12 - root % 12) % 12))
        })
    }

    /// Choose the nearest allowed MIDI pitch, preferring the lower pitch on ties.
    /// An empty scale intersection retains the requested pitch within the group bounds.
    pub fn snap_pitch(self, pitch: u8, bounds: RangeInclusive<u8>, bypass: bool) -> u8 {
        let pitch = pitch.clamp(*bounds.start(), *bounds.end());
        if !self.snap || self.root.is_none() || bypass {
            return pitch;
        }
        bounds
            .filter(|note| self.contains_pitch(*note))
            .min_by_key(|note| note.abs_diff(pitch))
            .unwrap_or(pitch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapping_obeys_root_scale_bounds_and_bypass() {
        for root in 0..12 {
            for scale in MidiScale::ALL {
                let options = MidiOptions {
                    root: Some(root),
                    scale,
                    snap: true,
                };
                for pitch in 0..=127 {
                    let next = options.snap_pitch(pitch, 0..=127, false);
                    assert!(scale.intervals().contains(&((next + 12 - root) % 12)));
                    assert_eq!(options.snap_pitch(pitch, 0..=127, true), pitch);
                    let expected = (0..=127u8)
                        .filter(|p| scale.intervals().contains(&((p + 12 - root) % 12)))
                        .map(|p| p.abs_diff(pitch))
                        .min()
                        .unwrap();
                    assert_eq!(next.abs_diff(pitch), expected);
                }
            }
        }
        let options = MidiOptions {
            root: Some(0),
            snap: true,
            ..Default::default()
        };
        assert_eq!(options.snap_pitch(61, 0..=127, false), 60);
        assert_eq!(options.snap_pitch(61, 61..=63, false), 62);
        assert_eq!(options.snap_pitch(61, 61..=61, false), 61);
        assert_eq!(options.snap_pitch(127, 0..=127, false), 127);
        let off = MidiOptions {
            snap: true,
            ..Default::default()
        };
        assert_eq!(off.snap_pitch(61, 0..=127, false), 61);
        assert!((0..=127).all(|pitch| off.contains_pitch(pitch)));
        assert!(!options.contains_pitch(61));
        assert!(options.contains_pitch(60));
    }
}
