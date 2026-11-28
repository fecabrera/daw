//! Stereo drive with color and tone filters before clipping.
//! FunDSP filter types stay private; callback operations do not allocate or block.
use daw_dsp_support::drive::{DriveKernel, amplitude, gain_db, modulated_gain, output};
use daw_unit_core::ramps::ExactRamp32 as Ramp;
use daw_unit_core::{
    DRIVE_TYPE_ID, DriveSettings, EffectUnit, LevelMonitor, PortKind, ProcessingContext,
    UnitDescriptor,
};
use std::sync::{Arc, atomic::AtomicU32};

mod v2;
pub use v2::DriveV2;

#[derive(Clone)]
pub struct Drive {
    kernel: DriveKernel,
    gain: Ramp,
    tone: Ramp,
    volume: Ramp,
    strength: Ramp,
    frequency_log: Ramp,
    colors: [Ramp; 4],
    mix: Ramp,
    fade_frames: u32,
    external: daw_unit_core::ModulationSmoother<5>,
    effective: [f32; 5],
    monitor: LevelMonitor<3>,
}
impl Drive {
    /// Prepare on a control thread with validated settings and a supported sample rate.
    pub fn new(context: ProcessingContext, settings: &DriveSettings) -> Self {
        let mut drive = Self {
            kernel: DriveKernel::new(context.sample_rate_hz, settings.color_frequency_hz),
            gain: Ramp::new(settings.gain),
            tone: Ramp::new(settings.tone),
            volume: Ramp::new(amplitude(settings.volume_db)),
            strength: Ramp::new(settings.color_strength),
            frequency_log: Ramp::new(settings.color_frequency_hz.ln()),
            colors: std::array::from_fn(|index| {
                Ramp::new(f32::from(index == settings.color_shape.index()))
            }),
            mix: Ramp::new(f32::from(settings.enabled)),
            fade_frames: (context.sample_rate_hz / 200).max(1),
            monitor: LevelMonitor::new(context.sample_rate_hz, settings.enabled),
            external: daw_unit_core::ModulationSmoother::new(context),
            effective: [
                amplitude(settings.volume_db),
                settings.gain,
                settings.tone,
                settings.color_strength,
                settings.color_frequency_hz,
            ],
        };
        drive.configure(drive.base_parameters());
        drive
    }
    /// Attach a prepared output monitor on the control thread.
    pub fn set_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }
    /// Attach current output and a click-resettable sample peak on the control thread.
    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak_prepared(meter);
    }
    /// Preserve filter history and any unrelated in-flight ramps on live edits.
    pub fn retarget(&mut self, settings: &DriveSettings) {
        self.monitor.set_enabled(settings.enabled);
        let frames = self.fade_frames;
        self.gain.retarget(settings.gain, frames);
        self.tone.retarget(settings.tone, frames);
        self.volume.retarget(amplitude(settings.volume_db), frames);
        self.strength.retarget(settings.color_strength, frames);
        self.frequency_log
            .retarget(settings.color_frequency_hz.ln(), frames);
        for (index, color) in self.colors.iter_mut().enumerate() {
            color.retarget(f32::from(index == settings.color_shape.index()), frames);
        }
        self.mix.retarget(f32::from(settings.enabled), frames);
    }

    fn base_parameters(&self) -> [f32; 4] {
        [
            self.gain.value(),
            self.tone.value(),
            self.strength.value(),
            self.frequency_log.value(),
        ]
    }
    fn configure(&mut self, values: [f32; 4]) {
        if self.kernel.configure(values) {
            self.effective[1..].copy_from_slice(&[
                values[0],
                values[1],
                values[2],
                values[3].exp(),
            ]);
        }
    }
    fn ramps(&mut self) -> impl Iterator<Item = &mut Ramp> {
        [
            &mut self.gain,
            &mut self.tone,
            &mut self.volume,
            &mut self.strength,
            &mut self.frequency_log,
            &mut self.mix,
        ]
        .into_iter()
        .chain(self.colors.iter_mut())
    }
}
impl EffectUnit for Drive {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: DRIVE_TYPE_ID,
            name: "Drive",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.monitor.reset();
        for ramp in self.ramps() {
            ramp.reset();
        }
        self.kernel.reset();
        self.external.reset();
        self.effective[0] = self.volume.value();
        self.configure(self.base_parameters());
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 5])
    }
}
impl Drive {
    /// Last processed Volume dB, Gain, Tone, Color strength, and Color frequency Hz.
    pub fn effective_parameters(&self) -> [f32; 5] {
        let mut values = self.effective;
        values[0] = gain_db(values[0]);
        values
    }
    /// Offsets in the same order: dB, normalized fractions, and octaves for Frequency.
    /// Sources and route amounts belong to the host; this portable stage smooths offsets by 1 ms.
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 5]) -> [f32; 2] {
        let clipped = self.process_wet(input, offsets, 0.0, 0.0);
        let output = output(input, clipped, self.effective[0], self.mix.value());
        self.monitor.sample_output(output);
        output
    }
    /// Color/Tone/clipping output before the legacy Volume and power blend.
    pub(crate) fn process_wet(
        &mut self,
        input: [f32; 2],
        offsets: [f32; 5],
        hard: f32,
        tube: f32,
    ) -> [f32; 2] {
        let offsets = self.external.process(offsets);
        for ramp in self.ramps() {
            ramp.advance();
        }
        let offsets = offsets.map(|v| v as f32);
        let [gain, tone, strength, frequency_log] = self.base_parameters();
        self.configure([
            (gain + offsets[1]).clamp(0.0, 1.0),
            (tone + offsets[2]).clamp(0.0, 1.0),
            (strength + offsets[3]).clamp(0.0, 1.0),
            (frequency_log + offsets[4] * std::f32::consts::LN_2)
                .clamp(250.0_f32.ln(), 2000.0_f32.ln()),
        ]);
        self.effective[0] = modulated_gain(self.volume.value(), offsets[0]);
        self.kernel
            .sample(input, self.colors.each_ref().map(|r| r.value()), hard, tube)
    }
}

impl daw_unit_core::ModulationDestination for Drive {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [daw_unit_core::ModulationParameter] = <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: daw_unit_core::ModulationScope = <daw_unit_core::DriveSettings as daw_unit_core::ModulationDestinationSettings>::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &daw_unit_core::ModulationInputs,
        _context: daw_unit_core::ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(
            input,
            modulation.offsets(daw_unit_core::ModulationParameter::DRIVE),
        )
    }
}

#[cfg(test)]
mod tests;
