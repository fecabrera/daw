//! Supported-target Charon adapter. Backend types stay private.
use super::{Error, MODEL_RATE, ModelInfo, ModelKind, Models, Output, Progress, StemSet};
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
) -> Result<Output, Error> {
    progress.check()?;
    if samples.is_empty() || samples.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error(
            "Separation needs finite, nonempty stereo audio".into(),
        ));
    }
    run_models(models, progress, |config| {
        detect_and_run(samples, config, Some(directory), progress)
    })
}

fn config_for(kind: ModelKind, path: &Path) -> Result<SeparatorConfig, Error> {
    if !kind.available() {
        return Err(Error(format!(
            "{} is unavailable on this platform",
            kind.name()
        )));
    }
    if !path.is_file() {
        return Err(Error(format!(
            "{} file is missing: {}",
            kind.name(),
            path.display()
        )));
    }
    match kind {
        ModelKind::Cpu => Ok(SeparatorConfig::htdemucs(path)),
        ModelKind::CoreMl => {
            #[cfg(target_os = "macos")]
            {
                let cache = dirs::cache_dir()
                    .ok_or_else(|| Error("User cache directory is unavailable".into()))?;
                let mut config = SeparatorConfig::htdemucs_split(path);
                config.model.onnx.execution_provider = charon_audio::ExecutionProvider::CoreMl;
                config.model.onnx.coreml_cache_dir = Some(cache.join("DAW/Charon/CoreML/ort-1.31"));
                Ok(config)
            }
            #[cfg(not(target_os = "macos"))]
            unreachable!("CoreML availability was checked above")
        }
    }
}

fn run_models(
    models: &Models,
    progress: &Progress,
    run: impl FnMut(SeparatorConfig) -> Result<StemSet, Error>,
) -> Result<Output, Error> {
    #[cfg(target_os = "macos")]
    let mut run = run;
    progress.check()?;
    #[cfg(target_os = "macos")]
    let mut warnings = Vec::new();
    #[cfg(not(target_os = "macos"))]
    let warnings: Vec<String> = Vec::new();
    #[cfg(target_os = "macos")]
    if let Some(model) = &models.coreml {
        match config_for(ModelKind::CoreMl, model).and_then(&mut run) {
            Ok(stems) => return Ok(Output { stems, warnings }),
            Err(e) => {
                progress.check()?;
                if models.cpu.is_none() {
                    return Err(Error(format!(
                        "CoreML separation failed: {e}\nNo CPU fallback model is selected"
                    )));
                }
                warnings.push(format!("CoreML failed; used CPU instead: {e}"));
                progress.done.store(0, Ordering::Relaxed);
                progress.total.store(0, Ordering::Relaxed);
            }
        }
    }
    let cpu = models.cpu.as_ref().ok_or_else(|| {
        Error("Select and validate a supported HTDemucs ONNX model in Settings".into())
    })?;
    #[cfg(target_os = "macos")]
    if models.coreml.is_none() {
        warnings.push("No CoreML model selected; used CPU".into());
    }
    let stems = config_for(ModelKind::Cpu, cpu).and_then(run).map_err(|e| {
        Error(if warnings.is_empty() {
            e.to_string()
        } else {
            format!("{}\nCPU separation failed: {e}", warnings.join("\n"))
        })
    })?;
    Ok(Output { stems, warnings })
}

pub(super) fn validate_model(
    kind: ModelKind,
    path: &Path,
    progress: &Progress,
) -> Result<ModelInfo, Error> {
    // A full model window is padded by Charon, but no stem files are created.
    let probe: [[f32; 2]; 128] = std::array::from_fn(|i| {
        let value = (i as f32 * 0.1).sin() * 0.1;
        [value, -value * 0.5]
    });
    let stems = detect_and_run(&probe, config_for(kind, path)?, None, progress)?;
    let file_size_bytes = std::fs::metadata(path)
        .map_err(|e| Error(format!("Cannot read model size: {e}")))?
        .len();
    progress.check()?;
    Ok(ModelInfo {
        stems,
        file_size_bytes,
    })
}

/// The exports use symbolic output dimensions. Probe the supported contracts
/// through Charon itself; do not infer a count from a filename or error text.
fn detect_and_run(
    samples: &[[f32; 2]],
    mut config: SeparatorConfig,
    directory: Option<&Path>,
    progress: &Progress,
) -> Result<StemSet, Error> {
    let mut errors = Vec::new();
    for stems in [StemSet::Four, StemSet::Six] {
        progress.check()?;
        progress.done.store(0, Ordering::Relaxed);
        progress.total.store(0, Ordering::Relaxed);
        if let Some(directory) = directory {
            // Only fixed output names in this job's reserved directory are
            // removed. Failed attempts may have used a different stem count.
            for name in StemSet::Six.names() {
                match std::fs::remove_file(directory.join(format!("{name}.wav"))) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(Error(format!("Cannot clear staged stem: {e}"))),
                }
            }
        }
        config.model.sources = stems
            .names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        match attempt(samples, config.clone(), stems, directory, progress) {
            Ok(()) => return Ok(stems),
            Err(error) => {
                progress.check()?;
                errors.push(format!("{}-stem contract: {error}", stems.names().len()));
            }
        }
    }
    Err(Error(format!(
        "Model does not produce a supported four- or six-stem output:\n{}",
        errors.join("\n")
    )))
}

