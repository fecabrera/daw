#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Apply(String),
    Cancel,
}

/// Draft project metadata. Apply commits it; cancellation discards it.
pub struct ProjectSettings {
    pub name: String,
    focus: bool,
}

pub fn valid_name(name: &str) -> Result<&str, &'static str> {
    let name = name.trim();
    if name.is_empty() {
        Err("Enter a project name.")
    } else if name.chars().any(char::is_control) {
        Err("Project names cannot contain control characters.")
    } else {
        Ok(name)
    }
}

impl ProjectSettings {
    pub fn new(name: String) -> Self {
        Self { name, focus: true }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Action> {
        let mut enter = false;
        let action = crate::dialogs::Dialog::new(
            "Project Settings",
            "",
            &[("Apply", true), ("Cancel", false)],
        )
        .show_with_content(ctx, |ui| {
            crate::rows::centered(ui, crate::toolbars::CONTROL_HEIGHT, |ui| {
                ui.label("Project name");
                let mut output = egui::TextEdit::singleline(&mut self.name)
                    .id_salt("project_settings_name")
                    .desired_width(220.0)
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
                    enter = ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    });
                }
            });
            if let Err(message) = valid_name(&self.name) {
                ui.colored_label(crate::theme::palette(ctx).error, message);
            }
        });
        if action == Some(false)
            || ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            return Some(Action::Cancel);
        }
        if action == Some(true) || enter {
            return valid_name(&self.name)
                .ok()
                .map(|name| Action::Apply(name.to_owned()));
        }
        None
    }
}
