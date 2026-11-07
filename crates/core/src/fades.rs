//! Saved audio envelopes. Shapes are shared by rendering and visual controls.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipFade {
    /// Zero retains the legacy boundary fade. A crossfade extends into a neighbor.
    pub length_frames: u64,
    /// Gain at the saved normalized midpoint position.
    #[serde(deserialize_with = "read_level")]
    pub midpoint: f32,
    /// Normalized position from zero gain toward full gain; legacy curves use half.
    #[serde(
        default = "half",
        skip_serializing_if = "is_half",
        deserialize_with = "read_position"
    )]
    pub midpoint_position: f32,
    #[serde(default)]
    pub crossfade: bool,
}
impl Default for ClipFade {
    fn default() -> Self {
        Self {
            length_frames: 0,
            midpoint: 0.5,
            midpoint_position: 0.5,
            crossfade: false,
        }
    }
}
impl ClipFade {
    pub const MIN_POSITION: f32 = 0.15;
    pub const MAX_POSITION: f32 = 0.85;
    pub const MIN_LEVEL: f32 = 0.15;
    pub const MAX_LEVEL: f32 = 0.85;

    pub fn valid(self) -> bool {
        self.midpoint.is_finite()
            && (Self::MIN_LEVEL..=Self::MAX_LEVEL).contains(&self.midpoint)
            && self.midpoint_position.is_finite()
            && (Self::MIN_POSITION..=Self::MAX_POSITION).contains(&self.midpoint_position)
            && self.length_frames < u64::MAX
    }
    /// Opposing transition gain shares the control point's time and gain.
    pub fn opposing_gain(self, progress: f32) -> f32 {
        let mirrored = Self {
            midpoint: 1.0 - self.midpoint,
            ..self
        };
        1.0 - mirrored.gain(progress)
    }
    /// Monotone cubic S-curve with flat endpoints through the saved control point.
    pub fn gain(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress == 0.0 || progress == 1.0 {
            return progress;
        }
        let x = self.midpoint_position;
        let y = self.midpoint;
        if progress == x {
            return y;
        }
        let left_secant = y / x;
        let right_secant = (1.0 - y) / (1.0 - x);
        // Match second derivatives at the knot where possible. The bounds keep
        // both cubic Bezier control polygons monotone, preventing overshoot.
        let slope = (1.5 * ((1.0 - x) * left_secant + x * right_secant))
            .min(3.0 * left_secant)
            .min(3.0 * right_secant);
        let value = if progress < x {
            let t = progress / x;
            t * t * (y * (3.0 - 2.0 * t) + x * slope * (t - 1.0))
        } else {
            let t = (progress - x) / (1.0 - x);
            let inverse = 1.0 - t;
            y + (1.0 - y) * t * t * (3.0 - 2.0 * t) + (1.0 - x) * slope * t * inverse * inverse
        };
        value.clamp(0.0, 1.0)
    }
}
fn read_coordinate<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
    minimum: f32,
    maximum: f32,
) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    // Earlier manifests allowed 1–99% coordinates. Migrate those values rather
    // than rejecting an otherwise valid project under the new editing bounds.
    if !value.is_finite() || !(0.01..=0.99).contains(&value) {
        return Err(serde::de::Error::custom("Invalid fade control coordinate"));
    }
    Ok(value.clamp(minimum, maximum))
}
fn read_level<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    read_coordinate(deserializer, ClipFade::MIN_LEVEL, ClipFade::MAX_LEVEL)
}
fn read_position<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    read_coordinate(deserializer, ClipFade::MIN_POSITION, ClipFade::MAX_POSITION)
}

