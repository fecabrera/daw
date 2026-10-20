//! Clip name, source, gain, and transpose for the selected clip.
use crate::{faders, fonts, panels, theme};
use daw_core::{Asset, Clip, Edit, Id};
use egui::{Rect, Response, Sense, Ui, Vec2};

const TRANSPOSE_LIMIT: i8 = 48;
/// Lowest finite level on the shared fader scale. Dragging up from silence starts here.
const AUDIBLE_GAIN_FLOOR_DB: f32 = -60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    Gain,
    Transpose,
}

struct Value<'a> {
    display: &'a str,
    editable: &'a str,
    tooltip: &'a str,
    numeric: bool,
    sense: Sense,
}

pub(super) struct FieldEdit {
    clip: Id,
    field: Field,
    text: String,
    focus: bool,
    width: f32,
}

pub(super) struct TransposeDrag {
    clip: Id,
    value: i8,
}

impl super::DawUi {
    pub(super) fn clip_settings_panel(
        &mut self,
        ui: &mut Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        enabled: bool,
    ) {
        let header = panel.header(ui, "Clip settings", |_| {});
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        let Some(clip) = self.selected_clip_record() else {
            self.clip_settings_draft = None;
            self.clip_transpose_drag = None;
            panel.padded_body(ui, |ui| {
                crate::rows::centered(ui, panels::ControlPanel::content_row_height(ui), |ui| {
                    ui.label(
                        egui::RichText::new("Select a clip")
                            .text_style(egui::TextStyle::Small)
                            .color(theme::palette(ui.ctx()).secondary),
                    )
                });
            });
            #[cfg(test)]
            {
                self.effects_bounds.clip_settings = bounds;
            }
            return;
        };
        let drag_enabled = enabled
            && ui.input(|input| input.focused)
            && !ui.input(|input| input.key_pressed(egui::Key::Escape));
        if !drag_enabled
            || self
                .clip_transpose_drag
                .as_ref()
                .is_some_and(|drag| drag.clip != clip.id)
        {
            self.clip_transpose_drag = None;
        }
        let audio_clip = self
            .session
            .project
            .assets
            .iter()
            .any(|asset| asset.id == clip.asset_id && asset.midi_pattern.is_none());
        if self
            .clip_settings_draft
            .as_ref()
            .is_some_and(|edit| edit.clip != clip.id)
        {
            self.clip_settings_draft = None;
        }
        let source = self
            .session
            .project
            .assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .map(source_label);
        let mut edit = self.clip_settings_draft.take();
        let was_editing = edit.is_some();
        let mut commands = Vec::new();
        let mut error = None;
        let (name, source_row, gain, transpose) = panel
            .padded_body(ui, |ui| {
                let label_width = ui
                    .painter()
                    .layout_no_wrap(
                        "Transpose".into(),
                        egui::TextStyle::Small.resolve(ui.style()),
                        theme::palette(ui.ctx()).text,
                    )
                    .size()
                    .x;
                ui.add_enabled_ui(enabled, |ui| {
                    let name = panel.field_row(
                        ui,
                        label_width,
                        |ui| {
                            field_label(ui, "Name");
                        },
                        |ui| {
                            self.setting_value(
                                ui,
                                &clip,
                                &mut edit,
                                Field::Name,
                                Value {
                                    display: &clip.name,
                                    editable: &clip.name,
                                    tooltip: "Clip name. Double-click to edit.",
                                    numeric: false,
                                    sense: Sense::click(),
                                },
                            )
                        },
                    )
                    .inner;
                    let source_row = panel.field_row(
                        ui,
                        label_width,
                        |ui| {
                            field_label(ui, "Source");
                        },
                        |ui| {
                            let (text, tip) = source.clone().unwrap_or_else(|| {
                                ("Missing source".into(), "Missing source".into())
                            });
                            value_label(ui, &text, &tip, false, true, Sense::hover())
                        },
                    )
                    .inner;
                    let gain = panel.field_row(
                        ui,
                        label_width,
                        |ui| {
                            field_label(ui, "Gain");
                        },
                        |ui| {
                            let gain_text = format_gain_value(clip.gain_db);
                            let gain_edit = faders::format_gain(clip.gain_db);
                            let response = self.setting_value(
                                ui,
                                &clip,
                                &mut edit,
                                Field::Gain,
                                Value {
                                    display: &gain_text,
                                    editable: &gain_edit,
                                    tooltip: "Clip gain (dB): drag up or down in 0.1 dB steps; hold Shift for finer adjustment. Double-click to edit.",
                                    numeric: true,
                                    sense: Sense::click_and_drag(),
                                },
                            );
                            if let Some(value) = drag_gain(ui, &response, clip.gain_db) {
                                commands.push(Edit::SetClipGain {
                                    clip_id: clip.id,
                                    gain_db: value,
                                });
                            }
                            response
                        },
                    )
                    .inner;
                    let transpose = panel.field_row(
                        ui,
                        label_width,
                        |ui| {
                            field_label(ui, "Transpose");
                        },
                        |ui| {
                            let current = self.clip_transpose_drag.as_ref().map_or(clip.transpose, |drag| drag.value);
                            let transpose_text = format_transpose_value(current);
                            let transpose_edit = format_transpose(clip.transpose);
                            let response = self.setting_value(
                                ui,
                                &clip,
                                &mut edit,
                                Field::Transpose,
                                Value {
                                    display: &transpose_text,
                                    editable: &transpose_edit,
                                    tooltip: "Transpose (semitones): drag up or down in 1 semitone steps; hold Shift for finer adjustment. Double-click to edit.",
                                    numeric: true,
                                    sense: Sense::click_and_drag(),
                                },
                            );
                            if audio_clip && drag_enabled && response.drag_started() {
                                self.clip_transpose_drag = Some(TransposeDrag { clip: clip.id, value: clip.transpose });
                            }
                            if let Some(value) = drag_transpose(ui, &response, current) {
                                if audio_clip {
                                    if let Some(drag) = &mut self.clip_transpose_drag { drag.value = value; }
                                } else {
                                commands.push(Edit::SetClipTranspose {
                                    clip_id: clip.id,
                                    transpose: value,
                                });
                                }
                            }
                            if audio_clip && response.drag_stopped()
                                && let Some(drag) = self.clip_transpose_drag.take()
                                && drag.value != clip.transpose
                            {
                                commands.push(Edit::SetClipTranspose { clip_id: clip.id, transpose: drag.value });
                            }
                            response
                        },
                    )
                    .inner;
                    let finished = if was_editing {
                        let response = match edit.as_ref().map(|current| current.field) {
                            Some(Field::Name) => Some(&name),
                            Some(Field::Gain) => Some(&gain),
                            Some(Field::Transpose) => Some(&transpose),
                            None => None,
                        };
                        response.and_then(|response| super::inline_edit_action(ui, response))
                    } else {
                        None
                    };
                    if let Some(action) = finished
                        && let Some(current) = edit.take()
                        && action == super::InlineEditAction::Commit
                    {
                        match current.field {
                            Field::Name => match clip_name(&current.text) {
                                Ok(value) if value != clip.name => {
                                    commands.push(Edit::SetClipName {
                                        clip_id: clip.id,
                                        name: value.to_owned(),
                                    });
                                }
                                Ok(_) => {}
                                Err(message) => error = Some(message),
                            },
                            Field::Gain => match faders::parse_gain(&current.text) {
                                Some(value) => {
                                    let value = faders::clamp_gain(value);
                                    if value != clip.gain_db {
                                        commands.push(Edit::SetClipGain {
                                            clip_id: clip.id,
                                            gain_db: value,
                                        });
                                    }
                                }
                                None => error = Some("Gain must be a finite number or −∞"),
                            },
                            Field::Transpose => match parse_transpose(&current.text) {
                                Some(value) if value != clip.transpose => {
                                    commands.push(Edit::SetClipTranspose {
                                        clip_id: clip.id,
                                        transpose: value,
                                    });
                                }
                                Some(_) => {}
                                None => {
                                    error = Some(
                                        "Transpose must be a whole number from −48 to +48",
                                    );
                                }
                            },
                        }
                    }
                    (name, source_row, gain, transpose)
                })
                .inner
            })
            .inner;
        self.clip_settings_draft = edit;
        if let Some(message) = error {
            self.fail(message);
        }
        for command in commands {
            self.edit(command);
        }
        #[cfg(test)]
        {
            self.effects_bounds.clip_settings = bounds;
            self.effects_bounds.clip_name = name.rect;
            self.effects_bounds.clip_source = source_row.rect;
            self.effects_bounds.clip_gain = gain.rect;
            self.effects_bounds.clip_transpose = transpose.rect;
        }
        #[cfg(not(test))]
        {
            let _ = (bounds, name, source_row, gain, transpose);
        }
    }

