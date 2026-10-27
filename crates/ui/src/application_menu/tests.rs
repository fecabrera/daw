use super::*;
use egui::{Event, Pos2, Rect, Vec2, epaint::ClippedShape};

struct Frame {
    button: Rect,
    shapes: Vec<ClippedShape>,
}

fn frame(app: &mut DawUi, ctx: &egui::Context, events: Vec<Event>) -> Frame {
    let mut button = Rect::NOTHING;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
            crate::toolbars::Toolbar::row(ui, |ui| {
                button = app.application_menu(ui).rect;
            });
        },
    );
    output.textures_delta.clear();
    Frame {
        button,
        shapes: output.shapes,
    }
}

fn click(app: &mut DawUi, ctx: &egui::Context, pos: Pos2) -> Frame {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, ctx, vec![Event::PointerMoved(pos)])
}

fn text<'a>(frame: &'a Frame, label: &str) -> &'a egui::epaint::TextShape {
    frame
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => Some(text),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing menu label {label}"))
}

fn text_rect(frame: &Frame, label: &str) -> Rect {
    let text = text(frame, label);
    Rect::from_min_size(text.pos, text.galley.size())
}

fn highlight<'a>(
    frame: &'a Frame,
    fill: egui::Color32,
    label: &str,
) -> &'a egui::epaint::RectShape {
    let label = text_rect(frame, label);
    frame
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill == fill && rect.rect.contains_rect(label) => {
                Some(rect)
            }
            _ => None,
        })
        .expect("Shared full-row highlight")
}

#[test]
fn application_menu_reuses_dropdown_rows_submenus_and_dividers() {
    for os in [
        egui::os::OperatingSystem::Windows,
        egui::os::OperatingSystem::Nix,
    ] {
        for theme in crate::settings::ColorTheme::ALL {
            let ctx = egui::Context::default();
            crate::theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            crate::icons::configure(&ctx);
            ctx.set_os(os);
            let mut app = DawUi::default();
            let mut settings = app.settings();
            settings.theme = theme;
            app.set_settings(&ctx, settings);
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            let project = app.session.project.clone();
            let initial = frame(&mut app, &ctx, vec![]);
            let root = click(&mut app, &ctx, initial.button.center());
            for label in ["File", "Edit", "Clip", "Track", "View"] {
                let text = text(&root, label);
                assert_eq!(text.galley.rows.len(), 1);
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size == 13.0)
                );
            }
            assert!(
                !root.shapes.iter().any(|shape| matches!(
                    &shape.shape, egui::Shape::Text(text) if text.galley.text() == "⏵"
                )),
                "Submenu arrows use Lucide artwork"
            );
            let file = text_rect(&root, "File").center();
            click(&mut app, &ctx, file);
            let child = frame(&mut app, &ctx, vec![Event::PointerMoved(file)]);
            let row = highlight(&child, crate::theme::TOOLBAR_HOVER_FILL, "File");
            assert!(row.rect.width() >= menus::MIN_WIDTH);
            assert!(row.corner_radius.nw > 0);
            assert_eq!(row.stroke.width, 0.0);
            let row = row.rect;
            assert!(
                (text_rect(&child, "Edit").center().y
                    - text_rect(&child, "File").center().y
                    - row.height()
                    - menus::ITEM_SPACING)
                    .abs()
                    < 0.1
            );
            for label in [
                "New project",
                "Open…",
                "Save",
                "Save as…",
                "Export…",
                "Project Settings…",
                "Settings…",
                "Close project",
                "Ctrl+N",
            ] {
                let text = text(&child, label);
                assert_eq!(text.galley.rows.len(), 1, "{label}");
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size == 13.0)
                );
            }
            let open = text_rect(&child, "Open…").center();
            let hovered = frame(&mut app, &ctx, vec![Event::PointerMoved(open)]);
            let open_row = highlight(&hovered, crate::theme::TOOLBAR_HOVER_FILL, "Open…").rect;
            assert!(open_row.width() >= 220.0);
            let save = text_rect(&hovered, "Save");
            let divider_y = hovered
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, .. }
                        if points[0].y > open_row.bottom()
                            && points[0].y < save.top()
                            && points[0].x >= open_row.left() =>
                    {
                        Some(points[0].y)
                    }
                    _ => None,
                })
                .expect("File group divider");
            assert!((divider_y - 0.5 - open_row.bottom() - menus::ITEM_SPACING).abs() < 0.1);
            click(&mut app, &ctx, text_rect(&hovered, "Export…").center());
            assert!(app.export_dialog.is_none(), "Disabled export must not run");
            click(&mut app, &ctx, Pos2::new(800.0, 500.0));
            assert!(!egui::Popup::is_any_open(&ctx));
            assert_eq!(app.session.project, project);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn application_menu_view_action_works_from_row_padding() {
    let ctx = egui::Context::default();
    crate::theme::configure(&ctx);
    crate::fonts::configure(&ctx);
    crate::icons::configure(&ctx);
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    let mut app = DawUi::default();
    let project = app.session.project.clone();
    let initial = frame(&mut app, &ctx, vec![]);
    let root = click(&mut app, &ctx, initial.button.center());
    let view = text_rect(&root, "View").center();
    click(&mut app, &ctx, view);
    let child = frame(&mut app, &ctx, vec![Event::PointerMoved(view)]);
    assert_eq!(
        child
            .shapes
            .iter()
            .filter(|shape| matches!(
                &shape.shape, egui::Shape::Path(path)
                    if path.points.len() == 3 && path.stroke.width > 0.0
            ))
            .count(),
        1,
        "Only the active view has a visible checkmark"
    );
    assert!(
        highlight(
            &child,
            ctx.global_style().visuals.selection.bg_fill,
            "Timeline"
        )
        .rect
        .width()
            >= menus::MIN_WIDTH
    );
    let mixer = text_rect(&child, "Mixer").center();
    frame(&mut app, &ctx, vec![Event::PointerMoved(mixer)]);
    let hovered = frame(&mut app, &ctx, vec![Event::PointerMoved(mixer)]);
    let row = highlight(&hovered, crate::theme::TOOLBAR_HOVER_FILL, "Mixer").rect;
    click(
        &mut app,
        &ctx,
        egui::pos2(row.right() - 2.0, row.center().y),
    );
    assert_eq!(app.view_mode, crate::ViewMode::Mixer);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert_eq!(app.session.project, project);
    assert!(!app.dirty);
}
