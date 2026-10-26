//! Stereo amplitude quantization and whole-ratio sample holding.
use daw_unit_core::{
    BITCRUSHER_TYPE_ID, BitcrusherSettings, EffectUnit, LevelMonitor, ModulationContext,
    ModulationDestination, ModulationDestinationSettings, ModulationInputs, ModulationParameter,
    ModulationRamp, ModulationScope, ModulationSmoother, PortKind, ProcessingContext,
    SILENT_GAIN_DB, UnitDescriptor,
};
use std::sync::{Arc, atomic::AtomicU32};

fn amplitude(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
fn quantize(input: f64, bits: u8) -> f64 {
    let input = input.clamp(-1.0, 1.0);
    // Symmetric endpoint-inclusive levels. Preserve digital silence explicitly:
    // an even number of equally spaced levels has no zero, including at one bit.
    if input == 0.0 {
        return 0.0;
    }
    let intervals = f64::from((1_u32 << bits) - 1);
    let magnitude = ((input.abs() + 1.0) * 0.5 * intervals).round() * 2.0 / intervals - 1.0;
    magnitude.copysign(input)
}

#[derive(Clone)]
pub struct Bitcrusher {
    // Drive dB, Resolution bits, Downsampling ratio, Level amplitude, Dry/Wet, power.
    parameters: [ModulationRamp; 6],
    external: ModulationSmoother<5>,
    effective: [f32; 5],
    held: [f64; 2],
    elapsed: u8,
    fade_frames: u32,
    monitor: LevelMonitor<3>,
}
impl Bitcrusher {
    fn values(s: &BitcrusherSettings) -> [f32; 6] {
        [
            s.drive_db,
            f32::from(s.resolution_bits),
            f32::from(s.downsampling),
            amplitude(s.level_db),
            s.dry_wet,
            f32::from(s.enabled),
        ]
    }
    /// Prepare with validated settings on the control thread.
    pub fn new(context: ProcessingContext, s: &BitcrusherSettings) -> Self {
        Self {
            parameters: Self::values(s).map(ModulationRamp::new),
            external: ModulationSmoother::new(context),
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
            effective: [
                s.drive_db,
                f32::from(s.resolution_bits),
                f32::from(s.downsampling),
                s.level_db,
                s.dry_wet,
            ],
            held: [0.0; 2],
            elapsed: u8::MAX,
            fade_frames: (context.sample_rate_hz / 200).max(1),
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
    pub fn retarget(&mut self, s: &BitcrusherSettings) {
        self.monitor.set_enabled(s.enabled);
        for (ramp, target) in self.parameters.iter_mut().zip(Self::values(s)) {
            ramp.set(target, self.fade_frames);
        }
    }

    /// Last processed Drive dB, whole Resolution/Downsampling, Level dB, and Dry/Wet.
    pub fn effective_parameters(&self) -> [f32; 5] {
        self.effective
    }
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 5]) -> [f32; 2] {
        let p = self.parameters.each_mut().map(ModulationRamp::next_value);
        let offsets = self.external.process(offsets);
        let drive = (f64::from(p[0]) + offsets[0]).clamp(0.0, 24.0);
        let bits = (f64::from(p[1]) + offsets[1]).round().clamp(1.0, 24.0) as u8;
        let ratio = (f64::from(p[2]) + offsets[2]).round().clamp(1.0, 40.0) as u8;
        // Gain multiplication retains exact base silence. Bound summed offsets before conversion.
        let gain = (f64::from(p[3]) * 10_f64.powf(offsets[3].clamp(-96.0, 96.0) / 20.0))
            .min(f64::from(amplitude(12.0)));
        let mix = (f64::from(p[4]) + offsets[4]).clamp(0.0, 1.0);
        self.effective = [
            drive as f32,
            f32::from(bits),
            f32::from(ratio),
            if gain > 0.0 {
                (20.0 * gain.log10()) as f32
            } else {
                SILENT_GAIN_DB
            },
            mix as f32,
        ];
        let dry = input.map(|v| if v.is_finite() { v } else { 0.0 });
        // One capture clock preserves stereo timing. A changed ratio retains elapsed time;
        // shortening the period captures now if that time already meets the new period.
        if self.elapsed >= ratio {
            let drive = 10_f64.powf(drive / 20.0);
            self.held = dry.map(|v| quantize(f64::from(v) * drive, bits));
            self.elapsed = 0;
        }
        self.elapsed += 1;
        // Keep clock and modulation history running through bypass; no stale frozen sample.
        if p[5] == 0.0 {
            return dry;
        }
        let output = std::array::from_fn(|ch| {
            let dry = f64::from(dry[ch]);
            // Weighted sums preserve clipped wet samples even when dry headroom is huge.
            let wet = (dry * (1.0 - mix) + self.held[ch] * mix) * gain;
            let enabled = f64::from(p[5]);
            (dry * (1.0 - enabled) + wet * enabled).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
                as f32
        });
        self.monitor.sample_output(output);
        output
    }
}
impl EffectUnit for Bitcrusher {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: BITCRUSHER_TYPE_ID,
            name: "Bitcrusher",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.monitor.reset();
        for ramp in &mut self.parameters {
            ramp.reset();
        }
        self.external.reset();
        self.held = [0.0; 2];
        self.elapsed = u8::MAX;
        let p = self.parameters.each_ref().map(ModulationRamp::target);
        self.effective = [
            p[0],
            p[1],
            p[2],
            if p[3] > 0.0 {
                20.0 * p[3].log10()
            } else {
                SILENT_GAIN_DB
            },
            p[4],
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 5])
    }
}
impl ModulationDestination for Bitcrusher {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        BitcrusherSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = BitcrusherSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        modulation: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated(input, modulation.offsets(ModulationParameter::BITCRUSHER))
    }
}

#[cfg(test)]
mod tests;
