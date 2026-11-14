//! Explicit, read-only diagnostic reports. Opening a report never edits the session.
use super::*;

pub(super) enum Report {
    Memory,
    Transients(String),
}
/// Visible captions stay compact; accessibility identifies each report's purpose.
pub(super) fn button(ui: &mut egui::Ui, enabled: bool, action: &str) -> egui::Response {
    let response = ui.add_enabled(enabled, egui::Button::new("Details"));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && enabled, action)
    });
    response.on_hover_text(action)
}

impl DawUi {
    pub(super) fn show_diagnostic_details(&mut self, ctx: &egui::Context) -> bool {
        let Some(report) = self.diagnostic_details.take() else {
            return false;
        };
        let title = match &report {
            Report::Memory => "Memory details",
            Report::Transients(_) => "Transient details",
        };
        let close = dialogs::Dialog::new(title, "", &[("Close", ())])
            .show_with_content(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| match &report {
                        Report::Memory => self.memory_details(ui),
                        Report::Transients(message) => {
                            ui.add(egui::Label::new(message).wrap());
                        }
                    });
            })
            .is_some()
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if !close {
            self.diagnostic_details = Some(report);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        app: &mut DawUi,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        out.textures_delta.clear();
        out.shapes
    }
    fn point(shapes: &[egui::epaint::ClippedShape], name: &str) -> Pos2 {
        shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == name => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap()
    }
    fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn memory_details_cannot_open_during_another_dialog_or_without_window_focus() {
        let ctx = egui::Context::default();
        fonts::configure(&ctx);
        theme::configure(&ctx);
        let mut app = DawUi::default();
        let shapes = frame(&mut app, &ctx, vec![]);
        let button = point(&shapes, "Details");
        app.error = Some("Existing error".into());
        click(&mut app, &ctx, button);
        assert!(app.diagnostic_details.is_none());
        assert_eq!(app.error.as_deref(), Some("Existing error"));
        app.error = None;
        for pressed in [true, false] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                    focused: false,
                    events: vec![
                        egui::Event::PointerMoved(button),
                        egui::Event::PointerButton {
                            pos: button,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            out.textures_delta.clear();
        }
        assert!(app.diagnostic_details.is_none());
        assert!(!app.dirty);
    }
    #[test]
    fn details_actions_retain_full_reports_without_editing_and_close_with_escape_or_button() {
        for escape in [false, true] {
            let ctx = egui::Context::default();
            fonts::configure(&ctx);
            theme::configure(&ctx);
            ctx.all_styles_mut(|s| s.animation_time = 0.0);
            let mut app = DawUi::default();
            let before = app.session.project.clone();
            let shapes = frame(&mut app, &ctx, vec![]);
            click(&mut app, &ctx, point(&shapes, "Details"));
            assert!(matches!(app.diagnostic_details, Some(Report::Memory)));
            assert!(!app.editing_enabled());
            let shapes = frame(&mut app, &ctx, vec![]);
            assert!(shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("Estimates exclude allocator"))));
            if escape {
                frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }],
                );
            } else {
                click(&mut app, &ctx, point(&shapes, "Close"));
            }
            assert!(app.diagnostic_details.is_none());
            assert!(app.editing_enabled());
            let error =
                "Cannot analyze source\nOriginal worker diagnostic: failed to decode block 17";
            app.diagnostic_details = Some(Report::Transients(error.into()));
            frame(&mut app, &ctx, vec![]);
            let shapes = frame(&mut app, &ctx, vec![]);
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == error))
            );
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            assert!(app.job.is_none());
        }
    }
}
