//! Original portable Shimmer Reverb. The supplied Solaris behavioral report
//! informed its features; no extracted code, tables or host runtime is embedded.
use daw_dsp_support::{gain::amplitude, reverb::shimmer::ShimmerKernel};
use daw_unit_core::{ShimmerReverbSettings as Settings, *};
use std::sync::{Arc, atomic::AtomicU32};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transition_tests;

#[derive(Clone)]
pub struct ShimmerReverb {
    context: ProcessingContext,
    parameters: [ModulationRamp; 14],
    offsets: ModulationSmoother<12>,
    effective: [f32; 12],
    kernel: ShimmerKernel,
    sync: bool,
    note: u8,
    monitor: LevelMonitor<3>,
}
impl ShimmerReverb {
    fn targets(s: &Settings) -> [f32; 14] {
        let mut p = [0.0; 14];
        p[..12].copy_from_slice(&s.values());
        p[10] = amplitude(s.level_db);
        p[12] = f32::from(s.enabled);
        p[13] = f32::from(s.warm);
        p
    }
    pub fn new(context: ProcessingContext, s: &Settings) -> Self {
        Self {
            context,
            parameters: Self::targets(s).map(ModulationRamp::new),
            offsets: ModulationSmoother::new(context),
            effective: s.values(),
            kernel: ShimmerKernel::new(
                context.sample_rate_hz,
                [s.pitch_1, s.pitch_2],
                [s.amount_1, s.amount_2],
            ),
            sync: s.sync,
            note: s.note,
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
        }
    }
    pub fn retarget(&mut self, s: &Settings) {
        let frames = (self.context.sample_rate_hz / 200).max(1);
        for (r, t) in self.parameters.iter_mut().zip(Self::targets(s)) {
            r.set(t, frames);
        }
        self.sync = s.sync;
        self.note = s.note.min(15);
        self.kernel
            .retarget_pitch([s.pitch_1, s.pitch_2], [s.amount_1, s.amount_2], frames);
        self.monitor.set_enabled(s.enabled);
    }
    pub fn heap_bytes(&self) -> u64 {
        self.kernel.heap_bytes()
    }
    pub fn set_meter(&mut self, m: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(m);
    }
    pub fn set_output_peak_meter(&mut self, m: Arc<BandMeter<3>>) {
        self.monitor.attach_peak_prepared(m);
    }
    pub fn effective_parameters(&self) -> [f32; 12] {
        self.effective
    }
    pub fn process_modulated_at(
        &mut self,
        input: [f32; 2],
        _: u64,
        tempo: f32,
        offsets: [f32; 12],
    ) -> [f32; 2] {
        let p = self.parameters.each_mut().map(ModulationRamp::next_value);
        let o = self.offsets.process(offsets);
        let mut v: [f64; 12] = std::array::from_fn(|i| {
            if i == 10 {
                if p[i] == 0.0 {
                    0.0
                } else {
                    (f64::from(p[i]) * 10.0f64.powf(o[i].clamp(-24.0, 24.0) / 20.0))
                        .min(10.0f64.powf(12.0 / 20.0))
                }
            } else {
                (f64::from(p[i]) + o[i]).clamp(0.0, if i == 1 { 1000.0 } else { 1.0 })
            }
        });
        let tempo = if tempo.is_finite() && tempo > 0.0 {
            f64::from(tempo)
        } else {
            120.0
        };
        if self.sync {
            v[1] = (60000.0 / tempo * f64::from(Settings::NOTE_BEATS[usize::from(self.note)]))
                .min(f64::from(Settings::MAX_PREDELAY_MS));
        }
        self.effective = std::array::from_fn(|i| {
            if i == 10 {
                if v[i] > 0.0 {
                    (20.0 * v[i].log10()) as f32
                } else {
                    SILENT_GAIN_DB
                }
            } else {
                v[i] as f32
            }
        });
        let output = self.kernel.sample(input, v, p[13], p[12]);
        self.monitor.sample_output(output);
        output
    }
}
impl EffectUnit for ShimmerReverb {
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: SHIMMER_REVERB_TYPE_ID,
            name: "Shimmer Reverb",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn heap_bytes(&self) -> u64 {
        self.heap_bytes()
    }
    fn tail_frames(&self) -> u64 {
        u64::from(self.context.sample_rate_hz) * u64::from(Settings::MAX_TAIL_SECONDS)
    }
    fn reset(&mut self) {
        self.kernel
            .reset([self.parameters[4].target(), self.parameters[5].target()]);
        for r in &mut self.parameters {
            r.reset();
        }
        self.offsets.reset();
        self.monitor.reset();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated_at(input, 0, 120.0, [0.0; 12])
    }
}
impl ModulationDestination for ShimmerReverb {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        &ModulationParameter::SHIMMER_REVERB;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        m: &ModulationInputs,
        c: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated_at(
            input,
            c.frame,
            c.tempo_bpm,
            m.offsets(ModulationParameter::SHIMMER_REVERB),
        )
    }
}
