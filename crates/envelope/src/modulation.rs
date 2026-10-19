//! Typed Envelope inputs. Duration changes retain fractional stage progress.
use super::*;
use daw_unit_core::{
    ModulationContext, ModulationDestination, ModulationInputs, ModulationParameter,
    ModulationScope,
};
impl Envelope {
    /// Hosts use Shared for Retrigger/Legato instances; standalone envelopes default to PerNote.
    pub fn with_modulation_scope(mut self, scope: ModulationScope) -> Self {
        self.input_scope = scope;
        self
    }

    pub fn has_modulation_offsets(&self) -> bool {
        self.offsets != [0.0; 9]
    }
    pub(super) fn modulated_settings(&self) -> EnvelopeSettings {
        let mut s = self.settings;
        let o = self.offsets;
        let time = |base: f32, offset: f32| {
            (base * 2_f32.powf(offset.clamp(-64.0, 64.0)))
                .clamp(0.0, daw_unit_core::MAX_ENVELOPE_TIME_MS)
        };
        s.attack.time_ms = time(s.attack.time_ms, o[0]);
        s.decay.time_ms = time(s.decay.time_ms, o[1]);
        s.sustain = (s.sustain + o[2]).clamp(0.0, 1.0);
        s.release.time_ms = time(s.release.time_ms, o[3]);
        s.attack.amount = (s.attack.amount + o[4]).clamp(0.0, 1.0);
        s.decay.amount = (s.decay.amount + o[5]).clamp(0.0, 1.0);
        s.release.amount = (s.release.amount + o[6]).clamp(0.0, 1.0);
        s.attack_note = (s.attack_note + o[7]).clamp(0.0, 1.0);
        s.decay_note = (s.decay_note + o[8]).clamp(0.0, 1.0);
        s
    }
    pub fn modulation_values(&self) -> [f32; 9] {
        self.effective.parameter_values()
    }
}
impl ModulationDestination for Envelope {
    type Audio = ();
    const MODULATION_PARAMETERS: &'static [ModulationParameter] = &ModulationParameter::ENVELOPE;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::PerNote;
    fn modulation_input_scope(&self) -> ModulationScope {
        self.input_scope
    }
    fn process_with_modulation(&mut self, (): (), input: &ModulationInputs, _: ModulationContext) {
        let offsets = input.offsets(ModulationParameter::ENVELOPE);
        if !self.has_modulation_offsets() && offsets == [0.0; 9] {
            return;
        }
        self.offsets = self.modulation.process(offsets).map(|v| v as f32);
        let settings = self.modulated_settings();
        if settings == self.effective {
            return;
        }
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
}
