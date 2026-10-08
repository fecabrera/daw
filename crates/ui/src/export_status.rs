use daw_media::{ExportFormat, ExportSettings};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Complete,
    Failed(String),
}

pub struct ExportStatus {
    pub path: PathBuf,
    pub settings: ExportSettings,
    pub frames: Arc<AtomicU64>,
    pub total: u64,
    pub state: State,
    pub warnings: Vec<String>,
}

impl ExportStatus {
    pub fn new(path: PathBuf, settings: ExportSettings, total: u64, warnings: Vec<String>) -> Self {
        Self {
            path,
            settings,
            frames: Arc::new(AtomicU64::new(0)),
            total,
            state: State::Running,
            warnings,
        }
    }

    /// Returns true only when a finished report is dismissed.
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let running = self.state == State::Running;
        let frames = self.frames.load(Ordering::Relaxed).min(self.total);
        let actions: &[(&str, ())] = if running { &[] } else { &[("Close", ())] };
        let closed = crate::dialogs::Dialog::new("Export status", "", actions)
            .show_with_content(ctx, |ui| {
                match &self.state {
                    State::Running => {
                        crate::rows::centered(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                            ui.spinner();
                            ui.label(if frames < self.total {
                                "Rendering and encoding audio…"
                            } else {
                                "Finalizing audio file…"
                            });
                        });
                    }
                    State::Complete => {
                        ui.label("Export complete");
                    }
                    State::Failed(_) => {
                        ui.colored_label(crate::theme::ERROR, "Export failed");
                    }
                }
                if !matches!(self.state, State::Failed(_)) {
                    let progress = if running {
                        (frames as f64 / self.total.max(1) as f64).min(0.99) as f32
                    } else {
                        1.0
                    };
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .show_percentage()
                            .desired_width(300.0),
                    );
                }
                egui::ScrollArea::vertical()
                    .id_salt("export_diagnostics")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let codec = match self.settings.format {
                            ExportFormat::Wav => self.settings.wav_codec.label().to_owned(),
                            ExportFormat::Mp3 => format!(
                                "MPEG Layer III · {} kbps CBR",
                                self.settings.mp3_bitrate.kbps()
                            ),
                        };
                        ui.add(
                            egui::Label::new(format!(
                                "{} · {codec} · Stereo · 48 kHz",
                                self.settings.format.label()
                            ))
                            .wrap(),
                        );
                        ui.label("Destination");
                        ui.add(egui::Label::new(self.path.display().to_string()).wrap());
                        if let State::Failed(error) = &self.state {
                            ui.colored_label(crate::theme::ERROR, "Error");
                            ui.add(egui::Label::new(error).wrap());
                        }
                        if !self.warnings.is_empty() {
                            ui.colored_label(crate::theme::WARNING, "Warnings");
                            for warning in &self.warnings {
                                ui.add(egui::Label::new(warning).wrap());
                            }
                        }
                    });
            })
            .is_some();
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        closed
            || (!running
                && ctx
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)))
    }
}
