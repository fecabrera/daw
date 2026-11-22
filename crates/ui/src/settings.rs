pub use daw_engine::CacheProfile;
use egui::{Color32, Context, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "daw.settings";

/// Material Design's 500 shades, in palette order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    #[default]
    Red,
    Pink,
    Purple,
    DeepPurple,
    Indigo,
    Blue,
    LightBlue,
    Cyan,
    Teal,
    Green,
    LightGreen,
    Lime,
    Yellow,
    Amber,
    Orange,
    DeepOrange,
    Brown,
    Grey,
    BlueGrey,
}

impl AccentColor {
    pub const ALL: [Self; 19] = [
        Self::Red,
        Self::Pink,
        Self::Purple,
        Self::DeepPurple,
        Self::Indigo,
        Self::Blue,
        Self::LightBlue,
        Self::Cyan,
        Self::Teal,
        Self::Green,
        Self::LightGreen,
        Self::Lime,
        Self::Yellow,
        Self::Amber,
        Self::Orange,
        Self::DeepOrange,
        Self::Brown,
        Self::Grey,
        Self::BlueGrey,
    ];

    const CHOICES: [Self; 8] = [
        Self::Red,
        Self::DeepOrange,
        Self::Amber,
        Self::Green,
        Self::Indigo,
        Self::DeepPurple,
        Self::Pink,
        Self::BlueGrey,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Pink => "Pink",
            Self::Purple => "Purple",
            Self::DeepPurple => "Deep Purple",
            Self::Indigo => "Indigo",
            Self::Blue => "Blue",
            Self::LightBlue => "Light Blue",
            Self::Cyan => "Cyan",
            Self::Teal => "Teal",
            Self::Green => "Green",
            Self::LightGreen => "Light Green",
            Self::Lime => "Lime",
            Self::Yellow => "Yellow",
            Self::Amber => "Amber",
            Self::Orange => "Orange",
            Self::DeepOrange => "Deep Orange",
            Self::Brown => "Brown",
            Self::Grey => "Grey",
            Self::BlueGrey => "Blue Grey",
        }
    }

    pub const fn color(self) -> Color32 {
        let hex = match self {
            Self::Red => 0xf44336,
            Self::Pink => 0xe91e63,
            Self::Purple => 0x9c27b0,
            Self::DeepPurple => 0x673ab7,
            Self::Indigo => 0x3f51b5,
            Self::Blue => 0x2196f3,
            Self::LightBlue => 0x03a9f4,
            Self::Cyan => 0x00bcd4,
            Self::Teal => 0x009688,
            Self::Green => 0x4caf50,
            Self::LightGreen => 0x8bc34a,
            Self::Lime => 0xcddc39,
            Self::Yellow => 0xffeb3b,
            Self::Amber => 0xffc107,
            Self::Orange => 0xff9800,
            Self::DeepOrange => 0xff5722,
            Self::Brown => 0x795548,
            Self::Grey => 0x9e9e9e,
            Self::BlueGrey => 0x607d8b,
        };
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorTheme {
    #[default]
    Dark,
    Light,
    Mist,
    Midnight,
}

impl ColorTheme {
    pub const ALL: [Self; 4] = [Self::Dark, Self::Light, Self::Mist, Self::Midnight];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Mist => "Mist",
            Self::Midnight => "Midnight",
        }
    }

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::Mist => 2,
            Self::Midnight => 3,
        }
    }

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Mist => "mist",
            Self::Midnight => "midnight",
        }
    }

    pub(super) const fn parent(self) -> Self {
        match self {
            Self::Mist => Self::Light,
            Self::Midnight => Self::Dark,
            _ => self,
        }
    }
}

/// Window presentation; Linux custom controls require a GTK host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitlebarStyle {
    #[cfg_attr(any(target_os = "macos", target_os = "linux"), default)]
    Custom,
    #[cfg_attr(not(any(target_os = "macos", target_os = "linux")), default)]
    Native,
}

impl TitlebarStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Custom => "Custom",
            Self::Native => "Native",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub plugin_folders: Vec<daw_plugin_host::SearchRoot>,
    pub midi: crate::MidiPreferences,
    pub audio: daw_output::AudioSettings,
    pub audio_quality: daw_core::AudioQuality,
    /// Copied into new projects; existing projects keep their saved engine.
    pub default_stretch_engine: daw_core::StretchEngine,
    pub separation_models: std::sync::Arc<daw_project::SeparationModels>,
    pub accent: AccentColor,
    pub theme: ColorTheme,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::theme_files::deserialize_custom_id"
    )]
    pub custom_theme_id: Option<String>,
    pub titlebar: TitlebarStyle,
    pub playback_seek: daw_output::PlaybackSeek,
    pub seek_cache: CacheProfile,
    /// How MIDI notes that cross the playhead are divided. Audio clips are unaffected.
    pub midi_note_split: daw_core::MidiNoteSplit,
}

impl AppSettings {
    pub fn from_json(json: Option<&str>) -> Self {
        let Some(value) =
            json.and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        else {
            return Self::default();
        };
        let Ok(mut settings) = serde_json::from_value::<Self>(value.clone()) else {
            return Self::default();
        };
        if settings.custom_theme_id.as_deref() == Some(crate::theme_files::LEGACY_MIDNIGHT_ID) {
            settings.theme = ColorTheme::Midnight;
            settings.custom_theme_id = None;
        }
        settings.plugin_folders.retain(|r| r.path.is_absolute());
        settings.plugin_folders.truncate(64);
        let mut seen = std::collections::HashSet::new();
        settings
            .plugin_folders
            .retain(|r| seen.insert((r.format, r.path.clone())));
        settings.midi.recording_offset_ms = settings.midi.recording_offset_ms.clamp(-200, 200);
        if settings.midi.count_in_bars == 0 {
            settings.midi.count_in_bars = 1;
            settings.midi.count_in_enabled = false;
        }
        if !matches!(settings.midi.count_in_bars, 1 | 2 | 4) {
            settings.midi.count_in_bars = 1;
        }
        if settings.midi.channel.is_some_and(|channel| channel > 15) {
            settings.midi.channel = None;
        }
        // Earlier preferences stored optional device-quality overrides. Preserve
        // explicit legacy values as new-project defaults, never as live overrides.
        if value.get("audio_quality").is_none()
            && let Some(audio) = value.get("audio")
        {
            if let Some(rate) = audio
                .get("sample_rate_hz")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                settings.audio_quality.sample_rate_hz = rate;
            }
            if let Some(frames) = audio
                .get("buffer_size_frames")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            {
                settings.audio_quality.buffer_size_frames = frames;
            }
            if let Some(format) = audio
                .get("sample_format")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
            {
                settings.audio_quality.output_sample_format = format;
            }
        }
        if settings.audio_quality.validate().is_err() {
            settings.audio_quality = Default::default();
        }
        if settings.default_stretch_engine.id == daw_core::StretchEngineId::Integrated
            && matches!(settings.default_stretch_engine.revision, 2..=6)
        {
            settings.default_stretch_engine = daw_core::StretchEngine::INTEGRATED;
        }
        if settings.default_stretch_engine.validate().is_err() {
            settings.default_stretch_engine = Default::default();
        }
        settings
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).expect("Settings contain only serializable preferences")
    }
}

