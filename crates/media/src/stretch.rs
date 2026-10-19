use crate::{AudioData, Error, Result};
#[cfg(test)]
use daw_core::SAMPLE_RATE;
use daw_core::{ClipStretch, StretchEngine, StretchEngineId};
use rubberband::{Options, Stretcher};
use std::sync::Arc;

/// A prepared buffer retains its source allocation, so pointer identity cannot be reused.
/// Matching also checks processing identity; stale buffers are never served as another engine.
#[derive(Clone)]
pub struct PreparedStretch {
    pub audio: AudioData,
    source: Arc<Vec<[f32; 2]>>,
    sample_rate_hz: u32,
    engine: StretchEngine,
    ratio: ClipStretch,
    pitch: timestretch::Pitch,
    pitch_revision: u32,
}
impl PreparedStretch {
    pub fn visit_memory(&self, usage: &mut crate::AudioMemoryUsage) {
        usage.samples(&self.source);
        usage.audio(&self.audio);
    }
    pub fn heap_bytes(&self) -> u64 {
        self.audio.metadata.sample_format.capacity() as u64
    }

    pub fn new(source: &AudioData, ratio: ClipStretch, engine: StretchEngine) -> Result<Self> {
        Self::new_cancellable(source, ratio, engine, &|| false)
    }
    pub fn new_cancellable(
        source: &AudioData,
        ratio: ClipStretch,
        engine: StretchEngine,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        Self::with_pitch_cancellable(source, ratio, engine, timestretch::Pitch::UNISON, cancelled)
    }
    pub fn with_pitch_cancellable(
        source: &AudioData,
        ratio: ClipStretch,
        engine: StretchEngine,
        pitch: timestretch::Pitch,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Self> {
        Ok(Self {
            audio: transform_audio(source, ratio, engine, pitch, cancelled)?,
            source: source.samples.clone(),
            sample_rate_hz: source.sample_rate_hz,
            engine,
            ratio,
            pitch,
            pitch_revision: timestretch::PITCH_REVISION,
        })
    }
    pub fn matches(&self, source: &AudioData, ratio: ClipStretch, engine: StretchEngine) -> bool {
        self.matches_pitch(source, ratio, engine, timestretch::Pitch::UNISON)
    }
    pub fn matches_pitch(
        &self,
        source: &AudioData,
        ratio: ClipStretch,
        engine: StretchEngine,
        pitch: timestretch::Pitch,
    ) -> bool {
        Arc::ptr_eq(&self.source, &source.samples)
            && self.sample_rate_hz == source.sample_rate_hz
            && self.engine == engine
            && self.ratio == ratio
            && self.pitch == pitch
            && self.pitch_revision == timestretch::PITCH_REVISION
    }
}
impl std::ops::Deref for PreparedStretch {
    type Target = AudioData;
    fn deref(&self) -> &AudioData {
        &self.audio
    }
}

pub fn stretch_audio_with_engine(
    audio: &AudioData,
    ratio: ClipStretch,
    engine: StretchEngine,
) -> Result<AudioData> {
    stretch_audio_with_engine_cancellable(audio, ratio, engine, &|| false)
}

fn stretch_audio_with_engine_cancellable(
    audio: &AudioData,
    ratio: ClipStretch,
    engine: StretchEngine,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    transform_audio(audio, ratio, engine, timestretch::Pitch::UNISON, cancelled)
}

fn transform_audio(
    audio: &AudioData,
    ratio: ClipStretch,
    engine: StretchEngine,
    pitch: timestretch::Pitch,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Time stretch cancelled".into()));
    }
    engine.validate().map_err(|e| Error(e.to_string()))?;
    if cancelled() {
        return Err(Error("Time stretch cancelled".into()));
    }
    if !ratio.valid() {
        return Err(Error("Unsupported clip stretch ratio".into()));
    }
    if engine.id == StretchEngineId::RubberBand && pitch.stages().len() > 1 {
        let mut prepared = audio.clone();
        for (index, stage) in pitch.stages().enumerate() {
            let duration = if index == 0 {
                ratio
            } else {
                ClipStretch::new(1, 1).unwrap()
            };
            prepared = transform_audio(&prepared, duration, engine, stage, cancelled)?;
        }
        return Ok(prepared);
    }
    if ratio.source_frames == ratio.output_frames && pitch == timestretch::Pitch::UNISON {
        return Ok(audio.clone());
    }
    crate::register_audio(audio);
    let frames = ratio
        .scale(audio.samples.len() as u64)
        .ok_or_else(|| Error("Stretched audio is too long".into()))?;
    // Integrated materializes planar/interleaved input and output; native scratch is
    // library-owned and is not represented as a strict RSS guarantee.
    let scratch = if engine.id == StretchEngineId::Integrated {
        (audio.samples.len() as u64)
            .saturating_mul(8)
            .saturating_add(frames.saturating_mul(8))
    } else {
        0
    };
    // Reuse the engine's preflight estimate, including its private pitch intermediate.
    let pitch_scratch =
        if engine.id == StretchEngineId::Integrated && pitch != timestretch::Pitch::UNISON {
            timestretch::Request::new(
                audio.sample_rate_hz,
                2,
                timestretch::Ratio::new(ratio.output_frames, ratio.source_frames)
                    .map_err(|e| Error(e.to_string()))?,
            )
            .with_pitch(pitch)
            .working_memory_bytes(audio.samples.len())
            .map_err(|e| Error(e.to_string()))? as u64
        } else {
            0
        };
    let _reservation = crate::AudioReservation::new(
        frames,
        scratch
            .saturating_add(pitch_scratch)
            .saturating_add(16 * 1024 * 1024),
    )?;
    match engine.id {
        StretchEngineId::RubberBand => stretch_audio_inner(audio, ratio, pitch, cancelled),
        StretchEngineId::Integrated => {
            let ratio = timestretch::Ratio::new(ratio.output_frames, ratio.source_frames)
                .map_err(|e| Error(e.to_string()))?;
            let mut input = Vec::new();
            input
                .try_reserve_exact(
                    audio
                        .samples
                        .len()
                        .checked_mul(2)
                        .ok_or_else(|| Error("Source too long".into()))?,
                )
                .map_err(|e| Error(e.to_string()))?;
            input.extend(audio.samples.iter().flatten().copied());
            let output = timestretch::Request::new(audio.sample_rate_hz, 2, ratio)
                .with_pitch(pitch)
                .process_with_progress(&input, |_| {
                    if cancelled() {
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                })
                .map_err(|e| Error(e.to_string()))?;
            let samples: Vec<_> = output.chunks_exact(2).map(|s| [s[0], s[1]]).collect();
            Ok(prepared_audio(audio, samples))
        }
    }
}

/// Offline R3 processing. Call on a worker, never from an audio callback.
pub fn stretch_audio(audio: &AudioData, ratio: ClipStretch) -> Result<AudioData> {
    stretch_audio_cancellable(audio, ratio, &|| false)
}
fn stretch_audio_cancellable(
    audio: &AudioData,
    ratio: ClipStretch,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Time stretch cancelled".into()));
    }
    if !ratio.valid() {
        return Err(Error("Unsupported clip stretch ratio".into()));
    }
    if ratio.source_frames == ratio.output_frames {
        return Ok(audio.clone());
    }
    crate::register_audio(audio);
    let frames = ratio
        .scale(audio.samples.len() as u64)
        .ok_or_else(|| Error("Stretched audio is too long".into()))?;
    let _reservation = crate::AudioReservation::new(frames, 16 * 1024 * 1024)?;
    stretch_audio_inner(audio, ratio, timestretch::Pitch::UNISON, cancelled)
}
fn stretch_audio_inner(
    audio: &AudioData,
    ratio: ClipStretch,
    pitch: timestretch::Pitch,
    cancelled: &impl Fn() -> bool,
) -> Result<AudioData> {
    if cancelled() {
        return Err(Error("Time stretch cancelled".into()));
    }
    if !ratio.valid() {
        return Err(Error("Unsupported clip stretch ratio".into()));
    }
    let expected = ratio
        .scale(audio.samples.len() as u64)
        .and_then(|length| usize::try_from(length).ok())
        .ok_or_else(|| Error("Stretched audio is too long".into()))?;
    if audio.samples.len() > u32::MAX as usize {
        return Err(Error("Source is too long for offline stretching".into()));
    }
    if ratio.source_frames == ratio.output_frames && pitch == timestretch::Pitch::UNISON {
        return Ok(audio.clone());
    }
    let mut samples = Vec::new();
    let mut produced = 0usize;
    samples
        .try_reserve_exact(expected)
        .map_err(|e| Error(e.to_string()))?;
    if !audio.samples.is_empty() {
        let mut processor = Stretcher::new(
            audio.sample_rate_hz,
            2,
            Options::PROCESS_OFFLINE
                | Options::ENGINE_FINER
                | Options::CHANNELS_TOGETHER
                | Options::THREADING_NEVER,
            ratio.ratio(),
            pitch.factor(),
        );
        if processor.engine_version() != 3 {
            return Err(Error("Rubber Band R3 is unavailable".into()));
        }
        const BLOCK: usize = 4096;
        processor.set_max_process_size(BLOCK as u32);
        processor.set_expected_input_duraction(audio.samples.len() as u32);
        let mut left = vec![0.0; BLOCK];
        let mut right = vec![0.0; BLOCK];
        for (index, block) in audio.samples.chunks(BLOCK).enumerate() {
            if cancelled() {
                return Err(Error("Time stretch cancelled".into()));
            }
            for (i, sample) in block.iter().enumerate() {
                if sample.iter().any(|value| !value.is_finite()) {
                    return Err(Error("Source contains invalid samples".into()));
                }
                left[i] = sample[0];
                right[i] = sample[1];
            }
            processor.study(
                &[&left[..block.len()], &right[..block.len()]],
                (index + 1) * BLOCK >= audio.samples.len(),
            );
        }
        let mut out_left = vec![0.0; BLOCK];
        let mut out_right = vec![0.0; BLOCK];
        for (index, block) in audio.samples.chunks(BLOCK).enumerate() {
            if cancelled() {
                return Err(Error("Time stretch cancelled".into()));
            }
            for (i, sample) in block.iter().enumerate() {
                left[i] = sample[0];
                right[i] = sample[1];
            }
            processor.process(
                &[&left[..block.len()], &right[..block.len()]],
                (index + 1) * BLOCK >= audio.samples.len(),
            );
            while let Some(available) = processor.available().filter(|frames| *frames > 0) {
                let size = (available as usize).min(BLOCK);
                let count = processor.retrieve(&mut [&mut out_left[..size], &mut out_right[..size]])
                    as usize;
                if count == 0 {
                    return Err(Error("Stretch processor produced no output".into()));
                }
                for i in 0..count {
                    if !out_left[i].is_finite() || !out_right[i].is_finite() {
                        return Err(Error("Stretch processor produced invalid audio".into()));
                    }
                    if samples.len() < expected {
                        samples.push([out_left[i], out_right[i]]);
                    }
                }
                produced = produced
                    .checked_add(count)
                    .ok_or_else(|| Error("Stretched audio is too long".into()))?;
            }
        }
        // Offline mode completes synchronously and compensates its own padding/delay.
        if processor.available().is_some() {
            return Err(Error("Stretch processor did not finish".into()));
        }
    }
    if produced.abs_diff(expected) > 1 {
        return Err(Error(format!(
            "Stretch processor returned {produced} frames; expected {expected}"
        )));
    }
    samples.resize(expected, [0.0; 2]);
    Ok(prepared_audio(audio, samples))
}

