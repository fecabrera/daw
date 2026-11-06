//! Synchronous, bounded-memory analysis for workers. No publication thread or audio ownership.
use crate::{
    filter::{Biquad, TruePeak},
    statistics::{Distribution, db},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeasureMode {
    #[default]
    SamplePeak,
    TruePeak,
    Integrated,
}
impl MeasureMode {
    pub const ALL: [Self; 3] = [Self::SamplePeak, Self::TruePeak, Self::Integrated];
    pub fn label(self) -> &'static str {
        match self {
            Self::SamplePeak => "Sample peak",
            Self::TruePeak => "True peak",
            Self::Integrated => "Integrated loudness",
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Self::SamplePeak => "dBFS",
            Self::TruePeak => "dBTP",
            Self::Integrated => "LUFS",
        }
    }
    pub fn default_target(self) -> f32 {
        if self == Self::Integrated {
            -23.0
        } else {
            -1.0
        }
    }
}

pub struct OfflineAnalyzer {
    mode: MeasureMode,
    peak: f64,
    tp: Option<TruePeak>,
    shelf: [Biquad; 2],
    highpass: [Biquad; 2],
    powers: Vec<f64>,
    distribution: Option<Distribution>,
    sum: f64,
    count: u64,
    hop: u64,
}
impl OfflineAnalyzer {
    pub fn new(rate: u32, mode: MeasureMode) -> Result<Self, &'static str> {
        if ![44100, 48000, 88200, 96000, 176400, 192000].contains(&rate) {
            return Err("Unsupported analysis sample rate");
        }
        Ok(Self {
            mode,
            peak: 0.0,
            tp: (mode == MeasureMode::TruePeak).then(TruePeak::new),
            shelf: [Biquad::shelf(rate as f64); 2],
            highpass: [Biquad::highpass(rate as f64); 2],
            powers: if mode == MeasureMode::Integrated {
                vec![0.0; rate as usize * 2 / 5]
            } else {
                vec![]
            },
            distribution: (mode == MeasureMode::Integrated).then(Distribution::new),
            sum: 0.0,
            count: 0,
            hop: (rate / 10) as u64,
        })
    }
    pub fn push(&mut self, frame: [f32; 2]) -> Result<(), &'static str> {
        if !frame.iter().all(|v| v.is_finite()) {
            return Err("Audio contains a non-finite sample");
        }
        let x = frame.map(f64::from);
        self.peak = self.peak.max(x[0].abs()).max(x[1].abs());
        if let Some(tp) = &mut self.tp {
            self.peak = self.peak.max(tp.tick(x, true));
        }
        if let Some(distribution) = &mut self.distribution {
            let mut power = 0.0;
            for (c, v) in x.into_iter().enumerate() {
                let y = self.highpass[c].tick(self.shelf[c].tick(v));
                power += y * y;
            }
            let index = (self.count % self.powers.len() as u64) as usize;
            self.sum += power - self.powers[index];
            self.powers[index] = power;
            let count = self.count + 1;
            if count >= self.powers.len() as u64 && count.is_multiple_of(self.hop) {
                let power = self.sum.max(0.0) / self.powers.len() as f64;
                if crate::statistics::lufs(power) > 40.0 {
                    return Err("Loudness exceeds the analysis range");
                }
                distribution.add(power);
            }
        }
        self.count += 1;
        Ok(())
    }
    /// Flush only peak reconstruction. Padding must not enter loudness gating.
    pub fn finish(mut self) -> Result<Option<f32>, &'static str> {
        if self.count == 0 {
            return Err("Audio clip is empty");
        }
        if self.peak == 0.0 {
            return Ok(None);
        }
        if self.mode == MeasureMode::Integrated {
            if self.count < self.powers.len() as u64 {
                return Err(
                    "Loudness normalization needs at least 400 ms; use a peak mode for shorter clips",
                );
            }
            let value = self.distribution.as_ref().unwrap().gated(-10.0).0;
            if !value.is_finite() {
                return Err("Audio is below the loudness gate; use a peak mode");
            }
            Ok(Some(value))
        } else {
            if let Some(tp) = &mut self.tp {
                for _ in 0..64 {
                    self.peak = self.peak.max(tp.tick([0.0; 2], true));
                }
            }
            Ok(Some(db(self.peak)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_normalization_analysis_calibrates_stereo_and_flushes_peak_tails() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut meter = OfflineAnalyzer::new(rate, MeasureMode::Integrated).unwrap();
            for i in 0..rate {
                let v =
                    (std::f64::consts::TAU * 1000.0 * i as f64 / rate as f64).sin() as f32 * 0.1;
                meter.push([v, -v]).unwrap();
            }
            assert!((meter.finish().unwrap().unwrap() + 20.0).abs() < 0.08);
        }
        let mut sample = OfflineAnalyzer::new(48000, MeasureMode::SamplePeak).unwrap();
        let mut tp = OfflineAnalyzer::new(48000, MeasureMode::TruePeak).unwrap();
        // The final samples reconstruct above their sample peak after input ends.
        for x in [[0.0, 0.0], [0.0, 0.8], [0.0, 0.8]] {
            sample.push(x).unwrap();
            tp.push(x).unwrap();
        }
        let peak = sample.finish().unwrap().unwrap();
        assert!((peak - 20.0 * 0.8f32.log10()).abs() < 0.001);
        assert!(tp.finish().unwrap().unwrap() > peak + 0.5);
    }
    #[test]
    fn offline_analysis_rejects_bad_input_and_undefined_loudness() {
        assert!(OfflineAnalyzer::new(0, MeasureMode::SamplePeak).is_err());
        for mode in MeasureMode::ALL {
            let mut silent = OfflineAnalyzer::new(48000, mode).unwrap();
            silent.push([0.0; 2]).unwrap();
            assert_eq!(silent.finish().unwrap(), None);
            let mut invalid = OfflineAnalyzer::new(48000, mode).unwrap();
            assert!(invalid.push([f32::NAN, 0.0]).is_err());
        }
        let mut short = OfflineAnalyzer::new(48000, MeasureMode::Integrated).unwrap();
        short.push([0.1; 2]).unwrap();
        assert!(short.finish().is_err());
        let mut quiet = OfflineAnalyzer::new(48000, MeasureMode::Integrated).unwrap();
        for i in 0..48000 {
            quiet.push([(i as f32 * 0.1).sin() * 1e-6; 2]).unwrap();
        }
        assert!(quiet.finish().is_err());
    }
}
