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

#[derive(Clone)]
pub struct Envelope {
    settings: EnvelopeSettings,
    input_scope: daw_unit_core::ModulationScope,
    modulation: daw_unit_core::ModulationSmoother<9>,
    offsets: [f32; 9],
    fractional_elapsed: f64,
    effective: EnvelopeSettings,
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
            input_scope: daw_unit_core::ModulationScope::PerNote,
            modulation: daw_unit_core::ModulationSmoother::new(context),
            offsets: [0.0; 9],
            fractional_elapsed: 0.0,
            effective: settings,
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
            inputs: &[PortKind::NoteEvents, PortKind::Modulation],
            outputs: &[PortKind::Modulation],
        }
    }
    pub fn reset(&mut self) {
        self.modulation.reset();
        self.offsets = [0.0; 9];
        self.effective = self.settings;
        self.fractional_elapsed = 0.0;
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
    /// Defer zero-duration stages until the first resolved modulation input is available.
    pub fn note_on_modulated(&mut self, duration_frames: Option<u64>) {
        self.note_duration_frames = duration_frames.map(|n| n.max(1));
        self.level = 0.0;
        self.start(Stage::Attack);
    }

    /// Retain normalized stage progress and its current level. Curve changes are
    /// anchored to that level and shape only the remaining part of the stage.
    pub fn set_settings(&mut self, settings: EnvelopeSettings) {
        self.settings = settings;
        self.set_effective(self.modulated_settings());
    }
    fn set_effective(&mut self, settings: EnvelopeSettings) {
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
        if self.effective.timing == daw_unit_core::EnvelopeTiming::Note
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
    fn stage_settings(&self) -> EnvelopeStageSettings {
        match self.stage {
            Stage::Attack => self.effective.attack,
            Stage::Decay => self.effective.decay,
            _ => self.effective.release,
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

mod modulation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod modulation_tests;
