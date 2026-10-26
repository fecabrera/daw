//! Redesigned Drive, composed with the original color/clipping/tone kernel.
use super::{Drive, Ramp, amplitude};
use daw_unit_core::{
    DRIVE_V2_TYPE_ID, DriveColorShape, DriveSettings, DriveV2Clip, DriveV2Settings, EffectUnit,
    LevelMonitor, ModulationContext, ModulationDestination, ModulationDestinationSettings,
    ModulationInputs, ModulationParameter, ModulationScope, ModulationSmoother, PortKind,
    ProcessingContext, SILENT_GAIN_DB, UnitDescriptor,
};
use std::sync::{Arc, atomic::AtomicU32};
#[derive(Clone)]
pub struct DriveV2 {
    core: Drive,
    input: Ramp,
    level: Ramp,
    blend: Ramp,
    bypass: Ramp,
    hard: Ramp,
    external: ModulationSmoother<3>,
    frames: u32,
    effective: [f32; 7],
    monitor: LevelMonitor<3>,
}
fn kernel(s: &DriveV2Settings) -> DriveSettings {
    DriveSettings {
        gain: s.gain,
        tone: s.tone,
        color_shape: if s.color_enabled {
            s.color_shape.legacy()
        } else {
            DriveColorShape::Off
        },
        color_strength: s.color_strength,
        color_frequency_hz: s.color_frequency_hz,
        ..DriveSettings::default()
    }
}
fn gain(base: f32, offset: f32) -> f32 {
    if offset == 0.0 {
        base
    } else {
        (base * amplitude(offset.clamp(-96.0, 96.0))).min(amplitude(12.0))
    }
}
fn db(value: f32) -> f32 {
    if value > 0.0 {
        20.0 * value.log10()
    } else {
        SILENT_GAIN_DB
    }
}
fn mix(dry: f32, wet: f32, amount: f32) -> f32 {
    if amount == 0.0 {
        dry
    } else if amount == 1.0 {
        wet
    } else {
        dry + (wet - dry) * amount
    }
}
impl DriveV2 {
    /// Prepare filter state on a control thread with validated settings.
    pub fn new(context: ProcessingContext, s: &DriveV2Settings) -> Self {
        Self {
            core: Drive::new(context, &kernel(s)),
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
            input: Ramp::new(amplitude(s.input_gain_db)),
            level: Ramp::new(amplitude(s.level_db)),
            blend: Ramp::new(s.dry_wet),
            bypass: Ramp::new(f32::from(s.enabled)),
            hard: Ramp::new(f32::from(s.clip == DriveV2Clip::Hard)),
            external: ModulationSmoother::new(context),
            frames: (context.sample_rate_hz / 200).max(1),
            effective: [
                s.input_gain_db,
                s.gain,
                s.tone,
                s.color_strength,
                s.color_frequency_hz,
                s.level_db,
                s.dry_wet,
            ],
        }
    }
    /// Attach a prepared output monitor on the control thread.
    pub fn set_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }
    /// Attach current output and a click-resettable sample peak on the control thread.
    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }
    fn ramps(&mut self) -> [&mut Ramp; 5] {
        [
            &mut self.input,
            &mut self.level,
            &mut self.blend,
            &mut self.bypass,
            &mut self.hard,
        ]
    }

    /// Preserve filter history and unrelated ramps without allocation.
    pub fn retarget(&mut self, s: &DriveV2Settings) {
        self.monitor.set_enabled(s.enabled);
        self.core.retarget(&kernel(s));
        self.input.retarget(amplitude(s.input_gain_db), self.frames);
        self.level.retarget(amplitude(s.level_db), self.frames);
        self.blend.retarget(s.dry_wet, self.frames);
        self.bypass.retarget(f32::from(s.enabled), self.frames);
        self.hard
            .retarget(f32::from(s.clip == DriveV2Clip::Hard), self.frames);
    }
    /// Processed Input dB, Gain, Tone, Strength, Frequency Hz, Level dB, and wet fraction.
    pub fn effective_parameters(&self) -> [f32; 7] {
        self.effective
    }
}
impl EffectUnit for DriveV2 {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: DRIVE_V2_TYPE_ID,
            name: "Drive v2",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.monitor.reset();
        self.core.reset();
        self.external.reset();
        for r in self.ramps() {
            r.reset();
        }
        let p = self.core.effective_parameters();
        self.effective = [
            db(self.input.value),
            p[1],
            p[2],
            p[3],
            p[4],
            db(self.level.value),
            self.blend.value,
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_with_modulation(
            input,
            &ModulationInputs::neutral(1.0),
            ModulationContext::default(),
        )
    }
}
impl ModulationDestination for DriveV2 {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        <DriveV2Settings as ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: [f32; 2],
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> [f32; 2] {
        for r in self.ramps() {
            r.advance();
        }
        let offsets = self
            .external
            .process([
                m.offset(ModulationParameter::DriveInput),
                m.offset(ModulationParameter::Level),
                m.offset(ModulationParameter::DryWet),
            ])
            .map(|v| v as f32);
        let input_gain = gain(self.input.value, offsets[0]);
        let level = gain(self.level.value, offsets[1]);
        let blend = (self.blend.value + offsets[2]).clamp(0.0, 1.0);
        let dry = input.map(|v| v * input_gain);
        let wet = self.core.process_tone(
            dry,
            [
                0.0,
                m.offset(ModulationParameter::DriveGain),
                m.offset(ModulationParameter::Tone),
                m.offset(ModulationParameter::ColorStrength),
                m.offset(ModulationParameter::ColorFrequency),
            ],
            self.hard.value,
        );
        let processed = self.core.effective_parameters();
        self.effective = [
            db(input_gain),
            processed[1],
            processed[2],
            processed[3],
            processed[4],
            db(level),
            blend,
        ];
        let output = std::array::from_fn(|i| {
            // Full wet retains the original Drive's output-level arithmetic exactly.
            let mixed = if blend == 1.0 {
                dry[i] + (wet[i] * level - dry[i])
            } else {
                mix(dry[i], wet[i], blend) * level
            };
            mix(input[i], mixed, self.bypass.value)
        });
        self.monitor.sample_output(output);
        output
    }
}

#[cfg(test)]
mod tests;
