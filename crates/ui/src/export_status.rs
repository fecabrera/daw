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
    pub total: u64,
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
            total,
            state: State::Running,
            warnings,
        }
    }

    /// Returns true only when a finished report is dismissed.
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let frames = self.frames.load(Ordering::Relaxed).min(self.total);
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
            running_text: if frames < self.total {
                "Rendering and encoding audio…"
            } else {
                "Finalizing audio file…"
            },
            complete_text: "Export complete",
            failed_text: "Export failed",
            detail: &format!(
                "{} · {codec} · Stereo · {} kHz",
                self.settings.format.label(),
                self.sample_rate_hz as f32 / 1000.0
            ),
            path: &self.path,
            progress: Some(frames as f32 / self.total.max(1) as f32),
            warnings: &self.warnings,
            cancel: None,
        }
        .show(ctx)
    }
}