    fn setting_value(
        &mut self,
        ui: &mut Ui,
        clip: &Clip,
        edit: &mut Option<FieldEdit>,
        field: Field,
        value: Value<'_>,
    ) -> Response {
        if edit.as_ref().is_some_and(|edit| edit.field == field) {
            let edit = edit.as_mut().unwrap();
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(ui.make_persistent_id(("clip-setting", clip.id, field_salt(field))))
                .font(value_font(ui, value.numeric))
                .text_color(theme::palette(ui.ctx()).text)
                .horizontal_align(egui::Align::LEFT)
                .desired_width(edit.width)
                .min_size(Vec2::new(edit.width, ui.spacing().interact_size.y))
                .show(ui);
            output.response.widget_info(|| {
                egui::WidgetInfo::text_edit(
                    ui.is_enabled(),
                    value.display,
                    &edit.text,
                    value.tooltip,
                )
            });
            if edit.focus && ui.is_enabled() {
                output.response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(edit.text.chars().count()),
                    )));
                output.state.store(ui.ctx(), output.response.id);
                edit.focus = false;
            }
            output.response.response
        } else {
            let response = value_label(
                ui,
                value.display,
                value.tooltip,
                value.numeric,
                false,
                value.sense,
            );
            if opened_by_double_click(ui, &response) {
                *edit = Some(FieldEdit {
                    clip: clip.id,
                    field,
                    text: value.editable.to_owned(),
                    focus: true,
                    width: response.rect.width(),
                });
            }
            response
        }
    }

    fn selected_clip_record(&self) -> Option<Clip> {
        let id = self.selected_clip?;
        self.session
            .project
            .tracks
            .iter()
            .flat_map(|track| &track.clips)
            .find(|clip| clip.id == id)
            .cloned()
    }
}

