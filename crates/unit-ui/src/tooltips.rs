//! Concise hover presentation. Accessible identities and actions stay with widgets.
/// A formatted current value, with its parameter name when context needs it.
pub fn parameter(name: &str, value: impl std::fmt::Display) -> String {
    if name.is_empty() {
        value.to_string()
    } else {
        format!("{name}: {value}")
    }
}
/// An action and its already platform-formatted, registered shortcut.
pub fn action(name: &str, shortcut: Option<&str>) -> String {
    match shortcut.filter(|s| !s.is_empty()) {
        Some(shortcut) => format!("{name} ({shortcut})"),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contextual_hover_keeps_accessible_parameter_identity_and_live_keyboard_value() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        crate::theme::configure(&ctx);
        ctx.all_styles_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.tooltip_grace_time = 0.0;
        });
        let mut value = -3.0;
        let mut rect = egui::Rect::NOTHING;
        let mut frame = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = ui
                        .add(
                            crate::knobs::Knob::new(
                                &mut value,
                                crate::knobs::KnobSpec::new("Gain", -12.0..=12.0, 0.0, 0.1)
                                    .format(|v| format!("{v:.1} dB")),
                            )
                            .tooltip_name("Send to Room"),
                        )
                        .rect;
                },
            );
            out.textures_delta.clear();
            (rect, out.shapes, out.platform_output.events)
        };
        let point = frame(vec![]).0.center();
        for _ in 0..3 {
            frame(vec![egui::Event::PointerMoved(point)]);
        }
        let (_, shapes, _) = frame(vec![]);
        assert!(shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "Send to Room:")
        ));
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "-3.0 dB"))
        );
        for pressed in [true, false] {
            frame(vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        let (_, _, events) = frame(vec![egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert!(events.iter().any(|e| matches!(e,egui::output::OutputEvent::ValueChanged(info) if info.label.as_deref()==Some("Gain"))));
        assert!(value > -3.0);
        assert_eq!(action("Play", Some("Space")), "Play (Space)");
        assert_eq!(parameter("", "−∞ dB"), "−∞ dB");
    }
}
