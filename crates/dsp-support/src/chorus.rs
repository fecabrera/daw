//! Interpolated delay voices and the bounded stereo feedback loop.
#[derive(Clone)]
pub struct ChorusDelay {
    delay: Vec<[f64; 2]>,
    cursor: usize,
    rate: f64,
}

pub struct VoiceFrame {
    pub time: f64,
    pub excursion: f64,
    pub feedback: f64,
    pub phase: f64,
    pub stereo_phase: f64,
    pub weights: [f32; 4],
    pub phases: [f32; 4],
}

impl ChorusDelay {
    pub fn new(sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate);
        Self {
            delay: vec![[0.0; 2]; (rate * 0.046).ceil() as usize + 4],
            cursor: 0,
            rate,
        }
    }

    pub fn reset(&mut self) {
        self.delay.fill([0.0; 2]);
        self.cursor = 0;
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn heap_bytes(&self) -> u64 {
        crate::memory::vec_bytes(&self.delay)
    }

    fn read(&self, frames: f64, channel: usize) -> f64 {
        let position = (self.cursor as f64 - frames).rem_euclid(self.delay.len() as f64);
        let index = position.floor() as usize;
        let a = self.delay[index][channel];
        let b = self.delay[(index + 1) % self.delay.len()][channel];
        // Linear interpolation is convex: the bounded feedback loop cannot gain energy here.
        a + (b - a) * position.fract()
    }
    pub fn sample(&mut self, dry: [f64; 2], frame: VoiceFrame) -> [f64; 2] {
        let VoiceFrame {
            time,
            excursion,
            feedback,
            phase,
            stereo_phase,
            weights,
            phases,
        } = frame;
        let total = f64::from(weights.iter().sum::<f32>()).max(1.0);
        let mut wet = [0.0; 2];
        for (i, weight) in weights.into_iter().enumerate() {
            if weight == 0.0 {
                continue;
            }
            for (ch, value) in wet.iter_mut().enumerate() {
                let lfo = crate::periodic::waveform(
                    0,
                    phase + f64::from(phases[i]) + if ch == 1 { stereo_phase } else { 0.0 },
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
        wet
    }
}
