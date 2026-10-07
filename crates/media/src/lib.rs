use daw_core::{SAMPLE_RATE, SourceMetadata};
use rubato::Resampler;
use std::{fs::File, path::Path, sync::Arc};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct AudioData {
    pub samples: Arc<Vec<[f32; 2]>>,
    pub metadata: SourceMetadata,
    pub peaks: Arc<Vec<[[f32; 2]; 2]>>,
}
pub trait AudioDecoder: Send + Sync {
    fn decode(&self, path: &Path) -> Result<AudioData>;
}
pub struct WavDecoder;
impl AudioDecoder for WavDecoder {
    fn decode(&self, path: &Path) -> Result<AudioData> {
        decode_wav(path)
    }
}

pub fn decode_wav(path: &Path) -> Result<AudioData> {
    let err = |e: &dyn std::fmt::Display| Error(format!("{}: {e}", path.display()));
    let reader = hound::WavReader::open(path).map_err(|e| err(&e))?;
    let spec = reader.spec();
    let declared_frames = reader.duration() as usize;
    if !(1..=2).contains(&spec.channels)
        || spec.sample_rate == 0
        || !matches!(
            (spec.sample_format, spec.bits_per_sample),
            (hound::SampleFormat::Int, 16 | 24) | (hound::SampleFormat::Float, 32)
        )
    {
        return Err(Error(format!(
            "{}: supported WAV formats are mono/stereo 16/24-bit PCM or 32-bit float",
            path.display()
        )));
    }
    let metadata = SourceMetadata {
        sample_rate_hz: spec.sample_rate,
        channels: spec.channels,
        sample_format: if spec.sample_format == hound::SampleFormat::Float {
            "ieee_float"
        } else {
            "pcm_int"
        }
        .into(),
        bits_per_sample: spec.bits_per_sample,
    };
    let file = File::open(path).map_err(|e| err(&e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| err(&e))?
        .format;
    let track = format
        .default_track()
        .ok_or_else(|| Error("WAV contains no audio track".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| err(&e))?;
    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(err(&e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio = decoder.decode(&packet).map_err(|e| err(&e))?;
        let mut buffer = SampleBuffer::<f32>::new(audio.capacity() as u64, *audio.spec());
        buffer.copy_interleaved_ref(audio);
        for frame in buffer.samples().chunks_exact(usize::from(spec.channels)) {
            let l = frame[0];
            let r = if spec.channels == 1 { l } else { frame[1] };
            if !l.is_finite() || !r.is_finite() {
                return Err(Error("Source contains non-finite samples".into()));
            }
            samples.push([l, r]);
        }
    }
    if samples.is_empty() {
        return Err(Error("Source contains no audio samples".into()));
    }
    if samples.len() != declared_frames {
        return Err(Error(format!(
            "{}: incomplete WAV sample data",
            path.display()
        )));
    }
    if spec.sample_rate != SAMPLE_RATE {
        samples = resample(&samples, spec.sample_rate, SAMPLE_RATE)?;
    }
    let peaks = samples
        .chunks(256)
        .map(|chunk| {
            let mut result = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
            for s in chunk {
                for ch in 0..2 {
                    result[ch][0] = result[ch][0].min(s[ch]);
                    result[ch][1] = result[ch][1].max(s[ch]);
                }
            }
            result
        })
        .collect();
    Ok(AudioData {
        samples: Arc::new(samples),
        metadata,
        peaks: Arc::new(peaks),
    })
}

pub fn resample(samples: &[[f32; 2]], input_rate: u32, output_rate: u32) -> Result<Vec<[f32; 2]>> {
    let mut converter =
        rubato::FftFixedIn::<f32>::new(input_rate as usize, output_rate as usize, 1024, 2, 2)
            .map_err(|e| Error(e.to_string()))?;
    let input_size = converter.input_frames_next();
    let delay = converter.output_delay();
    let target =
        (samples.len() as u64 * u64::from(output_rate)).div_ceil(u64::from(input_rate)) as usize;
    let mut out = Vec::new();
    let mut position = 0;
    while out.len() < target + delay {
        let mut input = vec![vec![0.0; input_size]; 2];
        for (channel, buffer) in input.iter_mut().enumerate() {
            for (i, value) in buffer.iter_mut().enumerate() {
                if let Some(s) = samples.get(position + i) {
                    *value = s[channel];
                }
            }
        }
        let block = converter
            .process(&input, None)
            .map_err(|e| Error(e.to_string()))?;
        out.extend(block[0].iter().zip(&block[1]).map(|(l, r)| [*l, *r]));
        position += input_size;
    }
    Ok(out[delay..delay + target].to_vec())
}

pub trait AudioEncoder {
    fn write_frames(&mut self, frames: &[[f32; 2]]) -> Result<()>;
    fn finish(self) -> Result<()>;
}
pub struct WavEncoder {
    writer: hound::WavWriter<std::io::BufWriter<File>>,
    random: [u64; 2],
    pub clipped: bool,
}
impl WavEncoder {
    pub fn create(path: &Path) -> Result<Self> {
        let writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: SAMPLE_RATE,
                bits_per_sample: 24,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| Error(e.to_string()))?;
        Ok(Self {
            writer,
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
        for frame in frames {
            for (channel, sample) in frame.iter().enumerate() {
                self.clipped |= sample.abs() > 1.0;
                let noise = self.noise(channel) - self.noise(channel);
                let q = (f64::from(*sample) * 8_388_608.0 + noise)
                    .round()
                    .clamp(-8_388_608.0, 8_388_607.0) as i32;
                self.writer
                    .write_sample(q)
                    .map_err(|e| Error(e.to_string()))?;
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<()> {
        self.writer.finalize().map_err(|e| Error(e.to_string()))
    }
}
