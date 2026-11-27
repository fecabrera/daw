//! Portable audio-derived continuous modulation. The host owns audio taps and clocks.
use crate::{
    ModulationBehavior, ModulationOutputDescriptor, ModulationRange, ModulationScope,
    ModulationSource, ModulationSourceContext, ModulationSourceSettings, ProcessingContext,
};
use daw_dsp_support::sidechain as dsp;
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
        dsp::map(
            level,
            self.curve.into(),
            self.lower_db,
            self.upper_db,
            self.invert,
        )
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

impl From<SidechainChannel> for dsp::Channel {
    fn from(value: SidechainChannel) -> Self {
        match value {
            SidechainChannel::Stereo => Self::Stereo,
            SidechainChannel::Left => Self::Left,
            SidechainChannel::Right => Self::Right,
            SidechainChannel::Mid => Self::Mid,
            SidechainChannel::Side => Self::Side,
        }
    }
}
impl From<SidechainDetector> for dsp::Detector {
    fn from(value: SidechainDetector) -> Self {
        match value {
            SidechainDetector::Peak => Self::Peak,
            SidechainDetector::Rms => Self::Rms,
        }
    }
}
impl From<SidechainCurve> for dsp::Curve {
    fn from(value: SidechainCurve) -> Self {
        match value {
            SidechainCurve::Decibels => Self::Decibels,
            SidechainCurve::Amplitude => Self::Amplitude,
        }
    }
}

#[derive(Clone)]
pub struct SidechainFollower {
    settings: AudioSidechain,
    rate: f32,
    kernel: dsp::Follower,
    value: f32,
}

impl SidechainFollower {
    pub fn new(settings: AudioSidechain, context: ProcessingContext) -> Self {
        Self {
            settings,
            rate: context.sample_rate_hz as f32,
            kernel: dsp::Follower::new(
                context.sample_rate_hz,
                dsp::Parameters {
                    channel: settings.channel.into(),
                    detector: settings.detector.into(),
                    filter: settings.filter,
                    high_pass_hz: settings.high_pass_hz,
                    low_pass_hz: settings.low_pass_hz,
                    attack_ms: settings.attack_ms,
                    release_ms: settings.release_ms,
                },
            ),
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
            self.kernel.inherit_history(&previous.kernel);
            self.value = self.settings.map(self.kernel.level());
        }
    }
    pub fn reset(&mut self) {
        self.kernel.reset();
        self.value = 0.0;
    }

    pub fn feed(&mut self, sample: [f32; 2]) {
        self.kernel.feed(sample);
        self.value = self.settings.map(self.kernel.level());
    }

    pub fn readings(&self) -> [f32; 2] {
        [self.kernel.level_db(), self.value]
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
                        (f.kernel.level() - expected).abs() < 0.001,
                        "{rate}: {} != {expected}",
                        f.kernel.level()
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
            assert!((f.kernel.level() - (1.0 - (-1_f32).exp())).abs() < 0.002);
            let peak = f.kernel.level();
            for _ in 0..rate / 50 {
                f.feed([0.0; 2]);
            }
            assert!((f.kernel.level() - peak * (-1_f32).exp()).abs() < 0.002);
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
        assert!(f.kernel.level() < 1e-4);
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
