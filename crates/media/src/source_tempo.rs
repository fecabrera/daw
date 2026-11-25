//! Worker adapter shared by original files and compound source schedules.
use crate::{AudioData, AudioReservation, Error, Result};
use audio_analysis::tempo::{Tempo, TempoWindow, TempoWindows};

const BLOCK: usize = 8192;

/// Analyze selected ranges of the original prepared audio.
pub fn audio_tempo(audio: &AudioData, cancelled: &impl Fn() -> bool) -> Result<Tempo> {
    let mut raw = Vec::new();
    source_tempo(
        audio.sample_rate_hz,
        audio.frame_count(),
        cancelled,
        |start, out| {
            if audio.read_frames(start, out, &mut raw)? != out.len() {
                return Err(Error("Incomplete source audio read".into()));
            }
            Ok(())
        },
    )
}

/// Feed bounded blocks to the portable detector without materializing the source.
pub fn source_tempo(
    rate: u32,
    frames: u64,
    cancelled: &impl Fn() -> bool,
    mut read: impl FnMut(u64, &mut [[f32; 2]]) -> Result<()>,
) -> Result<Tempo> {
    let _memory = AudioReservation::optional(1024 * 1024)?;
    let map_error = |error: audio_analysis::Error| Error(error.to_string());
    let windows = TempoWindows::new(rate, frames).map_err(map_error)?;
    let mut block = vec![[0.0; 2]; BLOCK];
    let mut results = Vec::with_capacity(windows.ranges.len());
    let candidate = windows.loop_candidate();
    for range in &windows.ranges {
        let mut detector =
            TempoWindow::with_spectral_history(rate, range.end - range.start).map_err(map_error)?;
        let preroll = range.start.min(u64::from(rate / 100) * 5);
        let warmup = range.start - preroll / u64::from(rate / 100) * u64::from(rate / 100);
        for (span, score) in [(warmup..range.start, false), (range.clone(), true)] {
            let mut start = span.start;
            while start < span.end {
                if cancelled() {
                    return Err(Error("Tempo analysis cancelled".into()));
                }
                let count = (span.end - start).min(BLOCK as u64) as usize;
                read(start, &mut block[..count])?;
                detector.push(&block[..count], score).map_err(map_error)?;
                start += count as u64;
            }
        }
        results.push(
            detector
                .into_evidence(candidate, cancelled)
                .map_err(map_error)?,
        );
    }
    windows
        .combine_evidence(&results, cancelled)
        .map_err(map_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn sampled_weak_source_keeps_the_existing_reads() {
        let rate = 48_000;
        let frames = 40 * u64::from(rate);
        let mut reads = Vec::new();
        let result = source_tempo(rate, frames, &|| false, |start, out| {
            reads.push((start, out.len() as u64));
            out.fill([0.0; 2]);
            Ok(())
        })
        .unwrap();
        assert_eq!(result, Tempo::Weak);
        let blocks = |start: u64, end: u64| {
            (start..end)
                .step_by(BLOCK)
                .map(move |frame| (frame, (end - frame).min(BLOCK as u64)))
        };
        let expected: Vec<_> = blocks(0, 576_000)
            .chain(blocks(669_600, 672_000))
            .chain(blocks(672_000, 1_248_000))
            .chain(blocks(1_341_600, 1_344_000))
            .chain(blocks(1_344_000, 1_920_000))
            .collect();
        assert_eq!(reads, expected);
    }

    #[test]
    fn source_tempo_matches_for_resident_and_streamed_pcm() {
        let rate = 48_000;
        let mut samples = vec![[0.0; 2]; 4 * rate as usize];
        for beat in 0..8 {
            samples[400 + beat * 24_000..450 + beat * 24_000].fill([0.4, -0.4]);
        }
        let metadata = daw_core::SourceMetadata {
            sample_rate_hz: rate,
            channels: 2,
            sample_format: "ieee_float".into(),
            bits_per_sample: 32,
        };
        let mut writer = crate::DiskAudioWriter::new(samples.len() as u64).unwrap();
        writer.write(&samples).unwrap();
        let streamed = writer.finish(rate, metadata.clone()).unwrap();
        let resident = AudioData {
            sample_rate_hz: rate,
            samples: Arc::new(samples),
            disk: None,
            metadata,
            peaks: Arc::default(),
        };
        let expected = audio_tempo(&resident, &|| false).unwrap();
        assert!(
            matches!(expected, Tempo::Estimate { bpm, sampled: false } if (bpm - 120.0).abs() < 1.0)
        );
        assert_eq!(audio_tempo(&streamed, &|| false).unwrap(), expected);
        assert!(audio_tempo(&streamed, &|| true).is_err());

        let mut limited = crate::FrameReader::new(&streamed);
        limited.limit_optional_reads(Arc::new(crate::OptionalReadBudget::new(0, Arc::default())));
        assert!(
            limited
                .sample(400)
                .unwrap_err()
                .0
                .contains("limit exceeded")
        );
        assert!(limited.error().is_some());
        assert!(!limited.failed());
        assert_eq!(
            crate::FrameReader::new(&streamed).sample(400).unwrap(),
            [0.4, -0.4]
        );
    }
}
