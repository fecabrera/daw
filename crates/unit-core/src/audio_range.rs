//! Shared immutable audio range mapping for clips and sampler inputs.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClipStretch {
    pub source_frames: u64,
    pub output_frames: u64,
}

impl ClipStretch {
    pub fn new(source_frames: u64, output_frames: u64) -> Option<Self> {
        if source_frames == 0 || output_frames == 0 {
            return None;
        }
        let mut a = source_frames;
        let mut b = output_frames;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Some(Self {
            source_frames: source_frames / a,
            output_frames: output_frames / a,
        })
    }

    pub fn ratio(self) -> f64 {
        self.output_frames as f64 / self.source_frames as f64
    }

    pub fn valid(self) -> bool {
        self.source_frames > 0 && self.output_frames > 0 && (0.125..=8.0).contains(&self.ratio())
    }

    pub fn scale(self, frames: u64) -> Option<u64> {
        if self.source_frames == 0 {
            return None;
        }
        u64::try_from(
            (u128::from(frames) * u128::from(self.output_frames)
                + u128::from(self.source_frames / 2))
                / u128::from(self.source_frames),
        )
        .ok()
    }

    pub fn original_frame(self, frame: u64) -> u64 {
        ((u128::from(frame) * u128::from(self.source_frames)) / u128::from(self.output_frames))
            as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipLoop {
    pub length_frames: u64,
    #[serde(default)]
    pub phase_frame: u64,
}

impl ClipLoop {
    /// Requires a nonzero loop length and a phase within that length.
    pub fn position(self, local: u64) -> u64 {
        let local = local % self.length_frames;
        let remaining = self.length_frames - self.phase_frame;
        if local >= remaining {
            local - remaining
        } else {
            local + self.phase_frame
        }
    }

    pub fn shifted(self, frames: i128) -> Self {
        let length = i128::from(self.length_frames);
        Self {
            phase_frame: ((i128::from(self.phase_frame) + frames.rem_euclid(length)) % length)
                as u64,
            ..self
        }
    }
}
