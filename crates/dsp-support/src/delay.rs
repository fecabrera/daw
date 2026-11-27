//! Fixed-size dry alignment with independent history per processor.
#[derive(Clone, Copy)]
pub struct FixedDelay<T, const N: usize> {
    frames: [T; N],
    cursor: usize,
}

impl<T: Copy + Default, const N: usize> Default for FixedDelay<T, N> {
    fn default() -> Self {
        Self {
            frames: [T::default(); N],
            cursor: 0,
        }
    }
}

impl<T: Copy + Default, const N: usize> FixedDelay<T, N> {
    pub fn sample(&mut self, input: T) -> T {
        if N == 0 {
            return input;
        }
        let dry = self.frames[self.cursor];
        self.frames[self.cursor] = input;
        self.cursor = (self.cursor + 1) % N;
        dry
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
