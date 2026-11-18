//! Prepared scalar rings. Linear reads are convex even when the delay moves.
use daw_reverb_core::DelayRing;

#[derive(Clone)]
pub(crate) struct Delay(DelayRing<1>);
impl Delay {
    pub fn new(frames: usize) -> Self {
        Self(DelayRing::new(frames + 2))
    }
    pub fn read(&self, frames: f64) -> f64 {
        self.0.read(frames)[0]
    }
    pub fn write(&mut self, x: f64) {
        self.0.write([x]);
    }
    pub fn reset(&mut self) {
        self.0.reset();
    }
    pub fn heap_bytes(&self) -> u64 {
        self.0.heap_bytes() as u64
    }
}

/// Fixed-coefficient Schroeder allpass. Both branches use the same current read.
#[derive(Clone)]
pub(crate) struct Diffuser {
    delay: Delay,
    frames: f64,
}
impl Diffuser {
    pub fn new(rate: u32, seconds: f64) -> Self {
        let frames = (f64::from(rate) * seconds).round();
        Self {
            delay: Delay::new(frames as usize),
            frames,
        }
    }
    pub fn sample(&mut self, x: f64) -> f64 {
        let q = self.delay.read(self.frames);
        let v = x - 0.5 * q;
        self.delay.write(v);
        0.5 * v + q
    }
    pub fn reset(&mut self) {
        self.delay.reset();
    }
    pub fn heap_bytes(&self) -> u64 {
        self.delay.heap_bytes()
    }
}
