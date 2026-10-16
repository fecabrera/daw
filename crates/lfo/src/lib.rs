//! Portable normalized LFO source and the clock/wave primitives shared by Auto-filter.
use daw_unit_core::{
    LfoPolarity, LfoRateMode, LfoRestart, LfoSettings, ModulationBehavior,
    ModulationOutputDescriptor, ModulationRange, ModulationSource, ModulationSourceContext,
    PortKind, ProcessingContext, UnitDescriptor,
};

pub fn waveform(index: usize, phase: f64) -> f64 {
    let phase = phase.rem_euclid(1.0);
    match index {
        0 => (phase * std::f64::consts::TAU).sin(),
        1 => 1.0 - 4.0 * ((phase + 0.25).rem_euclid(1.0) - 0.5).abs(),
        2 => 2.0 * phase - 1.0,
        3 => 1.0 - 2.0 * phase,
        _ => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}
pub fn rate_hz(mode: LfoRateMode, hz: f32, note_beats: f64, tempo_bpm: f32) -> f64 {
    match mode {
        LfoRateMode::Hz => f64::from(hz),
        LfoRateMode::Note => f64::from(tempo_bpm) / (60.0 * note_beats),
    }
}
/// Transport phase with the existing 5 ms shortest-path correction on rate/tempo changes.
pub struct LfoClock {
    sample_rate: f64,
    fade: u32,
    last_frame: Option<u64>,
    last_origin: i128,
    last_rate: f64,
    last_phase: f64,
    correction: f64,
    remaining: u32,
}
impl LfoClock {
    pub fn new(context: ProcessingContext) -> Self {
        Self {
            sample_rate: f64::from(context.sample_rate_hz),
            fade: (context.sample_rate_hz / 200).max(1),
            last_frame: None,
            last_origin: 0,
            last_rate: 0.0,
            last_phase: 0.0,
            correction: 0.0,
            remaining: 0,
        }
    }
    pub fn reset(&mut self) {
        self.last_frame = None;
        self.correction = 0.0;
        self.remaining = 0;
    }
    pub fn phase_at(&mut self, frame: u64, origin: i128, rate: f64) -> f64 {
        let elapsed = (i128::from(frame) - origin).max(0) as f64;
        let target = (elapsed * rate / self.sample_rate).rem_euclid(1.0);
        let continuous = self.last_origin == origin
            && self
                .last_frame
                .is_some_and(|last| frame == last.saturating_add(1) || frame == last);
        if !continuous {
            self.correction = 0.0;
            self.remaining = 0;
        } else if rate != self.last_rate {
            self.correction = (self.last_phase + self.last_rate / self.sample_rate - target + 0.5)
                .rem_euclid(1.0)
                - 0.5;
            self.remaining = self.fade;
        }
        let phase = (target + self.correction).rem_euclid(1.0);
        if self.remaining > 0 {
            self.correction -= self.correction / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.last_frame = Some(frame);
        self.last_origin = origin;
        self.last_rate = rate;
        self.last_phase = phase;
        phase
    }
}
#[derive(Clone, Copy)]
struct Ramp {
    value: f64,
    target: f64,
    remaining: u32,
}
impl Ramp {
    fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            remaining: 0,
        }
    }
    fn set(&mut self, target: f64, frames: u32) {
        if self.target != target {
            self.target = target;
            self.remaining = frames;
        }
    }
    fn next(&mut self) -> f64 {
        if self.remaining > 0 {
            self.value += (self.target - self.value) / f64::from(self.remaining);
            self.remaining -= 1;
        }
        self.value
    }
    fn reset(&mut self) {
        self.value = self.target;
        self.remaining = 0;
    }
}
pub struct Lfo {
    settings: LfoSettings,
    clock: LfoClock,
    fade: u32,
    waves: [Ramp; 5],
    phase: Ramp,
    smoothing: Ramp,
}
impl Lfo {
    pub fn new(settings: LfoSettings, context: ProcessingContext) -> Self {
        Self {
            settings,
            clock: LfoClock::new(context),
            fade: (context.sample_rate_hz / 200).max(1),
            waves: std::array::from_fn(|i| Ramp::new(f64::from(i == settings.wave.index()))),
            phase: Ramp::new(f64::from(settings.phase_degrees) / 360.0),
            smoothing: Ramp::new(f64::from(settings.smooth_ms)),
        }
    }
    pub fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::LFO_TYPE_ID,
            name: "LFO",
            is_source: false,
            inputs: &[PortKind::NoteEvents],
            outputs: &[PortKind::Modulation],
        }
    }
    pub fn settings(&self) -> LfoSettings {
        self.settings
    }
    pub fn set_settings(&mut self, settings: LfoSettings) {
        for (i, ramp) in self.waves.iter_mut().enumerate() {
            ramp.set(f64::from(i == settings.wave.index()), self.fade);
        }
        self.phase
            .set(f64::from(settings.phase_degrees) / 360.0, self.fade);
        self.smoothing.set(f64::from(settings.smooth_ms), self.fade);
        self.settings = settings;
    }
    pub fn reset(&mut self) {
        self.clock.reset();
        for wave in &mut self.waves {
            wave.reset();
        }
        self.phase.reset();
        self.smoothing.reset();
    }
    pub fn sample_at(&mut self, context: ModulationSourceContext) -> f32 {
        let origin = match self.settings.restart {
            LfoRestart::Transport => 0,
            LfoRestart::Retrigger => context.note_origin.unwrap_or(0),
            LfoRestart::Legato => context.legato_origin.unwrap_or(0),
        };
        let hz = rate_hz(
            self.settings.rate_mode,
            self.settings.rate_hz,
            self.settings.note_rate.beats(),
            context.tempo_bpm,
        );
        let phase = self.clock.phase_at(context.frame, origin, hz) + self.phase.next();
        for weight in &mut self.waves {
            weight.next();
        }
        let ms = self.smoothing.next();
        let smoothed = periodic_value(&self.waves, phase, hz, ms);
        let value = match self.settings.polarity {
            LfoPolarity::Unipolar => (smoothed + 1.0) * 0.5,
            LfoPolarity::Bipolar => smoothed,
        };
        value as f32
    }
}
// Exact periodic one-pole response for the five shapes; O(1), including very slow cycles.
fn periodic_value(waves: &[Ramp; 5], phase: f64, hz: f64, ms: f64) -> f64 {
    if ms <= 0.0 {
        return waves
            .iter()
            .enumerate()
            .map(|(i, w)| w.value * waveform(i, phase))
            .sum();
    }
    waves
        .iter()
        .enumerate()
        .filter(|(_, w)| w.value != 0.0)
        .map(|(i, w)| w.value * smooth_wave(i, phase, hz, ms / 1000.0))
        .sum::<f64>()
        .clamp(-1.0, 1.0)
}
fn smooth_wave(index: usize, phase: f64, hz: f64, tau: f64) -> f64 {
    let cycle = phase.rem_euclid(1.0);
    let k = 1.0 / (hz * tau);
    match index {
        0 => {
            let w = std::f64::consts::TAU * hz * tau;
            let angle = phase * std::f64::consts::TAU;
            (angle.sin() - w * angle.cos()) / (1.0 + w * w)
        }
        1 => {
            let p = (cycle + 0.25).rem_euclid(1.0);
            let slope = 4.0 / k;
            let e = (-0.5 * k).exp();
            let corner = 1.0 - slope * (1.0 - e) / (1.0 + e);
            if p < 0.5 {
                4.0 * p - 1.0 - slope + (-corner + 1.0 + slope) * (-p * k).exp()
            } else {
                3.0 - 4.0 * p + slope + (corner - 1.0 - slope) * (-(p - 0.5) * k).exp()
            }
        }
        2 | 3 => {
            let saw = 2.0 * cycle - 1.0 - 2.0 / k + 2.0 * (-cycle * k).exp() / -(-k).exp_m1();
            if index == 2 { saw } else { -saw }
        }
        _ => {
            if cycle < 0.5 {
                1.0 - 2.0 * (-cycle * k).exp() / (1.0 + (-0.5 * k).exp())
            } else {
                -1.0 + 2.0 * (-(cycle - 0.5) * k).exp() / (1.0 + (-0.5 * k).exp())
            }
        }
    }
}

impl ModulationSource for Lfo {
    const MODULATION_OUTPUT: ModulationOutputDescriptor = ModulationOutputDescriptor {
        name: "LFO",
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Continuous,
    };
    fn modulation_output(&self) -> ModulationOutputDescriptor {
        ModulationOutputDescriptor {
            range: self.settings.polarity.range(),
            ..Self::MODULATION_OUTPUT
        }
    }
    fn next_modulation(&mut self, context: ModulationSourceContext) -> f32 {
        self.sample_at(context)
    }
}

#[cfg(test)]
mod tests;
