//! Portable normalized LFO source and the clock/wave primitives shared by Auto-filter.
use daw_unit_core::ramps::LinearRamp64 as Ramp;
use daw_unit_core::{
    LfoPolarity, LfoRateMode, LfoRestart, LfoSettings, ModulationBehavior, ModulationContext,
    ModulationDestination, ModulationDestinationSettings, ModulationInputs,
    ModulationOutputDescriptor, ModulationParameter, ModulationRange, ModulationScope,
    ModulationSmoother, ModulationSource, ModulationSourceContext, PortKind, ProcessingContext,
    UnitDescriptor,
};

pub use daw_dsp_support::periodic::waveform;
use daw_dsp_support::periodic::{PhaseClock, RateOffsetPhase, periodic_value};

pub fn rate_hz(mode: LfoRateMode, hz: f32, note_beats: f64, tempo_bpm: f32) -> f64 {
    match mode {
        LfoRateMode::Hz => f64::from(hz),
        LfoRateMode::Note => f64::from(tempo_bpm) / (60.0 * note_beats),
    }
}
#[derive(Clone)]
pub struct LfoClock(PhaseClock);

impl LfoClock {
    pub fn new(context: ProcessingContext) -> Self {
        Self(PhaseClock::new(context.sample_rate_hz))
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }

    /// Use musical coordinates for note divisions when the caller has a position.
    pub fn phase(
        &mut self,
        frame: u64,
        origin: i128,
        rate: f64,
        mode: LfoRateMode,
        beats: f64,
        quarter: Option<f64>,
    ) -> f64 {
        match (mode, quarter) {
            (LfoRateMode::Note, Some(quarter)) => {
                self.phase_at_quarters(frame, origin, quarter, beats, rate)
            }
            _ => self.phase_at(frame, origin, rate),
        }
    }

    pub fn phase_at_quarters(
        &mut self,
        frame: u64,
        origin: i128,
        quarter: f64,
        beats: f64,
        rate: f64,
    ) -> f64 {
        self.0
            .phase_at_quarters(frame, origin, quarter, beats, rate)
    }

    pub fn phase_at(&mut self, frame: u64, origin: i128, rate: f64) -> f64 {
        self.0.phase_at(frame, origin, rate)
    }
}
#[derive(Clone)]
pub struct Lfo {
    settings: LfoSettings,
    clock: LfoClock,
    fade: u32,
    waves: [Ramp; 5],
    phase: Ramp,
    smoothing: Ramp,
    depth: Ramp,
    modulation: ModulationSmoother<2>,
    offsets: [f64; 2],
    rate_phase: RateOffsetPhase,
    effective: [f32; 2],
}
impl Lfo {
    pub fn new(settings: LfoSettings, context: ProcessingContext) -> Self {
        Self {
            settings,
            clock: LfoClock::new(context),
            fade: (context.sample_rate_hz / 200).max(1),
            waves: Ramp::weights(settings.wave.index()),
            phase: Ramp::new(f64::from(settings.phase_degrees) / 360.0),
            smoothing: Ramp::new(f64::from(settings.smooth_ms)),
            depth: Ramp::new(f64::from(settings.depth)),
            modulation: ModulationSmoother::new(context),
            offsets: [0.0; 2],
            rate_phase: RateOffsetPhase::new(context.sample_rate_hz),
            effective: [settings.depth, settings.rate_hz],
        }
    }
    pub fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::LFO_TYPE_ID,
            name: "LFO",
            is_source: false,
            inputs: &[PortKind::NoteEvents, PortKind::Modulation],
            outputs: &[PortKind::Modulation],
        }
    }
    pub fn settings(&self) -> LfoSettings {
        self.settings
    }
    pub fn set_settings(&mut self, settings: LfoSettings) {
        for (i, ramp) in self.waves.iter_mut().enumerate() {
            ramp.retarget(f64::from(i == settings.wave.index()), self.fade);
        }
        self.phase
            .retarget(f64::from(settings.phase_degrees) / 360.0, self.fade);
        self.smoothing
            .retarget(f64::from(settings.smooth_ms), self.fade);
        self.depth.retarget(f64::from(settings.depth), self.fade);
        self.settings = settings;
    }
    pub fn reset(&mut self) {
        self.clock.reset();
        for wave in &mut self.waves {
            wave.reset();
        }
        self.phase.reset();
        self.smoothing.reset();
        self.depth.reset();
        self.modulation.reset();
        self.offsets = [0.0; 2];
        self.rate_phase.reset();
    }
    /// Direct signal sample, including Depth. Hosts should publish through ModulationOutput,
    /// which carries Depth as a neutral contribution weight for all destination semantics.
    pub fn sample_at(&mut self, context: ModulationSourceContext) -> f32 {
        let value = self.control_at(context);
        value * self.effective[0]
    }
    fn control_at(&mut self, context: ModulationSourceContext) -> f32 {
        let origin = match self.settings.restart {
            LfoRestart::Transport => 0,
            LfoRestart::Retrigger => context.note_origin.unwrap_or(0),
            LfoRestart::Legato => context.legato_origin.unwrap_or(0),
        };
        let base_hz = self.settings.base_rate_hz(context.tempo_bpm);
        let [minimum, maximum] = self.settings.rate_bounds(context.tempo_bpm);
        let hz =
            (base_hz * 2.0_f64.powf(self.offsets[1].clamp(-24.0, 24.0))).clamp(minimum, maximum);
        let rate_phase = self.rate_phase.sample(context.frame, origin, hz - base_hz);
        let quarter = context.musical.map(|m| {
            let origin = match self.settings.restart {
                LfoRestart::Transport => 0.0,
                LfoRestart::Retrigger => m.note_origin.unwrap_or(0.0),
                LfoRestart::Legato => m.legato_origin.unwrap_or(0.0),
            };
            (m.quarter - origin).max(0.0)
        });
        let base_phase = self.clock.phase(
            context.frame,
            origin,
            base_hz,
            self.settings.rate_mode,
            self.settings.note_rate.beats(),
            quarter,
        );
        let phase = base_phase + rate_phase + self.phase.next_value();
        let depth = (self.depth.next_value() + self.offsets[0]).clamp(0.0, 1.0);
        self.effective = [depth as f32, hz as f32];
        for weight in &mut self.waves {
            weight.next_value();
        }
        let ms = self.smoothing.next_value();
        let smoothed = periodic_value(&self.waves, phase, hz, ms);
        let value = match self.settings.polarity {
            LfoPolarity::Unipolar => (smoothed + 1.0) * 0.5,
            LfoPolarity::Bipolar => smoothed,
        };
        value as f32
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
        self.control_at(context)
    }
    fn modulation_weight(&self) -> f32 {
        self.effective[0]
    }
}

#[cfg(test)]
mod tests;

impl Lfo {
    /// Effective Depth and Rate (Hz) from the last published sample.
    pub fn modulation_values(&self) -> [f32; 2] {
        self.effective
    }
}
impl ModulationDestination for Lfo {
    type Audio = ();
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        <daw_unit_core::LfoInstance as ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope =
        <daw_unit_core::LfoInstance as ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        (): (),
        input: &ModulationInputs,
        context: ModulationContext,
    ) {
        if self
            .rate_phase
            .last_frame()
            .is_some_and(|last| context.frame != last && context.frame != last.saturating_add(1))
        {
            self.modulation.reset();
        }
        self.offsets = self
            .modulation
            .process(input.offsets(ModulationParameter::LFO));
    }
}
