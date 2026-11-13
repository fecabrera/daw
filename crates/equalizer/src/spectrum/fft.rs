//! Incremental overlapped FFT. Capture copies precede ring overwrite.
use super::{Complex, mapping};
use daw_unit_core::SPECTRUM_BINS;
#[derive(Clone)]
pub(super) struct Branch {
    ring: Vec<Complex>,
    work: Vec<Complex>,
    window: Vec<f32>,
    reverse: Vec<usize>,
    twiddles: Vec<Complex>,
    cells: [mapping::Cell; SPECTRUM_BINS],
    cells_len: usize,
    pub powers: [f32; SPECTRUM_BINS],
    pub pending: bool,
    pub valid: bool,
    pub end_frame: u64,
    position: usize,
    seen: u64,
    minimum_capture: u64,
    hop: usize,
    start: usize,
    copied: usize,
    len: usize,
    butterfly: usize,
    bin: usize,
    scan: usize,
    accum: f32,
}
impl Branch {
    pub fn new(
        n: usize,
        rate: f32,
        hop: usize,
        minimum_capture: usize,
        maximum_hz: f32,
        per_cell: usize,
        total: usize,
    ) -> Self {
        let (cells, cells_len) = mapping::prepare(n, rate, maximum_hz, per_cell, total);
        let bits = n.trailing_zeros();
        Self {
            ring: vec![[0.0; 2]; n],
            work: vec![[0.0; 2]; n],
            window: (0..n)
                .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos()) as f32)
                .collect(),
            reverse: (0..n)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            twiddles: (0..n / 2)
                .map(|i| {
                    let w = -std::f64::consts::TAU * i as f64 / n as f64;
                    [w.cos() as f32, w.sin() as f32]
                })
                .collect(),
            cells,
            cells_len,
            powers: [0.0; SPECTRUM_BINS],
            pending: false,
            valid: false,
            end_frame: 0,
            position: 0,
            seen: 0,
            minimum_capture: minimum_capture as u64,
            hop,
            start: 0,
            copied: 0,
            len: 2,
            butterfly: 0,
            bin: 0,
            scan: 0,
            accum: 0.0,
        }
    }
    pub fn heap_bytes(&self) -> u64 {
        daw_unit_core::memory::vec_bytes(&self.ring)
            + daw_unit_core::memory::vec_bytes(&self.work)
            + daw_unit_core::memory::vec_bytes(&self.window)
            + daw_unit_core::memory::vec_bytes(&self.reverse)
            + daw_unit_core::memory::vec_bytes(&self.twiddles)
    }
    pub fn reset(&mut self) {
        self.pending = false;
        self.valid = false;
        self.position = 0;
        self.seen = 0;
        self.end_frame = 0;
    }
    /// Returns false on deadline/overwrite failure; the host discards analysis only.
    pub fn feed(&mut self, value: Complex, clock: u64) -> bool {
        let n = self.ring.len();
        if self.pending && self.copied < n {
            let overwritten = (self.position + n - self.start) % n;
            if overwritten >= self.copied {
                return false;
            }
        }
        self.ring[self.position] = value;
        self.position = (self.position + 1) % n;
        self.seen += 1;
        if self.seen >= self.minimum_capture && self.seen.is_multiple_of(self.hop as u64) {
            if self.pending {
                return false;
            }
            self.start = self.position;
            self.copied = 0;
            self.len = 2;
            self.butterfly = 0;
            self.bin = 0;
            self.scan = 0;
            self.accum = 0.0;
            self.end_frame = clock;
            self.pending = true;
        }
        true
    }
    pub fn copy(&mut self) -> bool {
        let n = self.ring.len();
        if !self.pending || self.copied == n {
            return false;
        }
        let i = self.copied;
        self.work[self.reverse[i]] = self.ring[(self.start + i) % n].map(|v| v * self.window[i]);
        self.copied += 1;
        true
    }
    pub fn step(&mut self) -> bool {
        let n = self.ring.len();
        if !self.pending || self.copied != n {
            return false;
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
            let cell = self.cells[self.bin];
            let k = usize::from(cell.first) + self.scan;
            let power = |v: Complex| v[0] * v[0] + v[1] * v[1];
            // L+iR: |Zk|²+|Z−k|² preserves mean channel energy, including anti-phase.
            let power = (power(self.work[k]) + power(self.work[n - k])) * (4.0 / (n * n) as f32);
            if self.scan == 0 {
                self.accum = 0.0;
            }
            if cell.interpolate {
                self.accum += power
                    * if self.scan == 0 {
                        1.0 - cell.fraction
                    } else {
                        cell.fraction
                    };
            } else {
                self.accum = self.accum.max(power);
            }
            self.scan += 1;
            if self.scan == usize::from(cell.count) {
                self.powers[self.bin] = self.accum;
                self.scan = 0;
                self.bin += 1;
                if self.bin == self.cells_len {
                    self.pending = false;
                    self.valid = true;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_copy_service_rejects_overwrite_and_logical_reset_recovers() {
        let mut b = Branch::new(4096, 48000.0, 1024, 4096, 20000.0, 26, 2400);
        for frame in 1..=4096 {
            assert!(b.feed([0.25, -0.25], frame));
        }
        assert!(b.pending);
        assert!(!b.feed([0.0; 2], 4097));
        b.reset();
        assert!(!b.pending && !b.valid);
        for frame in 1..=4096 {
            assert!(b.feed([0.0; 2], frame));
        }
        while b.copy() {}
        while b.step() {}
        assert!(b.valid);
        assert!(b.powers.iter().all(|p| *p == 0.0));
    }
}
