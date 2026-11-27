//! Numeric released levels and peak bins. Publication belongs to the caller.
pub const LEVEL_METER_RELEASE_SECONDS: f32 = 0.3;

#[derive(Clone)]
pub struct ReleasedLevels<const N: usize> {
    peaks: [f32; N],
    release: f32,
}

impl<const N: usize> ReleasedLevels<N> {
    pub fn new(rate: u32) -> Self {
        Self {
            peaks: [0.0; N],
            release: (-1.0 / (LEVEL_METER_RELEASE_SECONDS * rate as f32)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.peaks = [0.0; N];
    }

    pub fn current(&self) -> [f32; N] {
        self.peaks
    }

    #[inline]
    pub fn sample(&mut self, levels: [f32; N]) {
        for (held, level) in self.peaks.iter_mut().zip(levels) {
            let level = if level.is_finite() {
                level.max(0.0)
            } else {
                0.0
            };
            *held = level.max(*held * self.release);
        }
    }
}

#[derive(Clone)]
pub struct PeakBins {
    period: u32,
    frames: u32,
    peaks: [f32; 2],
}

impl PeakBins {
    pub fn new(rate: u32) -> Self {
        Self {
            period: (rate / 50).max(1),
            frames: 0,
            peaks: [0.0; 2],
        }
    }

    pub fn reset(&mut self) {
        self.frames = 0;
        self.peaks = [0.0; 2];
    }

    pub fn sample(&mut self, values: [f32; 2]) -> Option<[f32; 2]> {
        for (peak, value) in self.peaks.iter_mut().zip(values) {
            if value.is_finite() {
                *peak = peak.max(value);
            }
        }
        self.frames += 1;
        if self.frames == self.period {
            let peaks = self.peaks;
            self.reset();
            Some(peaks)
        } else {
            None
        }
    }
}
