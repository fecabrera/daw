use crate::{BUFFER_SIZES, Error, Project, Result, SAMPLE_RATES};

impl Project {
    /// Prepare new frame coordinates without changing durations or source recordings.
    /// The caller must prepare matching audio before publishing this project.
    pub fn with_audio_settings(
        &self,
        sample_rate_hz: u32,
        buffer_size_frames: u32,
    ) -> Result<Self> {
        self.validate()?;
        if !SAMPLE_RATES.contains(&sample_rate_hz) || !BUFFER_SIZES.contains(&buffer_size_frames) {
            return Err(Error("Unsupported project audio settings".into()));
        }
        let mut next = self.clone();
        next.buffer_size_frames = buffer_size_frames;
        if sample_rate_hz == self.sample_rate_hz {
            return Ok(next);
        }
        let numerator = u128::from(sample_rate_hz);
        let denominator = u128::from(self.sample_rate_hz);
        let scale = |frame: u64| -> Result<u64> {
            u64::try_from((u128::from(frame) * numerator + denominator / 2) / denominator)
                .map_err(|_| Error("Sample rate change exceeds timeline bounds".into()))
        };
        for asset in &mut next.assets {
            asset.decoded_frame_count = u64::try_from(
                (u128::from(asset.decoded_frame_count) * numerator).div_ceil(denominator),
            )
            .map_err(|_| Error("Sample rate change exceeds source bounds".into()))?;
        }
        for clip in next.tracks.iter_mut().flat_map(|track| &mut track.clips) {
            let end = scale(clip.end())?;
            let source_length = clip
                .repeat
                .map_or(clip.length_frames, |repeat| repeat.length_frames);
            let source_end = scale(
                clip.source_offset_frame
                    .checked_add(source_length)
                    .ok_or_else(|| Error("Source range exceeds supported bounds".into()))?,
            )?;
            clip.start_frame = scale(clip.start_frame)?;
            clip.length_frames = end
                .checked_sub(clip.start_frame)
                .filter(|length| *length > 0)
                .ok_or_else(|| Error("Sample rate change would make a clip empty".into()))?;
            clip.source_offset_frame = scale(clip.source_offset_frame)?;
            if let Some(repeat) = &mut clip.repeat {
                repeat.length_frames = source_end
                    .checked_sub(clip.source_offset_frame)
                    .filter(|length| *length > 0)
                    .ok_or_else(|| Error("Sample rate change would make a repeat empty".into()))?;
                repeat.phase_frame = scale(repeat.phase_frame)?.min(repeat.length_frames - 1);
            }
        }
        next.transport.playhead_frame = scale(next.transport.playhead_frame)?;
        next.transport.r#loop.start_frame = scale(next.transport.r#loop.start_frame)?;
        next.transport.r#loop.end_frame = scale(next.transport.r#loop.end_frame)?;
        next.sample_rate_hz = sample_rate_hz;
        next.validate()?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipLoop, ClipStretch};

    fn fixture() -> Project {
        let mut project = Project::default();
        let track = project.add_track().unwrap();
        let asset = crate::Id::new_v4();
        project.assets.push(crate::Asset {
            id: asset,
            name: "source.wav".into(),
            source: crate::Source {
                kind: "external".into(),
                path: "source.wav".into(),
                path_kind: "relative".into(),
            },
            source_metadata: crate::SourceMetadata {
                sample_rate_hz: 48_000,
                channels: 2,
                bits_per_sample: 16,
                sample_format: "pcm_int".into(),
            },
            decoded_frame_count: 96_000,
        });
        project
            .tracks
            .iter_mut()
            .find(|t| t.id == track)
            .unwrap()
            .clips
            .push(crate::Clip {
                id: crate::Id::new_v4(),
                asset_id: asset,
                name: "source".into(),
                color: None,
                start_frame: 0,
                source_offset_frame: 0,
                length_frames: 48_000,
                repeat: None,
                stretch: None,
            });
        project
    }

    #[test]
    fn rate_change_preserves_contiguous_boundaries_loops_sources_and_stretch() {
        let mut project = fixture();
        let clip = &mut project.tracks[0].clips[0];
        clip.start_frame = 48_000;
        clip.length_frames = 24_000;
        clip.source_offset_frame = 6_000;
        clip.repeat = Some(ClipLoop {
            length_frames: 12_000,
            phase_frame: 3_000,
        });
        clip.stretch = ClipStretch::new(2, 3);
        let mut neighbor = clip.clone();
        neighbor.id = crate::Id::new_v4();
        neighbor.start_frame = clip.end();
        project.tracks[0].clips = vec![clip.clone(), neighbor];
        project.transport.playhead_frame = 60_000;
        project.transport.r#loop = crate::Loop {
            enabled: false,
            start_frame: 24_000,
            end_frame: 96_000,
        };
        let before = format!("{project:?}");
        for rate in SAMPLE_RATES {
            let next = project.with_audio_settings(rate, 128).unwrap();
            let clip = &next.tracks[0].clips[0];
            assert_eq!(next.seconds(clip.start_frame), 1.0);
            assert_eq!(next.seconds(clip.length_frames), 0.5);
            assert!(
                (next.seconds(clip.source_offset_frame) - 0.125).abs()
                    <= 0.5 / f64::from(rate) + 1e-12
            );
            assert_eq!(next.seconds(clip.repeat.unwrap().length_frames), 0.25);
            assert!(
                (next.seconds(clip.repeat.unwrap().phase_frame) - 0.0625).abs()
                    <= 0.5 / f64::from(rate) + 1e-12
            );
            assert_eq!(clip.stretch, ClipStretch::new(2, 3));
            assert_eq!(clip.id, project.tracks[0].clips[0].id);
            assert_eq!(clip.end(), next.tracks[0].clips[1].start_frame);
            assert_eq!(next.seconds(next.transport.playhead_frame), 1.25);
            assert_eq!(next.seconds(next.transport.r#loop.start_frame), 0.5);
            assert_eq!(next.seconds(next.transport.r#loop.end_frame), 2.0);
            assert_eq!(next.buffer_size_frames, 128);
            assert_eq!(
                next.assets[0].source_metadata.sample_rate_hz,
                project.assets[0].source_metadata.sample_rate_hz
            );
        }
        assert_eq!(format!("{project:?}"), before);
    }

    #[test]
    fn invalid_settings_empty_clips_and_overflow_are_rejected_without_mutation() {
        let mut project = fixture();
        for (rate, size) in [
            (0, 512),
            (22_050, 512),
            (48_000, 0),
            (48_000, 63),
            (48_000, 8192),
        ] {
            assert!(project.with_audio_settings(rate, size).is_err());
        }
        project.sample_rate_hz = 192_000;
        project.tracks[0].clips[0].length_frames = 1;
        assert!(project.with_audio_settings(44_100, 512).is_err());
        project.sample_rate_hz = 48_000;
        project.tracks[0].clips[0].length_frames = 1;
        project.tracks[0].clips[0].start_frame = u64::MAX - 10;
        assert!(project.with_audio_settings(192_000, 512).is_err());
    }
}
