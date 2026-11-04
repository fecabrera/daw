//! Prepared, bounded stereo reverb runtime. All FFT and delay storage is owned here.
use crate::{BloomDelays, Delay, Noise, junction, mix_gains, wet_width};
use daw_filter::svf::{HighShelf, StateVariableFilter};
use daw_unit_core::{BandMeter, LevelMonitor, ModulationRamp, ProcessingContext};
use std::sync::{Arc, atomic::AtomicU32};
const HOP: usize = 1024;
const FFT: usize = HOP * 2;
const PARTS: usize = 32;
#[derive(Clone, Copy)]
pub struct Parameters {
    /// Decay seconds, predelay ms, HP Hz, LP Hz, Drive dB, Bloom, Tone,
    /// Motion, Width, Level dB, Mix, power.
    pub values: [f32; 12],
    pub hq: bool,
    pub gated: bool,
    pub dark: bool,
    pub drive_enabled: bool,
    pub drive_mode: daw_unit_core::PlateDriveMode,
    pub highpass_enabled: bool,
    pub lowpass_enabled: bool,
    pub highpass_slope: daw_unit_core::ReverbFilterSlope,
    pub lowpass_slope: daw_unit_core::ReverbFilterSlope,
}
#[derive(Clone, Copy, Default)]
struct Complex {
    re: f64,
    im: f64,
}
impl Complex {
    fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
    fn add(self, b: Self) -> Self {
        Self {
            re: self.re + b.re,
            im: self.im + b.im,
        }
    }
    fn sub(self, b: Self) -> Self {
        Self {
            re: self.re - b.re,
            im: self.im - b.im,
        }
    }
}
/// Radix-two transforms use immutable twiddles prepared on the control thread.
#[derive(Clone)]
struct Convolution {
    twiddle: Vec<Complex>,
    history: Vec<Complex>,
    kernels: Vec<Complex>,
    initial_kernels: Vec<Complex>,
    initial_noise: Noise,
    kernel_valid: [bool; PARTS],
    valid_parts: usize,
    scratch: Vec<Complex>,
    input: Vec<[f64; 2]>,
    output: Vec<[f64; 2]>,
    overlap: Vec<[f64; 2]>,
    cursor: usize,
    head: usize,
    kernel_cursor: usize,
    noise: Noise,
}
impl Convolution {
    fn new() -> Self {
        let mut s = Self {
            twiddle: (0..FFT / 2)
                .map(|i| {
                    let a = -std::f64::consts::TAU * i as f64 / FFT as f64;
                    Complex {
                        re: a.cos(),
                        im: a.sin(),
                    }
                })
                .collect(),
            history: vec![Complex::default(); 2 * PARTS * FFT],
            kernels: vec![Complex::default(); 4 * PARTS * FFT],
            initial_kernels: Vec::new(),
            initial_noise: Noise::new([0; 4]),
            kernel_valid: [false; PARTS],
            valid_parts: 0,
            scratch: vec![Complex::default(); FFT],
            input: vec![[0.; 2]; HOP],
            output: vec![[0.; 2]; HOP],
            overlap: vec![[0.; 2]; HOP],
            cursor: 0,
            head: 0,
            kernel_cursor: 0,
            noise: Noise::new([0x54748b2, 0xff1cd65a, 0xffd4708b, 0x577edd]),
        };
        for part in 0..PARTS {
            s.update_kernel(part);
        }
        s.initial_kernels = s.kernels.clone();
        s.initial_noise = s.noise.clone();
        s
    }
    fn transform(data: &mut [Complex], twiddle: &[Complex], inverse: bool) {
        let mut j = 0;
        for i in 1..FFT {
            let mut bit = FFT >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j ^= bit;
            if i < j {
                data.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= FFT {
            for start in (0..FFT).step_by(len) {
                for i in 0..len / 2 {
                    let mut w = twiddle[i * FFT / len];
                    if inverse {
                        w.im = -w.im;
                    }
                    let a = data[start + i];
                    let b = data[start + i + len / 2].mul(w);
                    data[start + i] = a.add(b);
                    data[start + i + len / 2] = a.sub(b);
                }
            }
            len *= 2;
        }
        if inverse {
            for x in data {
                x.re /= FFT as f64;
                x.im /= FFT as f64;
            }
        }
    }
    fn update_kernel(&mut self, part: usize) {
        // Four continuously regenerated noise streams. Window both boundaries so
        // replacement of a partition cannot introduce a discontinuous kernel edge.
        for lane in 0..4 {
            self.scratch.fill(Complex::default());
            for i in 0..HOP {
                let noise = self.noise.sample()[lane];
                let q = i as f64 / HOP as f64;
                let window = (2. * q).min(2. - 2. * q).sqrt();
                self.scratch[i].re = f64::from(noise) * window / ((HOP * PARTS) as f64).sqrt();
            }
            Self::transform(&mut self.scratch, &self.twiddle, false);
            let offset = (lane * PARTS + part) * FFT;
            self.kernels[offset..offset + FFT].copy_from_slice(&self.scratch);
        }
        self.kernel_valid[part] = true;
    }
    fn sample(
        &mut self,
        input: [f64; 2],
        quality: f64,
        gate: f64,
        decay: f64,
        rate: f64,
    ) -> [f64; 2] {
        let result = self.output[self.cursor];
        self.input[self.cursor] = input;
        self.cursor += 1;
        if self.cursor == HOP {
            self.cursor = 0;
            for ch in 0..2 {
                self.scratch.fill(Complex::default());
                for i in 0..HOP {
                    self.scratch[i].re = self.input[i][ch];
                }
                Self::transform(&mut self.scratch, &self.twiddle, false);
                let offset = (ch * PARTS + self.head) * FFT;
                self.history[offset..offset + FFT].copy_from_slice(&self.scratch);
            }
            self.valid_parts = (self.valid_parts + 1).min(PARTS);
            for out in 0..2 {
                self.scratch.fill(Complex::default());
                for input_ch in 0..2 {
                    for part in 0..self.valid_parts {
                        let mode = if part < 8 {
                            1.0
                        } else if part < 16 {
                            1.0 - gate
                        } else {
                            quality * (1.0 - gate)
                        };
                        if mode == 0.0 {
                            continue;
                        }
                        let weight =
                            mode * 10_f64.powf(-3. * part as f64 * HOP as f64 / (rate * decay));
                        let h = (input_ch * PARTS + (self.head + PARTS - part) % PARTS) * FFT;
                        let k = ((out * 2 + input_ch) * PARTS + part) * FFT;
                        let kernels = if self.kernel_valid[part] {
                            &self.kernels
                        } else {
                            &self.initial_kernels
                        };
                        for ((sum, sample), kernel) in self
                            .scratch
                            .iter_mut()
                            .zip(&self.history[h..h + FFT])
                            .zip(&kernels[k..k + FFT])
                        {
                            let mut term = sample.mul(*kernel);
                            term.re *= weight;
                            term.im *= weight;
                            *sum = sum.add(term);
                        }
                    }
                }
                Self::transform(&mut self.scratch, &self.twiddle, true);
                for i in 0..HOP {
                    self.output[i][out] = self.scratch[i].re + self.overlap[i][out];
                    self.overlap[i][out] = self.scratch[i + HOP].re;
                }
            }
            self.head = (self.head + 1) % PARTS;
            self.update_kernel(self.kernel_cursor);
            self.kernel_cursor = (self.kernel_cursor + 1) % PARTS;
        }
        result
    }
    fn reset(&mut self) {
        self.output.fill([0.; 2]);
        self.overlap.fill([0.; 2]);
        self.cursor = 0;
        self.head = 0;
        self.kernel_cursor = 0;
        self.valid_parts = 0;
        self.noise = self.initial_noise.clone();
        self.kernel_valid.fill(false);
    }
    fn heap_bytes(&self) -> usize {
        (self.twiddle.capacity()
            + self.history.capacity()
            + self.kernels.capacity()
            + self.initial_kernels.capacity()
            + self.scratch.capacity())
            * size_of::<Complex>()
            + (self.input.capacity() + self.output.capacity() + self.overlap.capacity())
                * size_of::<[f64; 2]>()
    }
}
/// Fade between stationary read heads instead of jumping between prime delays.
/// Retain only the latest request while a fade runs; never restart an active fade.
#[derive(Clone)]
struct BloomTransition {
    from: [u32; 16],
    to: [u32; 16],
    pending: [u32; 16],
    position: u32,
    frames: u32,
    initialized: bool,
}
impl BloomTransition {
    fn new(rate: u32) -> Self {
        Self {
            from: [1; 16],
            to: [1; 16],
            pending: [1; 16],
            position: (rate / 20).max(1),
            frames: (rate / 20).max(1),
            initialized: false,
        }
    }
    fn target(&mut self, delays: [u32; 16]) {
        self.pending = delays;
        if !self.initialized {
            self.from = delays;
            self.to = delays;
            self.initialized = true;
        }
    }
    fn advance(&mut self) -> f64 {
        if self.position == self.frames && self.pending != self.to {
            self.from = self.to;
            self.to = self.pending;
            self.position = 0;
        }
        self.position = self.position.saturating_add(1).min(self.frames);
        let t = f64::from(self.position) / f64::from(self.frames);
        t * t * (3.0 - 2.0 * t)
    }
    fn delay(&self, index: usize, weight: f64) -> f64 {
        f64::from(self.from[index]) * (1.0 - weight) + f64::from(self.to[index]) * weight
    }
    fn read(&self, line: &Delay, index: usize, movement: f64, weight: f64) -> [f64; 2] {
        let to = line.read(f64::from(self.to[index]) + movement);
        if weight == 1.0 || self.from[index] == self.to[index] {
            return to;
        }
        let from = line.read(f64::from(self.from[index]) + movement);
        std::array::from_fn(|ch| from[ch] * (1.0 - weight) + to[ch] * weight)
    }
    fn reset(&mut self) {
        self.position = self.frames;
        self.initialized = false;
    }
}

#[derive(Clone)]
pub struct Reverb {
    plate: bool,
    rate: f64,
    ramps: [ModulationRamp; 12],
    flags: Parameters,
    convolution: Convolution,
    predelay: Delay,
    lines: Vec<Delay>,
    late_lines: Vec<Delay>,
    maximum_delays: [u32; 16],
    primes: BloomDelays,
    feedback: [f64; 2],
    cross: [f64; 2],
    low: [f64; 2],
    high: [f64; 2],
    ambient_highpass: crate::filters::SlopeFilter,
    damping: [f64; 2],
    detector: f64,
    previous_shaper: [f64; 2],
    phase: f64,
    monitor: LevelMonitor<3>,
    effective: [f32; 12],
    mode_weight: ModulationRamp,
    makeup: crate::makeup::MakeupRamp,
    switches: [ModulationRamp; 5],
    drive_modes: [ModulationRamp; 3],
    drive_tone: [HighShelf; 2],
    drive_tone_value: f64,
    bloom_transition: BloomTransition,
    control_countdown: u8,
    input_low: crate::filters::SlopeFilter,
    input_high: crate::filters::SlopeFilter,
    detector_high: [StateVariableFilter; 2],
    detector_shelf: [HighShelf; 2],
}
impl Reverb {
    pub fn tail_frames(&self) -> u64 {
        if self.flags.values[11] == 0.0 {
            return 0;
        }
        let seconds = if self.plate {
            if self.flags.gated {
                2
            } else {
                daw_unit_core::PlateReverbSettings::MAX_TAIL_SECONDS
            }
        } else {
            daw_unit_core::AmbientReverbSettings::MAX_TAIL_SECONDS
        };
        self.rate as u64 * u64::from(seconds)
    }
    pub fn new(context: ProcessingContext, p: Parameters, plate: bool) -> Self {
        let primes = BloomDelays::default();
        let maximum_delays = primes.frames(1., 1., context.sample_rate_hz);
        let mut values = p.values;
        values[9] = gain(values[9]);
        Self {
            plate,
            rate: f64::from(context.sample_rate_hz),
            ramps: values.map(ModulationRamp::new),
            flags: p,
            convolution: Convolution::new(),
            predelay: Delay::new((context.sample_rate_hz as usize * 3) / 10 + 2),
            lines: maximum_delays
                .iter()
                .map(|d| Delay::new(*d as usize + 512))
                .collect(),
            late_lines: maximum_delays
                .iter()
                .map(|d| Delay::new(*d as usize + 512))
                .collect(),
            maximum_delays,
            primes,
            feedback: [0.; 2],
            cross: [0.; 2],
            low: [0.; 2],
            high: [0.; 2],
            ambient_highpass: crate::filters::SlopeFilter::new(
                context.sample_rate_hz,
                p.highpass_slope,
                true,
            ),
            damping: [0.; 2],
            detector: 0.,
            previous_shaper: [0.; 2],
            phase: 0.,
            monitor: LevelMonitor::new(context.sample_rate_hz, p.values[11] != 0.),
            effective: p.values,
            mode_weight: ModulationRamp::new(if p.hq && !p.gated { 1. } else { 0. }),
            makeup: crate::makeup::MakeupRamp::new(
                context.sample_rate_hz,
                crate::makeup::wet_makeup(
                    f64::from(p.values[0]),
                    f64::from(p.hq && !p.gated),
                    f64::from(p.gated),
                    plate,
                ),
            ),
            switches: [
                p.gated,
                p.dark,
                p.drive_enabled,
                p.highpass_enabled,
                p.lowpass_enabled,
            ]
            .map(|b| ModulationRamp::new(f32::from(b))),
            drive_modes: daw_unit_core::PlateDriveMode::ALL
                .map(|mode| ModulationRamp::new(f32::from(mode == p.drive_mode))),
            drive_tone: std::array::from_fn(|_| {
                HighShelf::new(
                    f64::from(context.sample_rate_hz),
                    2500.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                    (f64::from(p.values[6]) - 0.5) * 24.0,
                )
            }),
            drive_tone_value: f64::from(p.values[6]),
            bloom_transition: BloomTransition::new(context.sample_rate_hz),
            control_countdown: 0,
            input_low: crate::filters::SlopeFilter::new(
                context.sample_rate_hz,
                p.lowpass_slope,
                false,
            ),
            input_high: crate::filters::SlopeFilter::new(
                context.sample_rate_hz,
                p.highpass_slope,
                true,
            ),
            detector_high: std::array::from_fn(|_| StateVariableFilter::default()),
            detector_shelf: std::array::from_fn(|_| {
                HighShelf::new(f64::from(context.sample_rate_hz), 2500.0, 0.707, 12.0)
            }),
        }
    }
    pub fn retarget(&mut self, p: Parameters) {
        let mut values = p.values;
        values[9] = gain(values[9]);
        for (r, v) in self.ramps.iter_mut().zip(values) {
            r.set(v, (self.rate / 200.) as u32);
        }
        self.mode_weight.set(
            if p.hq && !p.gated { 1. } else { 0. },
            (self.rate / 200.) as u32,
        );
        self.monitor.set_enabled(p.values[11] != 0.);
        for (r, b) in self.switches.iter_mut().zip([
            p.gated,
            p.dark,
            p.drive_enabled,
            p.highpass_enabled,
            p.lowpass_enabled,
        ]) {
            r.set(f32::from(b), (self.rate / 200.0) as u32);
        }
        for (ramp, mode) in self
            .drive_modes
            .iter_mut()
            .zip(daw_unit_core::PlateDriveMode::ALL)
        {
            ramp.set(f32::from(mode == p.drive_mode), (self.rate / 200.0) as u32);
        }
        self.ambient_highpass.retarget(p.highpass_slope);
        self.input_high.retarget(p.highpass_slope);
        self.input_low.retarget(p.lowpass_slope);
        self.flags = p;
    }
    pub fn heap_bytes(&self) -> u64 {
        (self.convolution.heap_bytes()
            + self.predelay.heap_bytes()
            + self.lines.iter().map(Delay::heap_bytes).sum::<usize>()
            + self.late_lines.iter().map(Delay::heap_bytes).sum::<usize>()
            + self.late_lines.capacity() * size_of::<Delay>()
            + self.lines.capacity() * size_of::<Delay>()
            + 10000 * size_of::<u32>()) as u64
    }
    pub fn set_meter(&mut self, m: Arc<[AtomicU32; 3]>) {
        self.monitor.attach(m);
    }
    pub fn set_output_peak_meter(&mut self, m: Arc<BandMeter<3>>) {
        self.monitor.attach_peak(m);
    }
    pub fn effective(&self) -> [f32; 12] {
        self.effective
    }
    /// Called only by reset/preparation on the host's control/worker lifecycle.
    pub fn reset(&mut self) {
        self.convolution.reset();
        self.predelay.reset();
        for d in &mut self.lines {
            d.reset();
        }
        for d in &mut self.late_lines {
            d.reset();
        }
        self.feedback = [0.; 2];
        self.cross = [0.; 2];
        self.low = [0.; 2];
        self.high = [0.; 2];
        self.ambient_highpass.reset();
        self.damping = [0.; 2];
        self.detector = 0.;
        self.previous_shaper = [0.; 2];
        self.phase = 0.;
        self.monitor.reset();
        for r in &mut self.ramps {
            r.reset();
        }
        self.mode_weight.reset();
        self.makeup.reset(crate::makeup::wet_makeup(
            f64::from(self.flags.values[0]),
            f64::from(self.flags.hq && !self.flags.gated),
            f64::from(self.flags.gated),
            self.plate,
        ));
        for filter in &mut self.drive_tone {
            filter.reset();
        }
        for ramp in &mut self.drive_modes {
            ramp.reset();
        }
        for r in &mut self.switches {
            r.reset();
        }
        self.control_countdown = 0;
        self.bloom_transition.reset();
        self.input_low.reset();
        self.input_high.reset();
        for f in &mut self.detector_high {
            f.reset();
        }
        for f in &mut self.detector_shelf {
            f.reset();
        }
    }
    pub fn sample(&mut self, input: [f32; 2], offset: [f64; 12]) -> [f32; 2] {
        let mut v = self
            .ramps
            .each_mut()
            .map(ModulationRamp::next_value)
            .map(f64::from);
        for i in [0, 2, 3] {
            v[i] *= 2_f64.powf(offset[i].clamp(-6., 6.));
        }
        for i in [1, 4, 5, 6, 7, 8, 10] {
            v[i] += offset[i];
        }
        v[0] = v[0].clamp(
            if self.plate { 0.3 } else { 1. },
            if self.plate { 15. } else { 60. },
        );
        v[1] = v[1].clamp(0., 300.);
        v[2] = v[2].clamp(20., 1000.);
        v[3] = v[3].clamp(500., 15000.);
        v[4] = v[4].clamp(0., 12.);
        for i in [5, 6, 7, 8, 10] {
            v[i] = v[i].clamp(0., 1.);
        }
        v[9] = (v[9] * 10_f64.powf(offset[9].clamp(-96., 96.) / 20.)).min(f64::from(gain(12.)));
        self.effective = v.map(|x| x as f32);
        self.effective[9] = if v[9] > 0. {
            (20. * v[9].log10()) as f32
        } else {
            daw_unit_core::SILENT_GAIN_DB
        };
        let dry = input.map(|x| if x.is_finite() { f64::from(x) } else { 0. });
        let hq = f64::from(self.mode_weight.next_value());
        let [gate, dark, drive_enabled, highpass_enabled, lowpass_enabled] = self
            .switches
            .each_mut()
            .map(ModulationRamp::next_value)
            .map(f64::from);
        let mut wet = dry;
        let lp = if self.plate {
            v[3]
        } else {
            1000. * 15_f64.powf(v[6])
        };
        let hp = if self.plate { v[2] } else { 20. + 100. * v[6] };
        let lc = 1. - (-std::f64::consts::TAU * lp.min(self.rate * 0.45) / self.rate).exp();
        let hc = 1. - (-std::f64::consts::TAU * hp / self.rate).exp();
        let slope_weights = self.ambient_highpass.weights();
        let high_weights = self.input_high.weights();
        let low_weights = self.input_low.weights();
        for (ch, sample) in wet.iter_mut().enumerate() {
            self.low[ch] += lc * (*sample - self.low[ch]);
            self.high[ch] += hc * (self.low[ch] - self.high[ch]);
            *sample = if self.plate {
                let low = self.input_low.sample_input(
                    ch,
                    *sample,
                    lp.min(self.rate * 0.45),
                    self.rate,
                    low_weights,
                );
                let low = low * lowpass_enabled + *sample * (1.0 - lowpass_enabled);
                let high = self
                    .input_high
                    .sample_input(ch, low, hp, self.rate, high_weights);
                high * highpass_enabled + low * (1.0 - highpass_enabled)
            } else {
                let filtered = self.ambient_highpass.sample(
                    ch,
                    self.low[ch],
                    self.low[ch] - self.high[ch],
                    hp,
                    self.rate,
                    slope_weights,
                );
                let high = self.low[ch] * (1.0 - highpass_enabled) + filtered * highpass_enabled;
                let low = self
                    .input_low
                    .sample_input(ch, high, v[3], self.rate, low_weights);
                low * lowpass_enabled + high * (1.0 - lowpass_enabled)
            };
        }
        if self.plate {
            let mono = (wet[0] + wet[1]) * 0.5;
            wet = wet.map(|x| mono + (x - mono) * hq);
            let delayed = self.predelay.read((v[1] * self.rate / 1000.).max(1.));
            self.predelay.write(wet);
            if v[1] > 0. {
                wet = delayed;
            }
        }
        let bloom = if self.plate { 0.12 } else { v[5] };
        if self.control_countdown == 0 {
            self.bloom_transition
                .target(self.primes.frames(bloom, 0.5, self.rate as u32));
            self.control_countdown = 127;
        } else {
            self.control_countdown -= 1;
        }
        let bloom_weight = self.bloom_transition.advance();
        let delays: [f64; 16] =
            std::array::from_fn(|i| self.bloom_transition.delay(i, bloom_weight));
        self.phase = (self.phase + 0.1 / self.rate).fract();
        let mut stage = wet;
        let mut at12 = [0.; 2];
        for (i, line) in self.lines.iter_mut().enumerate() {
            let movement = if self.plate {
                0.
            } else {
                v[7] * (1. + 19. * bloom)
                    * (std::f64::consts::TAU * (self.phase + i as f64 / 16.)).sin()
            };
            let delay = (delays[i] + movement).clamp(1., f64::from(self.maximum_delays[i]) + 500.);
            let read = self.bloom_transition.read(line, i, movement, bloom_weight);
            let decay = v[0] * (1.0 - gate) + 0.18 * gate;
            let attenuation = 10_f64.powf(-3. * delay / (self.rate * decay));
            let mut branch = [0.; 2];
            for ch in 0..2 {
                let cross = if i == 0 {
                    -0.25 * self.cross[1 - ch]
                } else if i == 4 {
                    0.25 * self.feedback[1 - ch]
                } else {
                    0.
                };
                let j = junction([stage[ch], read[ch] * attenuation + cross]);
                stage[ch] = j[0];
                branch[ch] = j[1].clamp(-16., 16.);
            }
            line.write(branch);
            if i == 11 {
                at12 = stage;
            }
        }
        self.cross = at12;
        self.feedback = stage;
        let convolution = self.convolution.sample(stage, hq, gate, v[0], self.rate);
        // A normalized Hadamard feedback matrix is orthogonal. Each line's
        // attenuation is derived from its own delay and the requested T60.
        // This keeps long decay independent of the finite convolution span.
        let mut reads = [[0.0; 2]; 16];
        let mut late = [0.0; 2];
        for (i, line) in self.late_lines.iter().enumerate() {
            reads[i] = self.bloom_transition.read(line, i, 0.0, bloom_weight);
            late[0] += reads[i][0] * if i & 1 == 0 { 0.25 } else { -0.25 };
            late[1] += reads[i][1] * if i & 2 == 0 { 0.25 } else { -0.25 };
        }
        let mut size = 1;
        while size < 16 {
            for block in (0..16).step_by(size * 2) {
                for i in 0..size {
                    let a = reads[block + i];
                    let b = reads[block + i + size];
                    reads[block + i] = [a[0] + b[0], a[1] + b[1]];
                    reads[block + i + size] = [a[0] - b[0], a[1] - b[1]];
                }
            }
            size *= 2;
        }
        let target_decay = v[0] * (1.0 - gate) + 0.18 * gate;
        for (i, line) in self.late_lines.iter_mut().enumerate() {
            let attenuation = 10_f64.powf(-3.0 * delays[i] / (self.rate * target_decay));
            let value = std::array::from_fn(|ch| {
                let sign = if (i >> (ch + 1)) & 1 == 0 { 1.0 } else { -1.0 };
                (convolution[ch] * sign * 0.25 + reads[i][ch] * 0.25 * attenuation)
                    .clamp(-16.0, 16.0)
            });
            line.write(value);
        }
        let convolution =
            std::array::from_fn::<_, 2, _>(|ch| convolution[ch] * 0.3 + late[ch] * 0.7);
        for (ch, convolved) in convolution.iter().enumerate() {
            let cutoff = if self.plate {
                10000.0 * 0.25_f64.powf(dark)
            } else {
                lp
            };
            let c = 1. - (-std::f64::consts::TAU * cutoff.min(self.rate * 0.45) / self.rate).exp();
            self.damping[ch] += c * (*convolved - self.damping[ch]);
        }
        wet = wet_width(self.damping, v[8]);
        let drive_weights = crate::drive::weights(&mut self.drive_modes);
        if self.plate {
            if v[6] != self.drive_tone_value {
                for filter in &mut self.drive_tone {
                    filter.configure(
                        self.rate,
                        2500.0,
                        std::f64::consts::FRAC_1_SQRT_2,
                        (v[6] - 0.5) * 24.0,
                    );
                }
                self.drive_tone_value = v[6];
            }
            let drive = 10_f64.powf(v[4] / 20.);
            for (ch, sample) in wet.iter_mut().enumerate() {
                let shaped =
                    crate::drive::shape(*sample * drive, drive_weights) / drive.sqrt().max(0.1);
                let toned = self.drive_tone[ch].sample(shaped);
                *sample = *sample * (1.0 - drive_enabled) + toned * drive_enabled;
            }
        }
        // Calibrate only the completed wet path: do not change Drive intensity,
        // feedback decay, or the dry branch. Slow gain rises protect stored tails.
        let makeup = self
            .makeup
            .sample(crate::makeup::wet_makeup(v[0], hq, gate, self.plate));
        wet = wet.map(|x| x * makeup);
        let [gd, gw] = mix_gains(v[10], self.plate);
        let mut result = std::array::from_fn::<_, 2, _>(|ch| dry[ch] * gd + wet[ch] * gw);
        if self.plate {
            result = result.map(|x| x * v[9]);
        } else {
            let detector = (0..2)
                .map(|ch| {
                    let x = self.detector_high[ch].high(
                        self.previous_shaper[ch],
                        25.0,
                        0.25,
                        self.rate,
                    );
                    let x = self.detector_shelf[ch].sample(x) * 4.0;
                    x * x
                })
                .sum::<f64>()
                * 0.5;
            self.detector += (detector - self.detector) / (0.027 * self.rate).max(1.);
            let a = self.detector.max(0.).sqrt().min(1.);
            for (ch, sample) in result.iter_mut().enumerate() {
                let x = *sample * v[9];
                let y = a * x + 2. * (1. - a) * x / (1. + 3. * x * x).sqrt();
                self.previous_shaper[ch] = y;
                *sample = 10_f64.powf(-4. / 20.) * y;
            }
        }
        let output = std::array::from_fn(|ch| {
            let x = dry[ch] * (1. - v[11]) + result[ch] * v[11];
            if x.is_finite() {
                x.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32
            } else {
                0.
            }
        });
        self.monitor.sample_output(output);
        output
    }
}
fn gain(db: f32) -> f32 {
    if db <= daw_unit_core::SILENT_GAIN_DB {
        0.
    } else {
        10_f32.powf(db / 20.)
    }
}

#[cfg(test)]
mod bloom_tests {
    use super::*;

    #[test]
    fn rapid_targets_preserve_continuity_and_reset_selects_the_current_taps() {
        let mut line = Delay::new(1024);
        for frame in 0..1024 {
            line.write(if frame % 128 < 64 {
                [1.0, -1.0]
            } else {
                [-1.0, 1.0]
            });
        }
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut transition = BloomTransition::new(rate);
            transition.target([32; 16]);
            let weight = transition.advance();
            let mut previous = transition.read(&line, 0, 0.0, weight);
            for frame in 0..transition.frames * 4 {
                if frame % 128 == 0 {
                    transition.target([if frame % 256 == 0 { 96 } else { 32 }; 16]);
                }
                let weight = transition.advance();
                let sample = transition.read(&line, 0, 0.0, weight);
                for ch in 0..2 {
                    assert!(
                        (sample[ch] - previous[ch]).abs() <= 4.0 / f64::from(transition.frames)
                    );
                }
                previous = sample;
            }
            transition.target([96; 16]);
            for _ in 0..transition.frames * 2 {
                transition.advance();
            }
            assert_eq!(transition.read(&line, 0, 0.0, 1.0), line.read(96.0));
            transition.reset();
            transition.target([32; 16]);
            let weight = transition.advance();
            assert_eq!(transition.read(&line, 0, 0.0, weight), line.read(32.0));
        }
    }
}
