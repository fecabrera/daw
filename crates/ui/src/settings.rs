pub use daw_engine::CacheProfile;
use egui::{Color32, Context, Pos2, Rect, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "daw.settings";

/// Material Design's 500 shades, in palette order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    Red,
    Pink,
    #[default]
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
}

impl ColorTheme {
    pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::Mist];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Mist => "Mist",
        }
    }
}

/// Window presentation on macOS and Windows. Linux retains its native titlebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitlebarStyle {
    #[cfg_attr(target_os = "macos", default)]
    Custom,
    #[cfg_attr(not(target_os = "macos"), default)]
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
    pub audio: daw_output::AudioSettings,
    pub separation_models: std::sync::Arc<daw_project::SeparationModels>,
    pub accent: AccentColor,
    pub theme: ColorTheme,
    pub titlebar: TitlebarStyle,
    pub seek_cache: CacheProfile,
    /// How MIDI notes that cross the playhead are divided. Audio clips are unaffected.
    pub midi_note_split: daw_core::MidiNoteSplit,
}

impl AppSettings {
    pub fn from_json(json: Option<&str>) -> Self {
        json.and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).expect("Settings contain only serializable preferences")
    }
}

fn swatch(ui: &egui::Ui, rect: Rect, color: Color32) {
    ui.painter().rect_filled(rect, 2.0, color);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(
            1.0,
            crate::theme::palette(ui.ctx()).text.gamma_multiply(0.4),
        ),
        StrokeKind::Inside,
    );
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
                .on_hover_text(path.map_or_else(
                    || {
                        "Choose a local HTDemucs ONNX model to validate. No models are downloaded."
                            .into()
                    },
                    |p| format!("Validated model: {}", p.display()),
                ));
                if ui.button("Choose…").clicked()
                    && let Some(selected) = rfd::FileDialog::new()
                        .add_filter("ONNX model", &["onnx"])
                        .pick_file()
                {
                    *action = Some(ModelAction {
                        kind,
                        path: Some(selected),
                    });
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
                let color = crate::theme::palette(ui.ctx()).secondary;
                let summary = format!("{} stems · 44.1 kHz · Stereo · {:.1} MB", info.stems.names().len(), info.file_size_bytes as f64 / 1_000_000.0);
                ui.add(egui::Label::new(egui::RichText::new(summary).small().color(color)).wrap())
                    .on_hover_text("Stem names and audio format follow the validated HTDemucs contract. File size is shown in decimal megabytes.");
                ui.add(egui::Label::new(egui::RichText::new(info.stems.description()).small().color(color)).wrap());
            });
            },
        );
    }
}

/// Discovery data and actions for the main Settings device selectors.
#[derive(Default)]
pub struct AudioControls<'a> {
    pub devices: Option<&'a daw_output::Devices>,
    pub error: Option<&'a str>,
    pub enabled: bool,
    pub selection: Option<daw_output::AudioSettings>,
    pub quality_requested: bool,
    pub refresh_requested: bool,
}

