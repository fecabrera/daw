use daw_core::Clip;
use daw_media::AudioData;

/// Inspect a visible timeline window without reading outside the clip's source range.
pub fn extrema(audio: &AudioData, clip: &Clip, local: u64, count: u64, channel: usize) -> [f32; 2] {
    let count = count.min(clip.length_frames.saturating_sub(local));
    if count == 0 {
        return [0.0; 2];
    }
    let Some(repeat) = clip.repeat else {
        return source_extrema(audio, clip.source_frame(local), count, channel);
    };
    if count >= repeat.length_frames {
        return source_extrema(
            audio,
            clip.source_offset_frame,
            repeat.length_frames,
            channel,
        );
    }
    let position = repeat.position(local);
    let first = count.min(repeat.length_frames - position);
    let a = source_extrema(audio, clip.source_offset_frame + position, first, channel);
    let b = source_extrema(audio, clip.source_offset_frame, count - first, channel);
    [a[0].min(b[0]), a[1].max(b[1])]
}

fn source_extrema(audio: &AudioData, start: u64, count: u64, channel: usize) -> [f32; 2] {
    let end = start.saturating_add(count).min(audio.samples.len() as u64) as usize;
    let mut position = start.min(audio.samples.len() as u64) as usize;
    let mut result = [0.0_f32; 2];
    while position < end {
        // Cache full 256-frame blocks; inspect partial boundary blocks exactly.
        if position.is_multiple_of(256)
            && end - position >= 256
            && let Some(peak) = audio.peaks.get(position / 256)
        {
            result[0] = result[0].min(peak[channel][0]);
            result[1] = result[1].max(peak[channel][1]);
            position += 256;
        } else {
            let sample = audio.samples[position][channel];
            result[0] = result[0].min(sample);
            result[1] = result[1].max(sample);
            position += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_core::{ClipLoop, Id, SourceMetadata};
    use std::sync::Arc;

    fn audio(samples: Vec<[f32; 2]>) -> AudioData {
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
        AudioData {
            samples: Arc::new(samples),
            peaks: Arc::new(peaks),
            metadata: SourceMetadata {
                sample_rate_hz: 48000,
                channels: 2,
                sample_format: "pcm_int".into(),
                bits_per_sample: 16,
            },
        }
    }

    fn clip(offset: u64, length: u64, repeat: Option<ClipLoop>) -> Clip {
        Clip {
            id: Id::nil(),
            asset_id: Id::nil(),
            name: "Loop".into(),
            color: None,
            start_frame: 0,
            source_offset_frame: offset,
            length_frames: length,
            repeat,
        }
    }

    #[test]
    fn repeated_waveform_windows_match_samples_without_leaking_outside_the_base() {
        let mut samples = vec![[1.0, -1.0]; 1200];
        for (index, sample) in samples[100..900].iter_mut().enumerate() {
            *sample = if index % 7 < 3 {
                [0.25, -0.5]
            } else {
                [-0.5, 0.25]
            };
        }
        let audio = audio(samples);
        for repeat in [
            None,
            Some(ClipLoop {
                length_frames: 800,
                phase_frame: 731,
            }),
        ] {
            let clip = clip(100, if repeat.is_some() { 3000 } else { 800 }, repeat);
            for local in [0, 1, 68, 69, 255, 500, 799] {
                for count in [1, 3, 70, 256, 700, 800, 2000] {
                    for channel in 0..2 {
                        let mut expected = [0.0_f32; 2];
                        for frame in local..(local + count).min(clip.length_frames) {
                            let sample = audio.samples[clip.source_frame(frame) as usize][channel];
                            expected[0] = expected[0].min(sample);
                            expected[1] = expected[1].max(sample);
                        }
                        assert_eq!(extrema(&audio, &clip, local, count, channel), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn tiny_loops_and_shortened_sources_keep_waveform_reads_bounded() {
        let mut clip = clip(
            1,
            u64::MAX,
            Some(ClipLoop {
                length_frames: 2,
                phase_frame: 1,
            }),
        );
        let audio = audio(vec![[1.0; 2], [0.25, -0.5], [-0.5, 0.25]]);
        assert_eq!(extrema(&audio, &clip, 0, u64::MAX, 0), [-0.5, 0.25]);
        assert_eq!(
            extrema(&audio, &clip, u64::MAX - 1, u64::MAX, 1),
            [0.0, 0.25]
        );
        assert_eq!(extrema(&audio, &clip, u64::MAX, 1, 0), [0.0; 2]);
        clip.source_offset_frame = 3;
        assert_eq!(extrema(&audio, &clip, 0, 3000, 0), [0.0; 2]);
        clip.source_offset_frame = 2;
        assert_eq!(extrema(&audio, &clip, 0, 3000, 0), [-0.5, 0.0]);
    }
}
