//! Bounded worker-side AAC-LC/ADTS export. The codec keeps history across writes.
use crate::{AudioEncoder, AudioReservation, Error, Result, resampled_frames};
use oxideav_aac::encoder::{EncoderConfig, StreamEncoder};
use rubato::Resampler;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct AacEncoder {
    encoder: StreamEncoder,
    writer: BufWriter<File>,
    pcm: Vec<i16>,
    converter: Option<rubato::FftFixedIn<f32>>,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    input_filled: usize,
    input_frames: u64,
    output_frames: u64,
    delay: usize,
    input_rate: u32,
    output_rate: u32,
    pub(super) clipped: bool,
    _reservation: AudioReservation,
}
impl AacEncoder {
    pub(super) fn create(path: &Path, bitrate: u16, input_rate: u32) -> Result<Self> {
        if !daw_core::SAMPLE_RATES.contains(&input_rate) {
            return Err(Error("Unsupported AAC input sample rate".into()));
        }
        let output_rate = input_rate.min(daw_core::SAMPLE_RATE);
        let reservation = AudioReservation::new(
            0,
            crate::resample_scratch(input_rate, output_rate)? + 8 * 1024 * 1024,
        )?;
        let converter = if input_rate != output_rate {
            Some(
                rubato::FftFixedIn::new(input_rate as usize, output_rate as usize, 1024, 2, 2)
                    .map_err(error)?,
            )
        } else {
            None
        };
        let input = converter
            .as_ref()
            .map_or_else(Vec::new, |c| c.input_buffer_allocate(true));
        let output = converter
            .as_ref()
            .map_or_else(Vec::new, |c| c.output_buffer_allocate(true));
        let delay = converter.as_ref().map_or(0, |c| c.output_delay());
        let encoder = StreamEncoder::new(EncoderConfig {
            sample_rate: output_rate,
            channels: 2,
            bitrate: u32::from(bitrate) * 1000,
        })
        .map_err(error)?;
        Ok(Self {
            encoder,
            writer: BufWriter::new(File::create(path).map_err(error)?),
            pcm: Vec::with_capacity(2048),
            converter,
            input,
            output,
            input_filled: 0,
            input_frames: 0,
            output_frames: 0,
            delay,
            input_rate,
            output_rate,
            clipped: false,
            _reservation: reservation,
        })
    }
    fn push_frame(&mut self, frame: [f32; 2]) -> Result<()> {
        for sample in frame {
            self.clipped |= sample.abs() > 1.0;
            self.pcm.push(
                (sample.clamp(-1.0, 1.0) * 32768.0)
                    .round()
                    .clamp(-32768.0, 32767.0) as i16,
            );
        }
        if self.pcm.len() == 2048 {
            self.writer
                .write_all(&self.encoder.encode_frame(&self.pcm).map_err(error)?)
                .map_err(error)?;
            self.pcm.clear();
        }
        self.output_frames += 1;
        Ok(())
    }
    fn convert_block(&mut self, target: u64) -> Result<()> {
        let (_, count) = self
            .converter
            .as_mut()
            .unwrap()
            .process_into_buffer(&self.input, &mut self.output, None)
            .map_err(error)?;
        let skip = self.delay.min(count);
        self.delay -= skip;
        let keep = (count - skip)
            .min(usize::try_from(target.saturating_sub(self.output_frames)).unwrap_or(usize::MAX));
        for i in skip..skip + keep {
            self.push_frame([self.output[0][i], self.output[1][i]])?;
        }
        self.input_filled = 0;
        Ok(())
    }
}
impl AudioEncoder for AacEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        if frames.iter().flatten().any(|s| !s.is_finite()) {
            return Err(Error("Cannot export non-finite audio samples".into()));
        }
        self.input_frames = self
            .input_frames
            .checked_add(frames.len() as u64)
            .ok_or_else(|| Error("AAC export length overflow".into()))?;
        let target = resampled_frames(self.input_frames, self.input_rate, self.output_rate)?;
        for frame in frames {
            self.clipped |= frame.iter().any(|s| s.abs() > 1.0);
            if self.converter.is_some() {
                for (channel, sample) in frame.iter().enumerate() {
                    self.input[channel][self.input_filled] = sample.clamp(-1.0, 1.0);
                }
                self.input_filled += 1;
                if self.input_filled == self.input[0].len() {
                    self.convert_block(target)?;
                }
            } else {
                self.push_frame(*frame)?;
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<()> {
        let target = resampled_frames(self.input_frames, self.input_rate, self.output_rate)?;
        while self.converter.is_some() && self.output_frames < target {
            for channel in &mut self.input {
                channel[self.input_filled..].fill(0.0);
            }
            self.convert_block(target)?;
        }
        if !self.pcm.is_empty() {
            self.writer
                .write_all(&self.encoder.encode_frame(&self.pcm).map_err(error)?)
                .map_err(error)?;
        }
        if self.input_frames > 0 {
            self.writer
                .write_all(&self.encoder.finish().map_err(error)?)
                .map_err(error)?;
        }
        self.writer.flush().map_err(error)
    }
}
fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("AAC export: {e}"))
}
