use daw_core::{SAMPLE_RATE, SourceMetadata};
use rubato::Resampler;
use std::{fs::File, path::Path, sync::Arc};

pub use memory::{AudioMemoryUsage, audio_memory_bytes};
mod aac_export;
mod cache;
mod source_revision;
pub use source_revision::{SourceRevision, verify_prepared_source};
mod decode;
pub use decode::{IMPORT_EXTENSIONS, decode_audio, decode_audio_at_rate, decode_audio_cancellable};
mod clip_audio;
pub use clip_audio::{ClipAudioCache, ClipAudioKey, clip_audio};
mod export;
mod memory;
pub use cache::decode_audio_cached;
pub use cache::decode_audio_cached as decode_wav_cached;
pub use memory::{AUDIO_MEMORY_BUDGET_BYTES, AudioReservation, register_audio};
mod sampler;
mod stretch;
pub use export::{
    AacBitrate, AudioEncoder, ExportEncoder, ExportFormat, ExportSettings, Mp3Bitrate, WavCodec,
    WavEncoder,
};
pub use sampler::PreparedSampler;
pub use stretch::{PreparedStretch, prepared_audio, stretch_audio, stretch_audio_with_engine};
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
    /// Rate of the prepared samples; metadata retains the original source rate.
    pub sample_rate_hz: u32,
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
    decode_wav_at_rate(path, SAMPLE_RATE)
}

pub fn decode_wav_at_rate(path: &Path, sample_rate_hz: u32) -> Result<AudioData> {
    decode_wav_cancellable(path, sample_rate_hz, &|| false)
}

/// Exact ceiling-rounded frame count shared by preparation and resampling.
pub fn resampled_frames(frames: u64, input_rate: u32, output_rate: u32) -> Result<u64> {
    if input_rate == 0 || output_rate == 0 {
        return Err(Error("Invalid resampling rate".into()));
    }
    u64::try_from((u128::from(frames) * u128::from(output_rate)).div_ceil(u128::from(input_rate)))
        .map_err(|_| Error("Resampled audio exceeds supported bounds".into()))
}

