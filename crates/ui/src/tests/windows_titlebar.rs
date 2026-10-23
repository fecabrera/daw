use super::*;

fn text_rect(shapes: &[egui::epaint::ClippedShape], label: &str) -> Rect {
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing text: {label}"))
}

fn click_at(app: &mut DawUi, ctx: &egui::Context, point: Pos2, size: Vec2) {
    frame_sized(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
        size,
    );
    frame_sized(app, ctx, vec![button(point, false)], size);
}

#[test]
fn custom_header_layout_reserves_native_buttons_and_switches_views_without_project_edits() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        for width in [900.0, 1280.0] {
            for name in ["Windows project".into(), "Long project title ".repeat(30)] {
                let mut app = DawUi::default();
                let ctx = context();
                app.set_settings(
                    &ctx,
                    settings::AppSettings {
                        theme,
                        titlebar: settings::TitlebarStyle::Custom,
                        ..Default::default()
                    },
                );
                app.session.project.name = name;
                app.caption_metrics.top_inset = if width == 1280.0 { 8.0 } else { 0.0 };
                let size = Vec2::new(width, 800.0);
                frame_sized(&mut app, &ctx, vec![], size);
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                let layout = app.header_layout(&ctx);
                assert!((layout.bounds.top() - app.caption_metrics.top_inset).abs() < 0.1);
                let menu = layout.controls[0];
                let timeline = text_rect(&shapes, "Timeline");
                let mixer = text_rect(&shapes, "Mixer");
                assert!(menu.right() < timeline.left());
                assert!(timeline.right() < mixer.left());
                assert!((menu.center().y - mixer.center().y).abs() < 0.1);
                assert!(
                    layout.controls[1].right() <= width - app.caption_metrics.width,
                    "width={width}, name={}, controls={:?}, timeline={timeline:?}, mixer={mixer:?}",
                    app.session.project.name.len(),
                    layout.controls
                );
                assert!(mixer.bottom() < app.time_bounds[0].top());
                assert!(!layout.can_drag(menu.center()));
                assert!(!layout.can_drag(mixer.center()));
                for rect in layout.captions {
                    assert!(!layout.can_drag(rect.center()));
                    for shape in &shapes {
                        if let egui::Shape::Rect(paint) = &shape.shape
                            && paint.fill.a() != 0
                        {
                            let visible = paint.rect.intersect(shape.clip_rect);
                            assert!(
                                !visible.intersect(rect).is_positive(),
                                "App backgrounds must not cover the native caption region"
                            );
                        }
                    }
                }
                if app.session.project.name == "Windows project" {
                    let title = text_rect(&shapes, &app.window_title);
                    assert!(menu.right() < title.left() && title.right() < timeline.left());
                    assert!((title.center().y - mixer.center().y).abs() < 0.1);
                    assert!(layout.can_drag(title.center()));
                }
                let project = format!("{:?}", app.session.project);
                click_at(&mut app, &ctx, mixer.center(), size);
                assert_eq!(app.view_mode, ViewMode::Mixer);
                assert_eq!(format!("{:?}", app.session.project), project);
                assert!(!app.dirty);
                app.settings_open = true;
                click_at(&mut app, &ctx, timeline.center(), size);
                assert_eq!(app.view_mode, ViewMode::Mixer);
                assert!(!app.header_layout(&ctx).can_drag(menu.center()));
                app.settings_open = false;
                app.settings.titlebar = settings::TitlebarStyle::Native;
                let shapes = frame_shapes(&mut app, &ctx, vec![], size);
                text_rect(&shapes, "Timeline");
                text_rect(&shapes, "Mixer");
                assert!(!app.header_layout(&ctx).bounds.is_positive());
                assert_eq!(format!("{:?}", app.session.project), project);
            }
        }
    }
}

#[test]
fn burger_menu_contains_the_existing_menus_and_settings_action() {
    for titlebar in [
        settings::TitlebarStyle::Native,
        settings::TitlebarStyle::Custom,
    ] {
        let mut app = DawUi::default();
        app.settings.titlebar = titlebar;
        let ctx = context();
        let size = Vec2::new(900.0, 600.0);
        frame_sized(&mut app, &ctx, vec![], size);
        let menu = if titlebar == settings::TitlebarStyle::Custom {
            app.header_layout(&ctx).controls[0].center()
        } else {
            let shapes = frame_shapes(&mut app, &ctx, vec![], size);
            Pos2::new(
                f32::from(theme::TOOLBAR_PADDING) + 14.0,
                text_rect(&shapes, "Timeline").center().y,
            )
        };
        click_at(&mut app, &ctx, menu, size);
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        assert!(egui::Popup::is_any_open(&ctx));
        assert!(!app.header_layout(&ctx).can_drag(Pos2::new(200.0, 20.0)));
        for label in ["File", "Edit", "Clip", "Track", "View"] {
            text_rect(&shapes, label);
        }
        let file = text_rect(&shapes, "File").center();
        click_at(&mut app, &ctx, file, size);
        let shapes = frame_shapes(&mut app, &ctx, vec![], size);
        let settings = text_rect(&shapes, "Settings…").center();
        click_at(&mut app, &ctx, settings, size);
        assert!(app.settings_open);
        assert!(!app.dirty);
    }
}
