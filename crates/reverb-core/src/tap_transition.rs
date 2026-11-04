use crate::Delay;

/// Fade between stationary read positions instead of moving an active read head.
/// Positions and movement use the same caller-defined units. Targets must be finite.
/// Retain only the latest request while a fade runs; never restart an active fade.
#[derive(Clone)]
pub struct TapTransition<const N: usize> {
    from: [f64; N],
    to: [f64; N],
    pending: [f64; N],
    position: u32,
    frames: u32,
    initialized: bool,
}
impl<const N: usize> TapTransition<N> {
    pub fn new(rate: u32) -> Self {
        Self {
            from: [0.0; N],
            to: [0.0; N],
            pending: [0.0; N],
            position: (rate / 20).max(1),
            frames: (rate / 20).max(1),
            initialized: false,
        }
    }
    pub fn target<T: Copy + Into<f64>>(&mut self, delays: [T; N]) {
        let delays = delays.map(Into::into);
        self.pending = delays;
        if !self.initialized {
            self.from = delays;
            self.to = delays;
            self.initialized = true;
        }
    }
    pub fn advance(&mut self) -> f64 {
        if self.position == self.frames && self.pending != self.to {
            self.from = self.to;
            self.to = self.pending;
            self.position = 0;
        }
        self.position = self.position.saturating_add(1).min(self.frames);
        let t = f64::from(self.position) / f64::from(self.frames);
        t * t * (3.0 - 2.0 * t)
    }
    /// Interpolated position for coefficients only. Audio must use `read` or
    /// `read_with`, which blend samples instead of moving an active position.
    pub fn delay(&self, index: usize, weight: f64) -> f64 {
        self.from[index] * (1.0 - weight) + self.to[index] * weight
    }
    pub fn read(&self, line: &Delay, index: usize, movement: f64, weight: f64) -> [f64; 2] {
        self.read_with(index, movement, weight, |frames| line.read(frames))
    }
    /// Read both fixed positions from prepared history, then blend their samples.
    /// The reader must not change the history between the two reads.
    pub fn read_with<const CHANNELS: usize>(
        &self,
        index: usize,
        movement: f64,
        weight: f64,
        read: impl Fn(f64) -> [f64; CHANNELS],
    ) -> [f64; CHANNELS] {
        let to = read(self.to[index] + movement);
        if weight == 1.0 || self.from[index] == self.to[index] {
            return to;
        }
        let from = read(self.from[index] + movement);
        std::array::from_fn(|ch| from[ch] * (1.0 - weight) + to[ch] * weight)
    }
    pub fn reset(&mut self) {
        self.position = self.frames;
        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_and_zero_positions_stay_fixed_while_latest_requests_wait() {
        for rate in [44100, 48000, 88200, 96000, 176400, 192000] {
            let mut transition = TapTransition::<1>::new(rate);
            transition.target([0.0]);
            let weight = transition.advance();
            assert_eq!(transition.read_with(0, 0.0, weight, |p| [p]), [0.0]);
            transition.target([12.5]);
            for i in 0..transition.frames {
                let weight = transition.advance();
                let sample = transition.read_with(0, 0.0, weight, |p| {
                    assert!(p == 0.0 || p == 12.5, "active position moved: {p}");
                    [p]
                });
                transition.target([100.0 + f64::from(i)]);
                if i + 1 == transition.frames {
                    assert_eq!(sample, [12.5], "pending requests restarted the fade");
                }
            }
            transition.target([30.25]);
            for _ in 0..transition.frames {
                transition.advance();
            }
            assert_eq!(transition.read_with(0, 0.0, 1.0, |p| [p]), [30.25]);
            transition.reset();
            transition.target([0.125]);
            let weight = transition.advance();
            assert_eq!(transition.read_with(0, 0.0, weight, |p| [p]), [0.125]);
        }
    }

    #[test]
    fn clones_preserve_active_and_pending_transitions_without_restarting() {
        let mut transition = TapTransition::<1>::new(48000);
        transition.target([0.0]);
        transition.advance();
        transition.target([10.5]);
        for _ in 0..1000 {
            transition.advance();
        }
        transition.target([20.25]);
        let mut copy = transition.clone();
        for _ in 0..6000 {
            let a = transition.advance();
            let b = copy.advance();
            assert_eq!(a, b);
            assert_eq!(
                transition.read_with(0, 0.0, a, |p| [p]),
                copy.read_with(0, 0.0, b, |p| [p])
            );
        }
        assert_eq!(copy.read_with(0, 0.0, 1.0, |p| [p]), [20.25]);
    }

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
            let mut transition = TapTransition::<16>::new(rate);
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