fn half() -> f32 {
    0.5
}
fn is_half(value: &f32) -> bool {
    *value == 0.5
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipFades {
    pub fade_in: ClipFade,
    pub fade_out: ClipFade,
}
impl ClipFades {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(self) -> bool {
        self.fade_in.valid() && self.fade_out.valid()
    }
    /// Custom fades replace the corresponding outer 5 ms fade. Repeats keep their seam fades.
    pub fn gain(self, local: u64, length: u64, rate: u32) -> f32 {
        if length < 2 || local >= length {
            return 0.0;
        }
        let legacy = (u64::from(rate) / 200).max(1).min(length / 2);
        let edge = |fade: ClipFade, position: u64| {
            if fade.crossfade && fade.length_frames > 0 {
                return 1.0;
            }
            let duration = if fade.length_frames == 0 {
                legacy
            } else {
                fade.length_frames.min(length - 1)
            };
            let progress = position as f32 / duration.max(1) as f32;
            if fade.length_frames == 0 {
                progress.clamp(0.0, 1.0)
            } else {
                fade.gain(progress)
            }
        };
        edge(self.fade_in, local).min(edge(self.fade_out, length - 1 - local))
    }
}
impl crate::Clip {
    /// Continue source time beyond either clip edge, retaining loop phase.
    /// Non-looped clips return None before source frame zero or on overflow.
    pub fn extended_source_frame(&self, local: i128) -> Option<u64> {
        let position = if let Some(repeat) = self.repeat {
            i128::from(repeat.shifted(local).phase_frame)
        } else {
            local
        };
        u64::try_from(i128::from(self.source_offset_frame).checked_add(position)?).ok()
    }
}

impl crate::Project {
    /// Maximum duration for an edge envelope. Crossfades use adjacent audio.
    pub fn clip_fade_limit(&self, id: crate::Id, left: bool, crossfade: bool) -> u64 {
        let Some(track) = self
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| c.id == id))
        else {
            return 0;
        };
        let clip = track.clips.iter().find(|c| c.id == id).unwrap();
        if !crossfade {
            return clip.length_frames.saturating_sub(1);
        }
        let own = clip.length_frames.saturating_sub(1) / 2;
        if clip.muted {
            return 0;
        }
        let Some(neighbor) = track.clips.iter().find(|other| {
            other.id != id
                && !other.muted
                && if left {
                    other.end() == clip.start_frame
                } else {
                    other.start_frame == clip.end()
                }
        }) else {
            return 0;
        };
        let Some(asset) = self
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id && a.is_audio())
        else {
            return 0;
        };
        let Some(other_asset) = self
            .assets
            .iter()
            .find(|a| a.id == neighbor.asset_id && a.is_audio())
        else {
            return 0;
        };
        if asset.source_metadata.channels != other_asset.source_metadata.channels {
            return 0;
        }
        let handles = if clip.repeat.is_some() {
            u64::MAX
        } else if left {
            clip.source_offset_frame
        } else {
            clip.source_length(asset.decoded_frame_count)
                .unwrap_or(0)
                .saturating_sub(clip.source_offset_frame.saturating_add(clip.length_frames))
        };
        own.min(neighbor.length_frames.saturating_sub(1) / 2)
            .min(handles)
    }
    pub(crate) fn set_clip_fades(&mut self, id: crate::Id, fades: ClipFades) -> crate::Result<()> {
        if !fades.valid() {
            return Err(crate::Error("Invalid fade shape or duration".into()));
        }
        let old = self.clip_mut(id)?.clone();
        for (left, fade, previous) in [
            (true, fades.fade_in, old.fades.fade_in),
            (false, fades.fade_out, old.fades.fade_out),
        ] {
            if fade != previous
                && fade.length_frames > self.clip_fade_limit(id, left, fade.crossfade)
            {
                return Err(crate::Error(
                    "Fade exceeds clip, adjacent clip or source bounds".into(),
                ));
            }
            if fade != previous && fade.length_frames > 0 {
                for track in self
                    .tracks
                    .iter_mut()
                    .filter(|t| t.clips.iter().any(|c| c.id == id))
                {
                    for other in &mut track.clips {
                        if other.id != id
                            && if left {
                                other.end() == old.start_frame
                            } else {
                                other.start_frame == old.end()
                            }
                        {
                            let opposite = if left {
                                &mut other.fades.fade_out
                            } else {
                                &mut other.fades.fade_in
                            };
                            if opposite.crossfade || fade.crossfade {
                                *opposite = ClipFade::default();
                            }
                        }
                    }
                }
            }
        }
        self.clip_mut(id)?.fades = fades;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_hit_endpoints_and_midpoint_and_remain_monotone() {
        for position in [0.15, 0.2, 0.5, 0.8, 0.85] {
            for midpoint in [0.15, 0.2, 0.5, 0.8, 0.85] {
                let fade = ClipFade {
                    midpoint,
                    midpoint_position: position,
                    ..Default::default()
                };
                assert!(fade.valid());
                assert_eq!(fade.gain(0.0), 0.0);
                assert_eq!(fade.gain(1.0), 1.0);
                assert!((fade.gain(position) - midpoint).abs() < 0.00001);
                for i in 0..100 {
                    assert!(fade.gain(i as f32 / 100.0) <= fade.gain((i + 1) as f32 / 100.0));
                }
            }
        }
    }
    #[test]
    fn stored_control_points_and_new_position_round_trip() {
        let old: ClipFade =
            serde_json::from_str(r#"{"length_frames":1000,"midpoint":0.2,"crossfade":true}"#)
                .unwrap();
        assert_eq!(old.midpoint_position, 0.5);
        assert_eq!(old.gain(0.5), 0.2);
        let explicit = ClipFade {
            midpoint_position: 0.5,
            ..old
        };
        for i in 0..100 {
            assert_eq!(old.gain(i as f32 / 100.0), explicit.gain(i as f32 / 100.0));
        }
        let new = ClipFade {
            midpoint_position: 0.75,
            ..old
        };
        assert_eq!(
            serde_json::from_str::<ClipFade>(&serde_json::to_string(&new).unwrap()).unwrap(),
            new
        );
        assert_ne!(old.gain(0.5), new.gain(0.5));
        for position in [0.0, 0.14, 0.86, 1.0, f32::NAN, f32::INFINITY] {
            assert!(
                !ClipFade {
                    midpoint_position: position,
                    ..new
                }
                .valid()
            );
        }
    }
    #[test]
    fn default_shape_is_s_shaped_and_automatic_boundaries_remain_linear() {
        let fade = ClipFade::default();
        assert_eq!(fade.gain(0.25), 0.15625);
        assert_eq!(fade.gain(0.5), 0.5);
        assert_eq!(fade.gain(0.75), 0.84375);
        let epsilon = 0.0001;
        assert!(fade.gain(epsilon) / epsilon < 0.001);
        assert!((1.0 - fade.gain(1.0 - epsilon)) / epsilon < 0.001);
        for rate in crate::SAMPLE_RATES {
            let boundary = u64::from(rate) / 200;
            assert_eq!(
                ClipFades::default().gain(boundary / 2, u64::from(rate), rate),
                (boundary / 2) as f32 / boundary as f32
            );
        }
    }
    #[test]
    fn legacy_coordinates_migrate_to_new_bounds_and_invalid_edits_are_rejected() {
        for (x, y, expected_x, expected_y) in [(0.01, 0.99, 0.15, 0.85), (0.99, 0.01, 0.85, 0.15)] {
            let json = serde_json::json!({"length_frames":1000,"midpoint":y,"midpoint_position":x,"crossfade":false});
            let fade: ClipFade = serde_json::from_value(json).unwrap();
            assert!(fade.valid());
            assert_eq!(fade.midpoint_position, expected_x);
            assert_eq!(fade.midpoint, expected_y);
        }
        for level in [0.149, 0.851, f32::NAN] {
            assert!(
                !ClipFade {
                    midpoint: level,
                    ..Default::default()
                }
                .valid()
            );
        }
    }
}

#[cfg(test)]
mod transition_gain_tests {
    use super::*;

    #[test]
    fn crossing_gain_and_time_are_independent() {
        for position in [0.2, 0.5, 0.8] {
            for level in [0.15, 0.5, 0.85] {
                let fade = ClipFade {
                    midpoint_position: position,
                    midpoint: level,
                    ..Default::default()
                };
                assert!((fade.gain(position) - level).abs() < 1e-6);
                assert!((fade.opposing_gain(position) - level).abs() < 1e-6);
                assert_eq!(fade.opposing_gain(0.0), 1.0);
                assert_eq!(fade.opposing_gain(1.0), 0.0);
            }
        }
    }
}
