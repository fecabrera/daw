//! Portable audio-derived continuous modulation. The host owns audio taps and clocks.
use crate::{
    ModulationBehavior, ModulationOutputDescriptor, ModulationRange, ModulationScope,
    ModulationSource, ModulationSourceContext, ModulationSourceSettings, ProcessingContext,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidechainTap {
    #[default]
    PreFader,
    PostFader,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidechainChannel {
    #[default]
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidechainDetector {
    #[default]
    Peak,
    Rms,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidechainCurve {
    #[default]
    Decibels,
    Amplitude,
}

/// One independent detector per connection. Its provider identity is the route ID.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSidechain {
    pub version: u32,
    pub track: Uuid,
    pub tap: SidechainTap,
    pub channel: SidechainChannel,
    pub detector: SidechainDetector,
    pub filter: bool,
    pub high_pass_hz: f32,
    pub low_pass_hz: f32,
    pub lower_db: f32,
    pub upper_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub curve: SidechainCurve,
    pub invert: bool,
}
impl AudioSidechain {
    pub fn new(track: Uuid) -> Self {
        Self {
            version: 1,
            track,
            tap: SidechainTap::PreFader,
            channel: SidechainChannel::Stereo,
            detector: SidechainDetector::Peak,
            filter: false,
            high_pass_hz: 30.0,
            low_pass_hz: 20_000.0,
            lower_db: -36.0,
            upper_db: -6.0,
            attack_ms: 5.0,
            release_ms: 120.0,
            curve: SidechainCurve::Decibels,
            invert: false,
        }
    }
    pub fn valid(&self) -> bool {
        self.version == 1
            && !self.track.is_nil()
            && self.high_pass_hz.is_finite()
            && self.low_pass_hz.is_finite()
            && (20.0..=20_000.0).contains(&self.high_pass_hz)
            && (20.0..=20_000.0).contains(&self.low_pass_hz)
            && self.high_pass_hz < self.low_pass_hz
            && self.lower_db.is_finite()
            && self.upper_db.is_finite()
            && (-120.0..=24.0).contains(&self.lower_db)
            && (-120.0..=24.0).contains(&self.upper_db)
            && self.lower_db < self.upper_db
            && self.attack_ms.is_finite()
            && (0.0..=10_000.0).contains(&self.attack_ms)
            && self.release_ms.is_finite()
            && (0.0..=10_000.0).contains(&self.release_ms)
    }
    pub fn map(&self, level: f32) -> f32 {
        let level = if level.is_finite() {
            level.max(0.0)
        } else {
            0.0
        };
        let value = match self.curve {
            SidechainCurve::Decibels => {
                (20.0 * level.max(1e-6).log10() - self.lower_db) / (self.upper_db - self.lower_db)
            }
            SidechainCurve::Amplitude => {
                let low = 10.0_f32.powf(self.lower_db / 20.0);
                let high = 10.0_f32.powf(self.upper_db / 20.0);
                // Extremely narrow, valid dB ranges can round to one amplitude.
                // Treat that unresolvable interval as a step rather than dividing by zero.
                if high == low {
                    f32::from(level > low)
                } else {
                    (level - low) / (high - low)
                }
            }
        }
        .clamp(0.0, 1.0);
        if self.invert { 1.0 - value } else { value }
    }
}

pub const SIDECHAIN_OUTPUT: ModulationOutputDescriptor = ModulationOutputDescriptor {
    name: "Audio sidechain",
    range: ModulationRange::Unipolar,
    behavior: ModulationBehavior::Continuous,
};

/// Host adapter for shared provider discovery and publication, independent of project types.
pub struct SidechainProviderSettings {
    pub id: Uuid,
    pub present: bool,
    pub enabled: bool,
}
impl ModulationSourceSettings for SidechainProviderSettings {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = SIDECHAIN_OUTPUT;
    fn modulation_source_id(&self) -> Uuid {
        self.id
    }
    fn modulation_scope(&self) -> ModulationScope {
        ModulationScope::Shared
    }
    fn modulation_present(&self) -> bool {
        self.present
    }
    fn modulation_enabled(&self) -> bool {
        self.enabled
    }
}

/// Two-pole detector-only band limiting, 10 ms exponential RMS, then attack/release.
/// Coefficients are prepared outside the callback; sample processing never allocates.
#[derive(Clone)]
pub struct SidechainFollower {
    settings: AudioSidechain,
    rate: f32,
    hp: f32,
    lp: f32,
    attack: f32,
    release: f32,
    rms: f32,
    high_state: [[f32; 2]; 2],
    low_state: [[f32; 2]; 2],
    power: f32,
    level: f32,
    value: f32,
}
impl SidechainFollower {
    pub fn new(settings: AudioSidechain, context: ProcessingContext) -> Self {
        let rate = context.sample_rate_hz as f32;
        let coefficient = |ms: f32| {
            if ms == 0.0 {
                0.0
            } else {
                (-1.0 / (ms * 0.001 * rate)).exp()
            }
        };
        let filter = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz.min(rate * 0.45) / rate).exp();
        Self {
            settings,
            rate,
            hp: filter(settings.high_pass_hz),
            lp: filter(settings.low_pass_hz),
            attack: coefficient(settings.attack_ms),
            release: coefficient(settings.release_ms),
            rms: coefficient(10.0),
            high_state: [[0.0; 2]; 2],
            low_state: [[0.0; 2]; 2],
            power: 0.0,
            level: 0.0,
            value: 0.0,
        }
    }
    /// Copy compatible state into an already prepared replacement. No coefficient work
    /// or allocation occurs during callback inheritance.
    pub fn inherit_history(&mut self, previous: &Self) {
        if previous.rate == self.rate
            && previous.settings.track == self.settings.track
            && previous.settings.tap == self.settings.tap
            && previous.settings.channel == self.settings.channel
            && previous.settings.detector == self.settings.detector
            && previous.settings.filter == self.settings.filter
            && previous.settings.high_pass_hz == self.settings.high_pass_hz
            && previous.settings.low_pass_hz == self.settings.low_pass_hz
        {
            self.high_state = previous.high_state;
            self.low_state = previous.low_state;
            self.power = previous.power;
            self.level = previous.level;
            self.value = self.settings.map(self.level);
        }
    }
    pub fn reset(&mut self) {
        self.high_state = [[0.0; 2]; 2];
        self.low_state = [[0.0; 2]; 2];
        self.power = 0.0;
        self.level = 0.0;
        self.value = 0.0;
    }
    pub fn feed(&mut self, mut sample: [f32; 2]) {
        sample = sample.map(|v| {
            if v.is_finite() {
                v.clamp(-1e12, 1e12)
            } else {
                0.0
            }
        });
        if self.settings.filter {
            for (ch, value) in sample.iter_mut().enumerate() {
                for stage in 0..2 {
                    self.high_state[stage][ch] += self.hp * (*value - self.high_state[stage][ch]);
                    *value -= self.high_state[stage][ch];
                    self.low_state[stage][ch] += self.lp * (*value - self.low_state[stage][ch]);
                    *value = self.low_state[stage][ch];
                }
            }
        }
        let [left, right] = sample;
        let channel = match self.settings.channel {
            SidechainChannel::Stereo => None,
            SidechainChannel::Left => Some(left),
            SidechainChannel::Right => Some(right),
            SidechainChannel::Mid => Some((left + right) * 0.5),
            SidechainChannel::Side => Some((left - right) * 0.5),
        };
        let peak = channel.map_or(left.abs().max(right.abs()), f32::abs);
        let power = channel.map_or((left * left + right * right) * 0.5, |v| v * v);
        self.power = self.rms * self.power + (1.0 - self.rms) * power;
        let measured = match self.settings.detector {
            SidechainDetector::Peak => peak,
            SidechainDetector::Rms => self.power.sqrt(),
        };
        let c = if measured > self.level {
            self.attack
        } else {
            self.release
        };
        self.level = c * self.level + (1.0 - c) * measured;
        self.value = self.settings.map(self.level);
    }
    pub fn readings(&self) -> [f32; 2] {
        [20.0 * self.level.max(1e-6).log10(), self.value]
    }
}
impl ModulationSource for SidechainFollower {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = SIDECHAIN_OUTPUT;
    fn next_modulation(&mut self, _: ModulationSourceContext) -> f32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidechain_mapping_is_bounded_invertible_and_rejects_invalid_settings() {
        let mut s = AudioSidechain::new(Uuid::new_v4());
        for curve in [SidechainCurve::Decibels, SidechainCurve::Amplitude] {
            s.curve = curve;
            for (level, expected) in [
                (0.0, 0.0),
                (1e-9, 0.0),
                (10_f32.powf(-36.0 / 20.0), 0.0),
                (10_f32.powf(-6.0 / 20.0), 1.0),
                (10.0, 1.0),
            ] {
                assert!((s.map(level) - expected).abs() < 1e-6);
                s.invert = true;
                assert!((s.map(level) - (1.0 - expected)).abs() < 1e-6);
                s.invert = false;
            }
        }
        let original = s;
        let mut narrow = s;
        narrow.curve = SidechainCurve::Amplitude;
        narrow.lower_db = 0.0;
        narrow.upper_db = f32::EPSILON;
        assert!(narrow.valid());
        assert_eq!(narrow.map(1.0), 0.0);
        assert_eq!(narrow.map(2.0), 1.0);
        for field in 0..8 {
            s = original;
            match field {
                0 => s.lower_db = s.upper_db,
                1 => s.attack_ms = f32::NAN,
                2 => s.release_ms = -1.0,
                3 => s.high_pass_hz = s.low_pass_hz,
                4 => s.lower_db = f32::NEG_INFINITY,
                5 => s.low_pass_hz = 20_001.0,
                6 => s.version = 2,
                _ => s.track = Uuid::nil(),
            };
            assert!(!s.valid());
        }
    }
    #[test]
    fn sidechain_peak_rms_channels_and_response_are_rate_independent() {
        for rate in [44_100, 48_000, 88_200, 96_000, 176_400, 192_000] {
            for detector in [SidechainDetector::Peak, SidechainDetector::Rms] {
                let mut s = AudioSidechain::new(Uuid::new_v4());
                s.detector = detector;
                s.attack_ms = 0.0;
                s.release_ms = 0.0;
                for (channel, expected) in [
                    (SidechainChannel::Stereo, 0.5),
                    (SidechainChannel::Left, 0.5),
                    (SidechainChannel::Right, 0.5),
                    (SidechainChannel::Mid, 0.0),
                    (SidechainChannel::Side, 0.5),
                ] {
                    s.channel = channel;
                    let mut f = SidechainFollower::new(
                        s,
                        ProcessingContext {
                            sample_rate_hz: rate,
                            max_block_frames: 512,
                        },
                    );
                    for _ in 0..rate / 2 {
                        f.feed([0.5, -0.5]);
                    }
                    assert!(
                        (f.level - expected).abs() < 0.001,
                        "{rate}: {} != {expected}",
                        f.level
                    );
                    f.feed([f32::NAN, f32::INFINITY]);
                    assert!(f.readings().iter().all(|v| v.is_finite()));
                }
            }
            let mut s = AudioSidechain::new(Uuid::new_v4());
            s.attack_ms = 10.0;
            s.release_ms = 20.0;
            let mut f = SidechainFollower::new(
                s,
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 32,
                },
            );
            for _ in 0..rate / 100 {
                f.feed([1.0; 2]);
            }
            assert!((f.level - (1.0 - (-1_f32).exp())).abs() < 0.002);
            let peak = f.level;
            for _ in 0..rate / 50 {
                f.feed([0.0; 2]);
            }
            assert!((f.level - peak * (-1_f32).exp()).abs() < 0.002);
        }
    }
    #[test]
    fn sidechain_filter_changes_only_the_detector_and_missing_weight_stays_neutral() {
        let mut s = AudioSidechain::new(Uuid::new_v4());
        s.filter = true;
        s.attack_ms = 0.0;
        s.release_ms = 0.0;
        s.invert = true;
        let context = ProcessingContext {
            sample_rate_hz: 48_000,
            max_block_frames: 512,
        };
        let mut f = SidechainFollower::new(s, context);
        for _ in 0..48_000 {
            f.feed([0.5; 2]);
        }
        assert!(f.level < 1e-4);
        let mut output = crate::ModulationOutput::new(
            crate::ModulationSourceId {
                layer: s.track,
                unit: Uuid::new_v4(),
            },
            false,
            context,
        );
        let value = output.sample(&mut f, Default::default());
        assert!(value.value > 0.99);
        assert_eq!(value.weight, 0.0);
    }
    #[test]
    fn sidechain_state_migrates_and_routes_use_the_shared_resolver() {
        let mut route = crate::Modulation::new(crate::ModulationParameter::Level);
        let legacy = serde_json::to_value(route).unwrap();
        assert!(legacy.get("sidechain").is_none());
        assert_eq!(
            serde_json::from_value::<crate::Modulation>(legacy).unwrap(),
            route
        );
        route.sidechain = Some(AudioSidechain::new(Uuid::new_v4()));
        route.amount = -12.0;
        let encoded = serde_json::to_string(&route).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::Modulation>(&encoded).unwrap(),
            route
        );
        assert_eq!(route.effective_source().unwrap().unit, route.id);
        route.source = Some(crate::ModulationSourceId {
            layer: Uuid::new_v4(),
            unit: Uuid::new_v4(),
        });
        assert!(!route.valid());
    }
}
