//! Reusable, allocation-free per-note ADSR modulator. Hosts own note identity and routing.
use daw_unit_core::{
    EnvelopeSettings, EnvelopeStageSettings, PortKind, ProcessingContext, UnitDescriptor,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct Envelope {
    settings: EnvelopeSettings,
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
impl Envelope {
    pub fn new(settings: EnvelopeSettings, context: ProcessingContext) -> Self {
        Self {
            settings,
            note_duration_frames: None,
            sample_rate_hz: context.sample_rate_hz,
            stage: Stage::Idle,
            elapsed: 0,
            duration: 0,
            anchor_progress: 0.0,
            anchor_level: 0.0,
            level: 0.0,
            sustain_remaining: 0,
        }
    }
    pub fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: "daw.envelope",
            name: "Envelope",
            is_source: false,
            inputs: &[PortKind::NoteEvents],
            outputs: &[PortKind::Modulation],
        }
    }
    pub fn reset(&mut self) {
        self.note_duration_frames = None;
        self.stage = Stage::Idle;
        self.level = 0.0;
        self.sustain_remaining = 0;
    }
    pub fn note_on(&mut self) {
        self.note_on_with_duration(None);
    }
    /// Known durations use processing-rate frames. Unknown live-note lengths retain Time
    /// timing as a fallback. The host supplies full note length, including hidden trim regions.
    pub fn note_on_with_duration(&mut self, duration_frames: Option<u64>) {
        self.note_duration_frames = duration_frames.map(|frames| frames.max(1));
        self.level = 0.0;
        self.start(Stage::Attack);
        self.advance(0);
    }
    pub fn note_off(&mut self) {
        if self.stage != Stage::Idle && self.stage != Stage::Release {
            self.start(Stage::Release);
            self.advance(0);
        }
    }
    pub fn level(&self) -> f32 {
        self.level
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }
    pub fn settings(&self) -> EnvelopeSettings {
        self.settings
    }

    /// Retain normalized stage progress and its current level. Curve changes are
    /// anchored to that level and shape only the remaining part of the stage.
    pub fn set_settings(&mut self, settings: EnvelopeSettings) {
        if self.settings == settings {
            return;
        }
        let progress = if self.duration == 0 {
            0.0
        } else {
            self.elapsed as f64 / self.duration as f64
        };
        let previous_stage = self.stage_settings();
        let previous_sustain = self.settings.sustain;
        self.settings = settings;
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
    pub fn next_sample(&mut self) -> f32 {
        self.advance(1);
        self.level
    }
    /// Seek reconstruction visits at most the three timed stages, never every sample.
    pub fn advance(&mut self, mut frames: u64) {
        loop {
            match self.stage {
                Stage::Idle => break,
                Stage::Sustain => {
                    let count = frames.min(u64::from(self.sustain_remaining)) as u32;
                    if count > 0 {
                        self.level += (self.settings.sustain - self.level) * count as f32
                            / self.sustain_remaining as f32;
                        self.sustain_remaining -= count;
                    }
                    if self.sustain_remaining == 0 {
                        self.level = self.settings.sustain;
                    }
                    break;
                }
                Stage::Attack | Stage::Decay | Stage::Release => {
                    let count = frames.min(self.duration.saturating_sub(self.elapsed));
                    self.elapsed += count;
                    frames -= count;
                    let target = match self.stage {
                        Stage::Attack => 1.0,
                        Stage::Decay => self.settings.sustain,
                        _ => 0.0,
                    };
                    if self.elapsed >= self.duration {
                        self.level = target;
                        match self.stage {
                            Stage::Attack => self.start(Stage::Decay),
                            Stage::Decay => self.start(Stage::Sustain),
                            _ => self.reset(),
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
    }
    fn progress(&self) -> f32 {
        if self.duration == 0 {
            1.0
        } else {
            self.elapsed as f32 / self.duration as f32
        }
    }
    fn start(&mut self, stage: Stage) {
        self.stage = stage;
        self.elapsed = 0;
        self.anchor_progress = 0.0;
        self.anchor_level = self.level;
        self.sustain_remaining = 0;
        self.duration = self.stage_duration();
    }
    fn stage_duration(&self) -> u64 {
        if self.settings.timing == daw_unit_core::EnvelopeTiming::Note
            && let Some(frames) = self.note_duration_frames
        {
            let fraction = match self.stage {
                Stage::Attack => Some(self.settings.attack_note),
                Stage::Decay => Some(self.settings.decay_note),
                _ => None,
            };
            if let Some(fraction) = fraction {
                return ((frames as f64 * f64::from(fraction)).round() as u64).min(frames);
            }
        }
        self.stage_settings().frames(self.sample_rate_hz)
    }
    fn stage_settings(&self) -> EnvelopeStageSettings {
        match self.stage {
            Stage::Attack => self.settings.attack,
            Stage::Decay => self.settings.decay,
            _ => self.settings.release,
        }
    }
}

impl daw_unit_core::ModulationSource for Envelope {
    const MODULATION_OUTPUT: daw_unit_core::ModulationOutputDescriptor =
        daw_unit_core::ENVELOPE_MODULATION_OUTPUT;
    fn next_modulation(&mut self, _context: daw_unit_core::ModulationSourceContext) -> f32 {
        self.next_sample()
    }
}

#[cfg(test)]
mod tests;
