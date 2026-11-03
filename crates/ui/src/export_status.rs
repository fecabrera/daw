use daw_media::{ExportFormat, ExportSettings};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub use crate::job_report::State;

pub struct ExportStatus {
    pub path: PathBuf,
    pub settings: ExportSettings,
    pub sample_rate_hz: u32,
    pub frames: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub state: State,
    pub warnings: Vec<String>,
}

impl ExportStatus {
    pub fn new(
        path: PathBuf,
        settings: ExportSettings,
        project_rate_hz: u32,
        total: u64,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            path,
            settings,
            sample_rate_hz: settings.output_sample_rate(project_rate_hz),
            frames: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(total)),
            state: State::Running,
            warnings,
        }
    }

    fn running_text(frames: u64, total: u64) -> &'static str {
        if frames < total {
            "Rendering and encoding audio…"
        } else {
            "Finalizing audio file…"
        }
    }

    /// Returns true only when a finished report is dismissed.
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let total = self.total.load(Ordering::Acquire);
        let frames = self.frames.load(Ordering::Relaxed).min(total);
        let codec = match self.settings.format {
            ExportFormat::Wav => self.settings.wav_codec.label().to_owned(),
            ExportFormat::Aac => {
                format!("AAC-LC · {} kbps target", self.settings.aac_bitrate.kbps())
            }
            ExportFormat::Mp3 => format!(
                "MPEG Layer III · {} kbps CBR",
                self.settings.mp3_bitrate.kbps()
            ),
        };
        crate::job_report::Report {
            title: "Export status",
            state: &self.state,
            running_text: Self::running_text(frames, total),
            complete_text: "Export complete",
            failed_text: "Export failed",
            detail: &format!(
                "{} · {codec} · Stereo · {} kHz",
                self.settings.format.label(),
                self.sample_rate_hz as f32 / 1000.0
            ),
            path: Some(&self.path),
            progress: Some(frames as f32 / total.max(1) as f32),
            warnings: &self.warnings,
            cancel: None,
        }
        .show(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appended_tail_keeps_rendering_phase_until_backend_total_is_reached() {
        let s = ExportStatus::new(
            PathBuf::from("mix.wav"),
            ExportSettings::default(),
            48000,
            4800,
            vec![],
        );
        let total = s.total.clone();
        let frames = s.frames.clone();
        total.store(484800, Ordering::Release);
        frames.store(4800, Ordering::Relaxed);
        assert_eq!(
            ExportStatus::running_text(
                frames.load(Ordering::Relaxed),
                s.total.load(Ordering::Acquire)
            ),
            "Rendering and encoding audio…"
        );
        assert_eq!(s.state, State::Running);
        frames.store(484800, Ordering::Relaxed);
        assert_eq!(
            ExportStatus::running_text(
                frames.load(Ordering::Relaxed),
                s.total.load(Ordering::Acquire)
            ),
            "Finalizing audio file…"
        );
        assert_eq!(s.state, State::Running);
    }
}
