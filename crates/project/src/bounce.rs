//! Background capture and all-or-none preparation of generated bounce media.
use crate::{Session, error, generated_media::GeneratedDirectory};
use daw_core::{
    Asset, BounceOutput, BounceRequest, Clip, ClipFades, Id, RenderedAudio, RenderedFeed, Source,
    SourceMetadata, SourceStorage, TrackKind,
};
use daw_media::{AudioEncoder, AudioReservation, WavCodec, WavEncoder};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Default)]
pub struct BounceProgress {
    pub cancel: Arc<AtomicBool>,
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
}
#[derive(Debug, thiserror::Error)]
pub enum BounceError {
    #[error("Bounce cancelled")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}
impl From<crate::Error> for BounceError {
    fn from(e: crate::Error) -> Self {
        Self::Failed(e.to_string())
    }
}
impl BounceProgress {
    pub(super) fn check(&self) -> Result<(), BounceError> {
        if self.cancel.load(Ordering::Acquire) {
            Err(BounceError::Cancelled)
        } else {
            Ok(())
        }
    }
}
/// The complete candidate retains shared generated-file owners. Dropping an
/// unpublished result removes all its files, including late multi-target failure.
pub struct PreparedBounce {
    directory: std::path::PathBuf,
    pub session: Session,
    pub warnings: Vec<String>,
    pub source_project: daw_core::Project,
}
impl PreparedBounce {
    /// Worker-only disposal with acknowledged filesystem errors.
    pub fn discard(self) -> crate::Result<()> {
        let path = self.directory.clone();
        drop(self);
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(error(format!(
                "Cannot remove unpublished bounce media: {e}"
            ))),
        }
    }
}
impl Session {
    pub fn bounce(
        &self,
        request: &BounceRequest,
        progress: &BounceProgress,
    ) -> Result<PreparedBounce, BounceError> {
        progress.check()?;
        let directory = Arc::new(GeneratedDirectory::new()?);
        let result = self.bounce_in_directory(request, progress, &directory);
        if result.is_err() {
            directory.cleanup()?;
        }
        result
    }
    fn bounce_in_directory(
        &self,
        request: &BounceRequest,
        progress: &BounceProgress,
        directory: &Arc<GeneratedDirectory>,
    ) -> Result<PreparedBounce, BounceError> {
        progress.check()?;
        self.project.validate().map_err(error)?;
        let selection = self.project.bounce_targets(request).map_err(error)?;
        let mut snapshot = self.clone();
        snapshot
            .prepare_stretches_cancellable(&|| progress.cancel.load(Ordering::Acquire))
            .map_err(|e| preparation_error(e, progress))?;
        progress.check()?;
        let rate = self.project.sample_rate_hz;
        let warnings = selection.warnings;
        let mut bounds = Vec::new();
        let mut total = 0u64;
        for target in &selection.targets {
            let (tail, capped) = daw_engine::bounce_tail_frames(&snapshot.plan(), target.track_id);
            let end = target
                .end_frame
                .checked_add(tail)
                .ok_or_else(|| error("Bounce duration overflow"))?;
            let length = end - target.start_frame;
            if length > (u64::from(u32::MAX) - 128) / 8 {
                return Err(
                    error("Bounce exceeds the WAV size limit; shorten the arrangement").into(),
                );
            }
            total = total
                .checked_add(end)
                .ok_or_else(|| error("Bounce progress overflow"))?;
            bounds.push((end, capped));
        }
        progress.total.store(total, Ordering::Release);
        let mut next = self.clone();
        next.temporary_stems.push(directory.clone());
        let mut outputs = Vec::new();
        let mut completed = 0;
        for (target, (end, capped)) in selection.targets.iter().zip(bounds) {
            progress.check()?;
            let original = self
                .project
                .tracks
                .iter()
                .find(|t| t.id == target.track_id)
                .unwrap();
            let mut capture = daw_engine::BounceCapture::new(snapshot.plan(), target)
                .map_err(BounceError::Failed)?;
            let _memory = AudioReservation::new(0, capture.memory_bytes()).map_err(error)?;
            let providers: Vec<_> = capture.lineage_providers().collect();
            let ids: Vec<_> = (0..3 + providers.len() * 2).map(|_| Id::new_v4()).collect();
            let paths: Vec<_> = ids
                .iter()
                .map(|id| directory.0.join(format!("{id}.wav")))
                .collect();
            let mut encoders = Vec::new();
            for path in &paths {
                encoders.push(
                    WavEncoder::at_sample_rate(path, WavCodec::Float32, rate).map_err(error)?,
                );
            }
            let block_frames = rate.min(self.project.buffer_size_frames) as usize;
            let _blocks_memory =
                AudioReservation::new(0, (block_frames * ids.len() * 8) as u64).map_err(error)?;
            let mut blocks: Vec<Vec<[f32; 2]>> = (0..ids.len())
                .map(|_| Vec::with_capacity(block_frames))
                .collect();
            let feed_end = end
                .checked_add(capture.latency_frames())
                .ok_or_else(|| error("Bounce latency overflow"))?;
            let mut content = 0;
            for feed in 0..feed_end {
                if feed % block_frames as u64 == 0 {
                    progress.check()?;
                }
                if let Some(sample) = capture.next_frame().map_err(BounceError::Failed)? {
                    if content >= target.start_frame {
                        let terminal = if capped {
                            ((end - 1 - content) as f32 / (rate / 200).max(1) as f32).min(1.0)
                        } else {
                            1.0
                        };
                        for (block, frame) in blocks.iter_mut().zip(
                            [sample.main, sample.raw, sample.pre_send]
                                .into_iter()
                                .chain(
                                    capture
                                        .lineage_frames()
                                        .iter()
                                        .flat_map(|(raw, post)| [*raw, *post]),
                                ),
                        ) {
                            block.push(frame.map(|v| v * terminal));
                        }
                        if blocks[0].len() == block_frames {
                            write_blocks(&mut encoders, &mut blocks)?;
                        }
                    }
                    content += 1;
                    progress.done.store(completed + content, Ordering::Release);
                }
            }
            write_blocks(&mut encoders, &mut blocks)?;
            for encoder in encoders {
                progress.check()?;
                encoder.finish().map_err(error)?;
            }
            drop(capture);
            drop(_memory);
            let length = crate::bounce_tail::compact(
                &paths,
                target.end_frame - target.start_frame,
                end - target.start_frame,
                rate,
                progress,
            )?;
            let mut assets = Vec::new();
            for (&id, path) in ids.iter().zip(&paths) {
                progress.check()?;
                let data = daw_media::decode_audio_cached(path, rate, &|| {
                    progress.cancel.load(Ordering::Acquire)
                })
                .map_err(|e| preparation_error(error(e), progress))?;
                if data.frame_count() != length {
                    return Err(error("Bounce output length mismatch").into());
                }
                next.audio.insert(id, data);
                assets.push(Asset {
                    rendered_audio: None,
                    compound_audio: None,
                    midi_pattern: None,
                    beat_pattern: None,
                    id,
                    name: original.name.clone(),
                    source: Source {
                        kind: "generated".into(),
                        storage: Some(SourceStorage::Project),
                        path: path.to_string_lossy().into_owned(),
                        path_kind: "absolute".into(),
                    },
                    source_metadata: SourceMetadata {
                        sample_rate_hz: rate,
                        channels: 2,
                        sample_format: "ieee_float".into(),
                        bits_per_sample: 32,
                    },
                    decoded_frame_count: length,
                });
            }
            assets[0].source.kind = "rendered_audio".into();
            assets[0].rendered_audio = Some(RenderedAudio {
                version: 1,
                pre_send_asset: ids[2],
                feeds: std::iter::once(RenderedFeed {
                    provider: original.id,
                    raw_asset: ids[1],
                    post_asset: ids[0],
                })
                .chain(
                    providers
                        .iter()
                        .enumerate()
                        .map(|(i, &provider)| RenderedFeed {
                            provider,
                            raw_asset: ids[3 + i * 2],
                            post_asset: ids[4 + i * 2],
                        }),
                )
                .collect(),
            });
            let mut track = original.clone();
            track.id = Id::new_v4();
            track.kind = TrackKind::Audio;
            track.group_collapsed = false;
            track.midi_options = Default::default();
            track.instrument = None;
            track.beat = None;
            track.effects.clear();
            track.gain_db = 0.0;
            track.pan = 0.0;
            track.name = unique_name(&self.project, &outputs, &original.name);
            for send in &mut track.sends {
                send.id = Id::new_v4();
            }
            let mut fades = ClipFades::default();
            fades.fade_in.inherit_source = true;
            fades.fade_out.inherit_source = true;
            track.clips = vec![Clip {
                id: Id::new_v4(),
                asset_id: ids[0],
                name: track.name.clone(),
                color: None,
                start_frame: target.start_frame,
                source_offset_frame: 0,
                length_frames: length,
                repeat: None,
                stretch: None,
                gain_db: 0.0,
                transpose: 0,
                channels: Default::default(),
                fades,
                muted: false,
            }];
            outputs.push(BounceOutput {
                source_track: original.id,
                assets,
                track,
            });
            completed += end;
        }
        progress.check()?;
        next.project
            .edit(daw_core::Edit::Bounce {
                request: request.clone(),
                outputs,
            })
            .map_err(error)?;
        next.prepare_stretches_cancellable(&|| progress.cancel.load(Ordering::Acquire))
            .map_err(|e| preparation_error(e, progress))?;
        // Verify complete playable readers on the worker. Activation/catch-up is
        // acknowledged separately by the host before it dismisses progress.
        let mut renderer = daw_engine::Renderer::new(next.plan());
        renderer.prepare_streaming().map_err(BounceError::Failed)?;
        let _renderer_memory =
            AudioReservation::new(0, renderer.owned_memory_bytes()).map_err(error)?;
        progress.check()?;
        next.warnings.extend(warnings.iter().cloned());
        Ok(PreparedBounce {
            directory: directory.0.clone(),
            session: next,
            warnings,
            source_project: self.project.clone(),
        })
    }
}
fn write_blocks(
    encoders: &mut [WavEncoder],
    blocks: &mut [Vec<[f32; 2]>],
) -> Result<(), BounceError> {
    for (encoder, block) in encoders.iter_mut().zip(blocks) {
        encoder.write_frames(block).map_err(error)?;
        block.clear();
    }
    Ok(())
}
pub(super) fn preparation_error(error: crate::Error, progress: &BounceProgress) -> BounceError {
    if progress.cancel.load(Ordering::Acquire)
        && matches!(
            error.0.as_str(),
            "Audio preparation cancelled"
                | "Sample preparation cancelled"
                | "Time stretch cancelled"
                | "time stretch cancelled"
                | "Source verification cancelled"
        )
    {
        BounceError::Cancelled
    } else {
        BounceError::Failed(error.to_string())
    }
}
fn unique_name(project: &daw_core::Project, outputs: &[BounceOutput], original: &str) -> String {
    let base = format!("{original} Bounce");
    let mut name = base.clone();
    let mut n = 2;
    while project.tracks.iter().any(|t| t.name == name)
        || outputs.iter().any(|o| o.track.name == name)
    {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}