fn attempt(
    samples: &[[f32; 2]],
    config: SeparatorConfig,
    stems: StemSet,
    directory: Option<&Path>,
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
        counts: vec![0; stems.names().len()],
        stems,
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
    directory: Option<&'a Path>,
    progress: &'a Progress,
    encoders: Vec<WavEncoder>,
    counts: Vec<usize>,
    stems: StemSet,
    expected: usize,
}
impl StemSink for Sink<'_> {
    fn begin(
        &mut self,
        stems: &[String],
        channels: usize,
        sample_rate: u32,
    ) -> charon_audio::Result<()> {
        if channels != 2
            || sample_rate != MODEL_RATE
            || !stems
                .iter()
                .map(String::as_str)
                .eq(self.stems.names().iter().copied())
        {
            return Err(charon_audio::CharonError::Processing(format!(
                "Expected {} stereo HTDemucs stems at 44.1 kHz",
                self.stems.names().len()
            )));
        }
        if let Some(directory) = self.directory {
            for name in self.stems.names() {
                self.encoders.push(
                    WavEncoder::at_sample_rate(
                        &directory.join(format!("{name}.wav")),
                        WavCodec::Float32,
                        MODEL_RATE,
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
        }
        Ok(())
    }
    fn write(&mut self, stem: usize, block: ndarray::ArrayView2<f32>) -> charon_audio::Result<()> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return Err(charon_audio::CharonError::Cancelled);
        }
        if stem >= self.counts.len() || block.nrows() != 2 {
            return Err("Invalid stem output".into());
        }
        if block.iter().any(|v| !v.is_finite()) {
            return Err("Stem output contains non-finite samples".into());
        }
        let count = self.counts[stem]
            .checked_add(block.ncols())
            .filter(|n| *n <= self.expected)
            .ok_or("Stem duration exceeds input")?;
        if self.directory.is_some() {
            let frames: Vec<_> = (0..block.ncols())
                .map(|i| [block[[0, i]], block[[1, i]]])
                .collect();
            self.encoders[stem]
                .write_frames(&frames)
                .map_err(|e| e.to_string())?;
        }
        self.counts[stem] = count;
        Ok(())
    }
    fn finish(&mut self) -> charon_audio::Result<()> {
        if self.counts.iter().any(|count| *count != self.expected) {
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
    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/charon-audio/tests/fixtures")
            .join(name)
    }

    #[test]
    fn six_stem_models_validate_their_count_and_stream_all_outputs() {
        let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/identity_6stems.onnx");
        let progress = super::Progress::default();
        let info = super::validate_model(super::ModelKind::Cpu, &model, &progress).unwrap();
        assert_eq!(info.stems, super::StemSet::Six);
        assert_eq!(
            info.file_size_bytes,
            std::fs::metadata(&model).unwrap().len()
        );
        let directory = std::env::temp_dir().join(format!(
            "daw-six-stems-{}-{}",
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
        let samples: Vec<_> = (0..128)
            .map(|n| [(n as f32 * 0.1).sin() * 0.2, (n as f32 * 0.07).cos() * 0.1])
            .collect();
        let result = super::separate(
            &samples,
            &super::Models {
                cpu: Some(model),
                coreml: Some(directory.join("missing.onnx")),
            },
            &directory,
            &progress,
        );
        let output = result.unwrap();
        assert_eq!(output.stems, super::StemSet::Six);
        if cfg!(target_os = "macos") {
            assert!(
                output
                    .warnings
                    .iter()
                    .any(|w| w.contains("CoreML failed; used CPU"))
            );
        }
        for name in super::StemSet::Six.names() {
            let audio = daw_media::decode_wav_at_rate(
                &directory.join(format!("{name}.wav")),
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

    #[test]
    fn validation_loads_the_backend_and_rejects_the_wrong_contract() {
        let p = super::Progress::default();
        let model = fixture("identity_4stems.onnx");
        let info = super::validate_model(super::ModelKind::Cpu, &model, &p).unwrap();
        assert_eq!(info.stems, super::StemSet::Four);
        assert_eq!(
            info.file_size_bytes,
            std::fs::metadata(model).unwrap().len()
        );
        assert!(
            super::validate_model(super::ModelKind::Cpu, &fixture("identity_split.onnx"), &p)
                .is_err()
        );
        assert!(
            super::validate_model(super::ModelKind::Cpu, &fixture("missing.onnx"), &p).is_err()
        );
        p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            crate::validate_model(super::ModelKind::Cpu, &fixture("identity_4stems.onnx"), &p)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn native_coreml_validation_checks_split_model_without_cpu_fallback() {
        let cache = std::env::temp_dir().join(format!(
            "daw-model-validation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&cache).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(cache.clone());
        for (model, accepted) in [
            ("identity_split.onnx", true),
            ("identity_4stems.onnx", false),
        ] {
            let mut config = super::config_for(super::ModelKind::CoreMl, &fixture(model)).unwrap();
            config.model.onnx.coreml_cache_dir = Some(cache.clone());
            let result = super::attempt(
                &[[0.1, -0.05]; 128],
                config,
                super::StemSet::Four,
                None,
                &super::Progress::default(),
            );
            assert_eq!(result.is_ok(), accepted, "{model}: {result:?}");
        }
    }

    #[test]
    fn validation_rejects_invalid_stem_metadata_samples_and_lengths() {
        use charon_audio::StemSink;
        let progress = super::Progress::default();
        let mut sink = super::Sink {
            directory: None,
            progress: &progress,
            encoders: Vec::new(),
            counts: vec![0; 4],
            stems: super::StemSet::Four,
            expected: 2,
        };
        let stems: Vec<_> = crate::STEMS.iter().map(|s| s.to_string()).collect();
        assert!(sink.begin(&stems, 1, super::MODEL_RATE).is_err());
        assert!(sink.begin(&stems, 2, 48_000).is_err());
        assert!(sink.begin(&stems[..3], 2, super::MODEL_RATE).is_err());
        sink.begin(&stems, 2, super::MODEL_RATE).unwrap();
        assert!(
            sink.write(0, ndarray::Array2::from_elem((2, 2), f32::NAN).view())
                .is_err()
        );
        assert!(
            sink.write(0, ndarray::Array2::zeros((1, 2)).view())
                .is_err()
        );
        assert!(
            sink.write(4, ndarray::Array2::zeros((2, 2)).view())
                .is_err()
        );
        assert!(
            sink.write(0, ndarray::Array2::zeros((2, 3)).view())
                .is_err()
        );
        assert!(sink.finish().is_err());
        for stem in 0..4 {
            sink.write(stem, ndarray::Array2::zeros((2, 2)).view())
                .unwrap();
        }
        sink.finish().unwrap();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn coreml_is_preferred_and_cpu_fallback_is_optional() {
        use super::{Error, ModelKind, Models, Progress};
        use charon_audio::ExecutionProvider;
        let model = fixture("identity_4stems.onnx");
        for cpu in [None, Some(fixture("missing.onnx"))] {
            let mut calls = 0;
            let warnings = super::run_models(
                &Models {
                    cpu,
                    coreml: Some(model.clone()),
                },
                &Progress::default(),
                |config| {
                    calls += 1;
                    assert_eq!(
                        config.model.onnx.execution_provider,
                        ExecutionProvider::CoreMl
                    );
                    Ok(super::StemSet::Four)
                },
            )
            .unwrap();
            assert_eq!(calls, 1);
            assert!(warnings.warnings.is_empty());
        }
        let models = Models {
            cpu: Some(model.clone()),
            coreml: Some(model.clone()),
        };
        let mut providers = Vec::new();
        let warnings = super::run_models(&models, &Progress::default(), |config| {
            let provider = config.model.onnx.execution_provider;
            providers.push(provider);
            if provider == ExecutionProvider::CoreMl {
                Err(Error("CoreML test failure".into()))
            } else {
                Ok(super::StemSet::Four)
            }
        })
        .unwrap();
        assert_eq!(
            providers,
            [ExecutionProvider::CoreMl, ExecutionProvider::Cpu]
        );
        assert!(warnings.warnings[0].contains("CoreML test failure"));
        let error = super::run_models(
            &Models {
                cpu: None,
                coreml: Some(model),
            },
            &Progress::default(),
            |_| Err(Error("CoreML test failure".into())),
        )
        .unwrap_err();
        assert!(error.to_string().contains("CoreML test failure"));
        assert!(error.to_string().contains("No CPU fallback"));
        assert!(ModelKind::CoreMl.available());

        let p = Progress::default();
        let mut calls = 0;
        let error = super::run_models(&models, &p, |_| {
            calls += 1;
            p.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            Err(Error("cancelled in CoreML".into()))
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert!(error.to_string().contains("cancelled"));
    }

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
            std::fs::write(directory.join("guitar.wav"), b"failed six-stem attempt").unwrap();
            std::fs::write(directory.join("piano.wav"), b"failed six-stem attempt").unwrap();
            std::fs::write(directory.join("keep.txt"), b"unrelated").unwrap();
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
                        .warnings
                        .iter()
                        .any(|w| w.contains("CoreML failed; used CPU"))
                );
            }
            assert!(!directory.join("guitar.wav").exists());
            assert!(!directory.join("piano.wav").exists());
            assert_eq!(
                std::fs::read(directory.join("keep.txt")).unwrap(),
                b"unrelated"
            );
            for stem in crate::STEMS {
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
