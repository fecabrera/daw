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
