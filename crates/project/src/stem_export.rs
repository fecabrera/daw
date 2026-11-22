//! Aligned delivery of isolated post-fader track captures.
use crate::{Result, Session, absolute, error, file_publish};
use daw_core::{BounceMode, BounceRequest, Id};
use daw_engine::{BounceCapture, bounce_tail_frames};
use daw_media::{AudioEncoder, AudioReservation, WavCodec, WavEncoder};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

impl Session {
    pub fn export_stems(
        &self,
        directory: &Path,
        tracks: Vec<Id>,
    ) -> Result<(Vec<PathBuf>, Vec<String>)> {
        let requested = absolute(directory)?;
        let directory = file_publish::canonical_entry(&requested).map_err(error)?;
        self.check_export_destination(&directory, false)?;
        let target = self
            .project
            .bounce_targets(&BounceRequest {
                mode: BounceMode::Tracks,
                ids: tracks,
            })
            .map_err(error)?;
        if target.targets.is_empty() {
            return Err(error("Select at least one track with arranged material"));
        }

        let mut snapshot = self.clone();
        snapshot.prepare_project_audio(&|| false, &|_, _| {})?;
        let plan = snapshot.plan();
        let rate = self.project.sample_rate_hz;
        let bounds = target
            .targets
            .iter()
            .map(|target| {
                let (tail, capped) = bounce_tail_frames(&plan, target.track_id);
                target
                    .end_frame
                    .checked_add(tail)
                    .map(|end| (end, capped))
                    .ok_or_else(|| error("Stem length overflow"))
            })
            .collect::<Result<Vec<_>>>()?;
        let end = bounds
            .iter()
            .map(|(end, _)| *end)
            .max()
            .expect("non-empty captures");
        if end > (u64::from(u32::MAX) - 128) / 8 {
            return Err(error("Stem exceeds the WAV size limit"));
        }
        let parent = directory.parent().expect("canonical parent");
        let stage = parent.join(format!(".daw-stems-{}", Id::new_v4()));
        fs::create_dir(&stage).map_err(error)?;
        let result = (|| {
            let mut files = Vec::new();
            let mut warnings = target.warnings;
            for (target, (track_end, capped)) in target.targets.iter().zip(bounds) {
                if capped {
                    warnings.push(format!(
                        "Track {} reached the render tail limit",
                        target.track_id
                    ));
                }
                let name = format!("{}.wav", target.track_id);
                self.check_export_destination(&directory.join(&name), false)?;
                let file = stage.join(&name);
                let mut encoder =
                    WavEncoder::at_sample_rate(&file, WavCodec::Float32, rate).map_err(error)?;
                let mut capture = BounceCapture::new(plan.clone(), target).map_err(error)?;
                let block_size = self.project.buffer_size_frames as usize;
                let _memory =
                    AudioReservation::new(0, capture.memory_bytes() + block_size as u64 * 8)
                        .map_err(error)?;
                let mut block = Vec::with_capacity(block_size);
                let mut content = 0;
                while content < end {
                    let sample = if content < track_end {
                        let Some(sample) = capture.next_frame().map_err(error)? else {
                            continue;
                        };
                        let fade = if capped {
                            ((track_end - 1 - content) as f32 / (rate / 200).max(1) as f32).min(1.0)
                        } else {
                            1.0
                        };
                        sample.main.map(|sample| sample * fade)
                    } else {
                        [0.0; 2]
                    };
                    content += 1;
                    block.push(sample);
                    if block.len() == block_size {
                        encoder.write_frames(&block).map_err(error)?;
                        block.clear();
                    }
                }
                encoder.write_frames(&block).map_err(error)?;
                encoder.finish().map_err(error)?;
                files.push(directory.join(name));
            }
            if file_publish::canonical_entry(&requested).map_err(error)? != directory {
                return Err(error("Stem destination changed during export"));
            }
            self.check_export_destination(&directory, false)?;
            for file in &files {
                self.check_export_destination(file, false)?;
            }
            file_publish::create_directory(&stage, &directory).map_err(error)?;
            Ok((files, warnings))
        })();
        if result.is_err()
            && let Err(cleanup) = fs::remove_dir_all(&stage)
            && cleanup.kind() != io::ErrorKind::NotFound
        {
            return result.map_err(|failure| {
                error(format!(
                    "{failure}; could not remove stem staging directory: {cleanup}"
                ))
            });
        }
        result
    }
}
