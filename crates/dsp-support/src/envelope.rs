//! Per-note ADSR progression and curve laws over prepared numeric parameters.

#[derive(Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    Exponential,
    Logarithmic,
    SCurve,
}
impl Curve {
    /// Normalized stage progress, with exact endpoints. Falling stages reverse
    /// exponential/logarithmic curvature so exponential releases have a long tail.
    pub fn progress(self, position: f32, amount: f32, falling: bool) -> f32 {
        let t = position.clamp(0.0, 1.0);
        let amount = amount.clamp(0.0, 1.0);
        if amount == 0.0 {
            return t;
        }
        let power = 1.0 + 7.0 * amount;
        match self {
            Self::Linear => t,
            Self::Exponential if !falling => t.powf(power),
            Self::Exponential => 1.0 - (1.0 - t).powf(power),
            Self::Logarithmic if !falling => 1.0 - (1.0 - t).powf(power),
            Self::Logarithmic => t.powf(power),
            Self::SCurve => {
                let smooth = t * t * (3.0 - 2.0 * t);
                t + amount * (smooth - t)
            }
        }
    }
}

pub fn stage_frames(time_ms: f32, sample_rate_hz: u32) -> u64 {
    (f64::from(time_ms) * f64::from(sample_rate_hz) / 1000.0) as u64
}

#[derive(Clone, Copy, PartialEq)]
pub struct StageParameters {
    pub time_ms: f32,
    pub shape: Curve,
    pub amount: f32,
}
impl StageParameters {
    fn frames(self, sample_rate: u32) -> u64 {
        stage_frames(self.time_ms, sample_rate)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Parameters {
    pub note_timing: bool,
    pub attack_note: f32,
    pub decay_note: f32,
    pub attack: StageParameters,
    pub decay: StageParameters,
    pub sustain: f32,
    pub release: StageParameters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone)]
pub struct EnvelopeState {
    fractional_elapsed: f64,
    effective: Parameters,
    note_duration_frames: Option<u64>,
    sample_rate_hz: u32,
    stage: Stage,
    elapsed: u64,
    duration: u64,
    anchor_progress: f32,
    anchor_level: f32,
    level: f32,
    sustain_remaining: u32,
}
impl EnvelopeState {
    pub fn new(settings: Parameters, sample_rate_hz: u32) -> Self {
        Self {
            fractional_elapsed: 0.0,
            effective: settings,
            note_duration_frames: None,
            sample_rate_hz,
            stage: Stage::Idle,
            elapsed: 0,
            duration: 0,
            anchor_progress: 0.0,
            anchor_level: 0.0,
            level: 0.0,
            sustain_remaining: 0,
        }
    }
    pub fn reset(&mut self, settings: Parameters) {
        self.effective = settings;
        self.fractional_elapsed = 0.0;
        self.note_duration_frames = None;
        self.stage = Stage::Idle;
        self.level = 0.0;
        self.sustain_remaining = 0;
    }
    /// Known durations use processing-rate frames. Unknown live-note lengths retain Time
    /// timing as a fallback. The host supplies full note length, including hidden trim regions.
    pub fn note_on_with_duration(&mut self, duration_frames: Option<u64>) {
        self.note_duration_frames = duration_frames.map(|frames| frames.max(1));
        self.level = 0.0;
        self.start(Stage::Attack);
        self.advance(0);
    }
    pub fn note_off(&mut self) -> bool {
        if self.stage != Stage::Idle && self.stage != Stage::Release {
            self.start(Stage::Release);
            self.advance(0)
        } else {
            false
        }
    }
    pub fn level(&self) -> f32 {
        self.level
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    /// The active ADSR position in stage coordinates and its current level.
    pub fn plot_position(&self) -> Option<[f32; 2]> {
        let position = match self.stage {
            Stage::Idle => return None,
            Stage::Attack => self.progress(),
            Stage::Decay => 1.0 + self.progress(),
            Stage::Sustain => 2.5,
            Stage::Release => 3.0 + self.progress(),
        };
        Some([position, self.level])
    }
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }
    /// Defer zero-duration stages until the first resolved modulation input is available.
    pub fn note_on_modulated(&mut self, duration_frames: Option<u64>) {
        self.note_duration_frames = duration_frames.map(|n| n.max(1));
        self.level = 0.0;
        self.start(Stage::Attack);
    }

