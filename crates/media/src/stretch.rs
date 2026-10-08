use crate::{AudioData, Error, Result};
use daw_core::{ClipStretch, SAMPLE_RATE};
use rubberband::{Options, Stretcher};
use std::sync::Arc;

/// Offline R3 processing. Call on a worker, never from an audio callback.
pub fn stretch_audio(audio: &AudioData, ratio: ClipStretch) -> Result<AudioData> {
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
    if ratio.source_frames == ratio.output_frames {
        return Ok(audio.clone());
    }
    let mut samples = Vec::new();
    let mut produced = 0usize;
    samples
        .try_reserve_exact(expected)
        .map_err(|e| Error(e.to_string()))?;
    if !audio.samples.is_empty() {
        let mut processor = Stretcher::new(
            SAMPLE_RATE,
            2,
            Options::PROCESS_OFFLINE
                | Options::ENGINE_FINER
                | Options::CHANNELS_TOGETHER
                | Options::THREADING_NEVER,
            ratio.ratio(),
            1.0,
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
    Ok(AudioData {
        samples: Arc::new(samples),
        peaks: Arc::new(peaks),
        metadata: audio.metadata.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn r3_changes_duration_without_changing_pitch_or_stereo_relationship() {
        let audio = AudioData {
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
