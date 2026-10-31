//! Shared clip settings for the current clip selection.
use crate::{faders, fonts, panels, theme};
use daw_core::{Asset, Clip, Edit, Id};
use egui::{Rect, Response, Sense, Ui, Vec2};

const TRANSPOSE_LIMIT: i8 = 48;
const MULTIPLE_VALUES: &str = "Multiple values";
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
    clips: Vec<Id>,
    field: Field,
    text: String,
    focus: bool,
    width: f32,
}

pub(super) struct TransposeDrag {
    clip: Id,
    clips: Vec<Id>,
    value: i8,
    changed: bool,
}

pub(super) struct DragChange {
    clip: Id,
    clips: Vec<Id>,
    field: Field,
    delta: f64,
}

impl DragChange {
    fn add(change: &mut Option<Self>, clip: Id, clips: &[Id], field: Field, delta: f64) {
        if change.as_ref().is_none_or(|change| {
            change.clip != clip || change.clips != clips || change.field != field
        }) {
            *change = Some(Self {
                clip,
                clips: clips.to_vec(),
                field,
                delta: 0.0,
            });
        }
        change.as_mut().unwrap().delta += delta;
    }

    fn label(&self, field: Field) -> Option<String> {
        (self.field == field).then(|| {
            match field {
                Field::Gain => format!(
                    "{:+.2} dB",
                    if self.delta.abs() < 0.005 {
                        0.0
                    } else {
                        self.delta
                    }
                ),
                Field::Transpose => format!(
                    "{:+} {}",
                    self.delta as i16,
                    if self.delta.abs() == 1.0 {
                        "semitone"
                    } else {
                        "semitones"
                    }
                ),
                Field::Name => unreachable!(),
            }
            .replace('-', "−")
        })
    }
}

impl super::DawUi {
    pub(super) fn clear_clip_settings_edits(&mut self) {
        self.clip_settings_draft = None;
        self.clip_transpose_drag = None;
        self.clip_settings_change = None;
        #[cfg(test)]
        {
            self.effects_bounds.clip_settings = Rect::NOTHING;
            self.effects_bounds.clip_name = Rect::NOTHING;
            self.effects_bounds.clip_source = Rect::NOTHING;
            self.effects_bounds.clip_gain = Rect::NOTHING;
            self.effects_bounds.clip_transpose = Rect::NOTHING;
        }
    }