pub fn show(
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
    let popup_was_open = egui::Popup::is_any_open(ctx);
    let close = crate::dialogs::Dialog::new("Settings", "", &[("Close", ())])
        .show_with_content(ctx, |ui| {
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
            let label_width = label_text("Split MIDI notes").max(
                label_text("Accent color") + ui.spacing().item_spacing.x + 14.0,
            );
            ui.strong("Appearance");
            #[cfg(any(target_os = "macos", target_os = "windows"))]
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
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Theme");
                },
                |ui| {
                    crate::menus::Dropdown::from_id_salt("color_theme")
                        .selected_text(settings.theme.name())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for theme in ColorTheme::ALL {
                                if ui
                                    .selectable_value(&mut settings.theme, theme, theme.name())
                                    .changed()
                                {
                                    crate::theme::apply(ctx, settings.clone());
                                }
                            }
                        });
                },
            );
            let swatch_color = settings.accent.color();
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Accent color");
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                    swatch(ui, rect, swatch_color);
                },
                |ui| {
                    crate::menus::Dropdown::from_id_salt("accent_color")
                        .selected_text(settings.accent.name())
                        .width(ui.available_width())
                        .height(300.0)
                        .show_ui(ui, |ui| {
                            for accent in AccentColor::ALL {
                                let selected = settings.accent == accent;
                                let response =
                                    ui.add(egui::Button::new("").selected(selected).min_size(
                                        Vec2::new(165.0, crate::toolbars::CONTROL_HEIGHT),
                                    ));
                                let rect = Rect::from_center_size(
                                    Pos2::new(
                                        response.rect.left() + 13.0,
                                        response.rect.center().y,
                                    ),
                                    Vec2::splat(14.0),
                                );
                                swatch(ui, rect, accent.color());
                                ui.painter().text(
                                    Pos2::new(rect.right() + 8.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    accent.name(),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    crate::theme::palette(ui.ctx()).text,
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        selected,
                                        accent.name(),
                                    )
                                });
                                if response.clicked() {
                                    settings.accent = accent;
                                    crate::theme::apply(ctx, settings.clone());
                                    ui.close();
                                }
                            }
                        });
                },
            );
            ui.separator();
            ui.strong("Playback");
            ui.add_enabled_ui(audio.enabled, |ui| {
                let empty = daw_output::Devices::default();
                let devices = audio.devices.unwrap_or(&empty);
                let mut draft = settings.audio.clone();
                let output_changed = crate::audio_settings::device_row(ui, label_width, "Output device", &mut draft.output_device, &devices.outputs);
                let input_changed = crate::audio_settings::device_row(ui, label_width, "Input device", &mut draft.input_device, &devices.inputs);
                if output_changed || input_changed { audio.selection = Some(draft); }
                crate::rows::wrapped(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                    if ui.button("Audio quality defaults…").clicked() { audio.quality_requested = true; }
                    if ui.button("Refresh devices").clicked() { audio.refresh_requested = true; }
                });
            });
            ui.label("Changing output stops playback. Input selection is saved for future recording; input monitoring is not available.");
            if !audio.enabled { ui.label("Checking audio devices…"); }
            if let Some(error) = audio.error { ui.label(error); }
            crate::rows::field(
                ui, label_width,
                |ui| { ui.label("Seek cache"); },
                |ui| {
                    crate::menus::Dropdown::from_id_salt("seek_cache")
                        .selected_text(settings.seek_cache.name())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for profile in CacheProfile::ALL {
                                let hint = match profile {
                                    CacheProfile::Light => "32 MiB of checkpoints. Lower background load; builds while stopped or paused.",
                                    CacheProfile::Balanced => "128 MiB of checkpoints. Moderate background work, including during playback.",
                                    CacheProfile::Responsive => "256 MiB of checkpoints. More background work for faster cache coverage.",
                                };
                                ui.selectable_value(&mut settings.seek_cache, profile, profile.name())
                                    .on_hover_text(hint);
                            }
                        })
                        .response.on_hover_text("Prepares exact seek history. Profiles change CPU and memory use; sound stays the same.");
                },
            );
            ui.separator();
            ui.strong("MIDI");
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
                                let hint = match mode {
                                    daw_core::MidiNoteSplit::Split => "A note that crosses the playhead becomes two notes.",
                                    daw_core::MidiNoteSplit::PreserveFirst => "A note that crosses the playhead stays on the first clip.",
                                };
                                ui.selectable_value(&mut settings.midi_note_split, mode, mode.name())
                                    .on_hover_text(hint);
                            }
                        })
                        .response
                        .on_hover_text("Applies when you split MIDI clips. Audio clips are unchanged. This choice stays on this computer.");
                },
            );
            ui.separator();
            ui.strong("Stem Separation");
            ui.add_enabled_ui(daw_project::SEPARATION_AVAILABLE && models_enabled, |ui| {
                model_row(ui, label_width, daw_project::SeparationModelKind::Cpu, models.cpu.as_deref(), model_info.get(&daw_project::SeparationModelKind::Cpu), model_action);
                #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
                model_row(ui, label_width, daw_project::SeparationModelKind::CoreMl, models.coreml.as_deref(), model_info.get(&daw_project::SeparationModelKind::CoreMl), model_action);
            });
            if !daw_project::SEPARATION_AVAILABLE {
                ui.label(daw_project::SEPARATION_UNAVAILABLE_REASON);
            }
        })
        .is_some();
    close || (!popup_was_open && ctx.input(|input| input.key_pressed(egui::Key::Escape)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &Context,
        settings: &mut AppSettings,
        events: Vec<egui::Event>,
    ) -> (bool, Vec<egui::epaint::ClippedShape>) {
        let mut closed = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| {
                closed = show(
                    ui.ctx(),
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
                for label in [theme.name(), accent.name()] {
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
                            egui::Shape::Text(text) if text.galley.text() == label => {
                                Some(text.pos)
                            }
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
                                egui::Shape::Text(text) if text.galley.text() == label => {
                                    Some(text)
                                }
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
    }

    #[test]
    fn mist_dropdown_applies_reference_surfaces_and_switches_back_without_leaking() {
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
        click(&ctx, &mut settings, "Teal");
        for _ in 0..12 {
            frame(&ctx, &mut settings, vec![]);
        }
        click(&ctx, &mut settings, "Red");
        assert_eq!(settings.accent, AccentColor::Red);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        click(&ctx, &mut settings, "Light");
        click(&ctx, &mut settings, "Dark");
        assert_eq!(crate::theme::palette(&ctx), crate::theme::DARK);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        let other = Context::default();
        crate::theme::configure(&other);
        assert_eq!(crate::theme::accent(&other), AccentColor::Purple.color());
    }

    #[test]
    fn dropdown_paints_named_swatches_on_the_left_and_updates_the_accent() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        let mut settings = AppSettings::default();
        frame(&ctx, &mut settings, vec![]);
        assert!(!click(&ctx, &mut settings, "Purple"));
        for _ in 0..12 {
            frame(&ctx, &mut settings, vec![]);
        }
        let shapes = frame(&ctx, &mut settings, vec![]).1;
        for accent in AccentColor::ALL {
            let label = shapes
                .iter()
                .rev()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == accent.name() => Some(text),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing option: {}", accent.name()));
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.fill == accent.color() && rect.rect.size() == Vec2::splat(14.0)
                && rect.rect.right() < label.pos.x && (rect.rect.center().y - (label.pos.y + label.galley.rect.center().y)).abs() < 1.0)), "Missing left swatch: {}", accent.name());
        }
        assert!(!click(&ctx, &mut settings, "Red"));
        assert_eq!(settings.accent, AccentColor::Red);
        assert_eq!(crate::theme::accent(&ctx), AccentColor::Red.color());
        assert!(!egui::Popup::is_any_open(&ctx));
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
        assert_eq!(
            visuals.widgets.open.bg_stroke.color,
            AccentColor::Teal.color()
        );
        assert_eq!(
            crate::theme::selection_outline(&a).color,
            AccentColor::Teal.color()
        );
        assert_eq!(crate::theme::ruler_loop_colors(&a, true), initial);
        assert_eq!(crate::theme::accent(&b), AccentColor::Purple.color());
    }

    #[test]
    fn escape_closes_the_dropdown_first_then_the_dialog() {
        let ctx = Context::default();
        crate::theme::configure(&ctx);
        let mut settings = AppSettings::default();
        frame(&ctx, &mut settings, vec![]);
        click(&ctx, &mut settings, "Purple");
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
}
