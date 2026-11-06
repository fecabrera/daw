use super::*;
use daw_core::TimeSignature;

const DENOMINATORS: [u16; 7] = [2, 3, 4, 6, 8, 12, 16];
#[cfg_attr(not(test), derive(Default))]
pub(super) struct State {
    edit: Option<NumberEdit>,
    drag: Option<NumberDrag>,
    cancelled: bool,
    #[cfg(test)]
    pub bounds: [Rect; 2],
    /// Editor frame, text clip and text bounds for paint regression checks.
    #[cfg(test)]
    pub editor_bounds: Option<[Rect; 3]>,
}
#[cfg(test)]
impl Default for State {
    fn default() -> Self {
        Self {
            edit: None,
            drag: None,
            cancelled: false,
            #[cfg(test)]
            bounds: [Rect::NOTHING; 2],
            editor_bounds: None,
        }
    }
}
struct NumberEdit {
    part: usize,
    text: String,
    focus: bool,
}
struct NumberDrag {
    part: usize,
    original: TimeSignature,
    preview: TimeSignature,
    units: f32,
}
impl State {
    pub fn active(&self) -> bool {
        self.edit.is_some() || self.drag.is_some()
    }

    pub fn preview(&self, signature: TimeSignature) -> TimeSignature {
        self.drag
            .as_ref()
            .filter(|drag| drag.original == signature)
            .map_or(signature, |drag| drag.preview)
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        signature: TimeSignature,
    ) -> Result<Option<TimeSignature>, &'static str> {
        if !ui.input(|i| i.pointer.primary_down()) {
            self.cancelled = false;
        }
        if !ui.is_enabled() || !ui.input(|i| i.focused) {
            self.cancelled = true;
            self.edit = None;
            self.drag = None;
        }
        if self.active()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.cancelled = true;
            self.edit = None;
            self.drag = None;
        }
        if self.drag.as_ref().is_some_and(|d| d.original != signature) {
            self.cancelled = true;
            self.drag = None;
        }
        let preview = self.preview(signature);
        #[cfg(test)]
        {
            self.editor_bounds = None;
        }
        let monitor = transport_monitor_control(
            ui,
            &format!("{}/{}", preview.numerator, preview.denominator),
            "Time signature: drag either number; double-click to type a value (1–65535).",
            0,
            Sense::hover(),
        );
        let responses: [egui::Response; 2] = std::array::from_fn(|part| {
            ui.interact(
                monitor.digit_bounds[part],
                monitor.response.id.with(part),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical)
            .on_hover_text(if part == 0 {
                "Beats per bar"
            } else {
                "Beat note value"
            })
        });
        for (part, response) in responses.iter().enumerate() {
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(number(preview, part)));
                info.label = Some(
                    if part == 0 {
                        "Time signature numerator"
                    } else {
                        "Time signature denominator"
                    }
                    .into(),
                );
                info
            });
        }
        #[cfg(test)]
        {
            self.bounds = responses.each_ref().map(|r| r.rect);
        }
        if let Some(mut edit) = self.edit.take() {
            let rect = responses[edit.part].rect;
            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
            let mut output = egui::TextEdit::singleline(&mut edit.text)
                .id(monitor.response.id.with("edit"))
                .font(fonts::semibold(13.0))
                // Keep the display slot fixed; leave enough room for its digit.
                .margin(egui::Margin::symmetric(1, 2))
                .desired_width(rect.width())
                .horizontal_align(egui::Align::Center)
                .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(rect)));
            #[cfg(test)]
            {
                self.editor_bounds = Some([
                    output.response.rect,
                    output.text_clip_rect,
                    Rect::from_min_size(output.galley_pos, output.galley.size()),
                ]);
            }
            output.response.widget_info(|| {
                egui::WidgetInfo::text_edit(
                    ui.is_enabled(),
                    "",
                    &edit.text,
                    "Time signature number",
                )
            });
            if edit.focus {
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
            if let Some(action) = inline_edit_action(ui, &output.response) {
                output.response.surrender_focus();
                if action == InlineEditAction::Commit {
                    let value = edit
                        .text
                        .trim()
                        .parse::<u16>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or("Time signature numbers must be between 1 and 65535")?;
                    return Ok(Some(replace(signature, edit.part, value)));
                }
            } else {
                self.edit = Some(edit);
            }
            return Ok(None);
        }
        if !ui.is_enabled() || self.cancelled {
            return Ok(None);
        }
        for (part, response) in responses.iter().enumerate() {
            if response.double_clicked() {
                self.drag = None;
                self.edit = Some(NumberEdit {
                    part,
                    text: number(signature, part).to_string(),
                    focus: true,
                });
                return Ok(None);
            }
            if response.dragged_by(egui::PointerButton::Primary) {
                let drag = self.drag.get_or_insert(NumberDrag {
                    part,
                    original: signature,
                    preview: signature,
                    units: 0.0,
                });
                if drag.part == part {
                    drag.units -= response.drag_delta().y / 4.0;
                    ui.ctx().request_repaint();
                    drag.preview = replace(
                        signature,
                        part,
                        dragged_value(number(signature, part), part, drag.units.trunc() as i32),
                    );
                }
            }
        }
        if let Some(drag) = self.drag.take() {
            if responses[drag.part].drag_stopped() {
                return Ok((drag.preview != signature).then_some(drag.preview));
            }
            if ui.input(|i| i.pointer.primary_down()) {
                self.drag = Some(drag);
            }
        }
        Ok(None)
    }
}
fn number(signature: TimeSignature, part: usize) -> u16 {
    if part == 0 {
        signature.numerator
    } else {
        signature.denominator
    }
}
fn replace(mut signature: TimeSignature, part: usize, value: u16) -> TimeSignature {
    if part == 0 {
        signature.numerator = value;
    } else {
        signature.denominator = value;
    }
    signature
}
fn dragged_value(previous: u16, part: usize, steps: i32) -> u16 {
    if part == 0 {
        return (i32::from(previous).saturating_add(steps)).clamp(1, i32::from(u16::MAX)) as u16;
    }
    if steps == 0 {
        return previous;
    }
    let index = if steps > 0 {
        (DENOMINATORS.partition_point(|n| *n <= previous) as i32)
            .saturating_add(steps)
            .saturating_sub(1)
    } else {
        (DENOMINATORS.partition_point(|n| *n < previous) as i32).saturating_add(steps)
    };
    DENOMINATORS[index.clamp(0, DENOMINATORS.len() as i32 - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn denominator_drag_walks_common_values_and_leaves_typed_values_exact() {
        for (index, value) in DENOMINATORS.into_iter().enumerate() {
            assert_eq!(dragged_value(value, 1, 0), value);
            assert_eq!(dragged_value(value, 1, 1), DENOMINATORS[(index + 1).min(6)]);
            assert_eq!(
                dragged_value(value, 1, -1),
                DENOMINATORS[index.saturating_sub(1)]
            );
        }
        assert_eq!(dragged_value(5, 1, 1), 6);
        assert_eq!(dragged_value(5, 1, -1), 4);
        assert_eq!(dragged_value(5, 1, 0), 5);
        assert_eq!(dragged_value(1, 0, -1), 1);
        assert_eq!(dragged_value(u16::MAX, 0, 1), u16::MAX);
        assert_eq!(dragged_value(4, 0, 3), 7);
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn text_and_drag_cancel_when_disabled_or_window_focus_is_lost() {
        for text in [false, true] {
            for disabled in [false, true] {
                let mut state = State::default();
                if text {
                    state.edit = Some(NumberEdit {
                        part: 0,
                        text: "99".into(),
                        focus: false,
                    });
                } else {
                    state.drag = Some(NumberDrag {
                        part: 1,
                        original: TimeSignature::default(),
                        preview: TimeSignature {
                            numerator: 4,
                            denominator: 12,
                        },
                        units: 12.0,
                    });
                }
                let ctx = egui::Context::default();
                fonts::configure(&ctx);
                let mut result = None;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        focused: disabled,
                        ..Default::default()
                    },
                    |ui| {
                        ui.add_enabled_ui(!disabled, |ui| {
                            result = state.show(ui, TimeSignature::default()).unwrap();
                        });
                    },
                );
                output.textures_delta.clear();
                assert!(result.is_none());
                assert!(!state.active());
            }
        }
    }
}
