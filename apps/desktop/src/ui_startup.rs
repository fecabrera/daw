use daw_ui::{DawUi, settings::AppSettings};
use std::path::PathBuf;

pub(crate) fn configure_context(ctx: &egui::Context) {
    daw_ui::theme::configure(ctx);
    daw_ui::fonts::configure(ctx);
    daw_ui::icons::configure(ctx);
}

pub(crate) fn create_ui(
    ctx: &egui::Context,
    project: Option<PathBuf>,
    settings: AppSettings,
) -> DawUi {
    let mut ui = DawUi::new_with_project_defaults(
        project,
        settings.audio_quality,
        settings.default_stretch_engine,
    );
    ui.set_settings(ctx, settings);
    ui
}