    pub(super) fn clip_settings_panel(
        &mut self,
        ui: &mut Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        enabled: bool,
    ) {
        let header = panel.header(ui, "Clip settings", |_| {});
        panels::ControlPanel::row_dividers(ui, header.response.rect, false, true);
        egui::ScrollArea::vertical()
            .id_salt("timeline-clip-settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.clip_settings_body(ui, panel, bounds, enabled));
    }

    /// Timeline and Editor share fields, selection rules, and release-only audio preparation.
    pub(super) fn clip_settings_body(
        &mut self,
        ui: &mut Ui,
        panel: &panels::ControlPanel,
        bounds: Rect,
        enabled: bool,
    ) {
        #[cfg(test)]
        {
            self.effects_bounds.clip_name = Rect::NOTHING;
            self.effects_bounds.clip_source = Rect::NOTHING;
            self.effects_bounds.clip_gain = Rect::NOTHING;
            self.effects_bounds.clip_transpose = Rect::NOTHING;
        }
        let clips: Vec<_> = self
            .selected_clip_snapshots()
            .into_iter()
            .map(|entry| entry.clip)
            .collect();
        let Some(clip) = clips
            .iter()
            .find(|clip| Some(clip.id) == self.selected_clip)
            .or_else(|| clips.first())
        else {
            self.clear_clip_settings_edits();
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
        let ids: Vec<_> = clips.iter().map(|clip| clip.id).collect();
        let multiple = clips.len() > 1;
        let mixed_gain = clips.iter().any(|other| other.gain_db != clip.gain_db);
        let mixed_transpose = clips.iter().any(|other| other.transpose != clip.transpose);
        // Silence has no finite dB offset. Keep silent members silent when an
        // audible member supplies the group's drag reference.
        let audible_gain = clips
            .iter()
            .any(|clip| daw_core::linear_gain(clip.gain_db) != 0.0);
        let gain_reference = if daw_core::linear_gain(clip.gain_db) != 0.0 {
            clip.gain_db
        } else {
            clips
                .iter()
                .find(|clip| daw_core::linear_gain(clip.gain_db) != 0.0)
                .map_or(clip.gain_db, |clip| clip.gain_db)
        };
        let drag_enabled = enabled
            && ui.input(|input| input.focused)
            && !ui.input(|input| input.key_pressed(egui::Key::Escape));
        if !drag_enabled
            || self
                .clip_settings_change
                .as_ref()
                .is_some_and(|change| change.clip != clip.id || change.clips != ids)
        {
            self.clip_settings_change = None;
        }
        if !drag_enabled
            || self
                .clip_transpose_drag
                .as_ref()
                .is_some_and(|drag| drag.clip != clip.id || drag.clips != ids)
        {
            self.clip_transpose_drag = None;
        }
        let contains_audio = self.session.project.assets.iter().any(|asset| {
            asset.midi_pattern.is_none() && clips.iter().any(|clip| clip.asset_id == asset.id)
        });
        let transpose_enabled = !contains_audio || self.media_ready();
        if !transpose_enabled {
            self.clip_transpose_drag = None;
            if self
                .clip_settings_change
                .as_ref()
                .is_some_and(|change| change.field == Field::Transpose)
            {
                self.clip_settings_change = None;
            }
            if self
                .clip_settings_draft
                .as_ref()
                .is_some_and(|edit| edit.field == Field::Transpose)
            {
                self.clip_settings_draft = None;
            }
        }
        if self
            .clip_settings_draft
            .as_ref()
            .is_some_and(|edit| edit.clip != clip.id || edit.clips != ids)
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
                            ui.add_enabled_ui(!multiple, |ui| field_label(ui, "Name"));
                        },
                        |ui| {
                            if multiple {
                                return multiple_clips_value(ui);
                            }
                            self.setting_value(
                                ui,
                                clip,
                                &ids,
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
                            ui.add_enabled_ui(!multiple, |ui| field_label(ui, "Source"));
                        },
                        |ui| {
                            if multiple {
                                return multiple_clips_value(ui);
                            }
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
                            let change = self.clip_settings_change.as_ref().filter(|_| multiple)
                                .and_then(|change| change.label(Field::Gain));
                            let changing = change.is_some();
                            let gain_text = change.unwrap_or_else(|| {
                                if mixed_gain { MULTIPLE_VALUES.into() }
                                else { format_gain_value(clip.gain_db) }
                            });
                            let gain_edit = faders::format_gain(clip.gain_db);
                            let response = self.setting_value(
                                ui,
                                clip,
                                &ids,
                                &mut edit,
                                Field::Gain,
                                Value {
                                    display: &gain_text,
                                    editable: &gain_edit,
                                    tooltip: "Clip gain (dB): drag up or down in 0.1 dB steps; hold Shift for finer adjustment. Double-click to edit.",
                                    numeric: !mixed_gain || changing,
                                    sense: Sense::click_and_drag(),
                                },
                            );
                            if let Some(value) = drag_gain(ui, &response, gain_reference) {
                                let delta = bounded_delta(
                                    clips.iter()
                                        .filter(|clip| !audible_gain || daw_core::linear_gain(clip.gain_db) != 0.0)
                                        .map(|clip| f64::from(clip.gain_db)),
                                    f64::from(value) - f64::from(gain_reference),
                                    f64::from(daw_core::SILENT_GAIN_DB),
                                    f64::from(faders::MAX_GAIN_DB),
                                );
                                DragChange::add(&mut self.clip_settings_change, clip.id, &ids, Field::Gain, delta);
                                ui.ctx().request_repaint();
                                commands.extend(gain_edits(&clips, |current| {
                                    if audible_gain && daw_core::linear_gain(current) == 0.0 {
                                        current
                                    } else {
                                        faders::clamp_gain((f64::from(current) + delta) as f32)
                                    }
                                }));
                            }
                            response
                        },
                    )
                    .inner;
                    let transpose = ui.add_enabled_ui(transpose_enabled, |ui| panel.field_row(
                        ui,
                        label_width,
                        |ui| {
                            field_label(ui, "Transpose");
                        },
                        |ui| {
                            let current = self.clip_transpose_drag.as_ref()
                                .map_or(clip.transpose, |drag| drag.value);
                            let change = self.clip_settings_change.as_ref().filter(|_| multiple)
                                .and_then(|change| change.label(Field::Transpose));
                            let changing = change.is_some();
                            let transpose_text = change.unwrap_or_else(|| {
                                if mixed_transpose { MULTIPLE_VALUES.into() }
                                else { format_transpose_value(current) }
                            });
                            let transpose_edit = format_transpose(clip.transpose);
                            let response = self.setting_value(
                                ui,
                                clip,
                                &ids,
                                &mut edit,
                                Field::Transpose,
                                Value {
                                    display: &transpose_text,
                                    editable: &transpose_edit,
                                    tooltip: "Transpose (semitones): drag up or down in 1 semitone steps; hold Shift for finer adjustment. Double-click to edit.",
                                    numeric: !mixed_transpose || changing,
                                    sense: Sense::click_and_drag(),
                                },
                            );
                            if contains_audio && drag_enabled && response.drag_started() {
                                self.clip_transpose_drag = Some(TransposeDrag {
                                    clip: clip.id,
                                    clips: ids.clone(),
                                    value: clip.transpose,
                                    changed: false,
                                });
                            }
                            if let Some(value) = drag_transpose(ui, &response, current) {
                                let requested = f64::from(value) - f64::from(clip.transpose);
                                let delta = bounded_delta(
                                    clips.iter().map(|clip| f64::from(clip.transpose)),
                                    requested,
                                    -f64::from(TRANSPOSE_LIMIT),
                                    f64::from(TRANSPOSE_LIMIT),
                                ) as i8;
                                if f64::from(delta) != requested {
                                    ui.data_mut(|data| data.remove::<f64>(response.id.with("remainder")));
                                }
                                ui.ctx().request_repaint();
                                if contains_audio {
                                    if let Some(drag) = &mut self.clip_transpose_drag {
                                        drag.value = clip.transpose + delta;
                                        drag.changed = true;
                                        DragChange::add(&mut self.clip_settings_change, clip.id, &ids,
                                            Field::Transpose, f64::from(drag.value) - f64::from(current));
                                    }
                                } else {
                                    DragChange::add(&mut self.clip_settings_change, clip.id, &ids, Field::Transpose, f64::from(delta));
                                    commands.extend(transpose_edits(&clips, |current| current + delta));
                                }
                            }
                            if contains_audio && response.drag_stopped()
                                && let Some(drag) = self.clip_transpose_drag.take().filter(|drag| drag.changed)
                            {
                                let delta = drag.value - clip.transpose;
                                commands.extend(transpose_edits(&clips, |current| current + delta));
                            }
                            response
                        },
                    )
                    .inner).inner.on_disabled_hover_text("Wait for project audio before transposing audio clips.");
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
                                    commands.extend(gain_edits(&clips, |_| value));
                                }
                                None => error = Some("Gain must be a finite number or −∞"),
                            },
                            Field::Transpose => match parse_transpose(&current.text) {
                                Some(value) => {
                                    commands.extend(transpose_edits(&clips, |_| value));
                                }
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
        if !ui.input(|input| input.pointer.primary_down()) {
            self.clip_settings_change = None;
        }
        if let Some(message) = error {
            self.fail(message);
        }
        if !commands.is_empty() {
            self.edit(Edit::Batch(commands));
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
        clips: &[Id],
        edit: &mut Option<FieldEdit>,
        field: Field,
        value: Value<'_>,
    ) -> Response {
        if edit.as_ref().is_some_and(|edit| edit.field == field) {
            let edit = edit.as_mut().unwrap();
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(ui.make_persistent_id(("clip-setting", clip.id, field_salt(field))))
                .font(value_font(ui, field != Field::Name))
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
            // Scope pointer/click state to the full group so an active gesture
            // cannot apply to a replacement selection, even with the same primary.
            let response = ui
                .push_id(("clip-setting", clip.id, clips, field_salt(field)), |ui| {
                    value_label(
                        ui,
                        value.display,
                        value.tooltip,
                        value.numeric,
                        false,
                        value.sense,
                    )
                })
                .inner;
            if opened_by_double_click(ui, &response) {
                *edit = Some(FieldEdit {
                    clip: clip.id,
                    clips: clips.to_vec(),
                    field,
                    text: value.editable.to_owned(),
                    focus: true,
                    width: response.rect.width(),
                });
            }
            response
        }
    }
}

/// Restrict one common offset instead of clamping each member independently.
fn bounded_delta(values: impl Iterator<Item = f64>, delta: f64, min: f64, max: f64) -> f64 {
    values.fold(delta, |delta, current| {
        delta.clamp(min - current, max - current)
    })
}

fn gain_edits<'a>(
    clips: &'a [Clip],
    value: impl Fn(f32) -> f32 + 'a,
) -> impl Iterator<Item = Edit> + 'a {
    clips.iter().filter_map(move |clip| {
        let gain_db = value(clip.gain_db);
        (gain_db != clip.gain_db).then_some(Edit::SetClipGain {
            clip_id: clip.id,
            gain_db,
        })
    })
}

fn transpose_edits<'a>(
    clips: &'a [Clip],
    value: impl Fn(i8) -> i8 + 'a,
) -> impl Iterator<Item = Edit> + 'a {
    clips.iter().filter_map(move |clip| {
        let transpose = value(clip.transpose);
        (transpose != clip.transpose).then_some(Edit::SetClipTranspose {
            clip_id: clip.id,
            transpose,
        })
    })
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

fn multiple_clips_value(ui: &mut Ui) -> Response {
    ui.add_enabled_ui(false, |ui| {
        value_label(
            ui,
            "Multiple clips selected",
            "Multiple clips selected",
            false,
            true,
            Sense::hover(),
        )
    })
    .inner
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
