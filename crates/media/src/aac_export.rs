//! Bounded worker-side AAC-LC/ADTS export. The codec keeps history across writes.
use crate::{
    AudioEncoder, AudioReservation, Error, ExportChannels, Result, export_resample::ExportResampler,
};
use oxideav_aac::encoder::{EncoderConfig, StreamEncoder};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct AacEncoder {
    encoder: StreamEncoder,
    writer: BufWriter<File>,
    pcm: Vec<i16>,
    converter: Option<ExportResampler>,
    channels: ExportChannels,
    wrote_audio: bool,
    pub(super) clipped: bool,
    _reservation: AudioReservation,
}
impl AacEncoder {
    pub(super) fn create(
        path: &Path,
        bitrate: u16,
        input_rate: u32,
        output_rate: u32,
        channels: ExportChannels,
    ) -> Result<Self> {
        if !daw_core::SAMPLE_RATES.contains(&input_rate) {
            return Err(Error("Unsupported AAC input sample rate".into()));
        }
        let reservation = AudioReservation::new(0, 8 * 1024 * 1024)?;
        let converter = ExportResampler::new(input_rate, output_rate)?;
        let encoder = StreamEncoder::new(EncoderConfig {
            sample_rate: output_rate,
            channels: channels.count() as u8,
            bitrate: u32::from(bitrate) * 1000,
        })
        .map_err(error)?;
        Ok(Self {
            encoder,
            writer: BufWriter::new(File::create(path).map_err(error)?),
            pcm: Vec::with_capacity(2048),
            converter,
            channels,
            wrote_audio: false,
            clipped: false,
            _reservation: reservation,
        })
    }
    fn push_frame(&mut self, frame: [f32; 2]) -> Result<()> {
        for sample in frame.into_iter().take(usize::from(self.channels.count())) {
            self.clipped |= sample.abs() > 1.0;
            self.pcm.push(
                (sample.clamp(-1.0, 1.0) * 32768.0)
                    .round()
                    .clamp(-32768.0, 32767.0) as i16,
            );
        }
        if self.pcm.len() == 1024 * usize::from(self.channels.count()) {
            self.writer
                .write_all(&self.encoder.encode_frame(&self.pcm).map_err(error)?)
                .map_err(error)?;
            self.pcm.clear();
        }
        Ok(())
    }
    pub(super) fn finish_with_clipping(mut self) -> Result<bool> {
        if let Some(mut converter) = self.converter.take() {
            converter.finish(|frame| self.push_frame(frame))?;
        }
        if !self.pcm.is_empty() {
            self.writer
                .write_all(&self.encoder.encode_frame(&self.pcm).map_err(error)?)
                .map_err(error)?;
        }
        if self.wrote_audio {
            self.writer
                .write_all(&self.encoder.finish().map_err(error)?)
                .map_err(error)?;
        }
        self.writer.flush().map_err(error)?;
        Ok(self.clipped)
    }
}
impl AudioEncoder for AacEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        if frames.iter().flatten().any(|s| !s.is_finite()) {
            return Err(Error("Cannot export non-finite audio samples".into()));
        }
        self.wrote_audio |= !frames.is_empty();
        let channels = self.channels;
        if let Some(mut converter) = self.converter.take() {
            let mut input_clipped = false;
            let inputs = frames.iter().map(|&frame| {
                channels.mix_frame(frame).map(|sample| {
                    input_clipped |= sample.abs() > 1.0;
                    sample.clamp(-1.0, 1.0)
                })
            });
            let result = converter.write(inputs, |frame| self.push_frame(frame));
            self.clipped |= input_clipped;
            self.converter = Some(converter);
            result?;
        } else {
            for &frame in frames {
                self.push_frame(channels.mix_frame(frame))?;
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<()> {
        self.finish_with_clipping().map(|_| ())
    }
}
fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("AAC export: {e}"))
}
