//! Portable stereo waveshaping with shared modulation and fixed-latency 4× anti-aliasing.
mod oversampling;
use daw_unit_core::{
    EffectUnit, LevelMonitor, ModulationContext, ModulationDestination,
    ModulationDestinationSettings, ModulationInputs, ModulationParameter, ModulationRamp,
    ModulationScope, ModulationSmoother, PortKind, ProcessingContext, SATURATOR_TYPE_ID,
    SILENT_GAIN_DB, SaturatorClipMode, SaturatorSettings, SaturatorShape, UnitDescriptor,
};
use std::sync::{Arc, atomic::AtomicU32};
fn gain(db: f32) -> f32 {
    if db <= SILENT_GAIN_DB {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}
fn knee(input: f64, start: f64, ceiling: f64) -> f64 {
    let x = input.abs();
    if x <= start {
        input
    } else {
        (start + (ceiling - start) * ((x - start) / (ceiling - start)).tanh()).copysign(input)
    }
}
fn shape(shape: SaturatorShape, x: f64) -> f64 {
    match shape {
        SaturatorShape::AnalogClip => knee(x, 0.75, 1.5),
        SaturatorShape::SoftSine => {
            // Wide sine arc: gentle compression, separated from the firmer cubic mode.
            3.0 * (x / 3.0)
                .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2)
                .sin()
        }
        // Broad rational knee with unity small-signal gain and no frequency-dependent gain boost.
        SaturatorShape::BassShaper => x / (1.0 + (x / 1.8).powi(2)).sqrt(),
        SaturatorShape::MediumCurve => {
            // Zero slope at the plateau joins the cubic continuously to its ceiling.
            const THRESHOLD: f64 = 1.65;
            let y = x.clamp(-THRESHOLD, THRESHOLD);
            y - y.powi(3) / (3.0 * THRESHOLD * THRESHOLD)
        }
        // A real hard knee adds upper odd harmonics. Its 1.2 ceiling remains
        // distinct from the optional final Hard Clip stage at unity.
        SaturatorShape::HardCurve => x.clamp(-1.2, 1.2),
    }
}
fn clip(mode: SaturatorClipMode, x: f64) -> f64 {
    match mode {
        SaturatorClipMode::NoClip => x,
        SaturatorClipMode::SoftClip => knee(x, 0.5, 1.0),
        SaturatorClipMode::HardClip => x.clamp(-1.0, 1.0),
    }
}
// Rapid retargets can leave independent ramps with different remaining lengths.
// Normalize the crossfade, retaining unity gain for identical small-signal curves.
fn weights<const N: usize>(ramps: &mut [ModulationRamp; N]) -> [f64; N] {
    let mut values = ramps.each_mut().map(|r| f64::from(r.next_value()));
    let sum = values.iter().sum::<f64>();
    if sum > 0.0 {
        for value in &mut values {
            *value /= sum;
        }
    }
    values
}
#[derive(Clone)]
pub struct Saturator {
    // Drive dB, output amplitude, wet proportion, enabled proportion.
    parameters: [ModulationRamp; 4],
    shapes: [ModulationRamp; 5],
    clips: [ModulationRamp; 3],
    external: ModulationSmoother<3>,
    effective: [f32; 3],
    fade_frames: u32,
    monitor: LevelMonitor<3>,
    oversampling: oversampling::Oversampling,
    dry: [[f32; 2]; oversampling::LATENCY],
    dry_index: usize,
}
impl Saturator {
    pub const LATENCY_FRAMES: u32 = oversampling::LATENCY as u32;
    fn values(s: &SaturatorSettings) -> [f32; 4] {
        [
            s.drive_db,
            gain(s.level_db),
            s.dry_wet,
            f32::from(s.enabled),
        ]
    }
    pub fn new(context: ProcessingContext, s: &SaturatorSettings) -> Self {
        Self {
            parameters: Self::values(s).map(ModulationRamp::new),
            shapes: SaturatorShape::ALL.map(|v| ModulationRamp::new(f32::from(v == s.shape))),
            clips: SaturatorClipMode::ALL.map(|v| ModulationRamp::new(f32::from(v == s.clip_mode))),
            external: ModulationSmoother::new(context),
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
            effective: [s.drive_db, s.level_db, s.dry_wet],
            fade_frames: (context.sample_rate_hz / 200).max(1),
            oversampling: oversampling::Oversampling::new(),
            dry: [[0.0; 2]; oversampling::LATENCY],
            dry_index: 0,
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
    pub fn retarget(&mut self, s: &SaturatorSettings) {
        self.monitor.set_enabled(s.enabled);
        for (r, v) in self.parameters.iter_mut().zip(Self::values(s)) {
            r.set(v, self.fade_frames);
        }
        for (r, v) in self.shapes.iter_mut().zip(SaturatorShape::ALL) {
            r.set(f32::from(v == s.shape), self.fade_frames);
        }
        for (r, v) in self.clips.iter_mut().zip(SaturatorClipMode::ALL) {
            r.set(f32::from(v == s.clip_mode), self.fade_frames);
        }
    }

    /// Processed Drive dB, Level dB, Dry/Wet, excluding the separate bypass ramp.
    pub fn effective_parameters(&self) -> [f32; 3] {
        self.effective
    }
    fn sample(&mut self, input: [f32; 2], offsets: [f32; 3]) -> [f32; 2] {
        let p = self.parameters.each_mut().map(ModulationRamp::next_value);
        let o = self.external.process(offsets);
        let drive_db = (f64::from(p[0]) + o[0]).clamp(-36.0, 36.0);
        let level = (f64::from(p[1]) * 10_f64.powf(o[1].clamp(-96.0, 96.0) / 20.0))
            .min(f64::from(gain(12.0)));
        let mix = (f64::from(p[2]) + o[2]).clamp(0.0, 1.0);
        self.effective = [
            drive_db as f32,
            if level > 0.0 {
                (20.0 * level.log10()) as f32
            } else {
                SILENT_GAIN_DB
            },
            mix as f32,
        ];
        let shapes = weights(&mut self.shapes);
        let clips = weights(&mut self.clips);
        let drive = 10_f64.powf(drive_db / 20.0);
        let input = input.map(|v| if v.is_finite() { v } else { 0.0 });
        let dry = self.dry[self.dry_index];
        self.dry[self.dry_index] = input;
        self.dry_index = (self.dry_index + 1) % self.dry.len();
        let wet = self.oversampling.process(input, |up| {
            up.map(|v| {
                let shaped = shapes
                    .iter()
                    .zip(SaturatorShape::ALL)
                    .filter(|(w, _)| **w != 0.0)
                    .map(|(w, s)| *w * shape(s, v * drive))
                    .sum::<f64>();
                clips
                    .iter()
                    .zip(SaturatorClipMode::ALL)
                    .filter(|(w, _)| **w != 0.0)
                    .map(|(w, c)| *w * clip(c, shaped))
                    .sum::<f64>()
            })
        });
        let output = std::array::from_fn(|ch| {
            // Reconstruction can overshoot a clipped high-rate sample. Guard sample ceilings again
            // only after the No Clip contribution has completely faded out.
            let wet = if clips[0] == 0.0 {
                wet[ch].clamp(-1.0, 1.0)
            } else {
                wet[ch]
            };
            let processed = f64::from(dry[ch]) * (1.0 - mix) + wet * level * mix;
            let enabled = f64::from(p[3]);
            (f64::from(dry[ch]) * (1.0 - enabled) + processed * enabled)
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        });
        self.monitor.sample_output(output);
        output
    }
}
impl EffectUnit for Saturator {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: SATURATOR_TYPE_ID,
            name: "Saturator",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn latency_frames(&self) -> u32 {
        Self::LATENCY_FRAMES
    }
    fn reset(&mut self) {
        self.monitor.reset();
        for r in &mut self.parameters {
            r.reset();
        }
        for r in &mut self.shapes {
            r.reset();
        }
        for r in &mut self.clips {
            r.reset();
        }
        self.external.reset();
        self.oversampling.reset();
        self.dry.fill([0.0; 2]);
        self.dry_index = 0;
        let p = self.parameters.each_ref().map(ModulationRamp::target);
        self.effective = [
            p[0],
            if p[1] > 0.0 {
                20.0 * p[1].log10()
            } else {
                SILENT_GAIN_DB
            },
            p[2],
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.sample(input, [0.0; 3])
    }
}
impl ModulationDestination for Saturator {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        SaturatorSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = SaturatorSettings::MODULATION_SCOPE;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        m: &ModulationInputs,
        _: ModulationContext,
    ) -> Self::Audio {
        self.sample(input, m.offsets(ModulationParameter::SATURATOR))
    }
}
#[cfg(test)]
mod tests;