const ACCENT_ROW_HEIGHT: f32 = 28.0;

fn accent_swatches(ui: &mut egui::Ui, current: AccentColor) -> AccentColor {
    let height = ACCENT_ROW_HEIGHT;
    let pitch = (ui.available_width() / AccentColor::CHOICES.len() as f32).min(height);
    let width = pitch * AccentColor::CHOICES.len() as f32;
    let (row, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let palette = crate::theme::palette(ui.ctx());
    let radius = (pitch * 0.5 - 3.0).max(0.0);
    let mut choice = current;
    for (index, accent) in AccentColor::CHOICES.into_iter().enumerate() {
        let center = egui::pos2(
            row.right() - width + pitch * (index as f32 + 0.5),
            row.center().y,
        );
        let rect = Rect::from_center_size(center, Vec2::new(pitch, height));
        let id = ui.make_persistent_id(("accent-color", accent.name()));
        let response = ui
            .interact(rect, id, egui::Sense::click())
            .on_hover_text(accent.name());
        let selected = current == accent;
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                ui.is_enabled(),
                selected,
                accent.name(),
            )
        });
        ui.painter().circle(
            center,
            radius,
            accent.color(),
            Stroke::new(1.0, palette.border),
        );
        if selected || response.has_focus() || response.hovered() {
            let color = if selected {
                palette.text
            } else {
                palette.secondary
            };
            ui.painter()
                .circle_stroke(center, radius + 1.5, Stroke::new(1.0, color));
        }
        if response.clicked() {
            choice = accent;
        }
    }
    choice
}

pub struct ModelAction {
    pub kind: daw_project::SeparationModelKind,
    pub path: Option<std::path::PathBuf>,
}

fn model_row(
    ui: &mut egui::Ui,
    width: f32,
    kind: daw_project::SeparationModelKind,
    path: Option<&std::path::Path>,
    info: Option<&daw_project::SeparationModelInfo>,
    action: &mut Option<ModelAction>,
    audio: &mut AudioControls<'_>,
) {
    crate::rows::field(
        ui,
        width,
        |ui| {
            ui.label(kind.name());
        },
        |ui| {
            ui.horizontal(|ui| {
                ui.label(path.and_then(|p| p.file_name()).map_or_else(
                    || "Not selected".into(),
                    |n| n.to_string_lossy().into_owned(),
                ))
                .on_hover_text(
                    path.map_or_else(|| "Choose model".into(), |p| p.display().to_string()),
                );
                if ui.button("Choose…").clicked() {
                    if audio.host_picker {
                        audio.picker = Some(PickerKind::Model(kind));
                    } else if let Some(selected) = rfd::FileDialog::new()
                        .add_filter("ONNX model", &["onnx"])
                        .pick_file()
                    {
                        *action = Some(ModelAction {
                            kind,
                            path: Some(selected),
                        });
                    }
                }
                if ui
                    .add_enabled(path.is_some(), egui::Button::new("Clear"))
                    .clicked()
                {
                    *action = Some(ModelAction { kind, path: None });
                }
            });
        },
    );
    if path.is_some()
        && let Some(info) = info
    {
        crate::rows::field(
            ui,
            width,
            |_| {},
            |ui| {
                ui.vertical(|ui| {
                    let summary = format!(
                        "{} stems · 44.1 kHz · Stereo · {:.1} MB",
                        info.stems.names().len(),
                        info.file_size_bytes as f64 / 1_000_000.0
                    );
                    crate::rows::advice(ui, summary).on_hover_text("Model information");
                    crate::rows::advice(ui, info.stems.description());
                });
            },
        );
    }
}

/// A file chooser owned by the active Settings surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Model(daw_project::SeparationModelKind),
    PluginFolder,
}

#[derive(Default)]
pub(super) struct Navigation {
    group: Group,
    scroll: [f32; Group::ALL.len()],
}

/// Discovery data and actions for the main Settings device selectors.
#[derive(Default)]
pub struct AudioControls<'a> {
    pub(super) host_picker: bool,
    pub(super) picker: Option<PickerKind>,
    pub(super) navigation: Option<&'a mut Navigation>,
    pub(super) folder_draft: Option<&'a mut crate::plugin_settings::FolderDraft>,
    pub(super) plugins: Option<&'a crate::plugin_settings::ScanStatus>,
    pub plugin_action: Option<daw_plugin_host::ScanMode>,
    pub plugin_cancel_requested: bool,
    pub midi_devices: Option<&'a [crate::MidiDevice]>,
    pub midi_enabled: bool,
    pub midi_connected: bool,
    pub midi_panic_requested: bool,
    pub midi_take_retained: bool,
    pub midi_retry_requested: bool,
    pub devices: Option<&'a daw_output::Devices>,
    pub error: Option<&'a str>,
    pub enabled: bool,
    pub selection: Option<daw_output::AudioSettings>,
    pub quality_draft: Option<daw_core::AudioQuality>,
    pub refresh_requested: bool,
    pub(super) themes: Option<&'a crate::theme_files::Runtime>,
    pub(super) theme_action: Option<crate::theme_files::Action>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Group {
    #[default]
    Appearance,
    Playback,
    TimeStretch,
    Midi,
    Recording,
    StemSeparation,
    Plugins,
}
impl Group {
    const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Playback,
        Self::TimeStretch,
        Self::Midi,
        Self::Recording,
        Self::StemSeparation,
        Self::Plugins,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Playback => "Playback",
            Self::TimeStretch => "Time stretch",
            Self::Midi => "MIDI",
            Self::Recording => "Recording",
            Self::StemSeparation => "Stem Separation",
            Self::Plugins => "Plugins",
        }
    }
}
fn group_id() -> egui::Id {
    egui::Id::new("app-settings-group")
}

