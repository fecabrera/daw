pub(super) struct Planar {
    pub channels: u32,
    pub data: [Box<[f32]>; 2],
    pub pointers: Box<[*mut f32; 2]>,
}
impl Planar {
    pub fn new(channels: u32, frames: usize) -> Self {
        let mut data = [
            vec![0.0; frames].into_boxed_slice(),
            vec![0.0; frames].into_boxed_slice(),
        ];
        let pointers = Box::new([data[0].as_mut_ptr(), data[1].as_mut_ptr()]);
        Self {
            channels,
            data,
            pointers,
        }
    }
    pub fn input(&mut self, input: Option<&[[f32; 2]]>, frames: usize) {
        for n in 0..frames {
            let sample = input.and_then(|v| v.get(n)).copied().unwrap_or([0.0; 2]);
            self.data[0][n] = if self.channels == 1 {
                (sample[0] + sample[1]) * 0.5
            } else {
                sample[0]
            };
            self.data[1][n] = sample[1];
        }
    }
    pub fn clear(&mut self, frames: usize) {
        self.data[0][..frames].fill(0.0);
        self.data[1][..frames].fill(0.0);
    }
    pub fn output(&self, out: &mut [[f32; 2]]) -> bool {
        for (n, sample) in out.iter_mut().enumerate() {
            *sample = [
                self.data[0][n],
                self.data[usize::from(self.channels != 1)][n],
            ];
            if sample.iter().any(|s| !s.is_finite()) {
                return false;
            }
        }
        true
    }
}