pub fn prepared_audio(audio: &AudioData, samples: Vec<[f32; 2]>) -> AudioData {
    let peaks = samples
        .chunks(256)
        .map(|chunk| {
            let mut peak = [[0.0_f32; 2]; 2];
            for sample in chunk {
                for channel in 0..2 {
                    peak[channel][0] = peak[channel][0].min(sample[channel]);
                    peak[channel][1] = peak[channel][1].max(sample[channel]);
                }
            }
            peak
        })
        .collect();
    let prepared = AudioData {
        sample_rate_hz: audio.sample_rate_hz,
        samples: Arc::new(samples),
        peaks: Arc::new(peaks),
        metadata: audio.metadata.clone(),
    };
    crate::register_audio(&prepared);
    prepared
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn r3_changes_duration_without_changing_pitch_or_stereo_relationship() {
        let audio = AudioData {
            sample_rate_hz: daw_core::SAMPLE_RATE,
            samples: Arc::new(
                (0..SAMPLE_RATE * 2)
                    .map(|frame| {
                        let tone = (frame as f32 * 440.0 * std::f32::consts::TAU
                            / SAMPLE_RATE as f32)
                            .sin()
                            * 0.4;
                        [tone, -tone]
                    })
                    .collect(),
            ),
            peaks: Arc::new(vec![]),
            metadata: daw_core::SourceMetadata {
                sample_rate_hz: SAMPLE_RATE,
                channels: 2,
                sample_format: "ieee_float".into(),
                bits_per_sample: 32,
            },
        };
        for ratio in [
            ClipStretch::new(2, 3).unwrap(),
            ClipStretch::new(2, 1).unwrap(),
        ] {
            let output = stretch_audio(&audio, ratio).unwrap();
            assert_eq!(
                output.samples.len() as u64,
                ratio.scale(audio.samples.len() as u64).unwrap()
            );
            let middle = &output.samples[output.samples.len() / 4..output.samples.len() * 3 / 4];
            let crossings = middle
                .windows(2)
                .filter(|pair| pair[0][0] <= 0.0 && pair[1][0] > 0.0)
                .count();
            let frequency = crossings as f64 * f64::from(SAMPLE_RATE) / middle.len() as f64;
            assert!((frequency - 440.0).abs() < 3.0, "frequency: {frequency}");
            assert!(
                middle
                    .iter()
                    .all(|sample| (sample[0] + sample[1]).abs() < 0.0001)
            );
            assert_eq!(audio.samples.len(), SAMPLE_RATE as usize * 2);
        }
    }
}
