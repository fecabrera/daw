use super::*;

struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-themes-{}", daw_core::Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, file: &str, text: &str) {
        fs::write(self.0.join(file), text).unwrap();
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn json(id: &str, name: &str, base: &str, colors: &str) -> String {
    format!(r#"{{"version":1,"id":"{id}","name":"{name}","base":"{base}","colors":{{{colors}}}}}"#)
}

#[test]
fn stock_templates_match_compiled_colors_and_mist_overlays_effective_light() {
    let dir = Folder::new();
    let stock = Catalog::scan(&dir.0).unwrap();
    assert!(stock.issues.is_empty(), "{:?}", stock.issues);
    for (entry, colors) in stock.entries.iter().zip([
        theme::DARK,
        theme::LIGHT,
        theme::MIST,
        stock_palette(ColorTheme::Midnight),
    ]) {
        assert_eq!(entry.colors, colors);
    }

    let light = json(
        "light",
        "Light",
        "light",
        r##""text":"#172B40","time_unit":"#28394A","button_background":"#E3EBF5""##,
    );
    dir.write("light.json", &light);
    dir.write(
        "dark.json",
        &json("dark", "Dark", "dark", r##""panel":"#0D1117""##),
    );
    dir.write("room.JSON", &json("user.room", "Room", "mist", ""));
    let catalog = Catalog::scan(&dir.0).unwrap();
    assert!(catalog.issues.is_empty(), "{:?}", catalog.issues);
    let [dark, light, mist, _, room] = catalog.entries.as_slice() else {
        panic!("Expected the four stock entries and one custom theme");
    };
    assert_eq!(dark.colors.panel, egui::Color32::from_rgb(13, 17, 23));
    assert_eq!(dark.colors.button_background, theme::DARK.button_background);
    assert_eq!(mist.colors.text, light.colors.text);
    assert_eq!(mist.colors.time_unit, light.colors.time_unit);
    assert_eq!(
        light.colors.button_background,
        egui::Color32::from_rgb(227, 235, 245)
    );
    assert_eq!(mist.colors.button_background, theme::MIST.button_background);
    assert_eq!(mist.colors.panel, theme::MIST.panel);
    assert_eq!(
        room.colors,
        theme::MIST,
        "Custom bases use the original stock"
    );
}

#[test]
fn strict_schema_and_conflicts_leave_valid_themes_available() {
    let dir = Folder::new();
    dir.write(
        "bad.json",
        &json("user.bad", "Bad", "light", r#""panel":null"#),
    );
    dir.write(
        "duplicate-role.json",
        &json(
            "user.dup",
            "Duplicate",
            "light",
            r##""text":"#111111","text":"#121212""##,
        ),
    );
    dir.write("unsupported.json", r#"{"version":42}"#);
    dir.write(
        "missing.json",
        &json(
            "user.missing",
            "Missing",
            "dark",
            r##""missing":"#FF0000""##,
        ),
    );
    dir.write(
        "contrast.json",
        &json(
            "user.contrast",
            "Contrast",
            "light",
            r##""text":"#FFFFFF""##,
        ),
    );
    dir.write("huge.json", &" ".repeat(MAX_BYTES + 1));
    dir.write("z.json", &json("user.conflict", "Z", "dark", ""));
    dir.write("a.json", &json("user.conflict", "A", "light", ""));
    dir.write("valid.json", &json("user.valid", "Valid", "dark", ""));
    let catalog = Catalog::scan(&dir.0).unwrap();
    assert_eq!(catalog.entries.len(), ColorTheme::ALL.len() + 1);
    assert_eq!(catalog.entries.last().unwrap().id, "user.valid");
    for diagnostic in [
        "panel",
        "duplicate color role text",
        "Unsupported theme version 42",
        "unknown variant",
        "contrast",
        "bytes",
        "Duplicate theme ID user.conflict",
    ] {
        assert!(
            catalog
                .issues
                .iter()
                .any(|issue| issue.contains(diagnostic)),
            "{diagnostic}: {:?}",
            catalog.issues
        );
    }
    let duplicate = catalog
        .issues
        .iter()
        .find(|issue| issue.contains("Duplicate theme ID"))
        .unwrap();
    assert!(duplicate.find("a.json").unwrap() < duplicate.find("z.json").unwrap());
}

#[test]
fn invalid_custom_preference_keeps_other_settings() {
    for custom in ["42", "null", r#""../outside""#, r#""dark""#] {
        let settings = AppSettings::from_json(Some(&format!(
            r#"{{"theme":"light","accent":"teal","custom_theme_id":{custom}}}"#
        )));
        assert_eq!(settings.theme, ColorTheme::Light);
        assert_eq!(settings.accent, crate::settings::AccentColor::Teal);
        assert!(settings.custom_theme_id.is_none());
    }
}

#[test]
fn publication_uses_current_selection_and_keeps_request_across_fallback() {
    let dir = Folder::new();
    let context = egui::Context::default();
    theme::configure(&context);
    let mut settings = AppSettings {
        theme: ColorTheme::Light,
        custom_theme_id: Some("user.room".into()),
        ..Default::default()
    };
    let mut runtime = Runtime {
        catalog: Catalog::scan(&dir.0).unwrap(),
        ..Default::default()
    };
    assert_eq!(runtime.active(&settings).id, "light");
    dir.write(
        "first.json",
        &json("user.room", "Room", "dark", r##""panel":"#0D1117""##),
    );
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Ok(Some(Catalog::scan(&dir.0).unwrap())))
        .unwrap();
    runtime.pending = Some(receiver);
    runtime.poll(&context, &settings);
    assert_eq!(theme::style_base(&context), theme::StyleBase::Dark);
    assert_eq!(
        theme::palette(&context).panel,
        egui::Color32::from_rgb(13, 17, 23)
    );
    fs::rename(dir.0.join("first.json"), dir.0.join("second.json")).unwrap();
    dir.write("second.json", &json("user.room", "New name", "dark", ""));
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Ok(Some(Catalog::scan(&dir.0).unwrap())))
        .unwrap();
    runtime.pending = Some(receiver);
    settings.custom_theme_id = None;
    runtime.poll(&context, &settings);
    assert_eq!(theme::style_base(&context), theme::StyleBase::Light);
    assert_eq!(runtime.entries().last().unwrap().name, "New name");
}

#[test]
fn midnight_is_built_in_and_reuses_legacy_selection_and_override() {
    let dir = Folder::new();
    let mut runtime = Runtime::default();
    let settings = AppSettings::from_json(Some(
        r#"{"theme":"light","custom_theme_id":"user.midnight","accent":"teal"}"#,
    ));
    assert_eq!(settings.theme, ColorTheme::Midnight);
    assert!(settings.custom_theme_id.is_none());
    assert_eq!(AppSettings::from_json(Some(&settings.to_json())), settings);

    let entry = runtime.active(&settings);
    assert_eq!(entry.id, "midnight");
    assert_eq!(entry.name, "Midnight");
    assert_eq!(entry.colors.background, egui::Color32::from_rgb(20, 21, 23));
    assert_eq!(
        entry.colors.button_background,
        egui::Color32::from_rgb(43, 44, 46)
    );

    let legacy = json(
        "user.midnight",
        "Midnight",
        "dark",
        r##""panel":"#0D1117""##,
    );
    dir.write("my-midnight.json", &legacy);
    runtime.catalog = Catalog::scan(&dir.0).unwrap();
    assert!(
        runtime.catalog.issues.is_empty(),
        "{:?}",
        runtime.catalog.issues
    );
    assert_eq!(runtime.entries().len(), ColorTheme::ALL.len());
    assert_eq!(
        fs::read_to_string(dir.0.join("my-midnight.json")).unwrap(),
        legacy
    );

    let ctx = egui::Context::default();
    runtime.apply(&ctx, &settings);
    assert_eq!(theme::style_base(&ctx), theme::StyleBase::Dark);
    assert_eq!(
        theme::palette(&ctx).panel,
        egui::Color32::from_rgb(13, 17, 23)
    );
    assert_eq!(theme::accent(&ctx), settings.accent.color());
}

#[test]
fn open_folder_creates_directory_without_reloading_the_catalog() {
    fn open_directory(path: &Path) -> Result<(), String> {
        assert!(path.is_dir());
        fs::write(path.join("opened"), "").map_err(|error| error.to_string())
    }

    let dir = Folder::new();
    let directory = dir.0.join("themes");
    let ctx = egui::Context::default();
    let mut runtime = Runtime {
        directory: Some(directory.clone()),
        open_folder: Some(open_directory),
        ..Default::default()
    };
    runtime.catalog.issues.push("Existing catalog".into());
    runtime.start(&ctx, Action::OpenFolder);
    let result = runtime
        .pending
        .as_ref()
        .unwrap()
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    sender.send(result).unwrap();
    runtime.pending = Some(receiver);
    runtime.poll(&ctx, &AppSettings::default());
    assert!(directory.join("opened").is_file());
    assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    assert_eq!(runtime.status().collect::<Vec<_>>(), ["Existing catalog"]);
}

#[test]
fn reload_button_applies_same_panel_edits_without_editing_the_project() {
    let dir = Folder::new();
    let ctx = crate::tests::context();
    let mut app = crate::DawUi::default();
    app.set_settings(
        &ctx,
        AppSettings {
            theme: ColorTheme::Light,
            ..Default::default()
        },
    );
    app.themes.directory = Some(dir.0.clone());
    app.settings_open = true;
    let project = app.session.project.clone();
    let draw = |events, app: &mut crate::DawUi| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 768.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        output.shapes
    };
    draw(vec![], &mut app);
    let shapes = draw(vec![], &mut app);
    let label_rect = |label: &str| {
        shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing Appearance label {label}"))
    };
    let dropdown = label_rect("Light");
    let accent = label_rect("Accent color");
    let folder_label = label_rect(dir.0.to_str().unwrap());
    let folder_offset = folder_label.center().y - accent.center().y;
    assert!(
        (0.0..=crate::toolbars::CONTROL_HEIGHT * 3.0).contains(&folder_offset),
        "The folder row must follow the Appearance controls, got offset {folder_offset}"
    );
    let icons_right_of = |label: egui::Rect| {
        let mut icons = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect.brush.is_some()
                        && rect.rect.left() > label.right()
                        && rect.rect.y_range().contains(label.center().y) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        icons.sort_by(|a, b| a.left().total_cmp(&b.left()));
        icons
    };
    let [refresh] = icons_right_of(dropdown)[..] else {
        panic!("Expected one refresh icon beside the Theme dropdown");
    };
    let [copy, open] = icons_right_of(folder_label)[..] else {
        panic!("Expected two icons on the folder row");
    };
    assert_eq!(
        refresh.size(),
        egui::Vec2::splat(crate::icons::TOOLBAR_ICON_SIZE)
    );
    assert_eq!(refresh.size(), copy.size());
    assert_eq!(copy.size(), open.size());
    assert_eq!(copy.center().y, open.center().y);
    assert!((refresh.center().x - open.center().x).abs() <= 1.0);
    let point = refresh.center();
    let panel = theme::palette(&ctx).panel;
    let border = theme::palette(&ctx).border;
    let separator = theme::unit_separator(&ctx);
    dir.write(
        "light.json",
        &json(
            "light",
            "Light",
            "light",
            r##""text":"#172B40","button_background":"#E3EBF5""##,
        ),
    );
    draw(
        vec![
            egui::Event::PointerMoved(point),
            crate::tests::button(point, true),
        ],
        &mut app,
    );
    draw(vec![crate::tests::button(point, false)], &mut app);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.themes.busy() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
        draw(vec![], &mut app);
    }
    assert!(!app.themes.busy());
    let expected = egui::Color32::from_rgb(23, 43, 64);
    assert_eq!(theme::palette(&ctx).text, expected);
    assert_eq!(
        ctx.global_style()
            .visuals
            .widgets
            .noninteractive
            .fg_stroke
            .color,
        expected
    );
    assert_eq!(crate::mixer::header_text(&ctx), expected);
    let visuals = &ctx.global_style().visuals;
    assert_eq!(
        visuals.widgets.inactive.weak_bg_fill,
        egui::Color32::from_rgb(227, 235, 245)
    );
    assert_eq!(visuals.widgets.hovered.weak_bg_fill, border);
    assert_eq!(visuals.window_stroke.color, border);
    assert_eq!(theme::unit_header_fill(&ctx, false), border);
    assert_eq!(theme::unit_separator(&ctx), separator);
    assert_eq!(theme::palette(&ctx).panel, panel);
    assert_eq!(app.session.project, project);
    assert!(!app.dirty);
}
