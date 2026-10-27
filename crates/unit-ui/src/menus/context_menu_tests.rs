use super::*;
use egui::{Event, Pos2, Rect};

const LABELS: [&str; 3] = [
    "Current choice",
    "A long context action that must remain on one line",
    "Disabled action",
];

struct Frame {
    trigger: Rect,
    rows: [Rect; 3],
    divider: Rect,
    opened: bool,
    shapes: Vec<egui::epaint::ClippedShape>,
}

fn frame(ctx: &egui::Context, events: Vec<Event>, guard: Option<bool>, chosen: &mut bool) -> Frame {
    let mut trigger = Rect::NOTHING;
    let mut rows = [Rect::NOTHING; 3];
    let mut divider = Rect::NOTHING;
    let mut opened = false;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.style_mut().override_text_style = Some(TextStyle::Small);
            let response = ui.button("Open context menu");
            trigger = response.rect;
            let mut menu = ContextMenu::new(&response);
            if let Some(allowed) = guard {
                menu = menu.open_memory(allowed.then_some(egui::SetOpenCommand::Bool(true)));
            }
            menu.show(|ui| {
                rows[0] = ui.selectable_label(true, LABELS[0]).rect;
                let action = ui.add(action_button(LABELS[1]).shortcut_text("Ctrl+G"));
                rows[1] = action.rect;
                if action.clicked() {
                    *chosen = true;
                    ui.close();
                }
                divider = separator(ui).rect;
                let disabled = ui.add_enabled(false, egui::Button::new(LABELS[2]));
                rows[2] = disabled.rect;
                assert!(!disabled.clicked());
            });
            opened = response.context_menu_opened();
        },
    );
    output.textures_delta.clear();
    Frame {
        trigger,
        rows,
        divider,
        opened,
        shapes: output.shapes,
    }
}

fn pointer(pos: Pos2, button: egui::PointerButton, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        },
    ]
}

fn open(ctx: &egui::Context, guard: Option<bool>, chosen: &mut bool) -> Frame {
    let start = frame(ctx, vec![], guard, chosen);
    for pressed in [true, false] {
        frame(
            ctx,
            pointer(
                start.trigger.center(),
                egui::PointerButton::Secondary,
                pressed,
            ),
            guard,
            chosen,
        );
    }
    frame(ctx, vec![], guard, chosen)
}

#[test]
fn context_menu_uses_shared_rows_and_preserves_identity_dismissal_and_release_actions() {
    for (theme, palette) in [
        (egui::Theme::Dark, crate::theme::LIGHT),
        (egui::Theme::Light, crate::theme::LIGHT),
        (egui::Theme::Light, crate::theme::MIST),
    ] {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::theme::configure(&ctx);
        crate::theme::apply(&ctx, theme, palette, crate::theme::PLUGIN_ACCENT);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let mut chosen = false;
        let opened = open(&ctx, None, &mut chosen);
        assert!(
            opened.opened,
            "Existing response popup identity must remain open"
        );
        assert!(!chosen);
        for (index, label) in LABELS.iter().enumerate() {
            let text = opened
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == *label => Some(text),
                    _ => None,
                })
                .expect("context action text");
            assert_eq!(text.galley.rows.len(), 1);
            assert!(
                text.galley
                    .job
                    .sections
                    .iter()
                    .all(|s| s.format.font_id.size == 13.0)
            );
            let row = opened.rows[index];
            assert!(row.width() > MIN_WIDTH, "Long labels must expand the menu");
            assert!((row.width() - opened.rows[0].width()).abs() < 0.1);
            assert!((text.pos.x - row.left() - f32::from(ITEM_PADDING)).abs() < 0.1);
        }
        assert_eq!(opened.divider.height(), 1.0);
        let text_bounds = |label: &str| {
            opened
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(Rect::from_min_size(text.pos, text.galley.size()))
                    }
                    _ => None,
                })
                .expect("Action label or shortcut")
        };
        assert!(text_bounds("Ctrl+G").left() - text_bounds(LABELS[1]).right() >= ITEM_SPACING);
        assert!((opened.divider.top() - opened.rows[1].bottom() - ITEM_SPACING).abs() < 0.1);
        assert!((opened.rows[2].top() - opened.divider.bottom() - ITEM_SPACING).abs() < 0.1);
        let point = egui::pos2(opened.rows[1].right() - 1.0, opened.rows[1].center().y);
        frame(&ctx, vec![Event::PointerMoved(point)], None, &mut chosen);
        let hovered = frame(&ctx, vec![Event::PointerMoved(point)], None, &mut chosen);
        assert_eq!(
            hovered.rows, opened.rows,
            "Hovering row padding must retain geometry"
        );
        assert!(
            hovered.shapes.iter().any(|shape| matches!(
                &shape.shape, egui::Shape::Rect(rect)
                    if rect.rect == hovered.rows[1]
                        && rect.fill == ctx.global_style().visuals.selection.bg_fill
                        && rect.stroke.width == 0.0 && rect.corner_radius.nw > 0
            )),
            "Padding must receive the shared full-row hover highlight"
        );
        frame(
            &ctx,
            pointer(point, egui::PointerButton::Primary, true),
            None,
            &mut chosen,
        );
        assert!(!chosen, "Actions must wait for release");
        frame(
            &ctx,
            pointer(point, egui::PointerButton::Primary, false),
            None,
            &mut chosen,
        );
        assert!(chosen);
        assert!(!egui::Popup::is_any_open(&ctx));
        let reopened = open(&ctx, None, &mut chosen);
        assert!(reopened.opened);
        frame(
            &ctx,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            None,
            &mut chosen,
        );
        assert!(!egui::Popup::is_any_open(&ctx));
    }
}

#[test]
fn context_menu_retains_compound_control_opening_guards() {
    let ctx = egui::Context::default();
    crate::fonts::configure(&ctx);
    crate::theme::configure(&ctx);
    let mut chosen = false;
    assert!(!open(&ctx, Some(false), &mut chosen).opened);
    assert!(open(&ctx, Some(true), &mut chosen).opened);
    assert!(!chosen);
}
