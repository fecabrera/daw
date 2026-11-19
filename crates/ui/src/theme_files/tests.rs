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
    create_overrides(&dir.0).unwrap();
    let stock = Catalog::scan(&dir.0).unwrap();
    assert!(stock.issues.is_empty(), "{:?}", stock.issues);
    for (entry, colors) in stock.entries.iter().zip([
        theme::DARK,
        theme::LIGHT,
        theme::MIST,
        stock_palette(ColorTheme::Midnight),
    ]) {
        assert_eq!(entry.colors, colors);
        let base = entry.base;
        let parent = match base {
            ColorTheme::Mist => mist_from_light(theme::LIGHT),
            _ => colors,
        };
        let export = document::export_stock(base, colors, parent);
        assert_eq!(
            Document::parse(export.as_bytes()).unwrap().overlay(parent),
            colors
        );
    }

    let light = json(
        "light",
        "Light",
        "light",
        r##""text":"#172B40","time_unit":"#28394A","button_background":"#E3EBF5""##,
    );
    dir.write("light.json", &light);
    create_overrides(&dir.0).unwrap();
    assert_eq!(fs::read_to_string(dir.0.join("light.json")).unwrap(), light);
    fs::rename(dir.0.join("light.json"), dir.0.join("my-light.json")).unwrap();
    create_overrides(&dir.0).unwrap();
    assert!(!dir.0.join("light.json").exists());
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
    sender.send(Ok(Catalog::scan(&dir.0).unwrap())).unwrap();
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
    sender.send(Ok(Catalog::scan(&dir.0).unwrap())).unwrap();
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
    create_overrides(&dir.0).unwrap();
    assert!(!dir.0.join("midnight.json").exists());
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
    let point = shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Reload themes" => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .expect("Appearance has a reload action");
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