fn appearance(
    ui: &mut egui::Ui,
    ctx: &Context,
    settings: &mut AppSettings,
    label_width: f32,
    controls: &mut AudioControls<'_>,
) {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        let show_titlebar = cfg!(any(target_os = "macos", target_os = "windows"));
        #[cfg(target_os = "linux")]
        let show_titlebar = show_titlebar || crate::titlebar::linux_custom_available(ctx);
        if show_titlebar {
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Titlebar");
                },
                |ui| {
                    crate::menus::Dropdown::from_id_salt("titlebar_style")
                        .selected_text(settings.titlebar.name())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for style in [TitlebarStyle::Custom, TitlebarStyle::Native] {
                                ui.selectable_value(&mut settings.titlebar, style, style.name());
                            }
                        });
                },
            );
        }
    }
    crate::rows::field(
        ui,
        label_width,
        |ui| {
            ui.label("Theme");
        },
        |ui| {
            if let Some(runtime) = controls.themes {
                let group = crate::rows::input_group(ui, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::rows::input_group_edge(ui, true);
                        let refresh = crate::icons::button(
                            ui,
                            crate::icons::Lucide::RefreshCw,
                            "Refresh themes",
                            runtime.directory.is_some() && !runtime.busy(),
                            crate::theme::palette(ui.ctx()).text,
                        );
                        if refresh.clicked() {
                            controls.theme_action = Some(crate::theme_files::Action::Reload);
                        }
                        crate::rows::input_group_edge(ui, false);
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), crate::toolbars::CONTROL_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| theme_dropdown(ui, ctx, settings, Some(runtime)),
                        );
                        refresh
                    })
                    .inner
                });
                crate::rows::group_separator(ui, group.response.rect, group.inner.rect.left());
            } else {
                theme_dropdown(ui, ctx, settings, None);
            }
        },
    );
    let accent = crate::rows::field_with_height(
        ui,
        label_width,
        ACCENT_ROW_HEIGHT,
        |ui| {
            ui.label("Accent color")
                .on_hover_text(settings.accent.name());
        },
        |ui| accent_swatches(ui, settings.accent),
    )
    .inner;
    if accent != settings.accent {
        settings.accent = accent;
        apply_appearance(ctx, settings, controls.themes);
    }
    if let Some(runtime) = controls.themes {
        theme_actions(ui, settings, runtime, &mut controls.theme_action);
    }
}

fn theme_dropdown(
    ui: &mut egui::Ui,
    ctx: &Context,
    settings: &mut AppSettings,
    runtime: Option<&crate::theme_files::Runtime>,
) {
    let active = runtime.map(|themes| themes.active(settings));
    let label = active.map_or(settings.theme.name(), |entry| entry.name.as_str());
    crate::menus::Dropdown::from_id_salt("color_theme")
        .selected_text(label)
        .width(ui.available_width())
        .truncate()
        .show_ui(ui, |ui| {
            for theme in ColorTheme::ALL {
                let selected = settings.custom_theme_id.is_none() && settings.theme == theme;
                if ui.selectable_label(selected, theme.name()).clicked() {
                    settings.theme = theme;
                    settings.custom_theme_id = None;
                    apply_appearance(ctx, settings, runtime);
                }
            }
            if let Some(runtime) = runtime {
                let entries = runtime.entries();
                for entry in &entries[ColorTheme::ALL.len()..] {
                    let selected = settings.custom_theme_id.as_deref() == Some(entry.id.as_str());
                    let ambiguous = entries.iter().any(|other| {
                        other.id != entry.id && other.name.eq_ignore_ascii_case(&entry.name)
                    });
                    let label = if ambiguous {
                        format!("{} ({})", entry.name, entry.id)
                    } else {
                        entry.name.clone()
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        settings.custom_theme_id = Some(entry.id.clone());
                        runtime.apply(ctx, settings);
                    }
                }
            }
        });
}

fn apply_appearance(
    ctx: &Context,
    settings: &AppSettings,
    runtime: Option<&crate::theme_files::Runtime>,
) {
    if let Some(runtime) = runtime {
        runtime.apply(ctx, settings);
    } else {
        crate::theme::apply(ctx, settings.clone());
    }
}

fn theme_actions(
    ui: &mut egui::Ui,
    settings: &AppSettings,
    runtime: &crate::theme_files::Runtime,
    action: &mut Option<crate::theme_files::Action>,
) {
    use crate::theme_files::Action;

    ui.separator();
    let active = runtime.active(settings);
    if let Some(id) = &settings.custom_theme_id
        && id != &active.id
    {
        crate::rows::advice(
            ui,
            format!("Theme {id} is unavailable. Using {}.", active.name),
        );
    }
    if let Some(path) = &runtime.directory {
        let path = path.display().to_string();
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), crate::toolbars::CONTROL_HEIGHT),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let colors = crate::theme::palette(ui.ctx());
                if crate::icons::button(
                    ui,
                    crate::icons::Lucide::FolderOpen,
                    "Open folder",
                    !runtime.busy(),
                    colors.text,
                )
                .clicked()
                {
                    *action = Some(Action::OpenFolder);
                }
                if crate::icons::button(
                    ui,
                    crate::icons::Lucide::SquareText,
                    "Copy themes folder path",
                    true,
                    colors.text,
                )
                .clicked()
                {
                    ui.ctx().copy_text(path.clone());
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&path).small().color(colors.secondary),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&path);
                });
            },
        );
    } else {
        crate::rows::advice(ui, "Themes folder unavailable");
    }
    if runtime.busy() {
        ui.spinner();
    }
    if runtime.status().next().is_some() {
        egui::CollapsingHeader::new("Theme file issues")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("theme-file-issues")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for issue in runtime.status() {
                            crate::rows::advice(ui, issue);
                        }
                    });
            });
    }
}

fn playback(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    audio: &mut AudioControls<'_>,
    label_width: f32,
    quality_enabled: bool,
) {
    ui.add_enabled_ui(audio.enabled, |ui| {
        let empty = daw_output::Devices::default();
        let devices = audio.devices.unwrap_or(&empty);
        let mut draft = settings.audio.clone();
        let output_changed = crate::audio_settings::device_row(
            ui,
            label_width,
            "Output device",
            &mut draft.output_device,
            &devices.outputs,
        );
        let input_changed = crate::audio_settings::device_row(
            ui,
            label_width,
            "Input device",
            &mut draft.input_device,
            &devices.inputs,
        );
        if output_changed || input_changed {
            audio.selection = Some(draft);
        }
        crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
            if ui.button("Refresh devices").clicked() {
                audio.refresh_requested = true;
            }
        });
    });
    crate::rows::advice(
        ui,
        "Changing output stops playback. Input selection is saved for future recording; input monitoring is not available.",
    );
    if !audio.enabled {
        ui.label("Checking audio devices…");
    }
    if let Some(error) = audio.error {
        ui.label(error);
    }
    crate::rows::field(
        ui,
        label_width,
        |ui| {
            ui.label("Seek mode");
        },
        |ui| {
            crate::menus::Dropdown::from_id_salt("playback_seek")
                .selected_text(settings.playback_seek.name())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for mode in daw_output::PlaybackSeek::ALL {
                        let description = match mode {
                            daw_output::PlaybackSeek::Fast => {
                                "Start at the destination with fresh effects and new note starts"
                            }
                            daw_output::PlaybackSeek::Exact => {
                                "Reconstruct earlier notes and effect history"
                            }
                        };
                        ui.selectable_value(&mut settings.playback_seek, mode, mode.name())
                            .on_hover_text(description);
                    }
                });
        },
    );
    crate::rows::field(
        ui,
        label_width,
        |ui| {
            ui.label("Seek cache");
        },
        |ui| {
            crate::menus::Dropdown::from_id_salt("seek_cache")
                .selected_text(settings.seek_cache.name())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for profile in CacheProfile::ALL {
                        ui.selectable_value(&mut settings.seek_cache, profile, profile.name())
                            .on_hover_text(profile.name());
                    }
                });
        },
    );
    ui.separator();
    ui.strong("Audio quality defaults");
    ui.add_enabled_ui(quality_enabled, |ui| {
        crate::audio_settings::quality_defaults(
            ui,
            label_width,
            &mut settings.audio_quality,
            &mut audio.quality_draft,
        );
    });
}

