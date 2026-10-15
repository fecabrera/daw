#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Apply(Values),
    Cancel,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Values {
    pub name: String,
    pub stretch_engine: daw_core::StretchEngine,
    pub sample_rate_hz: u32,
    pub buffer_size_frames: u32,
}

/// Draft project metadata. Apply commits it; cancellation discards it.
pub struct ProjectSettings {
    pub name: String,
    pub stretch_engine: daw_core::StretchEngine,
    pub sample_rate_hz: u32,
    pub buffer_size_frames: u32,
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
    pub fn new(project: &daw_core::Project) -> Self {
        Self {
            name: project.name.clone(),
            stretch_engine: project.stretch_engine,
            sample_rate_hz: project.sample_rate_hz,
            buffer_size_frames: project.buffer_size_frames,
            focus: true,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Action> {
        let mut enter = false;
        let action = crate::dialogs::Dialog::new(
            "Project Settings",
            "",
            &[("Apply", true), ("Cancel", false)],
        )
        .show_with_content(ctx, |ui| {
            let label_width = ui
                .painter()
                .layout_no_wrap(
                    "Time stretch engine".into(),
                    egui::TextStyle::Body.resolve(ui.style()),
                    crate::theme::palette(ctx).text,
                )
                .size()
                .x;
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Project name");
                },
                |ui| {
                    let mut output = egui::TextEdit::singleline(&mut self.name)
                        .id_salt("project_settings_name")
                        .desired_width(ui.available_width())
                        .show(ui);
                    let response = output.response;
                    if self.focus {
                        response.request_focus();
                        output.state.cursor.set_char_range(Some(
                            egui::text::CCursorRange::select_all(&output.galley),
                        ));
                        output.state.store(ctx, response.id);
                        self.focus = false;
                    }
                    if response.has_focus() || response.lost_focus() {
                        enter = ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        });
                    }
                },
            );
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Sample rate");
                },
                |ui| {
                    egui::ComboBox::from_id_salt("project_sample_rate")
                        .selected_text(format!("{} Hz", self.sample_rate_hz))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for rate in daw_core::SAMPLE_RATES {
                                ui.selectable_value(
                                    &mut self.sample_rate_hz,
                                    rate,
                                    format!("{rate} Hz"),
                                );
                            }
                        });
                },
            );
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Buffer size");
                },
                |ui| {
                    egui::ComboBox::from_id_salt("project_buffer_size")
                        .selected_text(format!("{} samples", self.buffer_size_frames))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for size in daw_core::BUFFER_SIZES {
                                ui.selectable_value(
                                    &mut self.buffer_size_frames,
                                    size,
                                    format!("{size} samples"),
                                );
                            }
                        });
                },
            );
            crate::rows::field(
                ui,
                label_width,
                |ui| {
                    ui.label("Time stretch engine");
                },
                |ui| {
                    egui::ComboBox::from_id_salt("project_stretch_engine")
                        .selected_text(self.stretch_engine.label())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for engine in [
                                daw_core::StretchEngine::RUBBER_BAND,
                                daw_core::StretchEngine::INTEGRATED,
                            ] {
                                ui.selectable_value(
                                    &mut self.stretch_engine,
                                    engine,
                                    engine.label(),
                                );
                            }
                        });
                },
            );
            ui.label(format!(
                "Buffer duration: {:.2} ms",
                self.buffer_size_frames as f64 * 1000.0 / self.sample_rate_hz as f64
            ));
            ui.label("Applying audio settings stops playback.");
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
            return valid_name(&self.name).ok().map(|name| {
                Action::Apply(Values {
                    name: name.to_owned(),
                    stretch_engine: self.stretch_engine,
                    sample_rate_hz: self.sample_rate_hz,
                    buffer_size_frames: self.buffer_size_frames,
                })
            });
        }
        None
    }
}
