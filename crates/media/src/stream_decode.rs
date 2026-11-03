//! Bounded WAV-to-disk preparation with the same rate conversion as resident PCM.
use crate::{AudioData, DiskAudioWriter, Error, Result};
use daw_core::SourceMetadata;
use rubato::Resampler;
use std::{fs::File, io::BufReader};

pub const STREAM_PCM_THRESHOLD_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) fn should_stream(frames: u64) -> bool {
    frames.saturating_mul(8) >= STREAM_PCM_THRESHOLD_BYTES
}

struct RateWriter {
    output: DiskAudioWriter,
    converter: Option<rubato::FftFixedIn<f32>>,
    input: Vec<Vec<f32>>,
    converted: Vec<Vec<f32>>,
    stereo: Vec<[f32; 2]>,
    filled: usize,
    skip: usize,
    remaining: u64,
    _scratch: crate::AudioReservation,
}
impl RateWriter {
    fn new(input_rate: u32, output_rate: u32, frames: u64) -> Result<Self> {
        let scratch =
            crate::AudioReservation::new(0, crate::resample_scratch(input_rate, output_rate)?)?;
        let output = DiskAudioWriter::new(frames)?;
        let converter = (input_rate != output_rate)
            .then(|| {
                rubato::FftFixedIn::<f32>::new(
                    input_rate as usize,
                    output_rate as usize,
                    1024,
                    2,
                    2,
                )
            })
            .transpose()
            .map_err(|e| Error(e.to_string()))?;
        let input_size = converter.as_ref().map_or(8192, |c| c.input_frames_next());
        let converted = converter
            .as_ref()
            .map_or_else(Vec::new, |c| c.output_buffer_allocate(true));
        let output_size = converted.first().map_or(8192, Vec::len);
        let skip = converter.as_ref().map_or(0, |c| c.output_delay());
        Ok(Self {
            output,
            converter,
            input: vec![vec![0.; input_size]; 2],
            converted,
            stereo: vec![[0.; 2]; output_size],
            filled: 0,
            skip,
            remaining: frames,
            _scratch: scratch,
        })
    }
    fn push(&mut self, frame: [f32; 2]) -> Result<()> {
        for (channel, value) in self.input.iter_mut().zip(frame) {
            channel[self.filled] = value;
        }
        self.filled += 1;
        if self.filled == self.input[0].len() {
            self.flush()?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        let count = if let Some(converter) = &mut self.converter {
            let (_, frames) = converter
                .process_into_buffer(&self.input, &mut self.converted, None)
                .map_err(|e| Error(e.to_string()))?;
            for (index, frame) in self.stereo[..frames].iter_mut().enumerate() {
                *frame = [self.converted[0][index], self.converted[1][index]];
            }
            frames
        } else {
            for (index, frame) in self.stereo[..self.filled].iter_mut().enumerate() {
                *frame = [self.input[0][index], self.input[1][index]];
            }
            self.filled
        };
        let skip = self.skip.min(count);
        self.skip -= skip;
        let take = self.remaining.min((count - skip) as u64) as usize;
        self.output.write(&self.stereo[skip..skip + take])?;
        self.remaining -= take as u64;
        self.input.iter_mut().for_each(|channel| channel.fill(0.));
        self.filled = 0;
        Ok(())
    }
    fn finish(
        mut self,
        rate: u32,
        metadata: SourceMetadata,
        cancelled: &impl Fn() -> bool,
    ) -> Result<AudioData> {
        if cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
        if self.filled != 0 {
            self.flush()?;
        }
        while self.remaining != 0 {
            if cancelled() {
                return Err(Error("Audio preparation cancelled".into()));
            }
            if self.converter.is_none() {
                return Err(Error("WAV ended before its declared length".into()));
            }
            self.flush()?;
        }
        self.output.finish(rate, metadata)
    }
}

pub(crate) fn decode_wav(
    reader: hound::WavReader<BufReader<File>>,
    output_rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    let spec = reader.spec();
    let input_frames = reader.duration() as u64;
    let target = crate::resampled_frames(input_frames, spec.sample_rate, output_rate)?;
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
    let mut samples: Box<dyn Iterator<Item = std::result::Result<f32, hound::Error>>> =
        match (spec.sample_format, spec.bits_per_sample) {
            (hound::SampleFormat::Float, 32) => Box::new(reader.into_samples::<f32>()),
            (hound::SampleFormat::Int, 16) => Box::new(
                reader
                    .into_samples::<i16>()
                    .map(|s| s.map(|v| v as f32 / 32768.)),
            ),
            (hound::SampleFormat::Int, bits) => Box::new(
                reader
                    .into_samples::<i32>()
                    .map(move |s| s.map(|v| v as f32 / (1_u64 << (bits - 1)) as f32)),
            ),
            _ => return Err(Error("Unsupported WAV format for disk audio".into())),
        };
    let mut writer = RateWriter::new(spec.sample_rate, output_rate, target)?;
    for frame in 0..input_frames {
        if frame % 8192 == 0 && cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
        let mut read = || {
            samples
                .next()
                .ok_or_else(|| Error("Incomplete WAV sample data".into()))?
                .map_err(|e| Error(format!("Cannot decode WAV: {e}")))
        };
        let left = read()?;
        let right = if spec.channels == 1 { left } else { read()? };
        writer.push([left, right])?;
    }
    if samples.next().is_some() {
        return Err(Error("WAV sample data exceeds its declared length".into()));
    }
    writer.finish(output_rate, metadata, cancelled)
}

pub(crate) fn resample_audio(
    audio: &AudioData,
    rate: u32,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    let target = crate::resampled_frames(audio.frame_count(), audio.sample_rate_hz, rate)?;
    let mut writer = RateWriter::new(audio.sample_rate_hz, rate, target)?;
    let mut block = vec![[0.; 2]; crate::STREAM_CHUNK_FRAMES];
    let mut raw = Vec::with_capacity(crate::STREAM_CHUNK_FRAMES * 8);
    let mut position = 0;
    while position < audio.frame_count() {
        if cancelled() {
            return Err(Error("Audio preparation cancelled".into()));
        }
        let count = audio.read_frames(position, &mut block, &mut raw)?;
        if count == 0 {
            return Err(Error("Audio ended before its prepared length".into()));
        }
        for frame in &block[..count] {
            writer.push(*frame)?;
        }
        position += count as u64;
    }
    writer.finish(rate, audio.metadata.clone(), cancelled)
}

pub(crate) fn large_wav(path: &std::path::Path, rate: u32) -> bool {
    hound::WavReader::open(path)
        .ok()
        .and_then(|reader| {
            crate::resampled_frames(reader.duration() as u64, reader.spec().sample_rate, rate).ok()
        })
        .is_some_and(should_stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioEncoder;
    #[test]
    #[ignore = "requires DAW_STREAM_INPUT pointing to local long compressed audio"]
    fn local_long_compressed_source_uses_bounded_pcm_storage() {
        let path = std::env::var_os("DAW_STREAM_INPUT").expect("DAW_STREAM_INPUT");
        let audio = crate::decode_audio_at_rate(std::path::Path::new(&path), 44_100).unwrap();
        assert!(audio.disk.is_some());
        assert!(audio.samples.is_empty());
        assert!(audio.frame_count() * 8 >= STREAM_PCM_THRESHOLD_BYTES);
        if let Ok(frames) = std::env::var("DAW_STREAM_FRAMES") {
            assert_eq!(audio.frame_count(), frames.parse::<u64>().unwrap());
        }
        let mut usage = crate::AudioMemoryUsage::default();
        usage.audio(&audio);
        assert!(usage.bytes < 64 * 1024 * 1024);
        let converted = audio.at_sample_rate(48_000).unwrap();
        assert_eq!(
            converted.frame_count(),
            crate::resampled_frames(audio.frame_count(), 44_100, 48_000).unwrap()
        );
        assert!(converted.disk.is_some());
        assert!(
            converted
                .peaks
                .iter()
                .flatten()
                .flatten()
                .any(|value| value.abs() > 0.01)
        );
    }
    #[test]
    fn streamed_rate_conversion_matches_resident_samples_metadata_and_peaks() {
        let root =
            std::env::temp_dir().join(format!("daw-stream-rates-{}", daw_core::Id::new_v4()));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        for input_rate in [44_100, 48_000] {
            for codec in crate::WavCodec::ALL {
                let path = root.join(format!("{input_rate}-{}.wav", codec.label()));
                let mut encoder =
                    crate::WavEncoder::at_sample_rate(&path, codec, input_rate).unwrap();
                let frames: Vec<_> = (0..4193)
                    .map(|i| {
                        [
                            (i as f32 * 0.03).sin() * 0.2,
                            (i as f32 * 0.027).cos() * 0.3,
                        ]
                    })
                    .collect();
                encoder.write_frames(&frames).unwrap();
                encoder.finish().unwrap();
                for output_rate in daw_core::SAMPLE_RATES {
                    let expected = crate::decode_wav_at_rate(&path, output_rate).unwrap();
                    let streamed =
                        decode_wav(hound::WavReader::open(&path).unwrap(), output_rate, &|| {
                            false
                        })
                        .unwrap();
                    let actual = streamed.materialized(&|| false).unwrap();
                    assert_eq!(
                        actual.samples, expected.samples,
                        "{input_rate} -> {output_rate}"
                    );
                    assert_eq!(
                        actual.peaks, expected.peaks,
                        "{input_rate} -> {output_rate}"
                    );
                    assert_eq!(actual.sample_rate_hz, expected.sample_rate_hz);
                    assert_eq!(actual.metadata, expected.metadata);
                }
            }
        }
    }
}
