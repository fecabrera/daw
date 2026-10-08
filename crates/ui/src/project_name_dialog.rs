#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Continue(String),
    Cancel,
}

pub struct ProjectNameDialog {
    pub name: String,
    pub save_as: bool,
    focus: bool,
}

impl ProjectNameDialog {
    pub fn new(name: String, save_as: bool) -> Self {
        Self {
            name,
            save_as,
            focus: true,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Action> {
        let mut enter = false;
        let action = crate::dialogs::Dialog::new(
            if self.save_as {
                "Save project as"
            } else {
                "Save project"
            },
            "Enter a name, then choose where to create the project folder.",
            &[("Continue…", true), ("Cancel", false)],
        )
        .show_with_content(ctx, |ui| {
            ui.label("Project name");
            let mut output = egui::TextEdit::singleline(&mut self.name)
                .id_salt("project_folder_name")
                .desired_width(300.0)
                .show(ui);
            let response = output.response;
            if self.focus {
                response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::select_all(&output.galley)));
                output.state.store(ctx, response.id);
                self.focus = false;
            }
            if response.has_focus() || response.lost_focus() {
                enter = ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            }
            if let Err(error) = daw_project::project_folder_name(&self.name) {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(error.to_string()).color(crate::theme::ERROR),
                    )
                    .wrap(),
                );
            }
        });
        if action == Some(false)
            || ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            return Some(Action::Cancel);
        }
        if action == Some(true) || enter {
            return daw_project::project_folder_name(&self.name)
                .ok()
                .map(|name| Action::Continue(name.to_owned()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        dialog: &mut ProjectNameDialog,
        events: Vec<egui::Event>,
    ) -> (Option<Action>, Vec<egui::epaint::ClippedShape>) {
        let mut action = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| action = dialog.show(ui.ctx()),
        );
        output.textures_delta.clear();
        (action, output.shapes)
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }
    fn click(ctx: &egui::Context, dialog: &mut ProjectNameDialog, label: &str) -> Option<Action> {
        let (_, shapes) = frame(ctx, dialog, vec![]);
        let point = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            ctx,
            dialog,
            vec![egui::Event::PointerMoved(point), button(true)],
        );
        frame(ctx, dialog, vec![button(false)]).0
    }
    #[test]
    fn name_dialog_selects_text_validates_and_handles_enter_buttons_and_escape() {
        for save_as in [false, true] {
            let ctx = egui::Context::default();
            crate::theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            let mut dialog = ProjectNameDialog::new("Untitled".into(), save_as);
            frame(&ctx, &mut dialog, vec![]);
            frame(&ctx, &mut dialog, vec![]);
            frame(
                &ctx,
                &mut dialog,
                vec![egui::Event::Text("Música 01".into())],
            );
            assert_eq!(dialog.name, "Música 01");
            assert_eq!(
                frame(&ctx, &mut dialog, vec![key(egui::Key::Enter)]).0,
                Some(Action::Continue("Música 01".into()))
            );
            for invalid in ["", "../escape", "CON"] {
                dialog.name = invalid.into();
                assert_eq!(click(&ctx, &mut dialog, "Continue…"), None);
                let (_, shapes) = frame(&ctx, &mut dialog, vec![]);
                let error = daw_project::project_folder_name(invalid)
                    .unwrap_err()
                    .to_string();
                assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == error)));
            }
            dialog.name = "  New mix  ".into();
            assert_eq!(
                click(&ctx, &mut dialog, "Continue…"),
                Some(Action::Continue("New mix".into()))
            );
            assert_eq!(click(&ctx, &mut dialog, "Cancel"), Some(Action::Cancel));
            assert_eq!(
                frame(&ctx, &mut dialog, vec![key(egui::Key::Escape)]).0,
                Some(Action::Cancel)
            );
        }
    }
}
