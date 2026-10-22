//! Shared progress, cancellation, and diagnostics for offline audio jobs.
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Complete,
    Cancelled,
    Failed(String),
}
pub struct Report<'a> {
    pub title: &'a str,
    pub state: &'a State,
    pub running_text: &'a str,
    pub complete_text: &'a str,
    pub failed_text: &'a str,
    pub detail: &'a str,
    pub path: &'a Path,
    pub progress: Option<f32>,
    pub warnings: &'a [String],
    pub cancel: Option<&'a AtomicBool>,
}
impl Report<'_> {
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let running = *self.state == State::Running;
        let actions: &[(&str, ())] = if running {
            if self.cancel.is_some() {
                &[("Cancel", ())]
            } else {
                &[]
            }
        } else {
            &[("Close", ())]
        };
        let clicked = crate::dialogs::Dialog::new(self.title, "", actions)
            .show_with_content(ctx, |ui| {
                match self.state {
                    State::Running => {
                        crate::rows::centered(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                            ui.spinner();
                            ui.label(if self.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                                "Cancelling…"
                            } else {
                                self.running_text
                            });
                        });
                    }
                    State::Complete => {
                        ui.label(self.complete_text);
                    }
                    State::Cancelled => {
                        ui.label("Operation cancelled");
                    }
                    State::Failed(_) => {
                        ui.colored_label(crate::theme::palette(ctx).error, self.failed_text);
                    }
                }
                if !matches!(self.state, State::Failed(_) | State::Cancelled)
                    && let Some(progress) = self.progress
                {
                    ui.add(
                        egui::ProgressBar::new(if running { progress.min(0.99) } else { 1.0 })
                            .show_percentage()
                            .desired_width(ui.available_width()),
                    );
                }
                egui::ScrollArea::vertical()
                    .id_salt(self.title)
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(self.detail).wrap());
                        ui.label("Destination");
                        ui.add(egui::Label::new(self.path.display().to_string()).wrap());
                        if let State::Failed(error) = self.state {
                            ui.colored_label(crate::theme::palette(ctx).error, "Error");
                            ui.add(egui::Label::new(error).wrap());
                        }
                        if !self.warnings.is_empty() {
                            ui.colored_label(crate::theme::palette(ctx).warning, "Warnings");
                            for warning in self.warnings {
                                ui.add(egui::Label::new(warning).wrap());
                            }
                        }
                    });
            })
            .is_some();
        if running {
            if clicked && let Some(cancel) = self.cancel {
                cancel.store(true, Ordering::Relaxed);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            false
        } else {
            clicked || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        }
    }
}
