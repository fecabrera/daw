//! Worker analysis of arranged dry clip sections. Publication remains one core edit.
use crate::{Result, Session, error};
use daw_core::{Edit, Id};
pub use daw_loudness::MeasureMode as NormalizeMode;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug)]
pub struct NormalizeOptions {
    pub mode: NormalizeMode,
    pub target: f32,
    pub allow_increase: bool,
}
impl Default for NormalizeOptions {
    fn default() -> Self {
        Self {
            mode: NormalizeMode::SamplePeak,
            target: -1.0,
            allow_increase: true,
        }
    }
}
impl NormalizeOptions {
    pub fn validate(self) -> Result<()> {
        if !self.target.is_finite() || !(-70.0..=0.0).contains(&self.target) {
            return Err(error("Normalization target must be between −70 and 0"));
        }
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct NormalizeProgress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
}
impl NormalizeProgress {
    fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(error("Normalization cancelled"))
        } else {
            Ok(())
        }
    }
}
impl Session {
    /// Analyze without clip gain, fades, mute, track processing or effects.
    /// Each channel receives the same gain to preserve the stereo balance.
    pub fn normalize_clips(
        &self,
        ids: &[Id],
        options: NormalizeOptions,
        progress: &NormalizeProgress,
    ) -> Result<Vec<Edit>> {
        options.validate()?;
        progress.check()?;
        self.project.validate().map_err(error)?;
        if ids.is_empty() {
            return Err(error("Select at least one audio clip"));
        }
        let mut seen = std::collections::HashSet::new();
        let mut clips = Vec::new();
        let mut total = 0u64;
        for id in ids {
            if !seen.insert(*id) {
                continue;
            }
            let clip = self
                .project
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| c.id == *id)
                .ok_or_else(|| error("Normalization clip no longer exists"))?;
            if !self
                .project
                .assets
                .iter()
                .any(|a| a.id == clip.asset_id && a.is_audio())
            {
                return Err(error("Normalization requires audio clips only"));
            }
            total = total
                .checked_add(clip.length_frames)
                .ok_or_else(|| error("Normalization duration overflow"))?;
            clips.push(clip);
        }
        progress.total.store(total, Ordering::Relaxed);
        progress.done.store(0, Ordering::Relaxed);
        let mut done = 0;
        let mut edits = Vec::new();
        for clip in clips {
            let analyze = || -> Result<Option<f32>> {
                let audio = self
                    .audio_for_clip(clip)
                    .ok_or_else(|| error("Prepared clip audio is missing"))?;
                let mut reader = daw_media::FrameReader::new(audio);
                let mut analyzer =
                    daw_loudness::OfflineAnalyzer::new(self.project.sample_rate_hz, options.mode)
                        .map_err(error)?;
                for local in 0..clip.length_frames {
                    if local.is_multiple_of(4096) {
                        progress.check()?;
                        progress.done.store(done + local, Ordering::Relaxed);
                    }
                    let source = clip.source_frame(local);
                    if source >= audio.frame_count() {
                        return Err(error("Clip source range is unavailable"));
                    }
                    analyzer
                        .push(reader.sample(source).map_err(error)?)
                        .map_err(error)?;
                }
                analyzer.finish().map_err(error)
            };
            if let Some(measured) = analyze().map_err(|e| error(format!("{}: {e}", clip.name)))? {
                let requested = options.target - measured;
                let gain = if options.allow_increase {
                    requested
                } else {
                    requested.min(clip.gain_db)
                };
                if !(daw_core::SILENT_GAIN_DB..=daw_core::CLIP_MAX_GAIN_DB).contains(&gain) {
                    return Err(error(format!(
                        "{} needs {gain:.2} dB clip gain, outside the supported range",
                        clip.name
                    )));
                }
                if (gain - clip.gain_db).abs() > 0.0001 {
                    edits.push(Edit::SetClipGain {
                        clip_id: clip.id,
                        gain_db: gain,
                    });
                }
            }
            done += clip.length_frames;
            progress.done.store(done, Ordering::Relaxed);
        }
        progress.check()?;
        // Validate all results before returning anything publishable.
        self.project
            .edited(Edit::Batch(edits.clone()))
            .map_err(error)?;
        Ok(edits)
    }
}
