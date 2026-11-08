//! Worker adapter for optional visual analysis of resident or disk-backed PCM.
use crate::{AudioData, Error, Result};
use audio_analysis::TransientRequest;
use std::ops::ControlFlow;

/// Maximum additional analysis storage, excluding the caller's shared source.
pub const TRANSIENT_ANALYSIS_MEMORY_BYTES: usize = 128 * 1024 * 1024;

/// Analyze complete prepared PCM before clip gain/fades. Never call in painting
/// or an audio callback. The bounded read adapter preserves full-source detector
/// history for both storage modes; it does not reset the detector at chunks.
pub fn audio_transients(audio: &AudioData, cancelled: &impl Fn() -> bool) -> Result<Vec<u64>> {
    let channels = usize::from(audio.metadata.channels);
    let request = TransientRequest::new(audio.sample_rate_hz, channels);
    let frames = usize::try_from(audio.frame_count())
        .map_err(|_| Error("Transient source is too large".into()))?;
    let (samples, bytes) = analysis_storage(request, frames)?;
    let _reservation = crate::memory::AudioReservation::new(0, bytes as u64)?;
    let check = || {
        if cancelled() {
            Err(Error("Transient analysis cancelled".into()))
        } else {
            Ok(())
        }
    };
    check()?;
    let mut input = Vec::new();
    input
        .try_reserve_exact(samples)
        .map_err(|_| Error("Cannot allocate transient analysis input".into()))?;
    let mut block = vec![[0.0; 2]; 8192];
    let mut raw = Vec::new();
    let mut position = 0;
    while position < frames {
        check()?;
        let count = (frames - position).min(block.len());
        let read = audio.read_frames(position as u64, &mut block[..count], &mut raw)?;
        if read != count {
            return Err(Error("Incomplete transient analysis source".into()));
        }
        for frame in &block[..read] {
            input.extend_from_slice(&frame[..channels]);
        }
        position += read;
    }
    let positions = request
        .detect_with_progress(&input, |_| {
            if cancelled() {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .map_err(|e| Error(e.to_string()))?;
    // Convert positions explicitly: usize and u64 differ on some targets.
    // The returned list fits within the analysis scratch allowance.
    Ok(positions.into_iter().map(|p| p as u64).collect())
}

fn analysis_storage(request: TransientRequest, frames: usize) -> Result<(usize, usize)> {
    let channels = request.channels;
    let estimate = request
        .estimated_memory_bytes(frames)
        .map_err(|e| Error(e.to_string()))?;
    let samples = frames
        .checked_mul(channels)
        .ok_or_else(|| Error("Transient source is too large".into()))?;
    let bytes = samples
        .checked_mul(4)
        .and_then(|n| n.checked_add(estimate))
        .and_then(|n| n.checked_add(128 * 1024));
    if bytes.is_none_or(|n| n > TRANSIENT_ANALYSIS_MEMORY_BYTES) {
        return Err(Error(
            "Source exceeds the 128 MiB transient analysis limit".into(),
        ));
    }
    Ok((samples, bytes.unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn audio() -> AudioData {
        let mut seed = 37_u32;
        let samples = (0..48000)
            .map(|frame| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let age = frame as f32 / 48000.0 - if frame < 20000 { 0.2 } else { 0.6 };
                let sample = if (0.0..0.03).contains(&age) {
                    (seed as f32 / u32::MAX as f32 - 0.5) * (-age * 100.0).exp()
                } else {
                    0.0
                };
                [sample, -sample]
            })
            .collect();
        AudioData {
            samples: Arc::new(samples),
            disk: None,
            peaks: Default::default(),
            sample_rate_hz: 48000,
            metadata: daw_core::SourceMetadata {
                sample_rate_hz: 48000,
                channels: 2,
                sample_format: "float".into(),
                bits_per_sample: 32,
            },
        }
    }
    #[test]
    fn resident_and_disk_analysis_match_without_modifying_source() {
        let source = audio();
        let before = source.samples.clone();
        let markers = audio_transients(&source, &|| false).unwrap();
        assert_eq!(markers.len(), 2);
        for (frame, expected) in markers.iter().zip([9600_u64, 28800]) {
            assert!(frame.abs_diff(expected) <= 96);
        }
        let mut writer = crate::DiskAudioWriter::new(source.frame_count()).unwrap();
        writer.write(&source.samples).unwrap();
        let disk = writer.finish(48000, source.metadata.clone()).unwrap();
        assert!(disk.samples.is_empty());
        assert_eq!(audio_transients(&disk, &|| false).unwrap(), markers);
        assert_eq!(*source.samples, *before);
        assert!(
            audio_transients(&source, &|| true)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }
    #[test]
    fn memory_admission_rejects_large_sources_before_reading() {
        let request = TransientRequest::new(48000, 2);
        assert!(analysis_storage(request, 48000).is_ok());
        assert!(
            analysis_storage(request, 48_000 * 60 * 10)
                .unwrap_err()
                .to_string()
                .contains("128 MiB")
        );
        assert!(analysis_storage(request, usize::MAX).is_err());
    }
    #[test]
    fn invalid_layout_and_nonfinite_input_are_reported() {
        let mut source = audio();
        source.metadata.channels = 3;
        assert!(audio_transients(&source, &|| false).is_err());
        source.metadata.channels = 2;
        Arc::make_mut(&mut source.samples)[0][0] = f32::NAN;
        assert!(
            audio_transients(&source, &|| false)
                .unwrap_err()
                .to_string()
                .contains("NaN")
        );
    }
}
