use crate::{AudioReservation, Error, Result, resampled_frames, stereo_fft::StereoFft};

/// Streaming conversion of the rendered project clock into the file clock.
pub struct ExportResampler {
    converter: StereoFft,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    filled: usize,
    input_frames: u64,
    output_frames: u64,
    input_rate: u32,
    output_rate: u32,
    _reservation: AudioReservation,
}

impl ExportResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Option<Self>> {
        if input_rate == output_rate {
            return Ok(None);
        }
        let reservation =
            AudioReservation::new(0, crate::resample_scratch(input_rate, output_rate)?)?;
        let converter =
            StereoFft::new(input_rate, output_rate).map_err(|e| Error(e.to_string()))?;
        Ok(Some(Self {
            input: converter.input_buffer(),
            output: converter.output_buffer(),
            converter,
            filled: 0,
            input_frames: 0,
            output_frames: 0,
            input_rate,
            output_rate,
            _reservation: reservation,
        }))
    }

    pub fn write(
        &mut self,
        frames: impl ExactSizeIterator<Item = [f32; 2]>,
        mut sink: impl FnMut([f32; 2]) -> Result<()>,
    ) -> Result<()> {
        self.input_frames = self
            .input_frames
            .checked_add(frames.len() as u64)
            .ok_or_else(|| Error("Export length overflow".into()))?;
        let target = resampled_frames(self.input_frames, self.input_rate, self.output_rate)?;
        for [left, right] in frames {
            self.input[0][self.filled] = left;
            self.input[1][self.filled] = right;
            self.filled += 1;
            if self.filled == self.input[0].len() {
                self.convert(target, &mut sink)?;
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, mut sink: impl FnMut([f32; 2]) -> Result<()>) -> Result<()> {
        let target = resampled_frames(self.input_frames, self.input_rate, self.output_rate)?;
        while self.output_frames < target {
            for channel in &mut self.input {
                channel[self.filled..].fill(0.0);
            }
            self.convert(target, &mut sink)?;
        }
        Ok(())
    }

    fn convert(&mut self, target: u64, mut sink: impl FnMut([f32; 2]) -> Result<()>) -> Result<()> {
        let window = self
            .converter
            .process(
                &self.input,
                &mut self.output,
                target.saturating_sub(self.output_frames),
            )
            .map_err(|e| Error(e.to_string()))?;
        self.output_frames += window.len() as u64;
        for index in window {
            sink([self.output[0][index], self.output[1][index]])?;
        }
        self.filled = 0;
        Ok(())
    }
}
