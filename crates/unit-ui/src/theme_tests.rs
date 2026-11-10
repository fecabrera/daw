use super::*;
use egui::{Event, Pos2, Rect, Vec2};

#[test]
fn framed_controls_and_text_inputs_keep_geometry_without_interaction_outlines() {
    for colors in [DARK, LIGHT, MIST] {
        let ctx = Context::default();
        configure(&ctx);
        apply(
            &ctx,
            if colors == DARK {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            },
            colors,
            ACCENT,
        );
        let mut text = "Input".to_owned();
        let mut draw = |events, enabled| {
            let mut bounds = [Rect::NOTHING; 4];
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 240.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        bounds[0] = ui.button("Button").rect;
                        bounds[1] = crate::menus::Dropdown::from_id_salt("choice")
                            .selected_text("Choice")
                            .show_ui(ui, |ui| {
                                let _ = ui.button("Item");
                            })
                            .response
                            .rect;
                        bounds[2] = ui
                            .add(egui::TextEdit::singleline(&mut text).desired_width(100.0))
                            .rect;
                        bounds[3] = ui.add(egui::Button::new("Borderless").frame(false)).rect;
                    });
                },
            );
            output.textures_delta.clear();
            for rect in &bounds[..3] {
                let surface = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(surface) if surface.rect == *rect => Some(surface),
                        _ => None,
                    })
                    .expect("framed control surface");
                assert!(
                    surface.stroke.is_empty(),
                    "no idle, hover, press or focus outline"
                );
                assert_eq!(surface.corner_radius, CornerRadius::same(5));
            }
            assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Rect(surface) if surface.rect == bounds[3] && surface.fill != Color32::TRANSPARENT
            )), "an explicitly borderless button retains its transparent surface");
            bounds
        };
        let reference = draw(vec![], true);
        for enabled in [true, false] {
            for target in &reference[..3] {
                let point = target.center();
                let pointer = |pressed| Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                };
                for events in [
                    vec![Event::PointerMoved(point)],
                    vec![],
                    vec![pointer(true)],
                    vec![],
                    vec![pointer(false)],
                    vec![],
                ] {
                    let current = draw(events, enabled);
                    assert_eq!(
                        current.map(|rect| rect.size()),
                        reference.map(|rect| rect.size())
                    );
                }
                draw(
                    vec![Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                    enabled,
                );
            }
        }
        assert_eq!(
            ctx.global_style()
                .visuals
                .widgets
                .noninteractive
                .bg_stroke
                .width,
            1.0,
            "workspace separators keep their original border geometry"
        );
        assert_eq!(ctx.global_style().visuals.text_cursor.stroke.width, 1.0);
    }
}
