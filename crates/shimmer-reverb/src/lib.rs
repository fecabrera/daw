//! Original portable Shimmer Reverb. The supplied Solaris behavioral report
//! informed its features; no extracted code, tables or host runtime is embedded.
mod delay;
mod pitch;
mod tank;
#[cfg(test)]
mod tests;
mod vowel;
use daw_unit_core::{ShimmerReverbSettings as Settings, *};
use delay::Diffuser;
use std::sync::{Arc, atomic::AtomicU32};

#[derive(Clone)]
pub struct ShimmerReverb {
    context: ProcessingContext,
    parameters: [ModulationRamp; 14],
    offsets: ModulationSmoother<12>,
    effective: [f32; 12],
    predelay: daw_reverb_core::Delay,
    early: [Diffuser; 4],
    vowel: vowel::Vowel,
    pitch: [pitch::Pitch; 2],
    tank: tank::Tank,
    warm: [f64; 2],
    sync: bool,
    note: u8,
    intervals: [i8; 2],
    monitor: LevelMonitor<3>,
    counter: u64,
    warm_alpha: f64,
    predelay_frames: ModulationRamp,
    predelay_initialized: bool,
    injection: ModulationRamp,
}
impl ShimmerReverb {
    fn targets(s: &Settings) -> [f32; 14] {
        let mut p = [0.0; 14];
        p[..12].copy_from_slice(&s.values());
        p[10] = if s.level_db <= SILENT_GAIN_DB {
            0.0
        } else {
            10.0f32.powf(s.level_db / 20.0)
        };
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
            predelay: daw_reverb_core::Delay::new((context.sample_rate_hz as usize) * 8),
            early: [0.0043, 0.0059, 0.0097, 0.0127]
                .map(|t| Diffuser::new(context.sample_rate_hz, t)),
            vowel: vowel::Vowel::new(context.sample_rate_hz),
            pitch: [
                pitch::Pitch::new(context.sample_rate_hz, s.pitch_1, s.amount_1),
                pitch::Pitch::new(context.sample_rate_hz, s.pitch_2, s.amount_2),
            ],
            tank: tank::Tank::new(context.sample_rate_hz),
            warm: [0.0; 2],
            sync: s.sync,
            note: s.note,
            intervals: [s.pitch_1, s.pitch_2],
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
            counter: 0,
            warm_alpha: 1.0
                - (-std::f64::consts::TAU * 5000.0 / f64::from(context.sample_rate_hz)).exp(),
            predelay_frames: ModulationRamp::new(
                if s.sync {
                    500.0 * Settings::NOTE_BEATS[usize::from(s.note.min(15))]
                } else {
                    s.predelay_ms
                } * context.sample_rate_hz as f32
                    * 0.001,
            ),
            injection: ModulationRamp::new(if s.pitch_1 == s.pitch_2 { 0.5 } else { 1.0 }),
            predelay_initialized: false,
        }
    }
    pub fn retarget(&mut self, s: &Settings) {
        let frames = (self.context.sample_rate_hz / 200).max(1);
        for (r, t) in self.parameters.iter_mut().zip(Self::targets(s)) {
            r.set(t, frames);
        }
        self.sync = s.sync;
        self.note = s.note.min(15);
        self.intervals = [s.pitch_1, s.pitch_2];
        for ((voice, p), amount) in self
            .pitch
            .iter_mut()
            .zip(self.intervals)
            .zip([s.amount_1, s.amount_2])
        {
            voice.retarget(p, amount, frames);
        }
        self.injection
            .set(if s.pitch_1 == s.pitch_2 { 0.5 } else { 1.0 }, frames);
        self.monitor.set_enabled(s.enabled);
    }
    pub fn heap_bytes(&self) -> u64 {
        self.predelay.heap_bytes() as u64
            + self.early.iter().map(Diffuser::heap_bytes).sum::<u64>()
            + self.pitch.iter().map(pitch::Pitch::heap_bytes).sum::<u64>()
            + self.tank.heap_bytes()
    }
    pub fn set_meter(&mut self, m: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(m);
    }
    pub fn set_output_peak_meter(&mut self, m: Arc<BandMeter<3>>) {
        self.monitor.attach_peak(m);
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
        let dry = input.map(|x| if x.is_finite() { f64::from(x) } else { 0.0 });
        // Smooth tap travel separately; zero pre-delay uses the immediate input.
        let target_frames = (v[1] * f64::from(self.context.sample_rate_hz) * 0.001) as f32;
        if self.predelay_initialized {
            self.predelay_frames
                .set(target_frames, (self.context.sample_rate_hz / 200).max(1));
        } else {
            // Initial/reset timing uses the first valid host tempo. Do not ramp
            // from a construction-time fallback or a stale pre-reset read head.
            self.predelay_frames = ModulationRamp::new(target_frames);
            self.predelay_initialized = true;
        }
        let delay_frames = f64::from(self.predelay_frames.next_value());
        let delayed = self.predelay.read(delay_frames);
        self.predelay.write(dry);
        let blend = delay_frames.clamp(0.0, 1.0);
        let early = std::array::from_fn(|ch| {
            self.early[ch].sample(dry[ch] * (1.0 - blend) + delayed[ch] * blend)
        });
        if self.counter.is_multiple_of(32) {
            self.vowel.retarget(v[2] as f32);
        }
        let voiced = self.vowel.sample(early, v[3]);
        let early = std::array::from_fn(|ch| self.early[ch + 2].sample(voiced[ch]));
        let a = self.pitch[0].sample(
            early,
            v[4] as f32,
            self.intervals[0] != 0,
            (self.context.sample_rate_hz / 200).max(1),
        );
        let b = self.pitch[1].sample(
            early,
            v[5] as f32,
            self.intervals[1] != 0,
            (self.context.sample_rate_hz / 200).max(1),
        );
        self.injection.set(
            if self.intervals[0] == self.intervals[1] {
                0.5
            } else {
                1.0
            },
            (self.context.sample_rate_hz / 200).max(1),
        );
        let injection = f64::from(self.injection.next_value()) * (1.0 - 0.25 * (v[4].max(v[5])));
        let tank_input = std::array::from_fn(|ch| early[ch] + injection * (a[ch] + b[ch]));
        let wet = self.tank.sample(tank_input, v[0], v[6], v[7], v[8], v[9]);
        let output = std::array::from_fn(|ch| {
            self.warm[ch] += self.warm_alpha * (wet[ch] - self.warm[ch]);
            let wet = wet[ch] * (1.0 - f64::from(p[13])) + self.warm[ch] * f64::from(p[13]);
            let mix = v[11];
            let effected =
                ((1.0 - mix * mix) * dry[ch] + 0.9 * (1.0 - (mix - 1.0).powi(2)) * wet) * v[10];
            (dry[ch] * (1.0 - f64::from(p[12])) + effected * f64::from(p[12]))
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        });
        self.counter = self.counter.wrapping_add(1);
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
        self.predelay.reset();
        self.predelay_frames.reset();
        self.predelay_initialized = false;
        self.injection.reset();
        for d in &mut self.early {
            d.reset();
        }
        self.vowel.reset();
        for (i, p) in self.pitch.iter_mut().enumerate() {
            p.reset(self.parameters[4 + i].target(), self.intervals[i] != 0);
        }
        self.tank.reset();
        self.warm = [0.0; 2];
        self.counter = 0;
        for r in &mut self.parameters {
            r.reset();
        }
        self.offsets.reset();
        self.monitor.reset();
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated_at(input, self.counter, 120.0, [0.0; 12])
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
