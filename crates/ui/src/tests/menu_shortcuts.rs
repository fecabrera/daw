use super::*;

#[test]
fn all_menus_show_command_symbol_on_mac_and_ctrl_on_windows_and_linux() {
    use egui::os::OperatingSystem;
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Nix,
    ] {
        for color_theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
            for selected in [false, true] {
                let (mut app, track) = fixture();
                let ctx = context();
                ctx.set_os(os);
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme: color_theme,
                        ..Default::default()
                    },
                );
                if selected {
                    app.select_clip(app.session.project.tracks[0].clips[0].id, track, false);
                }
                for menu in ["File", "Edit", "Track", "Context"] {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(600.0, 600.0),
                            )),
                            ..Default::default()
                        },
                        |ui| match menu {
                            "File" => app.file_menu(ui),
                            "Edit" => app.edit_menu(ui),
                            "Track" => app.track_menu(ui),
                            _ => {
                                app.track_context_menu(ui);
                            }
                        },
                    );
                    let labels: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) => Some(text.galley.text()),
                            _ => None,
                        })
                        .collect();
                    for shape in &output.shapes {
                        if let egui::Shape::Text(text) = &shape.shape {
                            let label = text.galley.text();
                            if label.contains('⌘') || label.starts_with("Ctrl+") {
                                let family = if os == OperatingSystem::Mac {
                                    fonts::menu_shortcut_family()
                                } else {
                                    egui::FontFamily::Proportional
                                };
                                assert!(
                                    text.galley
                                        .job
                                        .sections
                                        .iter()
                                        .all(|section| { section.format.font_id.family == family }),
                                    "{menu} shortcut {label} uses the wrong font"
                                );
                            }
                        }
                    }
                    if os == OperatingSystem::Mac {
                        let expected: &[&str] = match menu {
                            "File" => &[
                                "⌘N",
                                "⌘O",
                                "Shift+⌘I",
                                "⌘S",
                                "Shift+⌘S",
                                "Shift+⌘E",
                                "⌘W",
                                "⌘,",
                            ],
                            "Edit" => &["⌘C", "⌘V"],
                            _ => &["⌘R"],
                        };
                        for label in expected {
                            assert!(labels.contains(label), "{menu} missing {label}: {labels:?}");
                        }
                        assert!(labels.iter().all(|label| !label.contains("Cmd")));
                    } else {
                        let expected: &[&str] = match menu {
                            "File" => &["Ctrl+N", "Ctrl+Shift+S"],
                            "Edit" => &["Ctrl+C", "Ctrl+V"],
                            _ => &["Ctrl+R"],
                        };
                        for label in expected {
                            assert!(labels.contains(label), "{menu} missing {label}: {labels:?}");
                        }
                        assert!(labels.iter().all(|label| !label.contains('⌘')));
                    }
                    output.textures_delta.clear();
                }
                assert!(!app.dirty);
            }
        }
    }
}