fn time_stretch(ui: &mut egui::Ui, settings: &mut AppSettings, label_width: f32) {
    crate::rows::field(
        ui,
        label_width,
        |ui| {
            ui.label("Default engine");
        },
        |ui| {
            crate::menus::Dropdown::from_id_salt("default_stretch_engine")
                .selected_text(settings.default_stretch_engine.label())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for engine in [
                        daw_core::StretchEngine::RUBBER_BAND,
                        daw_core::StretchEngine::INTEGRATED,
                    ] {
                        ui.selectable_value(
                            &mut settings.default_stretch_engine,
                            engine,
                            engine.label(),
                        );
                    }
                });
        },
    );
    crate::rows::advice(
        ui,
        "Used for new projects. Change an existing project's engine in Project Settings.",
    );
}

fn midi(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    audio: &mut AudioControls<'_>,
    label_width: f32,
) {
    crate::rows::field(
        ui,
        label_width,
        |ui| {
            ui.label("Split MIDI notes");
        },
        |ui| {
            crate::menus::Dropdown::from_id_salt("midi_note_split")
                .selected_text(settings.midi_note_split.name())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for mode in daw_core::MidiNoteSplit::ALL {
                        ui.selectable_value(&mut settings.midi_note_split, mode, mode.name())
                            .on_hover_text(mode.name());
                    }
                });
        },
    );
    ui.add_enabled_ui(audio.midi_enabled, |ui| {
        crate::rows::field(
            ui,
            label_width,
            |ui| {
                ui.label("MIDI input");
            },
            |ui| {
                crate::menus::Dropdown::from_id_salt("midi-device")
                    .selected_text(
                        settings
                            .midi
                            .device
                            .as_ref()
                            .and_then(|id| {
                                audio
                                    .midi_devices
                                    .unwrap_or(&[])
                                    .iter()
                                    .find(|d| &d.id == id)
                            })
                            .map_or("Off", |d| d.name.as_str()),
                    )
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.midi.device, None, "Off");
                        for device in audio.midi_devices.unwrap_or(&[]) {
                            ui.selectable_value(
                                &mut settings.midi.device,
                                Some(device.id.clone()),
                                &device.name,
                            );
                        }
                    });
            },
        );
        crate::rows::field(
            ui,
            label_width,
            |ui| {
                ui.label("MIDI channel");
            },
            |ui| {
                crate::menus::Dropdown::from_id_salt("midi-channel")
                    .selected_text(
                        settings
                            .midi
                            .channel
                            .map_or("All channels".into(), |c| format!("Channel {}", c + 1)),
                    )
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.midi.channel, None, "All channels");
                        for channel in 0..16 {
                            ui.selectable_value(
                                &mut settings.midi.channel,
                                Some(channel),
                                format!("Channel {}", channel + 1),
                            );
                        }
                    });
            },
        );
    });
    crate::rows::advice(
        ui,
        "Enable R after M/S to receive MIDI on several tracks. With no R enabled, the selected MIDI track receives input. Its R text shows the active color.",
    );
    if settings.midi.device.is_some() && !audio.midi_connected {
        ui.label("MIDI input is disconnected.");
    }
}

fn recording(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    audio: &mut AudioControls<'_>,
    label_width: f32,
) {
    ui.add_enabled_ui(audio.midi_enabled, |ui| {
        crate::rows::field(
            ui,
            label_width,
            |ui| {
                ui.label("Count-in");
            },
            |ui| {
                crate::menus::Dropdown::from_id_salt("midi-count-in")
                    .selected_text(if settings.midi.count_in_bars == 0 {
                        "Off".into()
                    } else {
                        format!(
                            "{} {}",
                            settings.midi.count_in_bars,
                            if settings.midi.count_in_bars == 1 {
                                "bar"
                            } else {
                                "bars"
                            }
                        )
                    })
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for bars in [1, 2, 4] {
                            ui.selectable_value(
                                &mut settings.midi.count_in_bars,
                                bars,
                                if bars == 0 {
                                    "Off".into()
                                } else {
                                    format!("{bars} {}", if bars == 1 { "bar" } else { "bars" })
                                },
                            );
                        }
                    });
            },
        );
        crate::rows::field(
            ui,
            label_width,
            |ui| {
                ui.label("Recording offset");
            },
            |ui| {
                ui.add(
                    egui::DragValue::new(&mut settings.midi.recording_offset_ms)
                        .range(-200..=200)
                        .suffix(" ms"),
                )
                .on_hover_text("Shift recorded notes; positive values place notes later");
            },
        );
        ui.checkbox(&mut settings.midi.count_in_enabled, "Count-in enabled")
            .on_hover_text(if settings.midi.count_in_enabled {
                "Count-in: On"
            } else {
                "Count-in: Off"
            });
    });
    if ui.button("Panic").on_hover_text("Panic").clicked() {
        audio.midi_panic_requested = true;
    }
    if audio.midi_take_retained && ui.button("Retry MIDI take publication").clicked() {
        audio.midi_retry_requested = true;
    }
}

