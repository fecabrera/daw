use crate::export_resample::ExportResampler;
use crate::{Error, Result};
use daw_core::SAMPLE_RATE;
use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExportFormat {
    #[default]
    Wav,
    Mp3,
    Aac,
}
impl ExportFormat {
    pub const ALL: [Self; 3] = [Self::Wav, Self::Mp3, Self::Aac];
    pub fn label(self) -> &'static str {
        match self {
            Self::Wav => "WAV",
            Self::Mp3 => "MP3",
            Self::Aac => "AAC",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
        }
    }
    pub fn export_sample_rates(self) -> &'static [u32] {
        match self {
            Self::Wav => &daw_core::SAMPLE_RATES,
            Self::Mp3 | Self::Aac => &[44_100, 48_000],
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WavCodec {
    Pcm16,
    #[default]
    Pcm24,
    Float32,
}
impl WavCodec {
    pub const ALL: [Self; 3] = [Self::Pcm16, Self::Pcm24, Self::Float32];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pcm16 => "PCM 16-bit",
            Self::Pcm24 => "PCM 24-bit",
            Self::Float32 => "IEEE float 32-bit",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mp3Bitrate {
    Kbps128,
    #[default]
    Kbps192,
    Kbps256,
    Kbps320,
}
impl Mp3Bitrate {
    pub const ALL: [Self; 4] = [Self::Kbps128, Self::Kbps192, Self::Kbps256, Self::Kbps320];
    pub fn kbps(self) -> u16 {
        match self {
            Self::Kbps128 => 128,
            Self::Kbps192 => 192,
            Self::Kbps256 => 256,
            Self::Kbps320 => 320,
        }
    }
    fn lame(self) -> mp3lame_encoder::Bitrate {
        match self {
            Self::Kbps128 => mp3lame_encoder::Bitrate::Kbps128,
            Self::Kbps192 => mp3lame_encoder::Bitrate::Kbps192,
            Self::Kbps256 => mp3lame_encoder::Bitrate::Kbps256,
            Self::Kbps320 => mp3lame_encoder::Bitrate::Kbps320,
        }
    }
}
/// AAC and MP3 expose the same target bitrate choices.
pub type AacBitrate = Mp3Bitrate;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExportChannels {
    #[default]
    Stereo,
    Mono,
}
impl ExportChannels {
    pub const ALL: [Self; 2] = [Self::Stereo, Self::Mono];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stereo => "Stereo",
            Self::Mono => "Mono",
        }
    }

    pub(crate) fn count(self) -> u16 {
        match self {
            Self::Stereo => 2,
            Self::Mono => 1,
        }
    }

    /// Map a stereo project frame to the selected output. Mono uses the first sample.
    pub fn mix_frame(self, [left, right]: [f32; 2]) -> [f32; 2] {
        match self {
            Self::Stereo => [left, right],
            Self::Mono => {
                let mono = ((f64::from(left) + f64::from(right)) * 0.5) as f32;
                [mono; 2]
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub wav_codec: WavCodec,
    pub mp3_bitrate: Mp3Bitrate,
    pub aac_bitrate: AacBitrate,
    pub channels: ExportChannels,
    /// None uses the project rate, capped at 48 kHz for MP3 and AAC.
    pub sample_rate_hz: Option<u32>,
}
impl ExportSettings {
    pub fn output_sample_rate(self, project_rate_hz: u32) -> u32 {
        if let Some(rate) = self.sample_rate_hz {
            return rate;
        }
        match self.format {
            ExportFormat::Wav => project_rate_hz,
            ExportFormat::Mp3 | ExportFormat::Aac => project_rate_hz.min(SAMPLE_RATE),
        }
    }
}

pub trait AudioEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()>;
    fn finish(self) -> Result<()>;
}
fn error(error: impl std::fmt::Display) -> Error {
    Error(error.to_string())
}
fn finite(frames: &[[f32; 2]]) -> Result<()> {
    if frames.iter().flatten().any(|sample| !sample.is_finite()) {
        Err(Error("Cannot export non-finite audio samples".into()))
    } else {
        Ok(())
    }
}
pub struct WavEncoder {
    writer: hound::WavWriter<BufWriter<File>>,
    converter: Option<Box<ExportResampler>>,
    codec: WavCodec,
    channels: ExportChannels,
    random: [u64; 2],
    pub clipped: bool,
}
impl WavEncoder {
    pub fn create(path: &Path) -> Result<Self> {
        Self::with_codec(path, WavCodec::Pcm24)
    }
    pub fn with_codec(path: &Path, codec: WavCodec) -> Result<Self> {
        Self::at_sample_rate(path, codec, SAMPLE_RATE)
    }
    pub fn at_sample_rate(path: &Path, codec: WavCodec, sample_rate_hz: u32) -> Result<Self> {
        Self::create_with_channels(path, codec, sample_rate_hz, ExportChannels::Stereo)
    }
    fn create_with_channels(
        path: &Path,
        codec: WavCodec,
        sample_rate_hz: u32,
        channels: ExportChannels,
    ) -> Result<Self> {
        let (bits_per_sample, sample_format) = match codec {
            WavCodec::Pcm16 => (16, hound::SampleFormat::Int),
            WavCodec::Pcm24 => (24, hound::SampleFormat::Int),
            WavCodec::Float32 => (32, hound::SampleFormat::Float),
        };
        let writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: channels.count(),
                sample_rate: sample_rate_hz,
                bits_per_sample,
                sample_format,
            },
        )
        .map_err(error)?;
        Ok(Self {
            writer,
            converter: None,
            codec,
            channels,
            random: [0x9e3779b97f4a7c15, 0xd1b54a32d192ed03],
            clipped: false,
        })
    }
    fn noise(&mut self, channel: usize) -> f64 {
        let x = &mut self.random[channel];
        *x ^= *x << 13;
        *x ^= *x >> 7;
        *x ^= *x << 17;
        (*x >> 11) as f64 / ((1u64 << 53) as f64)
    }
    fn write_frame(&mut self, frame: [f32; 2]) -> Result<()> {
        finite(std::slice::from_ref(&frame))?;
        for (channel, sample) in frame
            .into_iter()
            .take(usize::from(self.channels.count()))
            .enumerate()
        {
            self.clipped |= sample.abs() > 1.0;
            if self.codec == WavCodec::Float32 {
                self.writer.write_sample(sample).map_err(error)?;
            } else {
                let scale = if self.codec == WavCodec::Pcm16 {
                    32768.0
                } else {
                    8_388_608.0
                };
                let noise = self.noise(channel) - self.noise(channel);
                let q = (f64::from(sample) * scale + noise)
                    .round()
                    .clamp(-scale, scale - 1.0) as i32;
                self.writer.write_sample(q).map_err(error)?;
            }
        }
        Ok(())
    }

    fn finish_with_clipping(mut self) -> Result<bool> {
        if let Some(mut converter) = self.converter.take() {
            converter.finish(|frame| self.write_frame(frame))?;
        }
        self.writer.finalize().map_err(error)?;
        Ok(self.clipped)
    }
}
impl AudioEncoder for WavEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        finite(frames)?;
        let channels = self.channels;
        if let Some(mut converter) = self.converter.take() {
            let result = converter.write(
                frames.iter().map(|&frame| channels.mix_frame(frame)),
                |frame| self.write_frame(frame),
            );
            self.converter = Some(converter);
            return result;
        }
        for &frame in frames {
            self.write_frame(channels.mix_frame(frame))?;
        }
        Ok(())
    }
    fn finish(self) -> Result<()> {
        self.finish_with_clipping().map(|_| ())
    }
}
pub struct Mp3Encoder {
    encoder: mp3lame_encoder::Encoder,
    writer: BufWriter<File>,
    pcm: Vec<f32>,
    encoded: Vec<u8>,
    clipped: bool,
    channels: ExportChannels,
}
impl Mp3Encoder {
    fn create(
        path: &Path,
        bitrate: Mp3Bitrate,
        input_rate: u32,
        output_rate: u32,
        channels: ExportChannels,
    ) -> Result<Self> {
        let mut builder = mp3lame_encoder::Builder::new()
            .ok_or_else(|| Error("Cannot initialize MP3 encoder".into()))?;
        builder
            .set_num_channels(channels.count() as u8)
            .map_err(error)?;
        builder.set_sample_rate(input_rate).map_err(error)?;
        builder
            .set_output_sample_rate(std::num::NonZeroU32::new(output_rate))
            .map_err(error)?;
        builder.set_brate(bitrate.lame()).map_err(error)?;
        builder
            .set_mode(match channels {
                ExportChannels::Stereo => mp3lame_encoder::Mode::JointStereo,
                ExportChannels::Mono => mp3lame_encoder::Mode::Mono,
            })
            .map_err(error)?;
        builder
            .set_quality(mp3lame_encoder::Quality::Best)
            .map_err(error)?;
        builder
            .set_vbr_mode(mp3lame_encoder::VbrMode::Off)
            .map_err(error)?;
        builder.set_to_write_vbr_tag(true).map_err(error)?;
        let encoder = builder.build().map_err(error)?;
        Ok(Self {
            encoder,
            writer: BufWriter::new(File::create(path).map_err(error)?),
            pcm: Vec::new(),
            encoded: Vec::new(),
            clipped: false,
            channels,
        })
    }
}
impl AudioEncoder for Mp3Encoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        finite(frames)?;
        self.pcm.clear();
        let channels = self.channels;
        self.pcm.extend(
            frames
                .iter()
                .flat_map(|&frame| {
                    channels
                        .mix_frame(frame)
                        .into_iter()
                        .take(usize::from(channels.count()))
                })
                .map(|sample| {
                    self.clipped |= sample.abs() > 1.0;
                    sample.clamp(-1.0, 1.0)
                }),
        );
        self.encoded.clear();
        self.encoded
            .reserve(mp3lame_encoder::max_required_buffer_size(frames.len()));
        match self.channels {
            ExportChannels::Stereo => self.encoder.encode_to_vec(
                mp3lame_encoder::InterleavedPcm(&self.pcm),
                &mut self.encoded,
            ),
            ExportChannels::Mono => self
                .encoder
                .encode_to_vec(mp3lame_encoder::MonoPcm(&self.pcm), &mut self.encoded),
        }
        .map_err(error)?;
        self.writer.write_all(&self.encoded).map_err(error)
    }
    fn finish(mut self) -> Result<()> {
        self.encoded.clear();
        self.encoded.reserve(7200);
        self.encoder
            .flush_to_vec::<mp3lame_encoder::FlushGap>(&mut self.encoded)
            .map_err(error)?;
        self.writer.write_all(&self.encoded).map_err(error)?;
        // Replace the reserved first frame with seek/duration and encoder delay/padding metadata.
        self.encoded.clear();
        self.encoded.reserve(self.encoder.lame_tag_size());
        self.encoder
            .lame_tag_encode_to_vec(&mut self.encoded)
            .ok_or_else(|| Error("Cannot finalize MP3 duration metadata".into()))?;
        self.writer.seek(SeekFrom::Start(0)).map_err(error)?;
        self.writer.write_all(&self.encoded).map_err(error)?;
        self.writer.flush().map_err(error)
    }
}
pub enum ExportEncoder {
    Wav(WavEncoder),
    Mp3(Mp3Encoder),
    Aac(Box<crate::aac_export::AacEncoder>),
}
impl ExportEncoder {
    pub fn create(path: &Path, settings: ExportSettings) -> Result<Self> {
        Self::at_sample_rate(path, settings, SAMPLE_RATE)
    }
    pub fn at_sample_rate(path: &Path, settings: ExportSettings, input_rate: u32) -> Result<Self> {
        if settings
            .sample_rate_hz
            .is_some_and(|rate| !settings.format.export_sample_rates().contains(&rate))
        {
            return Err(Error(
                "Unsupported output sample rate for the export format".into(),
            ));
        }
        let output_rate = settings.output_sample_rate(input_rate);
        let codec = match settings.format {
            ExportFormat::Wav => {
                let resampler = ExportResampler::new(input_rate, output_rate)?.map(Box::new);
                let mut encoder = WavEncoder::create_with_channels(
                    path,
                    settings.wav_codec,
                    output_rate,
                    settings.channels,
                )?;
                encoder.converter = resampler;
                Self::Wav(encoder)
            }
            ExportFormat::Mp3 => Self::Mp3(Mp3Encoder::create(
                path,
                settings.mp3_bitrate,
                input_rate,
                output_rate,
                settings.channels,
            )?),
            ExportFormat::Aac => Self::Aac(Box::new(crate::aac_export::AacEncoder::create(
                path,
                settings.aac_bitrate.kbps(),
                input_rate,
                output_rate,
                settings.channels,
            )?)),
        };
        Ok(codec)
    }
    pub fn clipped(&self) -> bool {
        match self {
            Self::Wav(encoder) => encoder.clipped,
            Self::Mp3(encoder) => encoder.clipped,
            Self::Aac(encoder) => encoder.clipped,
        }
    }

    /// Flush the converters and encoder before reporting output clipping.
    pub fn finish_with_clipping(self) -> Result<bool> {
        match self {
            Self::Wav(encoder) => encoder.finish_with_clipping(),
            Self::Mp3(encoder) => {
                let clipped = encoder.clipped;
                encoder.finish()?;
                Ok(clipped)
            }
            Self::Aac(encoder) => encoder.finish_with_clipping(),
        }
    }
}
impl AudioEncoder for ExportEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        match self {
            Self::Wav(encoder) => encoder.write_frames(frames),
            Self::Mp3(encoder) => encoder.write_frames(frames),
            Self::Aac(encoder) => encoder.write_frames(frames),
        }
    }
    fn finish(self) -> Result<()> {
        self.finish_with_clipping().map(|_| ())
    }
}
