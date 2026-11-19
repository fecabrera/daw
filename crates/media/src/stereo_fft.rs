use rubato::Resampler;
use std::ops::Range;

/// Offline stereo converter with a cursor for the leading delay.
pub(crate) struct StereoFft {
    converter: rubato::FftFixedIn<f32>,
    delay: usize,
}

impl StereoFft {
    pub fn new(
        input_rate: u32,
        output_rate: u32,
    ) -> Result<Self, rubato::ResamplerConstructionError> {
        let converter =
            rubato::FftFixedIn::new(input_rate as usize, output_rate as usize, 1024, 2, 2)?;
        let delay = converter.output_delay();
        Ok(Self { converter, delay })
    }

    pub fn input_frames(&self) -> usize {
        self.converter.input_frames_next()
    }

    pub fn input_buffer(&self) -> Vec<Vec<f32>> {
        self.converter.input_buffer_allocate(true)
    }

    pub fn output_buffer(&self) -> Vec<Vec<f32>> {
        self.converter.output_buffer_allocate(true)
    }

    pub fn process(
        &mut self,
        input: &[Vec<f32>],
        output: &mut [Vec<f32>],
        maximum: u64,
    ) -> Result<Range<usize>, rubato::ResampleError> {
        let (_, count) = self.converter.process_into_buffer(input, output, None)?;
        let skip = self.delay.min(count);
        self.delay -= skip;
        let keep = (count - skip).min(usize::try_from(maximum).unwrap_or(usize::MAX));
        Ok(skip..skip + keep)
    }
}
