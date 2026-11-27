//! Stereo-linked transient gain from fast and slow peak envelopes.

#[derive(Clone)]
pub struct TransientDetector {
    // Update coefficients for fast rise, fast fall, slow rise, and slow fall.
    smooth: [f64; 4],
    snappy: [f64; 4],
    fast_envelope: f64,
    slow_envelope: f64,
}

impl TransientDetector {
    pub fn new(sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let coefficients = |times: [f64; 4]| times.map(|ms| -(-1000.0 / (rate * ms)).exp_m1());
        Self {
            smooth: coefficients([0.8, 25.0, 24.0, 150.0]),
            snappy: coefficients([0.06, 5.0, 5.0, 35.0]),
            fast_envelope: 0.0,
            slow_envelope: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.fast_envelope = 0.0;
        self.slow_envelope = 0.0;
    }

    pub fn shape_db(&mut self, peak: f64, [attack, pump, sustain, speed]: [f64; 4]) -> f64 {
        let update = |envelope: f64, rise: usize| {
            let index = rise + usize::from(peak < envelope);
            let coefficient =
                self.smooth[index] + (self.snappy[index] - self.smooth[index]) * speed;
            envelope + (peak - envelope) * coefficient
        };
        self.fast_envelope = update(self.fast_envelope, 0);
        self.slow_envelope = update(self.slow_envelope, 2);

        let fast = self.fast_envelope;
        let slow = self.slow_envelope;
        // The floor keeps signals below -120 dBFS from driving gain changes.
        let reference = fast.max(slow).max(1e-6);
        let onset = ((fast - slow) / reference).max(0.0);
        let decay = ((slow - fast) / reference).max(0.0);
        // Preserve the attack region when changing sustain, and fade shaping out in silence.
        let body = (slow / reference) * (1.0 - onset);
        attack * onset - pump * decay + sustain * body
    }
}
