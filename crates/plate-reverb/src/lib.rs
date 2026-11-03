//! Portable Plate Reverb; prepared storage and shared modulation/meter lifecycle.
use daw_reverb_core::{Parameters, Reverb};
use daw_unit_core::{
    EffectUnit, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter as P, ModulationScope, ModulationSmoother,
    PlateReverbSettings as Settings, PortKind, ProcessingContext, UnitDescriptor,
};
#[derive(Clone)]
pub struct PlateReverb {
    core: Reverb,
    offsets: ModulationSmoother<8>,
}
fn parameters(s: &Settings) -> Parameters {
    Parameters {
        values: [
            s.decay_seconds,
            s.predelay_ms,
            s.highpass_hz,
            s.lowpass_hz,
            s.drive_db,
            0.0,
            0.5,
            0.0,
            s.width,
            s.level_db,
            s.dry_wet,
            f32::from(s.enabled),
        ],
        hq: s.hq,
        gated: s.gated,
        dark: s.dark,
        drive_enabled: s.drive_enabled,
        highpass_enabled: true,
    }
}
impl PlateReverb {
    pub fn new(c: ProcessingContext, s: &Settings) -> Self {
        Self {
            core: Reverb::new(c, parameters(s), true),
            offsets: ModulationSmoother::new(c),
        }
    }
    pub fn retarget(&mut self, s: &Settings) {
        self.core.retarget(parameters(s));
    }
    pub fn heap_bytes(&self) -> u64 {
        self.core.heap_bytes()
    }
    pub fn set_meter(&mut self, m: std::sync::Arc<[std::sync::atomic::AtomicU32; 3]>) {
        self.core.set_meter(m);
    }
    pub fn set_output_peak_meter(&mut self, m: std::sync::Arc<daw_unit_core::BandMeter<3>>) {
        self.core.set_output_peak_meter(m);
    }
    pub fn effective_parameters(&self) -> [f32; 8] {
        let v = self.core.effective();
        [v[1], v[0], v[2], v[3], v[4], v[8], v[9], v[10]]
    }
    pub fn process_modulated_at(
        &mut self,
        input: [f32; 2],
        _: u64,
        _: f32,
        offsets: [f32; 8],
    ) -> [f32; 2] {
        let o = self.offsets.process(offsets);
        let mut offsets = [0.0; 12];
        offsets[1] = o[0];
        offsets[0] = o[1];
        offsets[2] = o[2];
        offsets[3] = o[3];
        offsets[4] = o[4];
        offsets[8] = o[5];
        offsets[9] = o[6];
        offsets[10] = o[7];
        self.core.sample(input, offsets)
    }
}
impl EffectUnit for PlateReverb {
    fn tail_frames(&self) -> u64 {
        self.core.tail_frames()
    }
    fn heap_bytes(&self) -> u64 {
        self.heap_bytes()
    }
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: daw_unit_core::PLATE_REVERB_TYPE_ID,
            name: "Plate Reverb",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.core.reset();
        self.offsets.reset();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated_at(input, 0, 120.0, [0.0; 8])
    }
}
impl ModulationDestination for PlateReverb {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [P] = Settings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        m: &ModulationInputs,
        c: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated_at(input, c.frame, c.tempo_bpm, m.offsets(P::PLATE_REVERB))
    }
}
