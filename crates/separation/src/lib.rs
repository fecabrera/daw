//! Offline model inference. No model downloads or audio-device dependencies.
use charon_audio::{AudioSource, CancelToken, Control, Separator, SeparatorConfig, StemSink};
use daw_media::{AudioEncoder, WavCodec, WavEncoder};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub const STEMS: [&str; 4] = ["drums", "bass", "other", "vocals"];
pub const MODEL_RATE: u32 = 44_100;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Models {
    pub cpu: Option<PathBuf>,
    pub coreml: Option<PathBuf>,
}
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

/// Counters describe model windows, not elapsed time. Zero total means loading.
#[derive(Clone, Default)]
pub struct Progress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: Arc<AtomicBool>,
}
impl Progress {
    pub fn check(&self) -> Result<(), Error> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error("Stem separation cancelled".into()))
        } else {
            Ok(())
        }
    }
}

/// Writes only into a newly created, caller-owned staging directory. A failed
/// CoreML attempt is discarded before the CPU retry opens fresh encoders.
pub fn separate(
    samples: &[[f32; 2]],
    models: &Models,
    directory: &Path,
    progress: &Progress,
) -> Result<Vec<String>, Error> {
    progress.check()?;
    if samples.is_empty() || samples.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error(
            "Separation needs finite, nonempty stereo audio".into(),
        ));
    }
    let cpu = models
        .cpu
        .as_ref()
        .filter(|p| p.is_file())
        .ok_or_else(|| Error("Select a local CPU HTDemucs ONNX model in Settings".into()))?;
    #[cfg(target_os = "macos")]
    let mut warnings = Vec::new();
    #[cfg(not(target_os = "macos"))]
    let warnings: Vec<String> = Vec::new();
    #[cfg(target_os = "macos")]
    if let Some(model) = &models.coreml {
        let result = dirs::cache_dir()
            .ok_or_else(|| Error("User cache directory is unavailable".into()))
            .and_then(|cache| {
                let mut config = SeparatorConfig::htdemucs_split(model);
                config.model.onnx.execution_provider = charon_audio::ExecutionProvider::CoreMl;
                config.model.onnx.coreml_cache_dir = Some(cache.join("DAW/Charon/CoreML/ort-1.28"));
                attempt(samples, config, directory, progress).map_err(|e| Error(e.to_string()))
            });
        match result {
            Ok(()) => return Ok(warnings),
            Err(e) => {
                progress.check()?;
                warnings.push(format!("CoreML failed; used CPU instead: {e}"));
                progress.done.store(0, Ordering::Relaxed);
                progress.total.store(0, Ordering::Relaxed);
            }
        }
    }
    #[cfg(target_os = "macos")]
    if models.coreml.is_none() {
        warnings.push("No CoreML model selected; used CPU".into());
    }
    attempt(samples, SeparatorConfig::htdemucs(cpu), directory, progress).map_err(|e| {
        Error(if warnings.is_empty() {
            e.to_string()
        } else {
            format!("{}\nCPU separation failed: {e}", warnings.join("\n"))
        })
    })?;
    Ok(warnings)
}
fn attempt(
    samples: &[[f32; 2]],
    config: SeparatorConfig,
    directory: &Path,
    progress: &Progress,
) -> charon_audio::Result<()> {
    let separator = Separator::new(config)?;
    let token = CancelToken::new();
    let callback_token = token.clone();
    let p = progress.clone();
    let control = Control::new()
        .with_cancel(token)
        .with_progress(move |window| {
            if p.cancel.load(Ordering::Relaxed) {
                callback_token.cancel();
            }
            p.total.store(window.total as u64, Ordering::Relaxed);
            p.done.store(window.done as u64, Ordering::Relaxed);
        });
    let mut source = Source { samples, progress };
    let mut sink = Sink {
        directory,
        progress,
        encoders: Vec::new(),
        counts: [0; 4],
        expected: samples.len(),
    };
    separator.separate_stream(&mut source, &mut sink, &control)?;
    progress
        .check()
        .map_err(|e| charon_audio::CharonError::Processing(e.to_string()))
}
struct Source<'a> {
    samples: &'a [[f32; 2]],
    progress: &'a Progress,
}
impl AudioSource for Source<'_> {
    fn channels(&self) -> usize {
        2
    }
    fn sample_rate(&self) -> u32 {
        MODEL_RATE
    }
    fn len(&self) -> usize {
        self.samples.len()
    }
    fn read(
        &mut self,
        start: usize,
        mut out: ndarray::ArrayViewMut2<f32>,
    ) -> charon_audio::Result<()> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return Err(charon_audio::CharonError::Cancelled);
        }
        for i in 0..out.ncols() {
            for ch in 0..2 {
                out[[ch, i]] = self.samples[start + i][ch];
            }
        }
        Ok(())
    }
}
struct Sink<'a> {
    directory: &'a Path,
    progress: &'a Progress,
    encoders: Vec<WavEncoder>,
    counts: [usize; 4],
    expected: usize,
}
impl StemSink for Sink<'_> {
    fn begin(
        &mut self,
        stems: &[String],
        channels: usize,
        sample_rate: u32,
    ) -> charon_audio::Result<()> {
        if channels != 2 || sample_rate != MODEL_RATE || !stems.iter().map(String::as_str).eq(STEMS)
        {
            return Err(charon_audio::CharonError::Processing(
                "Expected four stereo HTDemucs stems at 44.1 kHz".into(),
            ));
        }
        for name in STEMS {
            self.encoders.push(
                WavEncoder::at_sample_rate(
                    &self.directory.join(format!("{name}.wav")),
                    WavCodec::Float32,
                    MODEL_RATE,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }
    fn write(&mut self, stem: usize, block: ndarray::ArrayView2<f32>) -> charon_audio::Result<()> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return Err(charon_audio::CharonError::Cancelled);
        }
        if stem >= 4 || block.nrows() != 2 {
            return Err("Invalid stem output".into());
        }
        let frames: Vec<_> = (0..block.ncols())
            .map(|i| [block[[0, i]], block[[1, i]]])
            .collect();
        self.encoders[stem]
            .write_frames(&frames)
            .map_err(|e| e.to_string())?;
        self.counts[stem] += frames.len();
        Ok(())
    }
    fn finish(&mut self) -> charon_audio::Result<()> {
        if self.counts != [self.expected; 4] {
            return Err("Stem duration differs from input".into());
        }
        for encoder in self.encoders.drain(..) {
            encoder.finish().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_models_invalid_audio_and_cancellation_fail_without_output() {
        let directory = std::env::temp_dir().join("daw-separation-no-output");
        let p = Progress::default();
        assert!(separate(&[[f32::NAN, 0.0]], &Models::default(), &directory, &p).is_err());
        assert!(
            separate(&[[0.0; 2]], &Models::default(), &directory, &p)
                .unwrap_err()
                .to_string()
                .contains("Settings")
        );
        p.cancel.store(true, Ordering::Relaxed);
        assert!(
            separate(&[[0.0; 2]], &Models::default(), &directory, &p)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }
}