fn opened_by_double_click(ui: &Ui, response: &Response) -> bool {
    if !ui.is_enabled() || !response.clicked() {
        return false;
    }
    let now = ui.input(|input| input.time);
    let pos = response
        .interact_pointer_pos()
        .unwrap_or_else(|| response.rect.center());
    let id = response.id.with("last-click");
    let previous = ui.data(|data| data.get_temp::<(f64, egui::Pos2)>(id));
    let (delay, distance) = ui.ctx().options(|options| {
        (
            options.input_options.max_double_click_delay,
            options.input_options.max_click_dist,
        )
    });
    let open =
        previous.is_some_and(|(then, at)| now - then < delay && at.distance(pos) <= distance);
    if open {
        ui.data_mut(|data| data.remove::<(f64, egui::Pos2)>(id));
    } else {
        ui.data_mut(|data| data.insert_temp(id, (now, pos)));
    }
    open
}

fn field_label(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).text_style(egui::TextStyle::Small));
}

fn value_label(
    ui: &mut Ui,
    text: &str,
    tooltip: &str,
    numeric: bool,
    muted: bool,
    sense: Sense,
) -> Response {
    let response = ui
        .add(
            egui::Label::new(value_text(ui, text, numeric, muted))
                .selectable(false)
                .truncate()
                .halign(egui::Align::LEFT)
                .sense(sense)
                .show_tooltip_when_elided(false),
        )
        .on_hover_text(tooltip);
    if sense.senses_drag() {
        response.on_hover_cursor(egui::CursorIcon::ResizeVertical)
    } else {
        response
    }
}