    pub fn set_effective(&mut self, settings: Parameters) {
        if self.effective == settings {
            return;
        }
        let progress = if self.duration == 0 {
            0.0
        } else {
            (self.elapsed as f64 + self.fractional_elapsed) / self.duration as f64
        };
        let previous_stage = self.stage_settings();
        let previous_sustain = self.effective.sustain;
        self.effective = settings;
        match self.stage {
            Stage::Attack | Stage::Decay | Stage::Release => {
                let duration = self.stage_duration();
                if previous_stage == self.stage_settings()
                    && self.duration == duration
                    && (self.stage != Stage::Decay || previous_sustain == settings.sustain)
                {
                    return;
                }
                self.duration = duration;
                self.elapsed = (progress * self.duration as f64) as u64;
                self.fractional_elapsed = 0.0;
                self.anchor_progress = self.progress();
                self.anchor_level = self.level;
            }
            Stage::Sustain if previous_sustain != settings.sustain => {
                self.sustain_remaining = (self.sample_rate_hz / 200).max(1);
            }
            Stage::Sustain => {}
            Stage::Idle => {}
        }
    }
    /// Seek reconstruction visits at most the three timed stages, never every sample.
    pub fn advance(&mut self, mut frames: u64) -> bool {
        loop {
            match self.stage {
                Stage::Idle => break,
                Stage::Sustain => {
                    let count = frames.min(u64::from(self.sustain_remaining)) as u32;
                    if count > 0 {
                        self.level += (self.effective.sustain - self.level) * count as f32
                            / self.sustain_remaining as f32;
                        self.sustain_remaining -= count;
                    }
                    if self.sustain_remaining == 0 {
                        self.level = self.effective.sustain;
                    }
                    break;
                }
                Stage::Attack | Stage::Decay | Stage::Release => {
                    let count = frames.min(self.duration.saturating_sub(self.elapsed));
                    self.elapsed += count;
                    frames -= count;
                    let target = match self.stage {
                        Stage::Attack => 1.0,
                        Stage::Decay => self.effective.sustain,
                        _ => 0.0,
                    };
                    if self.elapsed >= self.duration {
                        self.level = target;
                        match self.stage {
                            Stage::Attack => self.start(Stage::Decay),
                            Stage::Decay => self.start(Stage::Sustain),
                            _ => return true,
                        }
                    } else {
                        let local = (self.progress() - self.anchor_progress)
                            / (1.0 - self.anchor_progress).max(f32::EPSILON);
                        let settings = self.stage_settings();
                        let shaped = settings.shape.progress(
                            local,
                            settings.amount,
                            target < self.anchor_level,
                        );
                        self.level = self.anchor_level + (target - self.anchor_level) * shaped;
                        break;
                    }
                }
            }
        }
        false
    }
    /// Preserve fractional progress when a resolved duration changes.
    pub fn modulate(&mut self, settings: Parameters) {
        let progress = if self.duration == 0 {
            0.0
        } else {
            (self.elapsed as f64 + self.fractional_elapsed) / self.duration as f64
        };
        self.effective = settings;
        let duration = self.stage_duration();
        if duration != self.duration {
            let elapsed = progress * duration as f64;
            self.elapsed = elapsed as u64;
            self.fractional_elapsed = elapsed.fract();
            self.duration = duration;
        }
    }

    fn progress(&self) -> f32 {
        if self.duration == 0 {
            1.0
        } else if self.fractional_elapsed == 0.0 {
            self.elapsed as f32 / self.duration as f32
        } else {
            ((self.elapsed as f64 + self.fractional_elapsed) / self.duration as f64) as f32
        }
    }
    fn start(&mut self, stage: Stage) {
        self.stage = stage;
        self.elapsed = 0;
        self.fractional_elapsed = 0.0;
        self.anchor_progress = 0.0;
        self.anchor_level = self.level;
        self.sustain_remaining = 0;
        self.duration = self.stage_duration();
    }
    fn stage_duration(&self) -> u64 {
        if self.effective.note_timing
            && let Some(frames) = self.note_duration_frames
        {
            let fraction = match self.stage {
                Stage::Attack => Some(self.effective.attack_note),
                Stage::Decay => Some(self.effective.decay_note),
                _ => None,
            };
            if let Some(fraction) = fraction {
                return ((frames as f64 * f64::from(fraction)).round() as u64).min(frames);
            }
        }
        self.stage_settings().frames(self.sample_rate_hz)
    }
    fn stage_settings(&self) -> StageParameters {
        match self.stage {
            Stage::Attack => self.effective.attack,
            Stage::Decay => self.effective.decay,
            _ => self.effective.release,
        }
    }
}
