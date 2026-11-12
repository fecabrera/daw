//! Compact only the fully rendered suffix. Never stop rendering on low energy:
//! a delayed hit, reverb predelay or routing-only signal may arrive later.
use crate::{
    bounce::{BounceError, BounceProgress, preparation_error},
    error,
};
use daw_media::{AudioEncoder, AudioReservation, FrameReader, WavCodec, WavEncoder};
use std::{path::PathBuf, sync::atomic::Ordering};

// Keep at least 160 dB below each signal's peak, and never raise the absolute
// floor above -160 dBFS for floating-point headroom. Quiet sources scale with
// their own peak; an inaudible main output cannot discard an active raw tap.
const RELATIVE_FLOOR: f32 = 1.0e-8;

pub(super) fn compact(
    paths: &[PathBuf],
    minimum_frames: u64,
    rendered_frames: u64,
    rate: u32,
    progress: &BounceProgress,
) -> Result<u64, BounceError> {
    if rendered_frames == minimum_frames {
        return Ok(rendered_frames);
    }
    let fade_frames = u64::from((rate / 200).max(1));
    let mut retained = minimum_frames;
    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        progress.check()?;
        let data =
            daw_media::decode_audio_cached(path, rate, &|| progress.cancel.load(Ordering::Acquire))
                .map_err(|e| preparation_error(error(e), progress))?;
        if data.frame_count() != rendered_frames {
            return Err(error("Bounce tail source length mismatch").into());
        }
        let peak = data
            .peaks
            .iter()
            .flatten()
            .flatten()
            .fold(0.0f32, |peak, value| peak.max(value.abs()));
        let floor = peak.min(1.0) * RELATIVE_FLOOR;
        if peak > 0.0 {
            let mut reader = FrameReader::new(&data);
            // Scan the whole suffix, backwards. A quiet gap before a late echo
            // therefore cannot determine the final cut.
            for frame in (minimum_frames..rendered_frames).rev() {
                if frame.is_multiple_of(4096) {
                    progress.check()?;
                }
                if reader
                    .sample(frame)
                    .map_err(error)?
                    .iter()
                    .any(|v| v.abs() > floor)
                {
                    retained = retained.max(
                        frame
                            .saturating_add(1)
                            .saturating_add(fade_frames)
                            .min(rendered_frames),
                    );
                    break;
                }
            }
        }
        sources.push(data);
    }
    if retained == rendered_frames {
        return Ok(retained);
    }
    let block_frames = 4096usize;
    let _memory = AudioReservation::new(0, (block_frames * 8) as u64).map_err(error)?;
    let mut block = vec![[0.0; 2]; block_frames];
    // Never fade the requested body a second time. Any new terminal fade starts
    // strictly after the last retained significant tail sample.
    let fade_start = minimum_frames.max(retained.saturating_sub(fade_frames));
    let fade_length = retained - fade_start;
    for (path, data) in paths.iter().zip(sources) {
        progress.check()?;
        let temporary = path.with_extension(format!("compact-{}.wav", daw_core::Id::new_v4()));
        let mut encoder =
            WavEncoder::at_sample_rate(&temporary, WavCodec::Float32, rate).map_err(error)?;
        let mut reader = FrameReader::new(&data);
        let mut position = 0;
        while position < retained {
            progress.check()?;
            let count = (retained - position).min(block_frames as u64) as usize;
            for (offset, sample) in block[..count].iter_mut().enumerate() {
                let frame = position + offset as u64;
                let gain = if fade_length > 1 && frame >= fade_start {
                    (retained - 1 - frame) as f32 / (fade_length - 1) as f32
                } else {
                    1.0
                };
                *sample = reader.sample(frame).map_err(error)?.map(|v| v * gain);
            }
            encoder.write_frames(&block[..count]).map_err(error)?;
            position += count as u64;
        }
        encoder.finish().map_err(error)?;
        progress.check()?;
        crate::file_publish::replace(&temporary, path).map_err(error)?;
    }
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_media::GeneratedDirectory;
    fn file(
        directory: &GeneratedDirectory,
        name: &str,
        body: f32,
        echo: Option<(usize, f32)>,
    ) -> PathBuf {
        let path = directory.0.join(format!("{name}.wav"));
        let mut frames = vec![[0.0; 2]; 28800];
        frames[..4800].fill([body; 2]);
        if let Some((frame, value)) = echo {
            frames[frame] = [value; 2];
        }
        let mut encoder = WavEncoder::at_sample_rate(&path, WavCodec::Float32, 48000).unwrap();
        encoder.write_frames(&frames).unwrap();
        encoder.finish().unwrap();
        path
    }
    #[test]
    fn a_late_quiet_routing_echo_survives_and_every_leaf_uses_the_same_cut() {
        let directory = GeneratedDirectory::new().unwrap();
        let paths = [
            file(&directory, "main", 0.5, None),
            file(&directory, "raw", 1e-12, Some((24000, 1e-9))),
            file(&directory, "pre", 0.25, None),
        ];
        let length = compact(&paths, 4800, 28800, 48000, &BounceProgress::default()).unwrap();
        assert_eq!(length, 24241);
        for (index, path) in paths.iter().enumerate() {
            let data = daw_media::decode_wav_at_rate(path, 48000).unwrap();
            assert_eq!(data.frame_count(), length);
            assert_eq!(
                data.samples[4799],
                [if index == 0 {
                    0.5
                } else if index == 1 {
                    1e-12
                } else {
                    0.25
                }; 2]
            );
            assert_eq!(data.samples.last().unwrap(), &[0.0; 2]);
            if index == 1 {
                assert_eq!(data.samples[24000], [1e-9; 2]);
            }
        }
    }
    #[test]
    fn silent_padding_is_removed_without_shortening_or_fading_the_requested_body() {
        let directory = GeneratedDirectory::new().unwrap();
        let paths = [file(&directory, "main", 0.25, None)];
        assert_eq!(
            compact(&paths, 4800, 28800, 48000, &BounceProgress::default()).unwrap(),
            4800
        );
        let data = daw_media::decode_wav_at_rate(&paths[0], 48000).unwrap();
        assert_eq!(data.samples.len(), 4800);
        assert!(data.samples.iter().all(|frame| *frame == [0.25; 2]));
    }
    #[test]
    fn cancellation_keeps_unpublished_files_unchanged() {
        let directory = GeneratedDirectory::new().unwrap();
        let paths = [file(&directory, "main", 0.25, None)];
        let original = std::fs::read(&paths[0]).unwrap();
        let progress = BounceProgress::default();
        progress.cancel.store(true, Ordering::Release);
        assert!(matches!(
            compact(&paths, 4800, 28800, 48000, &progress),
            Err(BounceError::Cancelled)
        ));
        assert_eq!(std::fs::read(&paths[0]).unwrap(), original);
    }
}
