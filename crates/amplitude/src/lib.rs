//! A per-voice amplitude stage. Hosts supply a resolved envelope factor and note gate.
use daw_dsp_support::gain::{amplitude as gain, voice_gain};
use daw_unit_core::ModulationRamp as Ramp;
use daw_unit_core::{AmplitudeSettings, ProcessingContext};
#[derive(Clone)]
pub struct Amplitude {
    level: Ramp,
    mix: Ramp,
    frames: u32,
    effective_gain: f32,
}
impl Amplitude {
    pub fn descriptor(&self) -> daw_unit_core::UnitDescriptor {
        daw_unit_core::UnitDescriptor {
            type_id: "daw.amplitude",
            name: "Amplitude",
            is_source: false,
            inputs: &[
                daw_unit_core::PortKind::MonoAudio,
                daw_unit_core::PortKind::Modulation,
            ],
            outputs: &[daw_unit_core::PortKind::MonoAudio],
        }
    }
    pub fn new(settings: &AmplitudeSettings, context: ProcessingContext) -> Self {
        Self {
            level: Ramp::new(gain(settings.level_db)),
            mix: Ramp::new(f32::from(settings.present && settings.enabled)),
            frames: (context.sample_rate_hz / 200).max(1),
            effective_gain: 0.0,
        }
    }
    pub fn set_settings(&mut self, settings: &AmplitudeSettings) {
        self.level.set(gain(settings.level_db), self.frames);
        self.mix
            .set(f32::from(settings.present && settings.enabled), self.frames);
    }
    pub fn retarget(&mut self, next: &Self) {
        self.level.set(next.level.target(), self.frames);
        self.mix.set(next.mix.target(), self.frames);
    }
    pub fn reset(&mut self) {
        self.level.reset();
        self.mix.reset();
        self.effective_gain = 0.0;
    }
    /// Last processed gain, including envelope, level smoothing and bypass crossfade.
    pub fn effective_gain(&self) -> f32 {
        self.effective_gain
    }
    pub fn process(&mut self, input: f32, factor: f32, gate: f32) -> f32 {
        let level = self.level.next_value();
        let mix = self.mix.next_value();
        let shape = voice_gain(level, mix, factor, gate);
        self.effective_gain = shape;
        input * shape
    }
}
impl daw_unit_core::ModulationDestination for Amplitude {
    type Audio = f32;
    const MODULATION_PARAMETERS: &'static [daw_unit_core::ModulationParameter] =
        <AmplitudeSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: daw_unit_core::ModulationScope = <daw_unit_core::AmplitudeSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: f32,
        modulation: &daw_unit_core::ModulationInputs,
        context: daw_unit_core::ModulationContext,
    ) -> f32 {
        self.process(input, modulation.amplitude(), context.note_gate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn level_power_and_silence_follow_five_millisecond_ramps_and_reset_targets() {
        let mut settings = AmplitudeSettings {
            present: true,
            ..Default::default()
        };
        let mut amp = Amplitude::new(
            &settings,
            ProcessingContext {
                sample_rate_hz: 1000,
                max_block_frames: 32,
            },
        );
        assert_eq!(amp.process(1.0, 0.25, 1.0), 0.25);
        settings.enabled = false;
        amp.set_settings(&settings);
        let mut previous = 0.25;
        for _ in 0..5 {
            let sample = amp.process(1.0, 0.25, 1.0);
            assert!(sample >= previous && sample <= 1.0);
            previous = sample;
        }
        assert_eq!(previous, 1.0);
        settings.enabled = true;
        settings.level_db = daw_unit_core::SILENT_GAIN_DB;
        amp.set_settings(&settings);
        amp.reset();
        assert_eq!(amp.process(1.0, 1.0, 1.0), 0.0);
    }
}
