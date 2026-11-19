use daw_ui::{DawUi, settings::AppSettings};
use std::path::{Path, PathBuf};

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
        ui.set_theme_directory(ctx, theme_directory(), open_theme_folder);
    }
    ui
}

fn open_theme_folder(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/bin/open")
            .arg(path)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        use gtk4::gio::{self, prelude::FileExt};
        gio::AppInfo::launch_default_for_uri(
            &gio::File::for_path(path).uri(),
            None::<&gio::AppLaunchContext>,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
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
