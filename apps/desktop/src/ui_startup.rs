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
    load_user_themes: bool,
) -> DawUi {
    let mut ui = DawUi::new_with_project_defaults(
        project,
        settings.audio_quality,
        settings.default_stretch_engine,
    );
    ui.set_settings(ctx, settings);
    if load_user_themes {
        ui.set_theme_directory(ctx, theme_directory());
    }
    ui
}

fn theme_directory() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/DAW"));
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("DAW"));
    #[cfg(target_os = "linux")]
    let root = Some(gtk4::glib::user_config_dir().join("daw"));
    root.filter(|path| path.is_absolute())
        .map(|path| path.join("themes"))
}
