//! Envelope adapter for shared note and modulation contracts.
pub use daw_dsp_support::envelope::Stage;
use daw_dsp_support::envelope::{EnvelopeState, Parameters, StageParameters};
#[cfg(test)]
use daw_unit_core::EnvelopeStageSettings;
use daw_unit_core::{
    EnvelopeSettings, EnvelopeTiming, PortKind, ProcessingContext, UnitDescriptor,
};

fn parameters(settings: EnvelopeSettings) -> Parameters {
    let stage = |s: daw_unit_core::EnvelopeStageSettings| StageParameters {
        time_ms: s.time_ms,
        shape: s.shape.into(),
        amount: s.amount,
    };
    Parameters {
        note_timing: settings.timing == EnvelopeTiming::Note,
        attack_note: settings.attack_note,
        decay_note: settings.decay_note,
        attack: stage(settings.attack),
        decay: stage(settings.decay),
        sustain: settings.sustain,
        release: stage(settings.release),
    }
}

#[derive(Clone)]
pub struct Envelope {
    settings: EnvelopeSettings,
    input_scope: daw_unit_core::ModulationScope,
    modulation: daw_unit_core::ModulationSmoother<9>,
    offsets: [f32; 9],
    effective: EnvelopeSettings,
    kernel: EnvelopeState,
}

impl Envelope {
    pub fn new(settings: EnvelopeSettings, context: ProcessingContext) -> Self {
        Self {
            settings,
            input_scope: daw_unit_core::ModulationScope::PerNote,
            modulation: daw_unit_core::ModulationSmoother::new(context),
            offsets: [0.0; 9],
            effective: settings,
            kernel: EnvelopeState::new(parameters(settings), context.sample_rate_hz),
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
        self.kernel.reset(parameters(self.settings));
    }

    pub fn note_on(&mut self) {
        self.note_on_with_duration(None);
    }

    pub fn note_on_with_duration(&mut self, duration_frames: Option<u64>) {
        self.kernel.note_on_with_duration(duration_frames);
    }

    pub fn note_on_modulated(&mut self, duration_frames: Option<u64>) {
        self.kernel.note_on_modulated(duration_frames);
    }

    pub fn note_off(&mut self) {
        if self.kernel.note_off() {
            self.reset();
        }
    }

    pub fn level(&self) -> f32 {
        self.kernel.level()
    }

    pub fn stage(&self) -> Stage {
        self.kernel.stage()
    }

    pub fn plot_position(&self) -> Option<[f32; 2]> {
        self.kernel.plot_position()
    }

    pub fn is_idle(&self) -> bool {
        self.kernel.is_idle()
    }

    pub fn settings(&self) -> EnvelopeSettings {
        self.settings
    }

    pub fn set_settings(&mut self, settings: EnvelopeSettings) {
        self.settings = settings;
        let effective = self.modulated_settings();
        if self.effective != effective {
            self.effective = effective;
            self.kernel.set_effective(parameters(effective));
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        self.advance(1);
        self.level()
    }

    pub fn advance(&mut self, frames: u64) {
        if self.kernel.advance(frames) {
            self.reset();
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
