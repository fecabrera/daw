//! Audio-derived modulation detector and response mapping.

#[derive(Clone, Copy)]
pub enum Channel {
    Stereo,
    Left,
    Right,
    Mid,
    Side,
}

#[derive(Clone, Copy)]
pub enum Detector {
    Peak,
    Rms,
}

#[derive(Clone, Copy)]
pub enum Curve {
    Decibels,
    Amplitude,
}

pub struct Parameters {
    pub channel: Channel,
    pub detector: Detector,
    pub filter: bool,
    pub high_pass_hz: f32,
    pub low_pass_hz: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}

pub fn map(level: f32, curve: Curve, lower_db: f32, upper_db: f32, invert: bool) -> f32 {
    let level = if level.is_finite() {
        level.max(0.0)
    } else {
        0.0
    };
    let value = match curve {
        Curve::Decibels => (20.0 * level.max(1e-6).log10() - lower_db) / (upper_db - lower_db),
        Curve::Amplitude => {
            let low = 10.0_f32.powf(lower_db / 20.0);
            let high = 10.0_f32.powf(upper_db / 20.0);
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
    if invert { 1.0 - value } else { value }
}
/// Two-pole detector-only band limiting, 10 ms exponential RMS, then attack/release.
/// Coefficients are prepared outside the callback; sample processing never allocates.
#[derive(Clone)]
pub struct Follower {
    channel: Channel,
    detector: Detector,
    filter: bool,
    hp: f32,
    lp: f32,
    attack: f32,
    release: f32,
    rms: f32,
    high_state: [[f32; 2]; 2],
    low_state: [[f32; 2]; 2],
    power: f32,
    level: f32,
}
impl Follower {
    pub fn new(rate: u32, settings: Parameters) -> Self {
        let rate = rate as f32;
        let coefficient = |ms: f32| {
            if ms == 0.0 {
                0.0
            } else {
                (-1.0 / (ms * 0.001 * rate)).exp()
            }
        };
        let filter = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz.min(rate * 0.45) / rate).exp();
        Self {
            channel: settings.channel,
            detector: settings.detector,
            filter: settings.filter,
            hp: filter(settings.high_pass_hz),
            lp: filter(settings.low_pass_hz),
            attack: coefficient(settings.attack_ms),
            release: coefficient(settings.release_ms),
            rms: coefficient(10.0),
            high_state: [[0.0; 2]; 2],
            low_state: [[0.0; 2]; 2],
            power: 0.0,
            level: 0.0,
        }
    }
    /// Copy numeric history after the caller checks tap and filter compatibility.
    pub fn inherit_history(&mut self, previous: &Self) {
        self.high_state = previous.high_state;
        self.low_state = previous.low_state;
        self.power = previous.power;
        self.level = previous.level;
    }
    pub fn reset(&mut self) {
        self.high_state = [[0.0; 2]; 2];
        self.low_state = [[0.0; 2]; 2];
        self.power = 0.0;
        self.level = 0.0;
    }
    pub fn feed(&mut self, mut sample: [f32; 2]) {
        sample = sample.map(|v| {
            if v.is_finite() {
                v.clamp(-1e12, 1e12)
            } else {
                0.0
            }
        });
        if self.filter {
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
        let channel = match self.channel {
            Channel::Stereo => None,
            Channel::Left => Some(left),
            Channel::Right => Some(right),
            Channel::Mid => Some((left + right) * 0.5),
            Channel::Side => Some((left - right) * 0.5),
        };
        let peak = channel.map_or(left.abs().max(right.abs()), f32::abs);
        let power = channel.map_or((left * left + right * right) * 0.5, |v| v * v);
        self.power = self.rms * self.power + (1.0 - self.rms) * power;
        let measured = match self.detector {
            Detector::Peak => peak,
            Detector::Rms => self.power.sqrt(),
        };
        let c = if measured > self.level {
            self.attack
        } else {
            self.release
        };
        self.level = c * self.level + (1.0 - c) * measured;
    }
    pub fn level(&self) -> f32 {
        self.level
    }
    pub fn level_db(&self) -> f32 {
        20.0 * self.level.max(1e-6).log10()
    }
}
