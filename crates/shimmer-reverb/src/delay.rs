//! Prepared scalar rings. Linear reads are convex even when the delay moves.
#[derive(Clone)]
pub(crate) struct Delay {
    data: Vec<f64>,
    cursor: usize,
    valid: usize,
}
impl Delay {
    pub fn new(frames: usize) -> Self {
        Self {
            data: vec![0.0; frames + 2],
            cursor: 0,
            valid: 0,
        }
    }
    pub fn read(&self, frames: f64) -> f64 {
        let d = frames.clamp(1.0, (self.data.len() - 2) as f64);
        let n = d.floor() as usize;
        let f = d - n as f64;
        let a = (self.cursor + self.data.len() - n) % self.data.len();
        let b = (a + self.data.len() - 1) % self.data.len();
        let x = if n <= self.valid { self.data[a] } else { 0.0 };
        let y = if n < self.valid { self.data[b] } else { 0.0 };
        x * (1.0 - f) + y * f
    }
    pub fn write(&mut self, x: f64) {
        self.data[self.cursor] = x;
        self.cursor = (self.cursor + 1) % self.data.len();
        self.valid = (self.valid + 1).min(self.data.len());
    }
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.valid = 0;
    }
    pub fn heap_bytes(&self) -> u64 {
        (self.data.capacity() * size_of::<f64>()) as u64
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