fn separation(
    ui: &mut egui::Ui,
    models: &daw_project::SeparationModels,
    model_info: &std::collections::HashMap<
        daw_project::SeparationModelKind,
        daw_project::SeparationModelInfo,
    >,
    models_enabled: bool,
    model_action: &mut Option<ModelAction>,
    label_width: f32,
    audio: &mut AudioControls<'_>,
) {
    ui.add_enabled_ui(daw_project::SEPARATION_AVAILABLE && models_enabled, |ui| {
        model_row(
            ui,
            label_width,
            daw_project::SeparationModelKind::Cpu,
            models.cpu.as_deref(),
            model_info.get(&daw_project::SeparationModelKind::Cpu),
            model_action,
            audio,
        );
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        model_row(
            ui,
            label_width,
            daw_project::SeparationModelKind::CoreMl,
            models.coreml.as_deref(),
            model_info.get(&daw_project::SeparationModelKind::CoreMl),
            model_action,
            audio,
        );
    });
    if !daw_project::SEPARATION_AVAILABLE {
        ui.label(daw_project::SEPARATION_UNAVAILABLE_REASON);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn show_surface(
    ctx: &Context,
    surface: Option<&mut egui::Ui>,
    settings: &mut AppSettings,
    models: &daw_project::SeparationModels,
    model_info: &std::collections::HashMap<
        daw_project::SeparationModelKind,
        daw_project::SeparationModelInfo,
    >,
    models_enabled: bool,
    model_action: &mut Option<ModelAction>,
    audio: &mut AudioControls<'_>,
) -> bool {
    let popup_was_open = egui::Popup::is_any_open(ctx);
    let mut navigation = audio.navigation.take();
    let mut group = navigation.as_ref().map_or_else(
        || {
            ctx.data(|data| data.get_temp::<Group>(group_id()))
                .unwrap_or_default()
        },
        |state| state.group,
    );
    let mut offsets = navigation.as_ref().map(|state| state.scroll);
    let native = surface.is_some();
    let width = (ctx.content_rect().width() - 48.0).clamp(280.0, 680.0);
    let height = (ctx.content_rect().height() - 150.0).clamp(160.0, 420.0);
    let content = |ui: &mut egui::Ui| {
        let width = if native { ui.available_width() } else { width };
        let height = if native {
            ui.available_height()
        } else {
            height
        };
        let sidebar_width = (width * 0.24).clamp(96.0, 160.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(sidebar_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(sidebar_width);
                    ui.set_min_height(height);
                    // Let the shared catalog body own the sidebar insets,
                    // including the space next to the dialog edges/divider.
                    let bounds = ui.max_rect().expand(f32::from(crate::dialogs::PADDING));
                    let mut sidebar = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("settings-sidebar")
                            .max_rect(bounds),
                    );
                    sidebar.set_clip_rect(
                        bounds.intersect(ui.clip_rect().expand(f32::from(crate::dialogs::PADDING))),
                    );
                    let panel =
                        crate::panels::ControlPanel::with_width(&sidebar, bounds, bounds.width());
                    panel.catalog_body(&mut sidebar, |ui| {
                        for item in Group::ALL {
                            if panel
                                .catalog_item(ui, item.label(), group == item, false)
                                .clicked()
                            {
                                group = item;
                            }
                        }
                    });
                },
            );
            let (divider, _) = ui.allocate_exact_size(Vec2::new(1.0, height), egui::Sense::hover());
            let padding = f32::from(crate::dialogs::PADDING);
            ui.painter()
                .with_clip_rect(ui.clip_rect().expand(padding))
                .vline(
                    divider.center().x,
                    (divider.top() - padding)..=(divider.bottom() + padding + 0.5),
                    egui::Stroke::new(1.0, crate::theme::palette(ctx).border),
                );
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_height(height);
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt(("settings-content", group))
                        .max_height(height);
                    let index = group as usize;
                    if let Some(offsets) = &offsets {
                        scroll = scroll.vertical_scroll_offset(offsets[index]);
                    }
                    let output = scroll.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let label_text = |label: &str| {
                            ui.painter()
                                .layout_no_wrap(
                                    label.into(),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    crate::theme::palette(ctx).text,
                                )
                                .size()
                                .x
                        };
                        let label_width = label_text(if group == Group::Playback {
                            "Output bit depth"
                        } else {
                            "Recording offset"
                        })
                        .max(label_text("Split MIDI notes"))
                        .max(label_text("Accent color"))
                        .min(ui.available_width() * 0.44);
                        ui.strong(group.label());
                        match group {
                            Group::Appearance => appearance(ui, ctx, settings, label_width, audio),
                            Group::Playback => {
                                playback(ui, settings, audio, label_width, models_enabled)
                            }
                            Group::TimeStretch => time_stretch(ui, settings, label_width),
                            Group::Midi => midi(ui, settings, audio, label_width),
                            Group::Recording => recording(ui, settings, audio, label_width),
                            Group::Plugins => crate::plugin_settings::show(ui, settings, audio),
                            Group::StemSeparation => separation(
                                ui,
                                models,
                                model_info,
                                models_enabled,
                                model_action,
                                label_width,
                                audio,
                            ),
                        }
                    });
                    if let Some(offsets) = &mut offsets {
                        offsets[index] = output.state.offset.y;
                    }
                },
            );
        });
    };
    let close = if let Some(surface) = surface {
        let palette = crate::theme::palette(ctx);
        let padding = crate::dialogs::PADDING;
        let frame = egui::Frame::new().fill(palette.panel).inner_margin(padding);
        let height = crate::toolbars::CONTROL_HEIGHT + 2.0 * f32::from(padding);
        let footer = egui::Panel::bottom("application-settings-footer")
            .exact_size(height)
            .resizable(false)
            .show_separator_line(false)
            .frame(frame)
            .show(surface, |ui| {
                crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                    ui.button("Close").clicked().then_some(())
                })
                .inner
            });
        egui::CentralPanel::default()
            .frame(frame)
            .show(surface, content);
        let bounds = footer.response.rect;
        surface.painter().hline(
            bounds.x_range(),
            bounds.top() + 0.5,
            egui::Stroke::new(1.0, palette.border),
        );
        footer.inner
    } else {
        crate::dialogs::Dialog::new("Settings", "", &[("Close", ())])
            .width(width)
            .footer_separator(true)
            .show_with_content(ctx, content)
    };
    if let Some(state) = &mut navigation {
        state.group = group;
        state.scroll = offsets.unwrap_or_default();
    }
    audio.navigation = navigation;
    ctx.data_mut(|data| data.insert_temp(group_id(), group));
    if !popup_was_open
        && ctx.input(|input| input.modifiers.is_none() && input.key_pressed(egui::Key::Escape))
    {
        if group == Group::Playback && audio.quality_draft.take().is_some() {
            return close.is_some();
        }
        return true;
    }
    close.is_some()
}

