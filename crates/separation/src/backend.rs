//! Supported-target Charon adapter. Backend types stay private.
use super::{Error, MODEL_RATE, Models, Progress, STEMS};
use charon_audio::{AudioSource, CancelToken, Control, Separator, SeparatorConfig, StemSink};
use daw_media::{AudioEncoder, WavCodec, WavEncoder};
use std::{path::Path, sync::atomic::Ordering};

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
                config.model.onnx.coreml_cache_dir = Some(cache.join("DAW/Charon/CoreML/ort-1.31"));
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
    #[test]
    fn native_cpu_stream_and_failed_coreml_fallback_preserve_stereo_and_duration() {
        let directory = std::env::temp_dir().join(format!(
            "daw-native-stems-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/charon-audio/tests/fixtures/identity_4stems.onnx");
        let samples: Vec<_> = (0..128)
            .map(|n| [(n as f32 * 0.1).sin() * 0.2, (n as f32 * 0.07).cos() * 0.1])
            .collect();
        for coreml in [None, Some(directory.join("missing.onnx"))] {
            let fallback = coreml.is_some();
            let warnings = super::separate(
                &samples,
                &super::Models {
                    cpu: Some(model.clone()),
                    coreml,
                },
                &directory,
                &super::Progress::default(),
            )
            .unwrap();
            if cfg!(target_os = "macos") && fallback {
                assert!(
                    warnings
                        .iter()
                        .any(|w| w.contains("CoreML failed; used CPU"))
                );
            }
            for stem in super::STEMS {
                let audio = daw_media::decode_wav_at_rate(
                    &directory.join(format!("{stem}.wav")),
                    super::MODEL_RATE,
                )
                .unwrap();
                assert_eq!(audio.samples.len(), samples.len());
                for (actual, expected) in audio.samples.iter().zip(&samples) {
                    for ch in 0..2 {
                        assert!((actual[ch] - expected[ch]).abs() < 1e-5);
                    }
                }
            }
        }
    }

    // Synthetic ONNX protobuf, using the malformed Loop topology from upstream
    // regression #32609. No model weights, protobuf generator, or Python is needed.
    fn varint(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        while value >= 128 {
            bytes.push(value as u8 | 128);
            value >>= 7;
        }
        bytes.push(value as u8);
        bytes
    }
    fn integer(field: u64, value: u64) -> Vec<u8> {
        [varint(field << 3), varint(value)].concat()
    }
    fn message(field: u64, value: &[u8]) -> Vec<u8> {
        [
            varint(field << 3 | 2),
            varint(value.len() as u64),
            value.to_vec(),
        ]
        .concat()
    }
    fn value(name: &str, kind: u64) -> Vec<u8> {
        let shape = message(1, &integer(1, 1));
        let tensor = [integer(1, kind), message(2, &shape)].concat();
        [
            message(1, name.as_bytes()),
            message(2, &message(1, &tensor)),
        ]
        .concat()
    }
    fn identity(input: &str, output: &str) -> Vec<u8> {
        [
            message(1, input.as_bytes()),
            message(2, output.as_bytes()),
            message(4, b"Identity"),
        ]
        .concat()
    }
    fn malformed_loop() -> Vec<u8> {
        let initializer = [
            integer(1, 1),
            integer(2, 1),
            message(8, b"state_in"),
            message(9, &[0; 4]),
        ]
        .concat();
        let body = [
            message(1, &identity("cond_in", "cond_out")),
            message(1, &identity("state_in", "state_out")),
            message(2, b"body"),
            message(5, &initializer),
            message(11, &value("iter_num_in", 7)),
            message(11, &value("cond_in", 9)),
            message(11, &value("state_in", 1)),
            message(12, &value("cond_out", 9)),
            message(12, &value("state_out", 1)),
        ]
        .concat();
        let attribute = [message(1, b"body"), message(6, &body), integer(20, 5)].concat();
        let node = [
            message(1, b"M"),
            message(1, b"cond"),
            message(1, b"state"),
            message(2, b"final_state"),
            message(3, b"loop"),
            message(4, b"Loop"),
            message(5, &attribute),
        ]
        .concat();
        let graph = [
            message(1, &node),
            message(2, b"malformed-loop"),
            message(11, &value("M", 7)),
            message(11, &value("cond", 9)),
            message(11, &value("state", 1)),
            message(12, &value("final_state", 1)),
        ]
        .concat();
        [
            integer(1, 8),
            message(7, &graph),
            message(8, &integer(2, 13)),
        ]
        .concat()
    }

    #[test]
    fn malformed_loop_is_rejected_in_a_separate_process() {
        const CHILD: &str = "DAW_TEST_MALFORMED_LOOP_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "backend::tests::malformed_loop_is_rejected_in_a_separate_process",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(ort::info().contains("1.31.0"), "{}", ort::info());
        let error = ort::session::Session::builder()
            .unwrap()
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)
            .unwrap()
            .commit_from_memory(&malformed_loop())
            .unwrap_err();
        let text = error.to_string();
        assert_eq!(error.code(), ort::error::ErrorCode::InvalidGraph);
        assert!(
            text.contains("Subgraph input count does not match"),
            "{text}"
        );
        assert!(text.contains("also one of its initializers"), "{text}");
    }
}