fn value_text(ui: &Ui, text: &str, numeric: bool, muted: bool) -> egui::RichText {
    let palette = theme::palette(ui.ctx());
    let color = if muted || !ui.is_enabled() {
        palette.secondary
    } else {
        palette.text
    };
    let rich = egui::RichText::new(text).color(color);
    if numeric {
        rich.font(fonts::numeric(
            egui::TextStyle::Small.resolve(ui.style()).size,
        ))
    } else {
        rich.text_style(egui::TextStyle::Small)
    }
}

fn value_font(ui: &Ui, numeric: bool) -> egui::FontId {
    let size = egui::TextStyle::Small.resolve(ui.style()).size;
    if numeric {
        fonts::numeric(size)
    } else {
        egui::FontId::new(size, egui::FontFamily::Proportional)
    }
}

fn field_salt(field: Field) -> &'static str {
    match field {
        Field::Name => "name",
        Field::Gain => "gain",
        Field::Transpose => "transpose",
    }
}

fn drag_gain(ui: &Ui, response: &Response, current: f32) -> Option<f32> {
    if !response.dragged_by(egui::PointerButton::Primary)
        || response.double_clicked()
        || !ui.is_enabled()
        || !ui.input(|input| input.focused)
    {
        return None;
    }
    let delta = response.drag_delta().y;
    if delta == 0.0 {
        return None;
    }
    let step = ui.input(|input| if input.modifiers.shift { 0.01 } else { 0.1 });
    let mut origin = f64::from(current);
    if daw_core::linear_gain(current) == 0.0 && delta < 0.0 {
        origin = f64::from(AUDIBLE_GAIN_FLOOR_DB);
    }
    let adjusted = ((origin - f64::from(delta) * step) * 100.0).round() / 100.0;
    let value = faders::clamp_gain(adjusted as f32);
    (value != current).then_some(value)
}

fn drag_transpose(ui: &Ui, response: &Response, current: i8) -> Option<i8> {
    let id = response.id.with("remainder");
    let active = response.dragged_by(egui::PointerButton::Primary)
        && !response.double_clicked()
        && ui.is_enabled()
        && ui.input(|input| input.focused);
    if !active {
        if !ui.input(|input| input.pointer.primary_down()) {
            ui.data_mut(|data| data.remove::<f64>(id));
        }
        return None;
    }
    let delta = response.drag_delta().y;
    let mut remainder = ui.data(|data| data.get_temp::<f64>(id)).unwrap_or(0.0);
    if delta != 0.0 {
        let step = ui.input(|input| if input.modifiers.shift { 0.1 } else { 1.0 });
        remainder -= f64::from(delta) * step;
    }
    let steps = remainder.trunc() as i32;
    remainder -= f64::from(steps);
    let unclamped = i32::from(current).saturating_add(steps);
    let value = unclamped.clamp(-i32::from(TRANSPOSE_LIMIT), i32::from(TRANSPOSE_LIMIT));
    if value != unclamped {
        remainder = 0.0;
    }
    ui.data_mut(|data| data.insert_temp(id, remainder));
    (value != i32::from(current)).then_some(value as i8)
}

fn source_label(asset: &Asset) -> (String, String) {
    if asset.midi_pattern.is_some() {
        (
            "MIDI pattern".into(),
            "This clip uses a MIDI pattern".into(),
        )
    } else {
        let path = asset.source.path.clone();
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| asset.name.clone());
        let tip = if path.is_empty() { name.clone() } else { path };
        (name, tip)
    }
}

fn clip_name(name: &str) -> Result<&str, &'static str> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        Err("Clip name must contain visible text")
    } else {
        Ok(name)
    }
}

fn format_gain_value(db: f32) -> String {
    format!("{} dB", faders::format_gain(db))
}

fn format_transpose_value(value: i8) -> String {
    format!("{} semitones", format_transpose(value))
}

fn format_transpose(value: i8) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn parse_transpose(text: &str) -> Option<i8> {
    let text = text.trim().replace('−', "-");
    let text = text.strip_prefix('+').unwrap_or(&text);
    text.parse::<i8>()
        .ok()
        .filter(|value| (-TRANSPOSE_LIMIT..=TRANSPOSE_LIMIT).contains(value))
}
