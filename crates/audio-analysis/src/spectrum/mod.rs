//! Prepared incremental stereo spectrum analysis over caller-owned samples.
mod fft;
mod mapping;

type Complex = [f32; 2];
const WORK_PER_FRAME: usize = 80;
pub const SPECTRUM_BINS: usize = 512;

/// Logarithmic centers shared by measurement and display.
pub fn spectrum_frequency(index: usize) -> f32 {
    20.0 * 1000.0_f32.powf(index as f32 / (SPECTRUM_BINS - 1) as f32)
}

pub enum Update<'a> {
    Pending,
    /// Raw power values and the exclusive source-frame clock since reset.
    Ready {
        powers: &'a [f32; SPECTRUM_BINS],
        end_frame: u64,
    },
    /// The pending capture missed its deadline. The analysis has reset.
    Overrun,
}

/// Incremental analyzer with storage prepared before sampling.
#[derive(Clone)]
pub struct SpectrumAnalyzer {
    branch: fft::Branch,
    frames: u64,
    last_publication: u64,
    #[cfg(test)]
    max_work: usize,
}

impl SpectrumAnalyzer {
    /// Prepare the analyzer for one of the supported DAW sample rates.
    pub fn new(rate: u32) -> Self {
        let scale = (rate / 44_100).max(1).next_power_of_two() as usize;
        let n = 4096 * scale;
        Self {
            branch: fft::Branch::new(n, rate as f32, n / 4, n, 20_000.0, 26, 2400),
            frames: 0,
            last_publication: 0,
            #[cfg(test)]
            max_work: 0,
        }
    }

    pub fn heap_bytes(&self) -> u64 {
        self.branch.heap_bytes()
    }

    pub fn is_pending(&self) -> bool {
        self.branch.pending
    }

    pub fn reset(&mut self) {
        self.branch.reset();
        self.frames = 0;
        self.last_publication = 0;
    }

    /// Accept one stereo frame. Copy and FFT work are bounded and allocation-free.
    #[inline]
    pub fn sample(&mut self, input: [f32; 2]) -> Update<'_> {
        // Service capture samples before the ring overwrites them. Eight copies
        // per frame complete the window in half a hop.
        let mut work = 0;
        for _ in 0..8 {
            work += usize::from(self.branch.copy());
        }
        self.frames += 1;
        let input = input.map(|v| {
            if v.is_finite() {
                v.clamp(-1e6, 1e6)
            } else {
                0.0
            }
        });
        if !self.branch.feed(input, self.frames) {
            self.reset();
            return Update::Overrun;
        }
        while work < WORK_PER_FRAME {
            if !self.branch.step() {
                break;
            }
            work += 1;
        }
        #[cfg(test)]
        {
            self.max_work = self.max_work.max(work);
        }
        if self.branch.valid
            && !self.branch.pending
            && self.branch.end_frame != self.last_publication
        {
            self.last_publication = self.branch.end_frame;
            Update::Ready {
                powers: &self.branch.powers,
                end_frame: self.branch.end_frame,
            }
        } else {
            Update::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_spectrum_publishes_tones_with_bounded_work() {
        for rate in crate::SAMPLE_RATES {
            let mut analyzer = SpectrumAnalyzer::new(rate);
            let mut last_frame = 0;
            for frame in 0..rate {
                let x = (std::f64::consts::TAU * f64::from(frame) * 80.0 / f64::from(rate)).sin()
                    as f32
                    * 0.5;
                match analyzer.sample([x, -x]) {
                    Update::Ready { powers, end_frame } => {
                        assert!(end_frame > last_frame);
                        assert!(powers.iter().enumerate().any(|(bin, power)| {
                            (75.0..=85.0).contains(&spectrum_frequency(bin)) && *power > 0.1
                        }));
                        last_frame = end_frame;
                    }
                    Update::Pending => {}
                    Update::Overrun => panic!("analysis exceeded its capture budget"),
                }
            }
            assert!(last_frame > 0);
            assert!(analyzer.max_work <= WORK_PER_FRAME);
        }
    }
}