#[cfg(test)]
pub(super) fn show(
    ctx: &Context,
    settings: &mut AppSettings,
    models: &daw_project::SeparationModels,
    model_info: &std::collections::HashMap<
        daw_project::SeparationModelKind,
        daw_project::SeparationModelInfo,
    >,
    models_enabled: bool,
    model_action: &mut Option<ModelAction>,
    audio: &mut AudioControls<'_>,
) -> bool {
    show_surface(
        ctx,
        None,
        settings,
        models,
        model_info,
        models_enabled,
        model_action,
        audio,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;

    fn frame(
        ctx: &Context,
        settings: &mut AppSettings,
        events: Vec<egui::Event>,
    ) -> (bool, Vec<egui::epaint::ClippedShape>) {
        frame_at_size(ctx, settings, events, Vec2::new(900.0, 700.0))
    }

    fn frame_at_size(
        ctx: &Context,
        settings: &mut AppSettings,
        events: Vec<egui::Event>,
        size: Vec2,
    ) -> (bool, Vec<egui::epaint::ClippedShape>) {
        let mut closed = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                closed = show_surface(
                    ui.ctx(),
                    None,
                    settings,
                    &Default::default(),
                    &Default::default(),
                    true,
                    &mut None,
                    &mut AudioControls::default(),
                )
            },
        );
        output.textures_delta.clear();
        (closed, output.shapes)
    }

    fn click(ctx: &Context, settings: &mut AppSettings, label: &str) -> bool {
        let shapes = frame(ctx, settings, vec![]).1;
        let point = shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing label: {label}"));
        click_at(ctx, settings, point)
    }

    fn click_at(ctx: &Context, settings: &mut AppSettings, point: Pos2) -> bool {
        let button = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            ctx,
            settings,
            vec![egui::Event::PointerMoved(point), button(true)],
        );
        frame(ctx, settings, vec![button(false)]).0
    }

    #[test]
    fn sidebar_shows_only_the_active_group_without_changing_preferences() {
        for theme in ColorTheme::ALL {
            let ctx = Context::default();
            crate::theme::configure(&ctx);
            let mut settings = AppSettings {
                theme,
                ..Default::default()
            };
            crate::theme::apply(&ctx, settings.clone());
            let before = settings.clone();
            for (group, field) in [
                (Group::Appearance, "Theme"),
                (Group::Playback, "Seek cache"),
                (Group::TimeStretch, "Default engine"),
                (Group::Midi, "Split MIDI notes"),
                (Group::Recording, "Count-in"),
                (Group::StemSeparation, "CPU model"),
                (Group::Plugins, "Default folders"),
                (Group::Appearance, "Theme"),
            ] {
                frame(&ctx, &mut settings, vec![]);
                if group != Group::Appearance
                    || ctx.data_mut(|d| d.get_temp::<Group>(group_id())) != Some(Group::Appearance)
                {
                    click(&ctx, &mut settings, group.label());
                }
                let shapes = frame(&ctx, &mut settings, vec![]).1;
                let text = |label: &str| {
                    shapes.iter().any(
                        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label),
                    )
                };
                for item in Group::ALL {
                    assert!(text(item.label()));
                }
                for label in [
                    "Theme",
                    "Seek cache",
                    "Default engine",
                    "Split MIDI notes",
                    "Count-in",
                    "CPU model",
                    "Default folders",
                ] {
                    assert_eq!(
                        text(label),
                        label == field,
                        "visible field {label} on {group:?}"
                    );
                }
                for label in ["MIDI input", "MIDI channel"] {
                    assert_eq!(text(label), group == Group::Midi, "{label} on {group:?}");
                }
                for label in ["Recording offset", "Count-in enabled", "Panic"] {
                    assert_eq!(
                        text(label),
                        group == Group::Recording,
                        "{label} on {group:?}"
                    );
                }
                assert_eq!(settings, before);
                assert_eq!(
                    ctx.data_mut(|d| d.get_temp::<Group>(group_id())),
                    Some(group)
                );
            }
            assert!(click(&ctx, &mut settings, "Close"));
        }
    }

    #[test]
    fn sidebar_and_close_remain_visible_on_each_page_and_smaller_windows() {
        for theme in ColorTheme::ALL {
            for size in [Vec2::new(640.0, 480.0), Vec2::new(900.0, 700.0)] {
                let ctx = Context::default();
                crate::theme::configure(&ctx);
                let mut settings = AppSettings {
                    theme,
                    ..Default::default()
                };
                crate::theme::apply(&ctx, settings.clone());
                for group in Group::ALL {
                    ctx.data_mut(|d| d.insert_temp(group_id(), group));
                    for _ in 0..3 {
                        frame_at_size(&ctx, &mut settings, vec![], size);
                    }
                    let shapes = frame_at_size(&ctx, &mut settings, vec![], size).1;
                    for label in Group::ALL.map(Group::label).into_iter().chain(["Close"]) {
                        let bounds = shapes
                            .iter()
                            .find_map(|s| match &s.shape {
                                egui::Shape::Text(t) if t.galley.text() == label => {
                                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                                }
                                _ => None,
                            })
                            .expect("navigation and Close stay visible");
                        assert!(
                            Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds),
                            "{label}: {bounds:?} in {size:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn validated_model_details_use_small_text_below_each_selector() {
        use daw_project::{
            SeparationModelInfo as Info, SeparationModelKind as Kind, SeparationStemSet as Stems,
        };
        for theme in [ColorTheme::Dark, ColorTheme::Light, ColorTheme::Mist] {
            for width in [340.0, 650.0] {
                for (has_path, has_info) in [(true, true), (true, false), (false, true)] {
                    let ctx = Context::default();
                    crate::theme::configure(&ctx);
                    crate::theme::apply(
                        &ctx,
                        AppSettings {
                            theme,
                            ..Default::default()
                        },
                    );
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(width, 500.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            for (kind, file, stems, bytes) in [
                                (Kind::Cpu, "cpu.onnx", Stems::Six, 258_000_000),
                                (Kind::CoreMl, "coreml.onnx", Stems::Four, 316_000_000),
                            ] {
                                let info = Info {
                                    stems,
                                    file_size_bytes: bytes,
                                };
                                model_row(
                                    ui,
                                    110.0,
                                    kind,
                                    has_path.then(|| std::path::Path::new(file)),
                                    has_info.then_some(&info),
                                    &mut None,
                                    &mut AudioControls::default(),
                                );
                            }
                        },
                    );
                    output.textures_delta.clear();
                    let text = |label: &str| {
                        output.shapes.iter().find_map(|s| match &s.shape {
                            egui::Shape::Text(t) if t.galley.text() == label => Some(t),
                            _ => None,
                        })
                    };
                    let mut previous_bottom = 0.0;
                    for (file, summary, names) in [
                        (
                            "cpu.onnx",
                            "6 stems · 44.1 kHz · Stereo · 258.0 MB",
                            Stems::Six.description(),
                        ),
                        (
                            "coreml.onnx",
                            "4 stems · 44.1 kHz · Stereo · 316.0 MB",
                            Stems::Four.description(),
                        ),
                    ] {
                        if has_path && has_info {
                            let filename = text(file).unwrap();
                            let summary = text(summary).unwrap();
                            let names = text(names).unwrap();
                            assert!(filename.pos.y >= previous_bottom);
                            assert!(summary.pos.y >= filename.pos.y + filename.galley.size().y);
                            assert!(names.pos.y >= summary.pos.y + summary.galley.size().y);
                            assert!(
                                (summary.pos.x - filename.pos.x).abs() < 1.0,
                                "width={width}, summary={:?}, file={:?}",
                                summary.pos,
                                filename.pos
                            );
                            assert_eq!(
                                summary.galley.job.sections[0].format.font_id.size,
                                egui::TextStyle::Small
                                    .resolve(&ctx.style_of(ctx.theme()))
                                    .size
                            );
                            assert_eq!(
                                summary.galley.job.sections[0].format.color,
                                crate::theme::palette(&ctx).secondary
                            );
                            previous_bottom = names.pos.y + names.galley.size().y;
                        } else {
                            assert!(text(summary).is_none());
                            assert!(text(names).is_none());
                        }
                    }
                    output.textures_delta.clear();
                }
            }
        }
    }

    #[test]
    fn preferences_round_trip_all_colors_and_recover_from_missing_or_invalid_data() {
        assert_eq!(AppSettings::from_json(None).accent, AccentColor::Red);
        for accent in AccentColor::ALL {
            for theme in ColorTheme::ALL {
                for titlebar in [TitlebarStyle::Custom, TitlebarStyle::Native] {
                    let settings = AppSettings {
                        accent,
                        theme,
                        titlebar,
                        ..Default::default()
                    };
                    assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
                }
            }
            assert!(!accent.name().contains("500"));
        }
        for json in [
            None,
            Some(""),
            Some("broken"),
            Some("{}"),
            Some(r#"{"accent":"unknown"}"#),
        ] {
            assert_eq!(AppSettings::from_json(json), AppSettings::default());
        }
        let settings = AppSettings::from_json(Some(r#"{"accent":"blue_grey","future":true}"#));
        assert_eq!(settings.accent, AccentColor::BlueGrey);
        assert_eq!(settings.theme, ColorTheme::Dark);
        assert_eq!(settings.titlebar, TitlebarStyle::default());
        let legacy = AppSettings::from_json(Some(r#"{"accent":"teal","theme":"light"}"#));
        assert_eq!(legacy.accent, AccentColor::Teal);
        assert_eq!(legacy.theme, ColorTheme::Light);
        assert_eq!(legacy.titlebar, TitlebarStyle::default());
    }

    #[test]
    fn cache_profile_dropdown_round_trips_and_legacy_settings_default_to_balanced() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings {
            accent: AccentColor::Teal,
            theme: ColorTheme::Light,
            ..Default::default()
        };
        frame(&ctx, &mut settings, vec![]);
        assert_eq!(settings.seek_cache, CacheProfile::Balanced);
        click(&ctx, &mut settings, "Playback");
        for profile in [
            CacheProfile::Light,
            CacheProfile::Responsive,
            CacheProfile::Balanced,
        ] {
            let current = settings.seek_cache.name();
            click(&ctx, &mut settings, current);
            click(&ctx, &mut settings, profile.name());
            assert_eq!(settings.seek_cache, profile);
            assert_eq!(settings.accent, AccentColor::Teal);
            assert_eq!(settings.theme, ColorTheme::Light);
            assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        }
        let legacy = AppSettings::from_json(Some(r#"{"theme":"light","accent":"teal"}"#));
        assert_eq!(legacy.seek_cache, CacheProfile::Balanced);
        let future = AppSettings::from_json(Some(
            r#"{"theme":"light","accent":"teal","seek_cache":"future_profile"}"#,
        ));
        assert_eq!(future.seek_cache, CacheProfile::Balanced);
        assert_eq!(future.theme, ColorTheme::Light);
        assert_eq!(future.midi_note_split, daw_core::MidiNoteSplit::Split);
        let legacy_split = AppSettings::from_json(Some(
            r#"{"theme":"light","accent":"teal","midi_note_split":"future_mode"}"#,
        ));
        assert_eq!(legacy_split.midi_note_split, daw_core::MidiNoteSplit::Split);
        assert_eq!(legacy_split.theme, ColorTheme::Light);
    }

    #[test]
    fn seek_mode_defaults_to_fast_and_persists_exact() {
        use daw_output::PlaybackSeek;

        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings::from_json(Some(r#"{"theme":"light"}"#));
        assert_eq!(settings.playback_seek, PlaybackSeek::Fast);
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, "Playback");
        click(&ctx, &mut settings, "Fast");
        click(&ctx, &mut settings, "Exact");
        assert_eq!(settings.playback_seek, PlaybackSeek::Exact);
        assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        click(&ctx, &mut settings, "Exact");
        click(&ctx, &mut settings, "Fast");
        assert_eq!(settings.playback_seek, PlaybackSeek::Fast);
    }

    #[test]
    fn midi_note_split_dropdown_round_trips_without_changing_other_preferences() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings {
            accent: AccentColor::Teal,
            theme: ColorTheme::Light,
            ..Default::default()
        };
        frame(&ctx, &mut settings, vec![]);
        assert_eq!(settings.midi_note_split, daw_core::MidiNoteSplit::Split);
        click(&ctx, &mut settings, "MIDI");
        click(&ctx, &mut settings, "Split into two");
        click(&ctx, &mut settings, "Preserve first");
        assert_eq!(
            settings.midi_note_split,
            daw_core::MidiNoteSplit::PreserveFirst
        );
        assert_eq!(settings.accent, AccentColor::Teal);
        assert_eq!(settings.theme, ColorTheme::Light);
        assert_eq!(settings.seek_cache, CacheProfile::Balanced);
        assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        click(&ctx, &mut settings, "Preserve first");
        click(&ctx, &mut settings, "Split into two");
        assert_eq!(settings.midi_note_split, daw_core::MidiNoteSplit::Split);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn titlebar_dropdown_switches_without_changing_theme_or_accent() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings {
            accent: AccentColor::Teal,
            theme: ColorTheme::Light,
            titlebar: TitlebarStyle::Custom,
            ..Default::default()
        };
        crate::theme::apply(&ctx, settings.clone());
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, "Custom");
        click(&ctx, &mut settings, "Native");
        assert_eq!(settings.titlebar, TitlebarStyle::Native);
        assert_eq!(crate::theme::palette(&ctx), crate::theme::LIGHT);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Teal.color());
        assert!(click(&ctx, &mut settings, "Close"));
        assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        click(&ctx, &mut settings, "Native");
        click(&ctx, &mut settings, "Custom");
        assert_eq!(settings.titlebar, TitlebarStyle::Custom);
        assert_eq!(settings.theme, ColorTheme::Light);
        assert_eq!(settings.accent, AccentColor::Teal);
    }

    #[test]
    fn dropdown_foregrounds_keep_the_selected_accent_after_release_and_pointer_exit() {
        for theme in ColorTheme::ALL {
            for accent in [AccentColor::DeepOrange, AccentColor::Teal] {
                let label = theme.name();
                let ctx = Context::default();
                crate::theme::configure(&ctx);
                let mut settings = AppSettings {
                    accent,
                    theme,
                    ..Default::default()
                };
                crate::theme::apply(&ctx, settings.clone());
                for _ in 0..12 {
                    frame(&ctx, &mut settings, vec![]);
                }
                let button_pos = frame(&ctx, &mut settings, vec![])
                    .1
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos),
                        _ => None,
                    })
                    .unwrap();
                click(&ctx, &mut settings, label);
                assert!(egui::Popup::is_any_open(&ctx));
                for _ in 0..12 {
                    frame(&ctx, &mut settings, vec![]);
                }
                for events in [vec![], vec![egui::Event::PointerMoved(Pos2::ZERO)]] {
                    let shapes = frame(&ctx, &mut settings, events).1;
                    // Locate the button even when the popup opens above it.
                    let text = shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                            _ => None,
                        })
                        .min_by(|a, b| {
                            a.pos
                                .distance_sq(button_pos)
                                .total_cmp(&b.pos.distance_sq(button_pos))
                        })
                        .unwrap();
                    assert_eq!(text.fallback_color, accent.color(), "{theme:?} / {label}");
                }
                assert_eq!(
                    ctx.global_style().visuals.widgets.open.fg_stroke.color,
                    accent.color()
                );
                assert_eq!(
                    settings,
                    AppSettings {
                        accent,
                        theme,
                        ..Default::default()
                    }
                );
            }
        }
    }

    #[test]
    fn theme_dropdown_applies_stock_surfaces_and_switches_back_without_leaking() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings {
            accent: AccentColor::Teal,
            ..Default::default()
        };
        crate::theme::apply(&ctx, settings.clone());
        frame(&ctx, &mut settings, vec![]);
        let original = ctx.global_style();
        for (name, colors) in [
            ("Mist", crate::theme::MIST),
            ("Light", crate::theme::LIGHT),
            ("Mist", crate::theme::MIST),
            (
                "Midnight",
                crate::theme_files::stock_palette(ColorTheme::Midnight),
            ),
            ("Dark", crate::theme::DARK),
        ] {
            let current = settings.theme.name();
            click(&ctx, &mut settings, current);
            click(&ctx, &mut settings, name);
            frame(&ctx, &mut settings, vec![]);
            let style = ctx.global_style();
            assert_eq!(crate::theme::palette(&ctx), colors);
            assert_eq!(style.visuals.panel_fill, colors.panel);
            assert_eq!(style.visuals.text_edit_bg_color, Some(colors.input));
            assert_eq!(style.visuals.window_stroke.color, colors.border);
            assert_eq!(style.spacing, original.spacing);
            assert_eq!(style.text_styles, original.text_styles);
            assert_eq!(crate::theme::accent(&ctx), AccentColor::Teal.color());
            assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        }
        let other = Context::default();
        crate::theme::configure(&other);
        assert_eq!(crate::theme::palette(&other), crate::theme::DARK);
    }

    #[test]
    fn theme_dropdown_switches_surfaces_and_retains_accent_and_layout() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings {
            accent: AccentColor::Teal,
            ..Default::default()
        };
        crate::theme::apply(&ctx, settings.clone());
        frame(&ctx, &mut settings, vec![]);
        let dark_style = ctx.global_style();
        click(&ctx, &mut settings, "Dark");
        click(&ctx, &mut settings, "Light");
        assert_eq!(settings.theme, ColorTheme::Light);
        for _ in 0..12 {
            frame(&ctx, &mut settings, vec![]);
        }
        let shapes = frame(&ctx, &mut settings, vec![]).1;
        let light_style = ctx.global_style();
        assert_eq!(crate::theme::palette(&ctx), crate::theme::LIGHT);
        assert_eq!(light_style.visuals.window_fill, crate::theme::LIGHT.panel);
        assert_eq!(
            light_style.visuals.text_edit_bg_color,
            Some(crate::theme::LIGHT.input)
        );
        assert_eq!(light_style.spacing, dark_style.spacing);
        assert_eq!(light_style.text_styles, dark_style.text_styles);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Teal.color());
        fn has_panel(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Rect(r) => r.fill == crate::theme::LIGHT.panel,
                egui::Shape::Vec(children) => children.iter().any(has_panel),
                _ => false,
            }
        }
        assert!(shapes.iter().any(|s| has_panel(&s.shape)));
        let red = accent_centers(&frame(&ctx, &mut settings, vec![]).1)[0];
        click_at(&ctx, &mut settings, red);
        assert_eq!(settings.accent, AccentColor::Red);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        click(&ctx, &mut settings, "Light");
        click(&ctx, &mut settings, "Dark");
        assert_eq!(crate::theme::palette(&ctx), crate::theme::DARK);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        let other = Context::default();
        crate::theme::configure(&other);
        assert_eq!(crate::theme::accent(&other), AccentColor::default().color());
    }

    fn accent_centers(shapes: &[egui::epaint::ClippedShape]) -> [Pos2; 8] {
        AccentColor::CHOICES.map(|accent| {
            shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Circle(circle) if circle.fill == accent.color() => {
                        assert!(clipped.clip_rect.contains_rect(Rect::from_center_size(
                            circle.center,
                            Vec2::splat(circle.radius * 2.0)
                        )));
                        Some(circle.center)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing swatch {}", accent.name()))
        })
    }

    #[test]
    fn accent_swatches_fit_one_row_and_apply_each_choice_on_click() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        let mut settings = AppSettings::default();
        for _ in 0..12 {
            frame(&ctx, &mut settings, vec![]);
        }
        let shapes = frame(&ctx, &mut settings, vec![]).1;
        let centers = accent_centers(&shapes);
        assert!(
            centers
                .windows(2)
                .all(|pair| pair[0].x < pair[1].x && pair[0].y == pair[1].y)
        );
        let accent = |settings: &AppSettings| crate::theme::accent(&ctx) == settings.accent.color();
        for (color, center) in AccentColor::CHOICES.into_iter().zip(centers) {
            assert!(!click_at(&ctx, &mut settings, center));
            assert_eq!(settings.accent, color);
            assert!(accent(&settings));
            assert!(!egui::Popup::is_any_open(&ctx));
            assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        }
        assert!(click(&ctx, &mut settings, "Close"));
    }

    #[test]
    fn accents_update_all_themed_controls_without_leaking_between_contexts() {
        let a = Context::default();
        let b = Context::default();
        crate::theme::configure(&a);
        crate::theme::configure(&b);
        let initial = crate::theme::ruler_loop_colors(&a, true);
        crate::theme::set_accent(&a, AccentColor::Teal.color());
        let visuals = a.global_style().visuals.clone();
        assert_eq!(visuals.hyperlink_color, AccentColor::Teal.color());
        assert_eq!(visuals.text_cursor.stroke.color, AccentColor::Teal.color());
        assert_eq!(visuals.selection.stroke.color, AccentColor::Teal.color());
        assert_eq!(
            visuals.widgets.active.fg_stroke.color,
            AccentColor::Teal.color()
        );
        assert!(visuals.widgets.open.bg_stroke.is_empty());
        assert!(visuals.widgets.active.bg_stroke.is_empty());
        assert_eq!(
            crate::theme::selection_outline(&a).color,
            AccentColor::Teal.color()
        );
        assert_eq!(crate::theme::ruler_loop_colors(&a, true), initial);
        assert_eq!(crate::theme::accent(&b), AccentColor::default().color());
    }

    #[test]
    fn escape_closes_the_dropdown_first_then_the_dialog() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings::default();
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, ColorTheme::default().name());
        let key = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        assert!(!frame(&ctx, &mut settings, vec![key(true)]).0);
        assert!(!egui::Popup::is_any_open(&ctx));
        frame(&ctx, &mut settings, vec![key(false)]);
        assert!(frame(&ctx, &mut settings, vec![key(true)]).0);
    }
    #[test]
    fn plugin_folder_preferences_round_trip_and_reject_relative_or_duplicate_roots() {
        let root = std::env::temp_dir().join("daw-plugin-preferences-test");
        let folder = daw_plugin_host::SearchRoot {
            format: daw_plugin_host::PluginFormat::Vst3,
            path: root,
        };
        let settings = AppSettings {
            plugin_folders: vec![folder.clone()],
            ..Default::default()
        };
        assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);
        let mut value = serde_json::to_value(&settings).unwrap();
        value["plugin_folders"] =
            serde_json::json!([folder.clone(),folder,{"format":"clap","path":"relative"}]);
        assert_eq!(
            AppSettings::from_json(Some(&value.to_string()))
                .plugin_folders
                .len(),
            1
        );
        assert!(AppSettings::from_json(Some("{}")).plugin_folders.is_empty());
    }
}
