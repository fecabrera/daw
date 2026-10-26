//! Bounded stereo chorus. All delay storage is prepared before audio processing.
use daw_unit_core::{
    CHORUS_TYPE_ID, ChorusSettings, EffectUnit, LevelMonitor, ModulationContext,
    ModulationDestination, ModulationDestinationSettings, ModulationInputs, ModulationParameter,
    ModulationRamp, ModulationScope, ModulationSmoother, PortKind, ProcessingContext,
    SILENT_GAIN_DB, UnitDescriptor,
};
use std::sync::{Arc, atomic::AtomicU32};

// Keep the original excursion at the default rate. Above it, inverse-rate
// scaling keeps sine-delay velocity (and thus steady detuning) independent of Hz.
const DEPTH_REFERENCE_RATE_HZ: f64 = 0.8;

#[derive(Clone)]
pub struct Chorus {
    delay: Vec<[f64; 2]>,
    cursor: usize,
    phase: f64,
    depth_compensation: Option<ModulationRamp>,
    clock: daw_lfo::LfoClock,
    rate_mode: daw_unit_core::LfoRateMode,
    note_rate: daw_unit_core::NoteRate,
    rate_phase: f64,
    last_frame: Option<u64>,
    next_frame: u64,
    rate: f64,
    fade: u32,
    parameters: [ModulationRamp; 9],
    weights: [ModulationRamp; 4],
    phases: [ModulationRamp; 4],
    external: ModulationSmoother<8>,
    effective: [f32; 8],
    monitor: LevelMonitor<3>,
}
fn gain(db: f32) -> f32 {
    if db <= SILENT_GAIN_DB {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}
impl Chorus {
    fn values(s: &ChorusSettings) -> [f32; 9] {
        let mut p = [0.0; 9];
        p[..8].copy_from_slice(&s.values());
        p[6] = gain(s.level_db);
        p[8] = f32::from(s.enabled);
        p
    }
    pub fn new(context: ProcessingContext, s: &ChorusSettings) -> Self {
        Self {
            delay: vec![[0.0; 2]; (f64::from(context.sample_rate_hz) * 0.046).ceil() as usize + 4],
            cursor: 0,
            phase: 0.0,
            depth_compensation: None,
            clock: daw_lfo::LfoClock::new(context),
            rate_mode: s.rate_mode,
            note_rate: s.note_rate,
            rate_phase: 0.0,
            last_frame: None,
            next_frame: 0,
            rate: f64::from(context.sample_rate_hz),
            fade: (context.sample_rate_hz / 200).max(1),
            parameters: Self::values(s).map(ModulationRamp::new),
            weights: std::array::from_fn(|i| {
                ModulationRamp::new(f32::from(i < usize::from(s.voices)))
            }),
            phases: std::array::from_fn(|i| ModulationRamp::new(i as f32 / f32::from(s.voices))),
            external: ModulationSmoother::new(context),
            effective: s.values(),
            monitor: LevelMonitor::new(context.sample_rate_hz, s.enabled),
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        (self.delay.capacity() * std::mem::size_of::<[f64; 2]>()) as u64
    }
    pub fn set_meter(&mut self, meter: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(meter);
    }
    /// Attach current output and a click-resettable sample peak on the control thread.
    pub fn set_output_peak_meter(&mut self, meter: Arc<daw_unit_core::BandMeter<3>>) {
        self.monitor.attach_peak(meter);
    }

    pub fn retarget(&mut self, s: &ChorusSettings) {
        self.rate_mode = s.rate_mode;
        self.note_rate = s.note_rate;
        for (r, t) in self.parameters.iter_mut().zip(Self::values(s)) {
            r.set(t, self.fade);
        }
        for i in 0..4 {
            self.weights[i].set(f32::from(i < usize::from(s.voices)), self.fade);
            self.phases[i].set(i as f32 / f32::from(s.voices), self.fade);
        }
        self.monitor.set_enabled(s.enabled);
    }
    pub fn effective_parameters(&self) -> [f32; 8] {
        self.effective
    }
    fn read(&self, frames: f64, channel: usize) -> f64 {
        let position = (self.cursor as f64 - frames).rem_euclid(self.delay.len() as f64);
        let index = position.floor() as usize;
        let a = self.delay[index][channel];
        let b = self.delay[(index + 1) % self.delay.len()][channel];
        // Linear interpolation is convex: the bounded feedback loop cannot gain energy here.
        a + (b - a) * position.fract()
    }
    pub fn process_modulated(&mut self, input: [f32; 2], offsets: [f32; 8]) -> [f32; 2] {
        let frame = self.next_frame;
        self.process_modulated_at(input, frame, 120.0, offsets)
    }
    /// Shared transport timing; Note follows tempo and Hz retains its own frequency.
    pub fn process_modulated_at(
        &mut self,
        input: [f32; 2],
        frame: u64,
        tempo_bpm: f32,
        offsets: [f32; 8],
    ) -> [f32; 2] {
        let p = self.parameters.each_mut().map(ModulationRamp::next_value);
        let o = self.external.process(offsets);
        let time = (f64::from(p[0]) + o[0]).clamp(5.0, 30.0);
        let depth = (f64::from(p[1]) + o[1]).clamp(0.0, 1.0);
        let feedback = (f64::from(p[2]) + o[2]).clamp(-0.8, 0.8);
        let base_rate = daw_lfo::rate_hz(self.rate_mode, p[3], self.note_rate.beats(), tempo_bpm);
        let bounds = match self.rate_mode {
            daw_unit_core::LfoRateMode::Hz => [0.05, 10.0],
            daw_unit_core::LfoRateMode::Note => {
                [daw_unit_core::NoteRate(1), daw_unit_core::NoteRate(26)]
                    .map(|r| daw_lfo::rate_hz(self.rate_mode, p[3], r.beats(), tempo_bpm))
            }
        };
        let rate = (base_rate * 2_f64.powf(o[3].clamp(-6.0, 6.0))).clamp(bounds[0], bounds[1]);
        let compensation_target = (DEPTH_REFERENCE_RATE_HZ / rate).min(1.0) as f32;
        // Initialize at the first host tempo rather than an assumed construction
        // tempo. Later rate/tempo/CV changes retain the common 5 ms transition.
        let compensation = self
            .depth_compensation
            .get_or_insert_with(|| ModulationRamp::new(compensation_target));
        compensation.set(compensation_target, self.fade);
        let excursion = 0.45 * depth * f64::from(compensation.next_value());
        // Integrate only the modulation offset. The shared clock corrects base
        // rate/tempo changes over 5 ms without multiplying elapsed time by CV.
        if self.last_frame != frame.checked_sub(1) {
            self.rate_phase = 0.0;
        }
        self.phase = (self.clock.phase_at(frame, 0, base_rate) + self.rate_phase).rem_euclid(1.0);
        self.rate_phase = (self.rate_phase + (rate - base_rate) / self.rate).rem_euclid(1.0);
        self.last_frame = Some(frame);
        self.next_frame = frame.saturating_add(1);
        let phase = (f64::from(p[4]) + o[4]).clamp(-180.0, 180.0) / 360.0;
        let width = (f64::from(p[5]) + o[5]).clamp(0.0, 2.0);
        let level = (f64::from(p[6]) * 10_f64.powf(o[6].clamp(-96.0, 96.0) / 20.0))
            .min(f64::from(gain(12.0)));
        let mix = (f64::from(p[7]) + o[7]).clamp(0.0, 1.0);
        self.effective = [
            time as f32,
            depth as f32,
            feedback as f32,
            rate as f32,
            (phase * 360.0) as f32,
            width as f32,
            if level > 0.0 {
                (20.0 * level.log10()) as f32
            } else {
                SILENT_GAIN_DB
            },
            mix as f32,
        ];
        let dry = input.map(|v| if v.is_finite() { f64::from(v) } else { 0.0 });
        let weights = self.weights.each_mut().map(ModulationRamp::next_value);
        let phases = self.phases.each_mut().map(ModulationRamp::next_value);
        let total = f64::from(weights.iter().sum::<f32>()).max(1.0);
        let mut wet = [0.0; 2];
        for (i, weight) in weights.into_iter().enumerate() {
            if weight == 0.0 {
                continue;
            }
            for (ch, value) in wet.iter_mut().enumerate() {
                let lfo = daw_lfo::waveform(
                    0,
                    self.phase + f64::from(phases[i]) + if ch == 1 { phase } else { 0.0 },
                );
                // 45% excursion leaves a strictly positive delay at every setting and rate.
                let delay = time * (1.0 + excursion * lfo) * self.rate / 1000.0;
                *value += self.read(delay, ch) * f64::from(weight) / total;
            }
        }
        self.delay[self.cursor] = std::array::from_fn(|ch| {
            (dry[ch] + wet[ch] * feedback).clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
        });
        self.cursor = (self.cursor + 1) % self.delay.len();
        self.phase = (self.phase + rate / self.rate).rem_euclid(1.0);
        let middle = (wet[0] + wet[1]) * 0.5;
        let side = (wet[0] - wet[1]) * 0.5 * width;
        wet = [middle + side, middle - side];
        let output = std::array::from_fn(|ch| {
            let effected = (dry[ch] * (1.0 - mix) + wet[ch] * mix) * level;
            (dry[ch] * (1.0 - f64::from(p[8])) + effected * f64::from(p[8]))
                .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
        });
        self.monitor.sample_output(output);
        output
    }
}
impl EffectUnit for Chorus {
    fn heap_bytes(&self) -> u64 {
        Chorus::heap_bytes(self)
    }
    fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: CHORUS_TYPE_ID,
            name: "Chorus",
            is_source: false,
            inputs: &[PortKind::StereoAudio, PortKind::Modulation],
            outputs: &[PortKind::StereoAudio],
        }
    }
    fn reset(&mut self) {
        self.delay.fill([0.0; 2]);
        self.cursor = 0;
        self.phase = 0.0;
        self.depth_compensation = None;
        self.clock.reset();
        self.rate_phase = 0.0;
        self.last_frame = None;
        self.next_frame = 0;
        for r in &mut self.parameters {
            r.reset();
        }
        for r in &mut self.weights {
            r.reset();
        }
        for r in &mut self.phases {
            r.reset();
        }
        self.external.reset();
        self.monitor.reset();
        let p = self.parameters.each_ref().map(ModulationRamp::target);
        self.effective = [
            p[0],
            p[1],
            p[2],
            p[3],
            p[4],
            p[5],
            if p[6] > 0.0 {
                20.0 * p[6].log10()
            } else {
                SILENT_GAIN_DB
            },
            p[7],
        ];
    }
    fn process_sample(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.process_modulated(input, [0.0; 8])
    }
}
impl ModulationDestination for Chorus {
    type Audio = [f32; 2];
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        ChorusSettings::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn process_with_modulation(
        &mut self,
        input: Self::Audio,
        inputs: &ModulationInputs,
        context: ModulationContext,
    ) -> Self::Audio {
        self.process_modulated_at(
            input,
            context.frame,
            context.tempo_bpm,
            inputs.offsets(ModulationParameter::CHORUS),
        )
    }
}

#[cfg(test)]
mod tests;
