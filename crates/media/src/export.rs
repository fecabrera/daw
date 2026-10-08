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
}
impl ExportFormat {
    pub const ALL: [Self; 2] = [Self::Wav, Self::Mp3];
    pub fn label(self) -> &'static str {
        match self {
            Self::Wav => "WAV",
            Self::Mp3 => "MP3",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub wav_codec: WavCodec,
    pub mp3_bitrate: Mp3Bitrate,
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
    codec: WavCodec,
    random: [u64; 2],
    pub clipped: bool,
}
impl WavEncoder {
    pub fn create(path: &Path) -> Result<Self> {
        Self::with_codec(path, WavCodec::Pcm24)
    }
    pub fn with_codec(path: &Path, codec: WavCodec) -> Result<Self> {
        let (bits_per_sample, sample_format) = match codec {
            WavCodec::Pcm16 => (16, hound::SampleFormat::Int),
            WavCodec::Pcm24 => (24, hound::SampleFormat::Int),
            WavCodec::Float32 => (32, hound::SampleFormat::Float),
        };
        let writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: SAMPLE_RATE,
                bits_per_sample,
                sample_format,
            },
        )
        .map_err(error)?;
        Ok(Self {
            writer,
            codec,
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
}
impl AudioEncoder for WavEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        finite(frames)?;
        for frame in frames {
            for (channel, sample) in frame.iter().enumerate() {
                self.clipped |= sample.abs() > 1.0;
                if self.codec == WavCodec::Float32 {
                    self.writer.write_sample(*sample).map_err(error)?;
                } else {
                    let scale = if self.codec == WavCodec::Pcm16 {
                        32768.0
                    } else {
                        8_388_608.0
                    };
                    let noise = self.noise(channel) - self.noise(channel);
                    let q = (f64::from(*sample) * scale + noise)
                        .round()
                        .clamp(-scale, scale - 1.0) as i32;
                    self.writer.write_sample(q).map_err(error)?;
                }
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<()> {
        self.writer.finalize().map_err(error)
    }
}
pub struct Mp3Encoder {
    encoder: mp3lame_encoder::Encoder,
    writer: BufWriter<File>,
    pcm: Vec<f32>,
    encoded: Vec<u8>,
    clipped: bool,
}
impl Mp3Encoder {
    fn create(path: &Path, bitrate: Mp3Bitrate) -> Result<Self> {
        let mut builder = mp3lame_encoder::Builder::new()
            .ok_or_else(|| Error("Cannot initialize MP3 encoder".into()))?;
        builder.set_num_channels(2).map_err(error)?;
        builder.set_sample_rate(SAMPLE_RATE).map_err(error)?;
        builder
            .set_output_sample_rate(std::num::NonZeroU32::new(SAMPLE_RATE))
            .map_err(error)?;
        builder.set_brate(bitrate.lame()).map_err(error)?;
        builder
            .set_mode(mp3lame_encoder::Mode::JointStereo)
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
        })
    }
}
impl AudioEncoder for Mp3Encoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        finite(frames)?;
        self.pcm.clear();
        self.pcm.extend(frames.iter().flatten().map(|sample| {
            self.clipped |= sample.abs() > 1.0;
            sample.clamp(-1.0, 1.0)
        }));
        self.encoded.clear();
        self.encoded
            .reserve(mp3lame_encoder::max_required_buffer_size(frames.len()));
        self.encoder
            .encode_to_vec(
                mp3lame_encoder::InterleavedPcm(&self.pcm),
                &mut self.encoded,
            )
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
}
impl ExportEncoder {
    pub fn create(path: &Path, settings: ExportSettings) -> Result<Self> {
        match settings.format {
            ExportFormat::Wav => WavEncoder::with_codec(path, settings.wav_codec).map(Self::Wav),
            ExportFormat::Mp3 => Mp3Encoder::create(path, settings.mp3_bitrate).map(Self::Mp3),
        }
    }
    pub fn clipped(&self) -> bool {
        match self {
            Self::Wav(encoder) => encoder.clipped,
            Self::Mp3(encoder) => encoder.clipped,
        }
    }
}
impl AudioEncoder for ExportEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        match self {
            Self::Wav(encoder) => encoder.write_frames(frames),
            Self::Mp3(encoder) => encoder.write_frames(frames),
        }
    }
    fn finish(self) -> Result<()> {
        match self {
            Self::Wav(encoder) => encoder.finish(),
            Self::Mp3(encoder) => encoder.finish(),
        }
    }
}
