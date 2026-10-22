//! Offline model inference. No model downloads or audio-device dependencies.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Cpu,
    CoreMl,
}
impl ModelKind {
    pub const ALL: [Self; 2] = [Self::CoreMl, Self::Cpu];

    pub fn available(self) -> bool {
        match self {
            Self::Cpu => AVAILABLE,
            Self::CoreMl => cfg!(all(target_os = "macos", target_arch = "aarch64")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cpu => "CPU model",
            Self::CoreMl => "CoreML model",
        }
    }
}
impl Models {
    pub fn path(&self, kind: ModelKind) -> Option<&Path> {
        match kind {
            ModelKind::Cpu => self.cpu.as_deref(),
            ModelKind::CoreMl => self.coreml.as_deref(),
        }
    }

    pub fn set(&mut self, kind: ModelKind, path: Option<PathBuf>) {
        match kind {
            ModelKind::Cpu => self.cpu = path,
            ModelKind::CoreMl => self.coreml = path,
        }
    }

    /// Configuration only; the host publishes paths after worker validation.
    pub fn is_configured(&self) -> bool {
        ModelKind::ALL
            .into_iter()
            .any(|kind| kind.available() && self.path(kind).is_some())
    }
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

/// Whether the native backend is available on this build target.
pub const AVAILABLE: bool = !cfg!(all(target_os = "macos", target_arch = "x86_64"));
pub const UNAVAILABLE_REASON: &str = "Stem separation is unavailable on Intel macOS";

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
mod backend;

/// Load the selected backend and check a short inference through the same
/// four-stem output contract as separation. Call only from a worker.
pub fn validate_model(kind: ModelKind, path: &Path, progress: &Progress) -> Result<(), Error> {
    progress.check()?;
    if !kind.available() {
        return Err(Error(format!(
            "{} is unavailable on this platform",
            kind.name()
        )));
    }
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    {
        backend::validate_model(kind, path, progress)
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        let _ = path;
        Err(Error(UNAVAILABLE_REASON.into()))
    }
}

/// Writes only into the caller's newly reserved staging directory.
pub fn separate(
    samples: &[[f32; 2]],
    models: &Models,
    directory: &Path,
    progress: &Progress,
) -> Result<Vec<String>, Error> {
    progress.check()?;
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
    {
        backend::separate(samples, models, directory, progress)
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        let _ = (samples, models, directory);
        Err(Error(UNAVAILABLE_REASON.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    fn unsupported_target_fails_without_loading_models() {
        let error = separate(
            &[[0.0; 2]],
            &Models::default(),
            Path::new("unused"),
            &Progress::default(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), UNAVAILABLE_REASON);
    }
    #[test]
    #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
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
