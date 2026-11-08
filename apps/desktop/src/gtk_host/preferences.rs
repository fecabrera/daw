//! Linux preferences and migration from the former eframe and GTK trial stores.
use super::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub fullscreen: bool,
}
impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
            maximized: false,
            fullscreen: false,
        }
    }
}
impl WindowState {
    pub fn capture(&mut self, window: &gtk4::ApplicationWindow) {
        if !window.is_maximized() && !window.is_fullscreen() {
            self.width = window.width().max(900);
            self.height = window.height().max(600);
        }
        self.maximized = window.is_maximized();
        self.fullscreen = window.is_fullscreen();
    }
}

#[derive(Default)]
pub(super) struct Loaded {
    pub settings: AppSettings,
    pub window: WindowState,
}

pub(super) fn path() -> PathBuf {
    glib::user_config_dir().join("daw/settings.json")
}

pub(super) fn load() -> Loaded {
    load_from(
        &path(),
        &glib::user_data_dir().join("daw/app.ron"),
        &glib::user_config_dir().join("daw/gtk-titlebar-settings.json"),
    )
}

fn load_from(current: &Path, legacy: &Path, trial: &Path) -> Loaded {
    if let Some(text) = read(current)
        && let Ok(document) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let settings = document.get("settings").unwrap_or(&document).to_string();
        let window = document
            .get("window")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        return Loaded {
            settings: AppSettings::from_json(Some(&settings)),
            window,
        };
    }
    // Preserve established device/quality/theme choices before trial defaults.
    // Legacy files remain untouched, including unrelated eframe storage keys.
    if let Some(text) = read(legacy)
        && let Ok(values) = ron::from_str::<std::collections::HashMap<String, String>>(&text)
        && let Some(settings) = values.get(daw_ui::settings::STORAGE_KEY)
    {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct LegacySize {
            x: f32,
            y: f32,
        }
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct LegacyWindow {
            inner_size_points: Option<LegacySize>,
            maximized: bool,
            fullscreen: bool,
        }
        let old = values
            .get("window")
            .and_then(|value| ron::from_str::<LegacyWindow>(value).ok())
            .unwrap_or_default();
        let mut window = WindowState {
            maximized: old.maximized,
            fullscreen: old.fullscreen,
            ..Default::default()
        };
        if let Some(size) = old.inner_size_points
            && size.x.is_finite()
            && size.y.is_finite()
        {
            window.width = (size.x as i32).max(900);
            window.height = (size.y as i32).max(600);
        }
        return Loaded {
            settings: AppSettings::from_json(Some(settings)),
            window,
        };
    }
    if let Some(text) = read(trial) {
        return Loaded {
            settings: AppSettings::from_json(Some(&text)),
            ..Default::default()
        };
    }
    Loaded::default()
}

fn read(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(%error, "Cannot load application preferences");
            None
        }
    }
}

pub(super) fn encode(settings: AppSettings, window: WindowState) -> String {
    serde_json::json!({ "settings": settings, "window": window }).to_string()
}

pub(super) fn save(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "daw-gtk-prefs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn migrates_legacy_choices_without_changing_either_old_store() {
        let directory = directory();
        let current = directory.join("settings.json");
        let legacy = directory.join("app.ron");
        let trial = directory.join("trial.json");
        let settings = AppSettings::from_json(Some(
            r#"{"titlebar":"native","audio":{"output_device":{"name":"pipewire","occurrence":0}}}"#,
        ));
        let values = std::collections::HashMap::from([
            (daw_ui::settings::STORAGE_KEY.to_owned(), settings.to_json()),
            ("unrelated".into(), "retained".into()),
            (
                "window".into(),
                "(inner_size_points:Some((x:1440.0,y:900.0)),maximized:true,fullscreen:false)"
                    .into(),
            ),
        ]);
        let original = ron::to_string(&values).unwrap();
        std::fs::write(&legacy, &original).unwrap();
        std::fs::write(&trial, AppSettings::default().to_json()).unwrap();
        let loaded = load_from(&current, &legacy, &trial);
        assert_eq!(
            loaded.settings.audio.output_device,
            settings.audio.output_device
        );
        assert_eq!(loaded.settings.titlebar, TitlebarStyle::Native);
        assert_eq!(
            loaded.settings.audio.output_device.as_ref().unwrap().name,
            "pipewire"
        );
        assert_eq!(loaded.window.width, 1440);
        assert!(loaded.window.maximized);
        let geometry = WindowState {
            width: 1400,
            height: 950,
            maximized: true,
            ..Default::default()
        };
        save(&current, &encode(loaded.settings, geometry)).unwrap();
        let loaded = load_from(&current, &legacy, &trial);
        assert_eq!(loaded.window, geometry);
        assert_eq!(loaded.settings.titlebar, TitlebarStyle::Native);
        assert_eq!(std::fs::read_to_string(legacy).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(trial).unwrap(),
            AppSettings::default().to_json()
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fresh_preferences_use_custom_and_trial_choices_are_a_fallback() {
        let directory = directory();
        let current = directory.join("settings.json");
        let legacy = directory.join("app.ron");
        let trial = directory.join("trial.json");
        assert_eq!(
            load_from(&current, &legacy, &trial).settings.titlebar,
            TitlebarStyle::Custom
        );
        let settings = AppSettings {
            titlebar: TitlebarStyle::Native,
            ..Default::default()
        };
        std::fs::write(&trial, settings.to_json()).unwrap();
        assert_eq!(
            load_from(&current, &legacy, &trial).settings.titlebar,
            TitlebarStyle::Native
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