/// Decode on a worker. Cancellation is checked between packets, conversion blocks and peaks.
pub fn decode_wav_cancellable(
    path: &Path,
    sample_rate_hz: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Audio preparation cancelled".into()));
    }
    if !daw_core::SAMPLE_RATES.contains(&sample_rate_hz) {
        return Err(Error("Unsupported processing sample rate".into()));
    }
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
    let target_frames = resampled_frames(declared_frames as u64, spec.sample_rate, sample_rate_hz)?;
    let scratch = if spec.sample_rate == sample_rate_hz {
        0
    } else {
        (declared_frames as u64).saturating_mul(8)
    };
    let _reservation = AudioReservation::new(
        target_frames,
        scratch.saturating_add(resample_scratch(spec.sample_rate, sample_rate_hz)?),
    )?;
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
    samples
        .try_reserve_exact(declared_frames)
        .map_err(|e| err(&e))?;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        if cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
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
        let capacity = audio.capacity() * usize::from(spec.channels);
        if buffer.as_ref().is_none_or(|b| b.capacity() < capacity) {
            buffer = Some(SampleBuffer::<f32>::new(
                audio.capacity() as u64,
                *audio.spec(),
            ));
        }
        let buffer = buffer.as_mut().unwrap();
        buffer.copy_interleaved_ref(audio);
        for frame in buffer.samples().chunks_exact(usize::from(spec.channels)) {
            let l = frame[0];
            let r = if spec.channels == 1 { l } else { frame[1] };
            if !l.is_finite() || !r.is_finite() {
                return Err(Error("Source contains non-finite samples".into()));
            }
            if samples.len() == declared_frames {
                return Err(Error("WAV sample data exceeds its declared length".into()));
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
    if spec.sample_rate != sample_rate_hz {
        samples = convert_samples(&samples, spec.sample_rate, sample_rate_hz, cancelled)?;
    }
    if cancelled() {
        return Err(Error("Audio preparation cancelled".into()));
    }
    let peaks = prepare_peaks(&samples, cancelled)?;
    let audio = AudioData {
        sample_rate_hz,
        samples: Arc::new(samples),
        metadata,
        peaks: Arc::new(peaks),
    };
    register_audio(&audio);
    Ok(audio)
}

pub fn resample(samples: &[[f32; 2]], input_rate: u32, output_rate: u32) -> Result<Vec<[f32; 2]>> {
    resample_cancellable(samples, input_rate, output_rate, &|| false)
}
pub fn resample_cancellable(
    samples: &[[f32; 2]],
    input_rate: u32,
    output_rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<[f32; 2]>> {
    let scratch = resample_scratch(input_rate, output_rate)?;
    let frames = (samples.len() as u64)
        .checked_mul(u64::from(output_rate))
        .map(|n| n.div_ceil(u64::from(input_rate)))
        .ok_or_else(|| Error("Resampled audio exceeds supported bounds".into()))?;
    let _reservation = AudioReservation::new(
        frames,
        scratch.saturating_add((samples.len() as u64).saturating_mul(8)),
    )?;
    convert_samples(samples, input_rate, output_rate, cancelled)
}
fn convert_samples(
    samples: &[[f32; 2]],
    input_rate: u32,
    output_rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<[f32; 2]>> {
    if cancelled() {
        return Err(Error("Audio preparation cancelled".into()));
    }
    if input_rate == 0 || output_rate == 0 || samples.iter().flatten().any(|s| !s.is_finite()) {
        return Err(Error("Invalid resampling input".into()));
    }
    if input_rate == output_rate {
        return Ok(samples.to_vec());
    }
    let mut converter =
        rubato::FftFixedIn::<f32>::new(input_rate as usize, output_rate as usize, 1024, 2, 2)
            .map_err(|e| Error(e.to_string()))?;
    let input_size = converter.input_frames_next();
    let delay = converter.output_delay();
    let target = usize::try_from(resampled_frames(
        samples.len() as u64,
        input_rate,
        output_rate,
    )?)
    .map_err(|_| Error("Resampled audio exceeds supported bounds".into()))?;
    let mut out = Vec::new();
    out.try_reserve_exact(target)
        .map_err(|e| Error(format!("Cannot reserve resampled audio: {e}")))?;
    let mut input = converter.input_buffer_allocate(true);
    let mut block = converter.output_buffer_allocate(true);
    let mut position = 0;
    let mut produced = 0usize;
    while out.len() < target {
        if cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
        for (channel, buffer) in input.iter_mut().enumerate() {
            for (i, value) in buffer.iter_mut().enumerate() {
                *value = samples.get(position + i).map_or(0.0, |s| s[channel]);
            }
        }
        let (_, frames) = converter
            .process_into_buffer(&input, &mut block, None)
            .map_err(|e| Error(e.to_string()))?;
        let first = delay.saturating_sub(produced).min(frames);
        let count = (frames - first).min(target - out.len());
        out.extend((first..first + count).map(|i| [block[0][i], block[1][i]]));
        produced += frames;
        position += input_size;
    }
    Ok(out)
}

impl AudioData {
    pub fn at_sample_rate(&self, sample_rate_hz: u32) -> Result<Self> {
        if self.sample_rate_hz == sample_rate_hz {
            return Ok(self.clone());
        }
        if self.sample_rate_hz == 0 || !daw_core::SAMPLE_RATES.contains(&sample_rate_hz) {
            return Err(Error("Unsupported processing sample rate".into()));
        }
        register_audio(self);
        let target = (self.samples.len() as u64)
            .checked_mul(u64::from(sample_rate_hz))
            .map(|n| n.div_ceil(u64::from(self.sample_rate_hz)))
            .ok_or_else(|| Error("Audio preparation exceeds supported bounds".into()))?;
        let _reservation = AudioReservation::new(
            target,
            resample_scratch(self.sample_rate_hz, sample_rate_hz)?,
        )?;
        let samples = convert_samples(&self.samples, self.sample_rate_hz, sample_rate_hz, &|| {
            false
        })?;
        let peaks = prepare_peaks(&samples, &|| false)?;
        let audio = Self {
            sample_rate_hz,
            samples: Arc::new(samples),
            metadata: self.metadata.clone(),
            peaks: Arc::new(peaks),
        };
        register_audio(&audio);
        Ok(audio)
    }
}

// Conservative setup estimate for the pinned FftFixedIn converter: rational-rate
// FFT blocks plus planning/scratch allowance. Reject pathological WAV rates before construction.
fn resample_scratch(input_rate: u32, output_rate: u32) -> Result<u64> {
    if input_rate == 0 || output_rate == 0 {
        return Err(Error("Invalid resampling rate".into()));
    }
    if input_rate == output_rate {
        return Ok(16 * 1024 * 1024);
    }
    let (mut a, mut b) = (u64::from(input_rate), u64::from(output_rate));
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let input = u64::from(input_rate) / a;
    let output = u64::from(output_rate) / a;
    let blocks = 512_u64.div_ceil(input);
    let bytes = (input + output)
        .saturating_mul(blocks)
        .saturating_mul(256)
        .saturating_add(16 * 1024 * 1024);
    if bytes > AUDIO_MEMORY_BUDGET_BYTES {
        return Err(Error(
            "Resampling setup exceeds the audio memory budget".into(),
        ));
    }
    Ok(bytes)
}

fn prepare_peaks(
    samples: &[[f32; 2]],
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<[[f32; 2]; 2]>> {
    samples
        .chunks(256)
        .map(|chunk| {
            if cancelled() {
                return Err(Error("Audio preparation cancelled".into()));
            }
            let mut peak = [[f32::INFINITY, f32::NEG_INFINITY]; 2];
            for sample in chunk {
                for ch in 0..2 {
                    peak[ch][0] = peak[ch][0].min(sample[ch]);
                    peak[ch][1] = peak[ch][1].max(sample[ch]);
                }
            }
            Ok(peak)
        })
        .collect()
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn resampled_frame_counts_use_checked_ceiling_arithmetic() {
        assert_eq!(resampled_frames(2401, 192_000, 44_100).unwrap(), 552);
        assert_eq!(resampled_frames(552, 44_100, 192_000).unwrap(), 2404);
        assert_eq!(resampled_frames(0, 48_000, 44_100).unwrap(), 0);
        assert!(resampled_frames(1, 0, 48_000).is_err());
        assert!(resampled_frames(1, 48_000, 0).is_err());
        assert!(resampled_frames(u64::MAX, 1, u32::MAX).is_err());
    }
    #[test]
    fn conversion_cancels_between_blocks_and_peaks_cancel_before_publication() {
        assert!(resample(&[[0.0; 2]], 1, 192_000).is_err());
        assert!(resample(&[[0.0; 2]], u32::MAX, 48_000).is_err());
        let audio = vec![[0.2, -0.1]; 48_000];
        let polls = Cell::new(0);
        let result = resample_cancellable(&audio, 48_000, 96_000, &|| {
            polls.set(polls.get() + 1);
            polls.get() > 3
        });
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert_eq!(polls.get(), 4);
        assert!(prepare_peaks(&audio, &|| true).is_err());
        for frames in [0, 1, 1023, 1024, 1025] {
            let source = vec![[0.2, -0.1]; frames];
            for rate in daw_core::SAMPLE_RATES {
                let output = resample(&source, 48_000, rate).unwrap();
                assert_eq!(output.len(), (frames * rate as usize).div_ceil(48_000));
                assert!(output.iter().flatten().all(|s| s.is_finite()));
                if rate == 48_000 {
                    assert_eq!(output, source);
                }
            }
        }
    }
}
