//! Prepared, incremental radix-2 spectrum analysis. At most eight butterflies/bin reads per sample.
//! No processing-thread allocation, FFT-sized burst, locks or audio-path changes.
use daw_unit_core::{SPECTRUM_BINS, SpectrumMeter, SpectrumReading, UnitMeter, spectrum_frequency};
use std::sync::Arc;
type Complex = [f32; 2];
#[derive(Clone)]
pub(crate) struct Spectrum {
    meter: UnitMeter<SpectrumMeter>,
    reading: SpectrumReading,
    accum: [f32; SPECTRUM_BINS],
    release: f32,
    input: Vec<Complex>,
    work: Vec<Complex>,
    window: Vec<f32>,
    reverse: Vec<usize>,
    twiddles: Vec<Complex>,
    position: usize,
    len: usize,
    butterfly: usize,
    bin: usize,
    scan: usize,
    ranges: [(usize, usize); SPECTRUM_BINS],
    pending: bool,
}
impl Spectrum {
    pub fn new(rate: u32, enabled: bool) -> Self {
        let n = 4096 * (rate / 44_100).max(1).next_power_of_two() as usize;
        let bits = n.trailing_zeros();
        Self {
            meter: UnitMeter::new(enabled),
            reading: SpectrumReading {
                sample_rate_hz: rate,
                ..Default::default()
            },
            accum: [0.0; SPECTRUM_BINS],
            release: (-(n as f32) / (rate as f32 * 0.3)).exp(),
            input: vec![[0.0; 2]; n],
            work: vec![[0.0; 2]; n],
            window: (0..n)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
                .collect(),
            reverse: (0..n)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            twiddles: (0..n / 2)
                .map(|i| {
                    let w = -std::f32::consts::TAU * i as f32 / n as f32;
                    [w.cos(), w.sin()]
                })
                .collect(),
            ranges: std::array::from_fn(|i| {
                let center = spectrum_frequency(i) * n as f32 / rate as f32;
                let half = (1000.0_f32.powf(0.5 / (SPECTRUM_BINS - 1) as f32) - 1.0) * center;
                (
                    (center - half).floor().max(1.0) as usize,
                    (center + half).ceil().min((n / 2 - 1) as f32).max(1.0) as usize,
                )
            }),
            position: 0,
            len: 2,
            butterfly: 0,
            bin: 0,
            scan: 0,
            pending: false,
        }
    }
    pub fn is_connected(&self) -> bool {
        self.meter.is_connected()
    }
    pub fn is_enabled(&self) -> bool {
        self.meter.is_enabled()
    }
    pub fn attach(&mut self, meter: Arc<SpectrumMeter>) {
        // Preparation can share this handle with an audible renderer. Do not
        // replace its last frame with the new analyzer's empty startup buffer.
        let previous = meter.reading();
        let initial = if previous.sample_rate_hz == self.reading.sample_rate_hz {
            previous
        } else {
            self.reading
        };
        self.meter.attach(meter, initial);
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.meter.set_enabled(enabled);
        if !enabled {
            self.reset();
        }
    }
    pub fn retarget(&mut self, prepared: &mut Self) {
        self.meter.retarget(&mut prepared.meter, self.reading);
        if !self.meter.is_connected() {
            self.reset();
        }
    }
    pub fn reset(&mut self) {
        self.position = 0;
        self.pending = false;
        self.reading.magnitudes.fill(0.0);
        self.accum.fill(0.0);
        self.meter.publish(self.reading);
    }
    pub fn sample(&mut self, input: [f32; 2]) {
        if !self.meter.is_connected() {
            return;
        }
        let n = self.input.len();
        self.input[self.reverse[self.position]] = input.map(|v| {
            if v.is_finite() {
                v.clamp(-1e6, 1e6) * self.window[self.position]
            } else {
                0.0
            }
        });
        for _ in 0..8 {
            if !self.pending {
                break;
            }
            if self.len <= n {
                let half = self.len / 2;
                let group = self.butterfly / half;
                let j = self.butterfly % half;
                let a = group * self.len + j;
                let b = a + half;
                let w = self.twiddles[j * n / self.len];
                let v = self.work[b];
                let t = [v[0] * w[0] - v[1] * w[1], v[0] * w[1] + v[1] * w[0]];
                let u = self.work[a];
                self.work[a] = [u[0] + t[0], u[1] + t[1]];
                self.work[b] = [u[0] - t[0], u[1] - t[1]];
                self.butterfly += 1;
                if self.butterfly == n / 2 {
                    self.butterfly = 0;
                    self.len *= 2;
                }
            } else {
                let (start, end) = self.ranges[self.bin];
                let k = start + self.scan;
                let power = |v: Complex| v[0] * v[0] + v[1] * v[1];
                // Packed L+iR preserves anti-phase stereo; mean channel power is |Zk|²+|Z−k|².
                let amplitude =
                    (power(self.work[k]) + power(self.work[n - k])).sqrt() * 2.0 / n as f32;
                self.accum[self.bin] = self.accum[self.bin].max(amplitude);
                self.scan += 1;
                if start + self.scan > end {
                    self.scan = 0;
                    self.bin += 1;
                }
                if self.bin == SPECTRUM_BINS {
                    self.pending = false;
                    for i in 0..SPECTRUM_BINS {
                        self.reading.magnitudes[i] =
                            self.accum[i].max(self.reading.magnitudes[i] * self.release);
                    }
                    self.meter.publish(self.reading);
                }
            }
        }
        self.position += 1;
        if self.position == n {
            debug_assert!(
                !self.pending,
                "bounded spectrum work must finish before buffer reuse"
            );
            std::mem::swap(&mut self.input, &mut self.work);
            self.position = 0;
            self.len = 2;
            self.butterfly = 0;
            self.bin = 0;
            self.scan = 0;
            self.accum.fill(0.0);
            self.pending = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::MeterStorage;
    #[test]
    fn preparing_repeated_updates_never_clears_the_visible_spectrum() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let meter = Arc::new(SpectrumMeter::default());
            let mut current = Spectrum::new(rate, true);
            current.attach(meter.clone());
            for i in 0..current.input.len() * 3 {
                let v = (std::f32::consts::TAU * 1000.0 * i as f32 / rate as f32).sin() * 0.5;
                current.sample([v, v]);
            }
            let before = meter.reading();
            assert!(before.magnitudes.iter().any(|v| *v > 0.3));
            for _ in 0..20 {
                let mut prepared = Spectrum::new(rate, true);
                prepared.attach(meter.clone());
                assert_eq!(meter.reading().magnitudes, before.magnitudes);
                current.retarget(&mut prepared);
                assert_eq!(meter.reading().magnitudes, before.magnitudes);
            }
            current.set_enabled(false);
            assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
        }
    }
    #[test]
    fn spectrum_tracks_tones_at_every_rate_without_stereo_cancellation_and_clears_bypass() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            for hz in [80.0, 1000.0, 12000.0] {
                let mut spectrum = Spectrum::new(rate, true);
                let meter = Arc::new(SpectrumMeter::default());
                spectrum.attach(meter.clone());
                for i in 0..spectrum.input.len() * 3 {
                    let v = 0.5 * (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin();
                    spectrum.sample([v, -v]);
                }
                let reading = meter.reading();
                let peak = reading
                    .magnitudes
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap();
                assert!(
                    (spectrum_frequency(peak.0) / hz).log2().abs() < 0.25,
                    "{rate} {hz} {}",
                    spectrum_frequency(peak.0)
                );
                assert!(*peak.1 > 0.3 && *peak.1 < 0.55, "{rate} {hz} {}", peak.1);
                assert_eq!(reading.sample_rate_hz, rate);
                let clone = spectrum.clone();
                assert!(!clone.meter.is_connected());
                spectrum.set_enabled(false);
                assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
                for _ in 0..100 {
                    spectrum.sample([1.0; 2]);
                }
                assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
                spectrum.set_enabled(true);
                spectrum.reset();
                assert!(meter.reading().magnitudes.iter().all(|v| *v == 0.0));
                meter.clear();
            }
        }
    }
}
