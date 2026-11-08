//! Non-destructive playback channel selection and polarity for audio clips.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipChannels {
    pub left: bool,
    pub right: bool,
    pub inverted: bool,
    pub swapped: bool,
}
impl Default for ClipChannels {
    fn default() -> Self {
        Self {
            left: true,
            right: true,
            inverted: false,
            swapped: false,
        }
    }
}
impl ClipChannels {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Per-output-channel sign/mask, applied before track processing.
    pub fn gain(self, channel: usize) -> f32 {
        if if channel == 0 { self.left } else { self.right } {
            if self.inverted { -1.0 } else { 1.0 }
        } else {
            0.0
        }
    }
    pub fn apply(self, sample: [f32; 2]) -> [f32; 2] {
        let sample = if self.swapped {
            [sample[1], sample[0]]
        } else {
            sample
        };
        [sample[0] * self.gain(0), sample[1] * self.gain(1)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swap_precedes_output_masks_and_polarity() {
        let mut channels = ClipChannels {
            swapped: true,
            ..Default::default()
        };
        assert_eq!(channels.apply([0.25, -0.5]), [-0.5, 0.25]);
        channels.left = false;
        channels.inverted = true;
        assert_eq!(channels.apply([0.25, -0.5]), [0.0, -0.25]);
        let saved = serde_json::to_string(&channels).unwrap();
        assert_eq!(
            serde_json::from_str::<ClipChannels>(&saved).unwrap(),
            channels
        );
        assert!(
            !serde_json::from_str::<ClipChannels>(r#"{"left":true,"right":false,"inverted":true}"#)
                .unwrap()
                .swapped
        );
    }
    #[test]
    fn channel_masks_and_polarity_preserve_stereo_layout() {
        for left in [false, true] {
            for right in [false, true] {
                for inverted in [false, true] {
                    let channels = ClipChannels {
                        left,
                        right,
                        inverted,
                        swapped: false,
                    };
                    let sign = if inverted { -1.0 } else { 1.0 };
                    assert_eq!(
                        channels.apply([0.25, -0.5]),
                        [
                            if left { 0.25 * sign } else { 0.0 },
                            if right { -0.5 * sign } else { 0.0 }
                        ]
                    );
                }
            }
        }
        assert_eq!(
            serde_json::from_str::<ClipChannels>("{}").unwrap(),
            ClipChannels::default()
        );
    }
}
